use crate::{
    error::{AppError, Result},
    models::user::*,
    services::auth::{OptionalUser, User},
    state::AppState,
    require_permission,
};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    routing::{get, post, put},
    Router,
    Extension,
};
use serde_json::{json, Value};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{info, debug};

fn require_authenticated_user(user: Option<User>) -> Result<User> {
    user.ok_or_else(|| AppError::Authentication("Authentication required".to_string()))
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        // 公开路由
        .route("/", get(list_users))
        .route("/popular", get(get_popular_users))
        .route("/search", get(search_users))
        
        // 基于用户ID的路由（需要在用户名路由之前，避免冲突）
        .route("/by-id/:user_id", get(get_user_profile_by_id))
        .route("/by-id/:user_id/articles", get(get_user_articles_by_id))
        .route("/by-id/:user_id/stats", get(get_user_activity_stats_by_id))
        
        // 基于用户名的路由
        .route("/:username", get(get_user_profile))
        .route("/:username/articles", get(get_user_articles))
        .route("/:username/stats", get(get_user_activity_stats))
        
        // 需要认证的路由
        .route("/me", get(get_current_user_profile))
        .route("/me", put(update_current_user_profile))
        .route("/me/articles", get(get_current_user_articles))
        
        // 用户资料创建（给前端注册后调用）
        .route("/profile", post(create_user_profile))
}

#[derive(Debug, Deserialize)]
pub struct UserListQuery {
    pub page: Option<usize>,
    pub limit: Option<usize>,
    pub search: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UserArticlesQuery {
    pub page: Option<usize>,
    pub limit: Option<usize>,
    pub status: Option<String>,
}

/// 获取用户列表
/// GET /api/users
pub async fn list_users(
    State(app_state): State<Arc<AppState>>,
    Query(query): Query<UserListQuery>,
) -> Result<Json<Value>> {
    debug!("Fetching users list with query: {:?}", query);

    let page = query.page.unwrap_or(1);
    let limit = query.limit.unwrap_or(20).min(100); // 限制最大每页数量

    let result = app_state.user_service.get_users(page, limit, query.search).await?;

    Ok(Json(json!({
        "success": true,
        "data": {
            "users": result.data,
            "pagination": {
                "current_page": result.page,
                "total_pages": result.total_pages,
                "total_items": result.total,
                "items_per_page": result.per_page,
                "has_next": result.page < result.total_pages,
                "has_prev": result.page > 1,
            }
        }
    })))
}

/// 获取热门用户
/// GET /api/users/popular
pub async fn get_popular_users(
    State(app_state): State<Arc<AppState>>,
) -> Result<Json<Value>> {
    debug!("Fetching popular users");

    let users = app_state.user_service.get_popular_users(20).await?;

    Ok(Json(json!({
        "success": true,
        "data": users
    })))
}

/// 搜索用户
/// GET /api/users/search
pub async fn search_users(
    State(app_state): State<Arc<AppState>>,
    Query(query): Query<serde_json::Value>,
) -> Result<Json<Value>> {
    let search_query = query.get("q")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::BadRequest("Search query parameter 'q' is required".to_string()))?;

    let limit = query.get("limit")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize)
        .unwrap_or(20)
        .min(100);

    debug!("Searching users with query: {}", search_query);

    let users = app_state.user_service.search_users(search_query, limit).await?;

    Ok(Json(json!({
        "success": true,
        "data": users
    })))
}

/// 根据用户名获取用户资料
/// GET /api/users/:username
pub async fn get_user_profile(
    State(app_state): State<Arc<AppState>>,
    Path(username): Path<String>,
) -> Result<Json<Value>> {
    debug!("Fetching user profile for username: {}", username);

    let profile = app_state.user_service.get_profile_by_username(&username).await?
        .ok_or_else(|| AppError::NotFound("User not found".to_string()))?;

    // 检查用户是否被暂停
    if profile.is_suspended {
        return Err(AppError::NotFound("User not found".to_string()));
    }

    // 获取用户最新文章
    let recent_articles_result = app_state.article_service.get_user_articles(
        &profile.user_id,
        1, // 第一页
        5, // 限制5篇
        false, // 不包括草稿
    ).await;
    
    let recent_articles = match recent_articles_result {
        Ok(result) => result.data.into_iter().map(|article| {
            json!({
                "id": article.id,
                "title": article.title,
                "slug": article.slug,
                "published_at": article.published_at,
                "clap_count": article.clap_count,
                "reading_time": article.reading_time
            })
        }).collect::<Vec<_>>(),
        Err(_) => vec![], // 如果获取文章失败，返回空数组
    };

    Ok(Json(json!({
        "profile": profile.to_response(),
        "recent_articles": recent_articles
    })))
}

