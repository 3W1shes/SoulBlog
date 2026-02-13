use crate::{
    error::{AppError, Result},
    services::auth::User,
    utils::middleware::OptionalAuth,
    state::AppState,
};
use axum::{
    extract::State,
    response::Json,
    routing::get,
    Router,
};
use serde_json::{json, Value};
use std::sync::Arc;
use tracing::{info, debug};

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        // 认证相关的信息路由
        .route("/me", get(get_current_user))
        .route("/status", get(get_auth_status))
        .route("/refresh", get(get_auth_info)) // 获取当前认证信息
        .route("/email-status", get(get_email_verification_status))
}

/// 获取当前用户信息
/// GET /api/auth/me
/// 
/// 注意：实际的用户认证是由 Rainbow-Gateway 和 Rainbow-Auth 处理的
/// 这个端点主要是返回通过 JWT 解析得到的用户信息
pub async fn get_current_user(
    State(app_state): State<Arc<AppState>>,
    OptionalAuth(user): OptionalAuth,
) -> Result<Json<Value>> {
    let user = match user {
        Some(user) => user,
        None => return Err(AppError::Authentication("Not authenticated".to_string())),
    };
    debug!("Getting current user info for user: {}", user.id);

    // 获取或创建用户资料（包含邮箱验证状态）
    let profile = app_state.user_service.get_or_create_profile(
        &user.id,
        &user.email,
        user.is_verified, // Rainbow-Auth的邮箱验证状态
        user.username.clone(),
        user.display_name.clone(),
    ).await?;

    // 获取用户活动统计
    let stats = app_state.user_service.get_user_stats(&user.id).await?;

    Ok(Json(json!({
        "success": true,
        "data": {
            "auth": {
                "id": user.id,
                "email": user.email,
                "username": user.username,
                "display_name": user.display_name,
                "avatar_url": user.avatar_url,
                "is_verified": user.is_verified,
                "created_at": user.created_at,
                "roles": user.roles,
                "permissions": user.permissions,
            },
            "profile": profile.to_response(),
            "activity": stats
        }
    })))
}

/// 获取认证状态
/// GET /api/auth/status
/// 
/// 这个端点可以被未认证的用户访问，用于检查当前的认证状态
pub async fn get_auth_status(
    State(_app_state): State<Arc<AppState>>,
    OptionalAuth(user): OptionalAuth,
) -> Result<Json<Value>> {
    debug!("Checking authentication status");

    match user {
        Some(user) => {
            Ok(Json(json!({
                "success": true,
                "data": {
                    "authenticated": true,
                    "user": {
                        "id": user.id,
                        "email": user.email,
                        "username": user.username,
                        "display_name": user.display_name,
                        "avatar_url": user.avatar_url,
                        "is_verified": user.is_verified,
                        "roles": user.roles,
                    }
                }
            })))
        }
        None => {
            Ok(Json(json!({
                "success": true,
                "data": {
                    "authenticated": false,
                    "user": null,
                    "message": "Not authenticated. Please login through Rainbow-Gateway."
                }
            })))
        }
    }
}

/// 获取认证信息和配置
/// GET /api/auth/refresh
/// 
/// 用于刷新用户的认证状态和获取最新的用户信息
pub async fn get_auth_info(
    State(app_state): State<Arc<AppState>>,
    OptionalAuth(user): OptionalAuth,
) -> Result<Json<Value>> {
    let user = match user {
        Some(user) => user,
        None => return Err(AppError::Authentication("Not authenticated".to_string())),
    };
    debug!("Refreshing auth info for user: {}", user.id);

    // 获取最新的用户资料
    let profile = app_state.user_service.get_or_create_profile(
        &user.id,
        &user.email,
        user.is_verified,
        user.username.clone(),
        user.display_name.clone(),
    ).await?;

    // 获取用户活动统计
    let stats = app_state.user_service.get_user_stats(&user.id).await?;

    // 获取系统配置（用户相关的）
    let user_config = json!({
        "features": {
            "can_create_articles": app_state.auth_service.check_permission(&user.id, "article.create").await.unwrap_or(false),
            "can_create_publications": app_state.auth_service.check_permission(&user.id, "publication.create").await.unwrap_or(false),
            "can_comment": app_state.auth_service.check_permission(&user.id, "comment.create").await.unwrap_or(false),
        },
        "limits": {
            "max_article_length": app_state.config.max_article_length,
            "max_comment_length": app_state.config.max_comment_length,
            "max_bio_length": app_state.config.max_bio_length,
        }
    });

    info!("Refreshed auth info for user: {}", user.id);

    Ok(Json(json!({
        "success": true,
        "data": {
            "auth": {
                "id": user.id,
                "email": user.email,
                "username": user.username,
                "display_name": user.display_name,
                "avatar_url": user.avatar_url,
                "is_verified": user.is_verified,
                "created_at": user.created_at,
                "roles": user.roles,
                "permissions": user.permissions,
            },
            "profile": profile.to_response(),
            "activity": stats,
            "config": user_config
        },
        "message": "Authentication info refreshed successfully"
    })))
}

/// 获取邮箱验证状态
/// GET /api/auth/email-status
/// 
/// 专门用于检查用户邮箱验证状态的端点
pub async fn get_email_verification_status(
    State(app_state): State<Arc<AppState>>,
    OptionalAuth(user): OptionalAuth,
) -> Result<Json<Value>> {
    let user = match user {
        Some(user) => user,
        None => return Err(AppError::Authentication("Not authenticated".to_string())),
    };
    debug!("Getting email verification status for user: {}", user.id);

    // 获取用户资料（包含最新的邮箱验证状态）
    let profile = app_state.user_service.get_or_create_profile(
        &user.id,
        &user.email,
        user.is_verified,
        user.username.clone(),
        user.display_name.clone(),
    ).await?;

    Ok(Json(json!({
        "success": true,
        "data": {
            "user_id": user.id,
            "email": user.email,
            "email_verified": user.is_verified,
            "verification_required_for": {
                "creating_articles": !user.is_verified,
                "commenting": !user.is_verified,
                "following_users": false,
                "publishing_articles": !user.is_verified
            },
            "rainbow_auth_url": format!("{}/api/auth", app_state.config.auth_service_url),
            "verification_help": {
                "message": if user.is_verified {
                    "您的邮箱已经通过验证"
                } else {
                    "您的邮箱尚未验证，某些功能可能受限"
                },
                "action_required": !user.is_verified,
                "action_url": if !user.is_verified {
                    Some(format!("{}/verify-email", app_state.config.auth_service_url))
                } else {
                    None
                }
            }
        }
    })))
}
