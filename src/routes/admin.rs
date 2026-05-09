//! 站长后台路由（admin only）
//!
//! Single 模式专用：站长（site_config.owner_user_id）的后台数据/管理接口
//! 文章 CRUD 仍走 /api/blog/articles/*；这里只暴露需要 admin 权限的统计/审核
//!
//! - GET    /api/blog/admin/overview         总览统计
//! - GET    /api/blog/admin/articles         所有文章（含草稿）
//! - GET    /api/blog/admin/comments         所有评论（含待审）
//! - DELETE /api/blog/admin/comments/:id     管理员强删评论
//! - GET    /api/blog/admin/users            用户/评论者列表

use crate::{
    error::{AppError, Result},
    routes::site::require_admin,
    services::auth::User,
    state::AppState,
    utils::middleware::OptionalAuth,
};
use axum::{
    extract::{Path, Query, State},
    response::Json,
    routing::{delete, get, post},
    Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;
use tracing::{debug, info};
use uuid::Uuid;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/overview", get(get_overview))
        .route("/articles", get(list_all_articles))
        .route("/comments", get(list_all_comments))
        .route("/comments/:id", delete(force_delete_comment))
        .route("/users", get(list_users))
}

#[derive(Debug, Deserialize, Default)]
struct ListQuery {
    page: Option<u32>,
    limit: Option<u32>,
    status: Option<String>,
}

async fn get_overview(
    State(state): State<Arc<AppState>>,
    OptionalAuth(user): OptionalAuth,
) -> Result<Json<Value>> {
    let user = require_user(&user)?;
    require_admin(&state, &user).await?;

    debug!("admin overview by {}", user.id);

    // 直接用 SQL 聚合一些核心计数（SurrealDB 3.0 SCHEMALESS）
    let mut resp = state
        .db
        .query(
            r#"
            SELECT * FROM (
              SELECT COUNT() AS total FROM article WHERE is_deleted != true
            )[0];
            SELECT * FROM (
              SELECT COUNT() AS total FROM article WHERE status = "published" AND is_deleted != true
            )[0];
            SELECT * FROM (
              SELECT COUNT() AS total FROM article WHERE status = "draft" AND is_deleted != true
            )[0];
            SELECT * FROM (
              SELECT COUNT() AS total FROM comment WHERE is_deleted != true
            )[0];
            SELECT * FROM (
              SELECT COUNT() AS total FROM tag
            )[0];
            SELECT * FROM (
              SELECT COUNT() AS total FROM user_profile
            )[0];
            "#,
        )
        .await?;

    let articles_total: Value = resp.take(0).unwrap_or(json!({"total": 0}));
    let articles_published: Value = resp.take(1).unwrap_or(json!({"total": 0}));
    let articles_draft: Value = resp.take(2).unwrap_or(json!({"total": 0}));
    let comments_total: Value = resp.take(3).unwrap_or(json!({"total": 0}));
    let tags_total: Value = resp.take(4).unwrap_or(json!({"total": 0}));
    let users_total: Value = resp.take(5).unwrap_or(json!({"total": 0}));

    Ok(Json(json!({
        "success": true,
        "data": {
            "articles": {
                "total": pluck(&articles_total),
                "published": pluck(&articles_published),
                "draft": pluck(&articles_draft),
            },
            "comments": { "total": pluck(&comments_total) },
            "tags": { "total": pluck(&tags_total) },
            "users": { "total": pluck(&users_total) },
        }
    })))
}