/// 获取用户的文章列表
/// GET /api/users/:username/articles
pub async fn get_user_articles(
    State(app_state): State<Arc<AppState>>,
    Path(username): Path<String>,
    Query(query): Query<UserArticlesQuery>,
) -> Result<Json<Value>> {
    debug!("Fetching articles for username: {}", username);

    // 获取用户资料
    let profile = app_state.user_service.get_profile_by_username(&username).await?
        .ok_or_else(|| AppError::NotFound("User not found".to_string()))?;

    let page = query.page.unwrap_or(1);
    let limit = query.limit.unwrap_or(20).min(100);

    // 默认只显示已发布的文章
    let result = app_state.article_service.get_user_articles(
        &profile.user_id,
        page,
        limit,
        false, // include_drafts = false for public access
    ).await?;

    Ok(Json(json!({
        "success": true,
        "data": {
            "articles": result.data,
            "user": profile.to_response(),
            "pagination": {
                "current_page": result.page,
                "total_pages": result.total_pages,
                "total_items": result.total,
                "items_per_page": result.per_page,
                "has_next": result.page < result.total_pages,
                "has_prev": result.page > 1,
            }
        }
    })))
}

/// 获取用户活动统计
/// GET /api/users/:username/stats
pub async fn get_user_activity_stats(
    State(app_state): State<Arc<AppState>>,
    Path(username): Path<String>,
) -> Result<Json<Value>> {
    debug!("Fetching activity stats for username: {}", username);

    // 获取用户资料
    let profile = app_state.user_service.get_profile_by_username(&username).await?
        .ok_or_else(|| AppError::NotFound("User not found".to_string()))?;

    let stats = app_state.user_service.get_user_stats(&profile.user_id).await?;

    Ok(Json(json!({
        "success": true,
        "data": {
            "user": profile.to_response(),
            "activity": stats
        }
    })))
}

/// 获取当前用户资料
/// GET /api/users/me
pub async fn get_current_user_profile(
    State(app_state): State<Arc<AppState>>,
    OptionalUser(user): OptionalUser,
) -> Result<Json<Value>> {
    let user = require_authenticated_user(user)?;
    debug!("Fetching current user profile for user: {}", user.id);

    let profile = app_state.user_service.get_or_create_profile(
        &user.id,
        &user.email,
        user.is_verified,
        user.username,
        user.display_name,
    ).await?;

    let stats = app_state.user_service.get_user_stats(&user.id).await?;

    Ok(Json(json!({
        "success": true,
        "data": {
            "profile": profile.to_response(),
            "auth_info": {
                "id": user.id,
                "email": user.email,
                "is_verified": user.is_verified,
                "created_at": user.created_at,
                "roles": user.roles,
                "permissions": user.permissions,
            },
            "activity": stats
        }
    })))
}

/// 更新当前用户资料
/// PUT /api/users/me
pub async fn update_current_user_profile(
    State(app_state): State<Arc<AppState>>,
    OptionalUser(user): OptionalUser,
    Json(request): Json<UpdateUserProfileRequest>,
) -> Result<Json<Value>> {
    let user = require_authenticated_user(user)?;
    debug!("Updating current user profile for user: {}", user.id);

    // 检查权限
    require_permission!(app_state.auth_service, user, "user.update_profile");

    // 确保资料存在，避免首次更新时返回 404
    app_state.user_service.get_or_create_profile(
        &user.id,
        &user.email,
        user.is_verified,
        user.username.clone(),
        user.display_name.clone(),
    ).await?;

    // 更新用户资料
    let profile = app_state.user_service.update_profile(&user.id, request).await?;

    info!("Updated user profile for user: {}", user.id);

    Ok(Json(json!({
        "success": true,
        "data": profile.to_response(),
        "message": "Profile updated successfully"
    })))
}

/// 获取当前用户的文章列表（包括草稿）
/// GET /api/users/me/articles
pub async fn get_current_user_articles(
    State(app_state): State<Arc<AppState>>,
    OptionalUser(user): OptionalUser,
    Query(query): Query<UserArticlesQuery>,
) -> Result<Json<Value>> {
    let user = require_authenticated_user(user)?;
    debug!("Fetching articles for current user: {}", user.id);

    let page = query.page.unwrap_or(1);
    let limit = query.limit.unwrap_or(20).min(100);

    // 用户可以查看自己的所有文章，包括草稿
    let include_drafts = match query.status.as_deref() {
        Some("draft") => true,
        Some("published") => false,
        _ => true, // 默认包括所有状态
    };

    let result = app_state.article_service.get_user_articles(
        &user.id,
        page,
        limit,
        include_drafts,
    ).await?;

    Ok(Json(json!({
        "success": true,
        "data": {
            "articles": result.data,
            "pagination": {
                "current_page": result.page,
                "total_pages": result.total_pages,
                "total_items": result.total,
                "items_per_page": result.per_page,
                "has_next": result.page < result.total_pages,
                "has_prev": result.page > 1,
            }
        }
    })))
}

