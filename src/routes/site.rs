//! 站点元信息与安装向导路由
//!
//! 在单/平台两种模式下都启用，提供：
//! - GET  /api/blog/site/status        是否已安装 + 当前模式（公开）
//! - POST /api/blog/site/install       首次安装（公开，仅 installed=false 时可用）
//! - GET  /api/blog/site/config        公开品牌信息（公开）
//! - GET  /api/blog/site/admin/config  完整配置（admin only）
//! - PUT  /api/blog/site/admin/config  更新配置（admin only）

use crate::{
    error::{AppError, Result},
    services::auth::{Claims, User},
    state::AppState,
    utils::middleware::OptionalAuth,
};
use argon2::password_hash::{rand_core::OsRng, SaltString};
use argon2::{Argon2, PasswordHasher};
use axum::{
    extract::State,
    response::Json,
    routing::{get, post, put},
    Router,
};
use chrono::{Duration, Utc};
use jsonwebtoken::{encode, EncodingKey, Header};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;
use tracing::info;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct InstallRequest {
    pub site_name: String,
    pub site_description: Option<String>,
    pub locale: Option<String>,
    pub admin_email: String,
    pub admin_password: String,
    pub admin_username: String,
    pub admin_display_name: Option<String>,
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/status", get(get_status))
        .route("/install", post(install))
        .route("/config", get(get_public_config))
        .route("/admin/config", get(get_admin_config))
        .route("/admin/config", put(update_admin_config))
}

async fn get_status(State(state): State<Arc<AppState>>) -> Result<Json<Value>> {
    let cfg = state.site_config_service.get().await?;
    let mode = current_mode();
    Ok(Json(json!({
        "success": true,
        "data": {
            "installed": cfg.as_ref().map(|c| c.installed).unwrap_or(false),
            "mode": mode,
            "site_name": cfg.as_ref().map(|c| c.site_name.clone()),
        }
    })))
}

async fn install(
    State(state): State<Arc<AppState>>,
    Json(req): Json<InstallRequest>,
) -> Result<Json<Value>> {
    if state.site_config_service.is_installed().await? {
        return Err(AppError::BadRequest("Site already installed".into()));
    }

    if req.admin_email.trim().is_empty()
        || req.admin_password.len() < 8
        || req.admin_username.trim().is_empty()
        || req.site_name.trim().is_empty()
    {
        return Err(AppError::BadRequest(
            "site_name / admin_email / admin_username 必填，密码至少 8 位".into(),
        ));
    }

    // 哈希密码
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let password_hash = argon2
        .hash_password(req.admin_password.as_bytes(), &salt)
        .map_err(|e| AppError::Internal(format!("hash failed: {}", e)))?
        .to_string();

    let user_id = Uuid::new_v4().to_string();
    let display_name = req
        .admin_display_name
        .clone()
        .unwrap_or_else(|| req.admin_username.clone());

    // 建 user_auth
    state
        .db
        .query_with_params(
            "CREATE user_auth SET user_id = $uid, email = $email, password_hash = $hash, created_at = time::now()",
            json!({ "uid": user_id, "email": req.admin_email, "hash": password_hash }),
        )
        .await
        .map_err(|e| AppError::Database(surrealdb::Error::thrown(e.to_string())))?;

    // 建 user_profile
    state
        .user_service
        .get_or_create_profile(
            &user_id,
            &req.admin_email,
            true,
            Some(req.admin_username.clone()),
            Some(display_name.clone()),
        )
        .await
        .map_err(|e| AppError::Internal(format!("create admin profile failed: {}", e)))?;

    // 写 site_config
    let mode = current_mode();
    let cfg = state
        .site_config_service
        .install(
            &mode,
            req.site_name.trim(),
            req.site_description.clone(),
            req.locale.as_deref().unwrap_or("zh-CN"),
            &user_id,
        )
        .await?;

    let token = create_jwt(&user_id, &req.admin_email, &state.config.jwt_secret)?;

    info!(
        "Site installed: name={} mode={} owner={} ({})",
        cfg.site_name, cfg.mode, user_id, req.admin_email
    );

    Ok(Json(json!({
        "success": true,
        "token": token,
        "user": {
            "id": user_id,
            "email": req.admin_email,
            "username": req.admin_username,
            "display_name": display_name,
            "is_admin": true,
        },
        "site": {
            "name": cfg.site_name,
            "mode": cfg.mode,
            "locale": cfg.locale,
        }
    })))
}

async fn get_public_config(State(state): State<Arc<AppState>>) -> Result<Json<Value>> {
    let cfg = state
        .site_config_service
        .get()
        .await?
        .ok_or_else(|| AppError::NotFound("Site not installed".to_string()))?;
    Ok(Json(json!({
        "success": true,
        "data": {
            "site_name": cfg.site_name,
            "site_description": cfg.site_description,
            "site_logo": cfg.site_logo,
            "site_favicon": cfg.site_favicon,
            "locale": cfg.locale,
            "theme_color": cfg.theme_color,
            "allow_register": cfg.allow_register,
            "allow_comments": cfg.allow_comments,
            "footer_text": cfg.footer_text,
            "icp_text": cfg.icp_text,
            "mode": cfg.mode,
        }
    })))
}

async fn get_admin_config(
    State(state): State<Arc<AppState>>,
    OptionalAuth(user): OptionalAuth,
) -> Result<Json<Value>> {
    let user = user.ok_or_else(|| AppError::Authentication("Not authenticated".to_string()))?;
    require_admin(&state, &user).await?;

    let cfg = state
        .site_config_service
        .get()
        .await?
        .ok_or_else(|| AppError::NotFound("Site not installed".to_string()))?;
    Ok(Json(json!({ "success": true, "data": cfg })))
}

async fn update_admin_config(
    State(state): State<Arc<AppState>>,
    OptionalAuth(user): OptionalAuth,
    Json(updates): Json<Value>,
) -> Result<Json<Value>> {
    let user = user.ok_or_else(|| AppError::Authentication("Not authenticated".to_string()))?;
    require_admin(&state, &user).await?;

    // 不可更新字段：mode/installed/owner_user_id/created_at
    let mut updates = updates;
    if let Some(obj) = updates.as_object_mut() {
        obj.remove("mode");
        obj.remove("installed");
        obj.remove("owner_user_id");
        obj.remove("created_at");
        obj.remove("id");
    }

    let cfg = state.site_config_service.update(updates).await?;
    Ok(Json(json!({ "success": true, "data": cfg })))
}

/// 检查当前 user 是否为站点 owner（admin）
pub async fn require_admin(state: &Arc<AppState>, user: &User) -> Result<()> {
    if state.site_config_service.is_admin(&user.id).await? {
        Ok(())
    } else {
        Err(AppError::Authorization("Admin only".to_string()))
    }
}

fn current_mode() -> String {
    if cfg!(feature = "single") {
        "single".to_string()
    } else {
        "platform".to_string()
    }
}

fn create_jwt(user_id: &str, email: &str, jwt_secret: &str) -> Result<String> {
    let now = Utc::now();
    let exp = (now + Duration::days(7)).timestamp();
    let claims = Claims {
        sub: user_id.to_string(),
        exp,
        iat: now.timestamp(),
        session_id: Some(Uuid::new_v4().to_string()),
        email: Some(email.to_string()),
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(jwt_secret.as_ref()),
    )
    .map_err(|e| AppError::Internal(format!("Failed to create JWT: {}", e)))
}