async fn list_all_articles(
    State(state): State<Arc<AppState>>,
    OptionalAuth(user): OptionalAuth,
    Query(q): Query<ListQuery>,
) -> Result<Json<Value>> {
    let user = require_user(&user)?;
    require_admin(&state, &user).await?;

    let limit = q.limit.unwrap_or(20).min(100) as i64;
    let page = q.page.unwrap_or(1).max(1) as i64;
    let start = (page - 1) * limit;

    let where_clause = match q.status.as_deref() {
        Some("draft") => "WHERE status = 'draft' AND is_deleted != true",
        Some("published") => "WHERE status = 'published' AND is_deleted != true",
        Some("archived") => "WHERE status = 'archived' AND is_deleted != true",
        _ => "WHERE is_deleted != true",
    };

    let sql = format!(
        "SELECT id, title, slug, status, author_id, view_count, comment_count, clap_count, created_at, updated_at, published_at FROM article {} ORDER BY created_at DESC LIMIT {} START {}",
        where_clause, limit, start
    );

    let mut resp = state.db.query(&sql).await?;
    let rows: Vec<Value> = resp.take(0).unwrap_or_default();

    Ok(Json(json!({
        "success": true,
        "data": {
            "articles": rows,
            "page": page,
            "limit": limit,
        }
    })))
}

async fn list_all_comments(
    State(state): State<Arc<AppState>>,
    OptionalAuth(user): OptionalAuth,
    Query(q): Query<ListQuery>,
) -> Result<Json<Value>> {
    let user = require_user(&user)?;
    require_admin(&state, &user).await?;

    let limit = q.limit.unwrap_or(50).min(200) as i64;
    let page = q.page.unwrap_or(1).max(1) as i64;
    let start = (page - 1) * limit;

    let sql = format!(
        "SELECT id, content, author_id, article_id, parent_id, created_at FROM comment WHERE is_deleted != true ORDER BY created_at DESC LIMIT {} START {}",
        limit, start
    );

    let mut resp = state.db.query(&sql).await?;
    let rows: Vec<Value> = resp.take(0).unwrap_or_default();

    Ok(Json(json!({
        "success": true,
        "data": {
            "comments": rows,
            "page": page,
            "limit": limit,
        }
    })))
}

async fn force_delete_comment(
    State(state): State<Arc<AppState>>,
    OptionalAuth(user): OptionalAuth,
    Path(id): Path<String>,
) -> Result<Json<Value>> {
    let user = require_user(&user)?;
    require_admin(&state, &user).await?;

    let pure_id = id.trim_start_matches("comment:").trim_matches('`');
    state
        .db
        .query_with_params(
            "UPDATE comment:`$id` SET is_deleted = true, updated_at = time::now()",
            json!({ "id": pure_id }),
        )
        .await
        .map_err(|e| AppError::Database(surrealdb::Error::thrown(e.to_string())))?;

    info!("admin {} force-deleted comment {}", user.id, pure_id);
    Ok(Json(json!({ "success": true })))
}

async fn list_users(
    State(state): State<Arc<AppState>>,
    OptionalAuth(user): OptionalAuth,
    Query(q): Query<ListQuery>,
) -> Result<Json<Value>> {
    let user = require_user(&user)?;
    require_admin(&state, &user).await?;

    let limit = q.limit.unwrap_or(50).min(200) as i64;
    let page = q.page.unwrap_or(1).max(1) as i64;
    let start = (page - 1) * limit;

    let sql = format!(
        "SELECT id, user_id, username, display_name, avatar_url, bio, created_at FROM user_profile ORDER BY created_at DESC LIMIT {} START {}",
        limit, start
    );

    let mut resp = state.db.query(&sql).await?;
    let rows: Vec<Value> = resp.take(0).unwrap_or_default();

    Ok(Json(json!({
        "success": true,
        "data": {
            "users": rows,
            "page": page,
            "limit": limit,
        }
    })))
}

fn require_user(user: &Option<User>) -> Result<User> {
    user.clone()
        .ok_or_else(|| AppError::Authentication("Not authenticated".to_string()))
}

fn pluck(v: &Value) -> i64 {
    v.get("total").and_then(|x| x.as_i64()).unwrap_or(0)
}


// API Key 管理已迁出到 routes/me.rs（普通用户自助 /api/blog/me/api-keys）
