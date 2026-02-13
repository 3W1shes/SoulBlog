use crate::{
    error::{AppError, Result},
    models::recommendation::*,
    services::auth::User,
    state::AppState,
    utils::middleware::OptionalAuth,
};
use axum::{
    extract::{Path, Query, State},
    response::Json,
    routing::get,
    Extension, Router,
};
use serde_json::{json, Value};
use std::sync::Arc;
use tracing::debug;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(get_recommendations))
        .route("/trending", get(get_trending))
        .route("/following", get(get_following_recommendations))
        .route("/related/:article_id", get(get_related_articles))
        .route("/content-based/:article_id", get(get_content_based_articles))
        .route("/update", get(update_recommendations)) // 管理员手动触发更新
}

/// 获取个性化推荐
/// GET /api/recommendations
async fn get_recommendations(
    State(state): State<Arc<AppState>>,
    Query(request): Query<RecommendationRequest>,
    OptionalAuth(user): OptionalAuth,
) -> Result<Json<Value>> {
    debug!("Getting personalized recommendations");

    let mut final_request = request;
    if let Some(user) = user {
        final_request.user_id = Some(user.id);
    }

    let recommendations = state
        .recommendation_service
        .get_recommendations(final_request)
        .await?;

    Ok(Json(json!({
        "success": true,
        "data": recommendations
    })))
}

/// 获取热门推荐
/// GET /api/recommendations/trending
async fn get_trending(
    State(state): State<Arc<AppState>>,
    Query(request): Query<RecommendationRequest>,
) -> Result<Json<Value>> {
    debug!("Getting trending recommendations");

    let trending_request = RecommendationRequest {
        algorithm: Some(RecommendationAlgorithm::Trending),
        ..request
    };

    let recommendations = state
        .recommendation_service
        .get_recommendations(trending_request)
        .await?;

    Ok(Json(json!({
        "success": true,
        "data": recommendations
    })))
}

/// 获取关注用户的文章推荐
/// GET /api/recommendations/following
async fn get_following_recommendations(
    State(state): State<Arc<AppState>>,
    Query(request): Query<RecommendationRequest>,
    Extension(user): Extension<User>,
) -> Result<Json<Value>> {
    debug!("Getting following recommendations for user: {}", user.id);

    let following_request = RecommendationRequest {
        user_id: Some(user.id),
        algorithm: Some(RecommendationAlgorithm::Following),
        ..request
    };

    let recommendations = state
        .recommendation_service
        .get_recommendations(following_request)
        .await?;

    Ok(Json(json!({
        "success": true,
        "data": recommendations
    })))
}

/// 获取相关文章推荐
/// GET /api/recommendations/related/:article_id
async fn get_related_articles(
    State(state): State<Arc<AppState>>,
    Path(article_id): Path<String>,
    Query(params): Query<RelatedArticlesQuery>,
) -> Result<Json<Value>> {
    debug!("Getting related articles for: {}", article_id);

    let limit = params.limit.unwrap_or(5);
    
    let related_articles = state
        .recommendation_service
        .get_related_articles(&article_id, limit)
        .await?;

    Ok(Json(json!({
        "success": true,
        "data": related_articles
    })))
}

/// 获取内容相关推荐（兼容旧前端路由）
/// GET /api/recommendations/content-based/:article_id
async fn get_content_based_articles(
    State(state): State<Arc<AppState>>,
    Path(article_id): Path<String>,
    Query(params): Query<RelatedArticlesQuery>,
) -> Result<Json<Value>> {
    debug!("Getting content-based recommendations for: {}", article_id);

    let limit = params.limit.unwrap_or(5);
    let related_articles = state
        .recommendation_service
        .get_related_articles(&article_id, limit)
        .await?;

    Ok(Json(json!({
        "success": true,
        "data": related_articles
    })))
}

/// 手动更新推荐系统缓存（管理员功能）
/// GET /api/recommendations/update
async fn update_recommendations(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
) -> Result<Json<Value>> {
    debug!("Manually updating recommendation cache");

    // 检查管理员权限
    if !user.permissions.contains(&"admin.recommendation".to_string()) {
        return Err(AppError::forbidden("Admin permission required"));
    }

    state.recommendation_service.update_recommendations().await?;

    Ok(Json(json!({
        "success": true,
        "message": "Recommendation cache updated successfully"
    })))
}

#[derive(serde::Deserialize)]
struct RelatedArticlesQuery {
    limit: Option<usize>,
}