/// 根据用户ID获取用户资料
/// GET /api/users/by-id/:user_id
pub async fn get_user_profile_by_id(
    State(app_state): State<Arc<AppState>>,
    Path(user_id): Path<String>,
) -> Result<Json<Value>> {
    debug!("Fetching user profile for user_id: {}", user_id);

    let profile = app_state.user_service.get_profile_by_user_id(&user_id).await?
        .ok_or_else(|| AppError::NotFound("User not found".to_string()))?;

    // 检查用户是否被暂停
    if profile.is_suspended {
        return Err(AppError::NotFound("User not found".to_string()));
    }

    // 计算准确的文章数量（仅统计已发布且未删除）
    let article_count = app_state.user_service.count_published_articles(&user_id).await?;

    // 获取用户最新文章
    let recent_articles_result = app_state.article_service.get_user_articles(
        &user_id,
        1, // 第一页
        5, // 限制5篇
        false, // 不包括草稿
    ).await;
    
    let recent_articles = match recent_articles_result {
        Ok(result) => result.data.into_iter().map(|article| {
            json!({
                "id": article.id,
                "title": article.title,
                "slug": article.slug,
                "published_at": article.published_at,
                "clap_count": article.clap_count,
                "reading_time": article.reading_time
            })
        }).collect::<Vec<_>>(),
        Err(_) => vec![], // 如果获取文章失败，返回空数组
    };

    // 实时校准粉丝/关注数
    let follower_count = app_state.user_service.count_followers(&user_id).await?;
    let following_count = app_state.user_service.count_following(&user_id).await?;

    let mut profile_json = serde_json::to_value(profile.to_response())
        .map_err(|e| AppError::internal(&format!("Serialize profile failed: {}", e)))?;
    profile_json["article_count"] = json!(article_count);
    profile_json["follower_count"] = json!(follower_count);
    profile_json["following_count"] = json!(following_count);

    Ok(Json(json!({
        "profile": profile_json,
        "recent_articles": recent_articles
    })))
}

/// 根据用户ID获取用户的文章列表
/// GET /api/users/by-id/:user_id/articles
pub async fn get_user_articles_by_id(
    State(app_state): State<Arc<AppState>>,
    Path(user_id): Path<String>,
    Query(query): Query<UserArticlesQuery>,
) -> Result<Json<Value>> {
    debug!("Fetching articles for user_id: {}", user_id);

    // 获取用户资料
    let profile = app_state.user_service.get_profile_by_user_id(&user_id).await?
        .ok_or_else(|| AppError::NotFound("User not found".to_string()))?;

    let page = query.page.unwrap_or(1);
    let limit = query.limit.unwrap_or(20).min(100);

    // 默认只显示已发布的文章
    let result = app_state.article_service.get_user_articles(
        &user_id,
        page,
        limit,
        false, // include_drafts = false for public access
    ).await?;

    Ok(Json(json!({
        "success": true,
        "data": {
            "articles": result.data,
            "user": profile.to_response(),
            "pagination": {
                "current_page": result.page,
                "total_pages": result.total_pages,
                "total_items": result.total,
                "items_per_page": result.per_page,
                "has_next": result.page < result.total_pages,
                "has_prev": result.page > 1,
            }
        }
    })))
}

/// 根据用户ID获取用户活动统计
/// GET /api/users/by-id/:user_id/stats
pub async fn get_user_activity_stats_by_id(
    State(app_state): State<Arc<AppState>>,
    Path(user_id): Path<String>,
) -> Result<Json<Value>> {
    debug!("Fetching activity stats for user_id: {}", user_id);

    // 获取用户资料
    let profile = app_state.user_service.get_profile_by_user_id(&user_id).await?
        .ok_or_else(|| AppError::NotFound("User not found".to_string()))?;

    let stats = app_state.user_service.get_user_stats(&user_id).await?;

    Ok(Json(json!({
        "success": true,
        "data": {
            "user": profile.to_response(),
            "activity": stats
        }
    })))
}

/// 创建用户资料（给前端注册后调用）
/// POST /api/users/profile
pub async fn create_user_profile(
    State(app_state): State<Arc<AppState>>,
    Json(request): Json<serde_json::Value>,
) -> Result<Json<Value>> {
    debug!("Creating user profile with request: {:?}", request);

    let auth_user_id = request.get("auth_user_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::BadRequest("auth_user_id is required".to_string()))?;

    let username = request.get("username")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::BadRequest("username is required".to_string()))?;

    let email = request.get("email")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::BadRequest("email is required".to_string()))?;

    let full_name = request.get("full_name")
        .and_then(|v| v.as_str());

    // 创建用户资料
    let profile = app_state.user_service.get_or_create_profile(
        auth_user_id,
        email,
        true, // is_verified
        Some(username.to_string()),
        full_name.map(|s| s.to_string()),
    ).await?;

    info!("Created user profile for user: {}", auth_user_id);

    Ok(Json(json!({
        "success": true,
        "data": profile.to_response(),
        "message": "User profile created successfully"
    })))
}
