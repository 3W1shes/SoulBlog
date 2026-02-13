use crate::{
    error::{AppError, Result},
    models::comment::*,
    services::AuthService,
    state::AppState,
    utils::middleware::OptionalAuth,
};
use axum::{
    extract::{Path, State},
    response::Json,
    routing::{delete, get, post, put},
    Extension, Router,
};
use serde_json::{json, Value};
use std::sync::Arc;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/article/:article_id", get(get_article_comments))
        .route("/", post(create_comment))
        .route("/test", post(test_create_comment))
        .route("/:id", put(update_comment))
        .route("/:id", delete(delete_comment))
        .route("/:id/clap", post(clap_comment))
        .route("/:id/clap", delete(remove_clap))
        .layer(axum::middleware::from_fn(|req: axum::http::Request<axum::body::Body>, next: axum::middleware::Next| async move {
            tracing::info!("Comments router: {} {}", req.method(), req.uri().path());
            next.run(req).await
        }))
}

async fn get_article_comments(
    State(state): State<Arc<AppState>>,
    Path(article_id): Path<String>,
    OptionalAuth(user): OptionalAuth,
) -> Result<Json<Value>> {
    let user_id = user.as_ref().map(|u| u.id.as_str());
    let comments = state
        .comment_service
        .get_article_comments(&article_id, user_id)
        .await?;

    Ok(Json(json!({
        "success": true,
        "data": comments
    })))
}

async fn create_comment(
    State(state): State<Arc<AppState>>,
    OptionalAuth(user): OptionalAuth,
    Json(request): Json<CreateCommentRequest>,
) -> Result<Json<Value>> {
    tracing::info!("create_comment handler called");
    tracing::info!("User from OptionalAuth: {:?}", user.is_some());
    
    let user = user
        .ok_or_else(|| {
            tracing::error!("User not authenticated");
            AppError::unauthorized("Authentication required")
        })?;
    
    tracing::info!("User ID: {}", user.id);
    tracing::info!("Request: {:?}", request);
    
    match state.comment_service.create_comment(&user.id, request).await {
        Ok(comment) => {
            tracing::info!("Comment created successfully: {:?}", comment);
            Ok(Json(json!({
                "success": true,
                "data": comment
            })))
        }
        Err(e) => {
            tracing::error!("Failed to create comment: {}", e);
            Err(e)
        }
    }
}

async fn test_create_comment(
    State(state): State<Arc<AppState>>,
    OptionalAuth(user): OptionalAuth,
    Json(request): Json<CreateCommentRequest>,
) -> Result<Json<Value>> {
    tracing::info!("test_create_comment handler called");
    tracing::info!("User: {:?}", user);
    tracing::info!("Request: {:?}", request);
    
    // 如果没有用户，返回错误
    let user = user.ok_or_else(|| AppError::unauthorized("Authentication required"))?;
    
    let comment = state
        .comment_service
        .create_comment(&user.id, request)
        .await?;

    Ok(Json(json!({
        "success": true,
        "data": comment
    })))
}

async fn update_comment(
    State(state): State<Arc<AppState>>,
    OptionalAuth(user): OptionalAuth,
    Path(comment_id): Path<String>,
    Json(request): Json<UpdateCommentRequest>,
) -> Result<Json<Value>> {
    let user = user.ok_or_else(|| AppError::unauthorized("Authentication required"))?;
    
    let comment = state
        .comment_service
        .update_comment(&comment_id, &user.id, request)
        .await?;

    Ok(Json(json!({
        "success": true,
        "data": comment
    })))
}

async fn delete_comment(
    State(state): State<Arc<AppState>>,
    OptionalAuth(user): OptionalAuth,
    Path(comment_id): Path<String>,
) -> Result<Json<Value>> {
    let user = user.ok_or_else(|| AppError::unauthorized("Authentication required"))?;
    
    state
        .comment_service
        .delete_comment(&comment_id, &user.id)
        .await?;

    Ok(Json(json!({
        "success": true,
        "message": "Comment deleted successfully"
    })))
}

async fn clap_comment(
    State(state): State<Arc<AppState>>,
    OptionalAuth(user): OptionalAuth,
    Path(comment_id): Path<String>,
) -> Result<Json<Value>> {
    let user = user.ok_or_else(|| AppError::unauthorized("Authentication required"))?;
    
    state
        .comment_service
        .clap_comment(&comment_id, &user.id)
        .await?;

    Ok(Json(json!({
        "success": true,
        "message": "Comment clapped successfully"
    })))
}

async fn remove_clap(
    State(state): State<Arc<AppState>>,
    OptionalAuth(user): OptionalAuth,
    Path(comment_id): Path<String>,
) -> Result<Json<Value>> {
    let user = user.ok_or_else(|| AppError::unauthorized("Authentication required"))?;
    
    state
        .comment_service
        .remove_clap(&comment_id, &user.id)
        .await?;

    Ok(Json(json!({
        "success": true,
        "message": "Clap removed successfully"
    })))
}
