//! 用户自助路由 /api/blog/me/*
//!
//! 任何登录用户都能管自己的资源，按 user_id 隔离（不需要 admin 权限）
//!
//! - GET    /api/blog/me/api-keys         列出当前用户的 API Key
//! - POST   /api/blog/me/api-keys         创建新 API Key（明文 key 仅返回一次）
//! - DELETE /api/blog/me/api-keys/:id     撤销自己的 API Key（软删）

use crate::{
    error::{AppError, Result},
    services::auth::User,
    state::AppState,
    utils::middleware::OptionalAuth,
};
use axum::{
    extract::{Path, State},
    response::Json,
    routing::{delete, get},
    Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;
use tracing::info;
use uuid::Uuid;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api-keys", get(list_api_keys).post(create_api_key))
        .route("/api-keys/:id", delete(revoke_api_key))
}

#[derive(Debug, Deserialize, Default)]
struct CreateApiKeyRequest {
    name: String,
    #[serde(default)]
    scopes: Option<Vec<String>>,
    #[serde(default)]
    expires_at: Option<String>,
}

async fn list_api_keys(
    State(state): State<Arc<AppState>>,
    OptionalAuth(user): OptionalAuth,
) -> Result<Json<Value>> {
    let user = require_user(&user)?;

    let mut resp = state
        .db
        .query_with_params(
            "SELECT id, name, key_prefix, scopes, expires_at, created_at, last_used_at
             FROM api_key WHERE created_by = $uid AND is_deleted != true
             ORDER BY created_at DESC",
            json!({ "uid": user.id }),
        )
        .await?;
    let items: Vec<Value> = resp.take(0).unwrap_or_default();
    Ok(Json(json!({ "success": true, "data": { "items": items } })))
}

async fn create_api_key(
    State(state): State<Arc<AppState>>,
    OptionalAuth(user): OptionalAuth,
    Json(req): Json<CreateApiKeyRequest>,
) -> Result<Json<Value>> {
    let user = require_user(&user)?;

    if req.name.trim().is_empty() {
        return Err(AppError::Validation("name 不能为空".into()));
    }

    let raw_key = format!("sk-blog-{}", Uuid::new_v4().to_string().replace('-', ""));
    let prefix = &raw_key[..14];
    let key_hash = sha256_hex(&raw_key);
    let scopes = req
        .scopes
        .unwrap_or_else(|| vec!["read".into(), "write".into()]);
    let expires_at = req.expires_at.as_deref().unwrap_or("");

    state
        .db
        .query_with_params(
            "CREATE api_key SET
                name = $name,
                key_hash = $key_hash,
                key_prefix = $prefix,
                scopes = $scopes,
                expires_at = $expires_at,
                created_by = $uid,
                user_id = $uid,
                is_deleted = false,
                last_used_at = NONE,
                created_at = time::now(),
                updated_at = time::now()",
            json!({
                "name": req.name.trim(),
                "key_hash": key_hash,
                "prefix": prefix,
                "scopes": scopes,
                "expires_at": expires_at,
                "uid": user.id,
            }),
        )
        .await?;

    info!("user {} created api_key {}", user.id, prefix);

    Ok(Json(json!({
        "success": true,
        "data": {
            "key": raw_key,
            "key_prefix": prefix,
            "name": req.name.trim(),
            "scopes": scopes,
            "expires_at": expires_at,
        }
    })))
}

async fn revoke_api_key(
    State(state): State<Arc<AppState>>,
    OptionalAuth(user): OptionalAuth,
    Path(id): Path<String>,
) -> Result<Json<Value>> {
    let user = require_user(&user)?;

    let pure_id = id.trim_start_matches("api_key:").trim_matches('`');
    state
        .db
        .query_with_params(
            "UPDATE api_key:`$id` SET is_deleted = true, updated_at = time::now()
             WHERE created_by = $uid",
            json!({ "id": pure_id, "uid": user.id }),
        )
        .await?;

    info!("user {} revoked api_key {}", user.id, pure_id);
    Ok(Json(json!({ "success": true })))
}

fn require_user(user: &Option<User>) -> Result<User> {
    user.clone()
        .ok_or_else(|| AppError::Authentication("Not authenticated".to_string()))
}

fn sha256_hex(input: &str) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(input.as_bytes());
    format!("{:x}", digest)
}
