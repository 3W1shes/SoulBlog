//! Agent API 认证与权限校验
//!
//! 为 OpenClaw / Agent 调用提供 JWT 认证和 scope 权限校验

use axum::{
    extract::{Request, State},
    middleware::Next,
    response::{IntoResponse, Response},
};
use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, TokenData, Validation};
use serde::{Deserialize, Serialize};
use std::{future::Future, pin::Pin, sync::Arc};

use crate::state::AppState;

use super::{request_id::RequestId, response::AgentResponse};

/// JWT Claims 结构
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AgentClaims {
    /// 用户标识
    pub sub: String,
    
    /// 用户角色
    #[serde(default)]
    pub role: Option<String>,
    
    /// 权限列表
    #[serde(default)]
    pub permissions: Vec<String>,
    
    /// 会话 ID
    #[serde(default)]
    pub session_id: Option<String>,
    
    /// 过期时间
    pub exp: usize,
    
    /// 签发时间
    pub iat: usize,
}

/// Agent 需要的权限 Scope
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AgentScope {
    /// 系统健康检查
    SystemHealth,
    
    /// 读取出版物
    PublicationRead,
    
    /// 写入出版物
    PublicationWrite,
    
    /// 读取文章
    ArticleRead,
    
    /// 写入文章
    ArticleWrite,
    
    /// 读取评论
    CommentRead,
    
    /// 写入评论
    CommentWrite,
    
    /// 搜索
    Search,
    
    /// 读取通知
    NotificationRead,
    
    /// 发送通知
    NotificationWrite,
}

impl AgentScope {
    /// 获取 scope 字符串表示
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::SystemHealth => "blog:system:health",
            Self::PublicationRead => "blog:publication:read",
            Self::PublicationWrite => "blog:publication:write",
            Self::ArticleRead => "blog:article:read",
            Self::ArticleWrite => "blog:article:write",
            Self::CommentRead => "blog:comment:read",
            Self::CommentWrite => "blog:comment:write",
            Self::Search => "blog:search:read",
            Self::NotificationRead => "blog:notification:read",
            Self::NotificationWrite => "blog:notification:write",
        }
    }
    
    /// 获取该 scope 对应的 legacy permission fallback
    pub fn legacy_fallbacks(&self) -> &[&'static str] {
        match self {
            Self::SystemHealth => &[],
            Self::PublicationRead => &["publication.read", "manage_publications"],
            Self::PublicationWrite => &["publication.write", "manage_publications"],
            Self::ArticleRead => &["article.read", "article.create", "publication.read"],
            Self::ArticleWrite => &["article.create", "article.publish", "manage_publications"],
            Self::CommentRead => &["comment.read", "comment.write"],
            Self::CommentWrite => &["comment.write", "manage_comments"],
            Self::Search => &["search", "article.read"],
            Self::NotificationRead => &["notification.read"],
            Self::NotificationWrite => &["notification.write", "manage_notifications"],
        }
    }
}

/// 检查 claims 是否包含指定 scope
pub fn has_scope(claims: &AgentClaims, scope: AgentScope) -> bool {
    // 管理员直接放行
    if claims.role.as_deref() == Some("admin") {
        return true;
    }
    
    let scope_str = scope.as_str();
    let permissions = &claims.permissions;
    
    // 直接检查 scope
    if permissions.iter().any(|p| p == scope_str) {
        return true;
    }
    
    // 检查 legacy fallback
    for fallback in scope.legacy_fallbacks() {
        if permissions.iter().any(|p| p == *fallback) {
            return true;
        }
    }
    
    false
}

/// 验证 JWT Token
pub fn verify_token(token: &str, jwt_secret: &str) -> Result<AgentClaims, jsonwebtoken::errors::Error> {
    let validation = Validation::new(Algorithm::HS256);
    let decoding_key = DecodingKey::from_secret(jwt_secret.as_bytes());
    
    let token_data: TokenData<AgentClaims> = decode(token, &decoding_key, &validation)?;
    
    Ok(token_data.claims)
}

