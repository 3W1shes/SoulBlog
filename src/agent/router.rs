//! Agent API 路由配置
//!
//! 定义 /agent/v1/* 的所有路由

use axum::{
    middleware,
    routing::{get, post},
    Router,
};
use std::sync::Arc;

use crate::{
    agent::{
        auth::{optional_auth_middleware, require_auth_middleware},
        handlers,
        request_id::inject_request_id,
    },
    state::AppState,
};

/// 创建 Agent API 路由
pub fn agent_router(state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        // 系统健康检查（公开）
        .route("/system/health", get(handlers::health_check))
        
        // 出版物相关（公开读取）
        .route("/publications", get(handlers::list_publications))
        .route("/publications/:id", get(handlers::get_publication))
        
        // 文章相关（公开读取）
        .route("/articles", get(handlers::list_articles))
        .route("/articles/:id", get(handlers::get_article))
        
        // 搜索（可选认证）
        .route("/search", get(handlers::search))
        
        // 评论（公开读取，需要认证写入）
        .route("/comments", get(handlers::list_comments))
        .route(
            "/comments",
            post(handlers::create_comment).route_layer(middleware::from_fn_with_state(
                state.clone(),
                require_auth_middleware,
            )),
        )
        .layer(middleware::from_fn(inject_request_id))
        
        // 全局中间件：可选认证
        .layer(middleware::from_fn_with_state(
            state,
            optional_auth_middleware,
        ))
}