/// 从 Authorization header 提取 token
pub fn extract_token_from_header(auth_header: &str) -> Option<&str> {
    auth_header.strip_prefix("Bearer ")
}

/// Agent 认证中间件
/// 
/// 可选认证：如果有 token 则验证，没有也放行（后续 handler 自行判断）
pub async fn optional_auth_middleware(
    State(state): State<Arc<AppState>>,
    mut request: Request,
    next: Next,
) -> Response {
    // 尝试从 header 提取 token
    if let Some(auth_header) = request
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
    {
        if let Some(token) = extract_token_from_header(auth_header) {
            // 验证 token
            match verify_token(token, &state.config.jwt_secret) {
                Ok(claims) => {
                    // 将 claims 存入 request extensions
                    request.extensions_mut().insert(claims);
                }
                Err(e) => {
                    tracing::debug!("Token validation failed: {}", e);
                    // 可选认证失败不影响请求，只是没有 claims
                }
            }
        }
    }
    
    next.run(request).await
}

/// 要求认证的中间件
/// 
/// 必须有有效的 token，否则返回 401
pub async fn require_auth_middleware(
    State(state): State<Arc<AppState>>,
    mut request: Request,
    next: Next,
) -> Response {
    let request_id = request.extensions().get::<RequestId>().cloned();
    // 尝试从 header 提取 token
    let auth_header = match request
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
    {
        Some(header) => header,
        None => {
            return AgentResponse::<()>::error_with_request_id(
                "unauthorized",
                "Missing authorization header",
                request_id,
            ).into_response();
        }
    };
    
    let token = match extract_token_from_header(auth_header) {
        Some(t) => t,
        None => {
            return AgentResponse::<()>::error_with_request_id(
                "unauthorized",
                "Invalid authorization header format, expected 'Bearer <token>'",
                request.extensions().get::<RequestId>().cloned(),
            ).into_response();
        }
    };
    
    // 验证 token
    match verify_token(token, &state.config.jwt_secret) {
        Ok(claims) => {
            // 将 claims 存入 request extensions
            request.extensions_mut().insert(claims);
            next.run(request).await
        }
        Err(e) => {
            tracing::debug!("Token validation failed: {}", e);
            AgentResponse::<()>::error_with_request_id(
                "unauthorized",
                "Invalid or expired token",
                request.extensions().get::<RequestId>().cloned(),
            ).into_response()
        }
    }
}

/// 检查特定 scope 的中间件工厂
/// 
/// 使用方式：
/// ```rust
/// .route_layer(middleware::from_fn_with_state(
///     state.clone(),
///     require_scope(AgentScope::ArticleRead)
/// ))
/// ```
pub fn require_scope(
    scope: AgentScope,
) -> impl FnOnce(State<Arc<AppState>>, Request, Next) -> Pin<Box<dyn Future<Output = Response> + Send>>
       + Clone {
    move |State(_state): State<Arc<AppState>>, request: Request, next: Next| {
        let scope = scope.clone();
        Box::pin(async move {
            // 从 extensions 获取 claims
            let claims = request.extensions().get::<AgentClaims>();
            
            match claims {
                Some(claims) => {
                    if has_scope(claims, scope) {
                        next.run(request).await
                    } else {
                        crate::agent::response::AgentResponse::<()>::error(
                            "forbidden",
                            format!("Missing required scope: {}", scope.as_str())
                        ).into_response()
                    }
                }
                None => {
                    crate::agent::response::AgentResponse::<()>::error(
                        "unauthorized",
                        "Authentication required"
                    ).into_response()
                }
            }
        })
    }
}

#[derive(Debug, Clone)]
struct ScopeCheck {
    scope: AgentScope,
}

impl ScopeCheck {
    fn new(scope: AgentScope) -> Self {
        Self { scope }
    }
}
