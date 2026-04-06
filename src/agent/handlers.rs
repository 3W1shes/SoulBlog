//! Agent API 处理器
//!
//! 处理 OpenClaw / Agent 的具体请求

use axum::{
    extract::{Path, Query, State},
    Extension,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::{
    agent::{
        auth::{has_scope, AgentClaims, AgentScope},
        request_id::RequestId,
        response::AgentResponse,
    },
    error::AppError,
    models::{
        article::{ArticleListItem, ArticleQuery, ArticleResponse},
        comment::{Comment, CommentWithAuthor},
        publication::{PublicationListItem, PublicationQuery, PublicationResponse},
    },
    state::AppState,
};

// ==================== System Health ====================

/// 健康检查响应
#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
    pub capabilities: Vec<String>,
}

/// 健康检查处理器
pub async fn health_check(
    request_id: Option<Extension<RequestId>>,
) -> AgentResponse<HealthResponse> {
    AgentResponse::success_with_request_id(HealthResponse {
        status: "ok".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        capabilities: vec![
            "system.health".to_string(),
            "publication.list".to_string(),
            "publication.get".to_string(),
            "article.list".to_string(),
            "article.get".to_string(),
            "search".to_string(),
            "comment.list".to_string(),
            "comment.create".to_string(),
        ],
    }, request_id.map(|Extension(request_id)| request_id))
}

// ==================== Publications ====================

/// 列出出版物查询参数
#[derive(Debug, Deserialize)]
pub struct ListPublicationsQuery {
    #[serde(flatten)]
    pub query: PublicationQuery,
}

/// 列出出版物响应
#[derive(Debug, Serialize)]
pub struct ListPublicationsResponse {
    pub publications: Vec<PublicationListItem>,
    pub total: i64,
    pub page: usize,
    pub per_page: usize,
}

/// 列出出版物处理器
pub async fn list_publications(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ListPublicationsQuery>,
    _claims: Option<Extension<AgentClaims>>,
    request_id: Option<Extension<RequestId>>,
) -> Result<AgentResponse<ListPublicationsResponse>, AppError> {
    let result = state
        .publication_service
        .get_publications(query.query)
        .await?;

    Ok(AgentResponse::success_with_request_id(
        ListPublicationsResponse {
            publications: result.data,
            total: result.total as i64,
            page: result.page,
            per_page: result.per_page,
        },
        request_id.map(|Extension(request_id)| request_id),
    ))
}

/// 获取单个出版物处理器
pub async fn get_publication(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    claims: Option<Extension<AgentClaims>>,
    request_id: Option<Extension<RequestId>>,
) -> Result<AgentResponse<PublicationResponse>, AppError> {
    let user_id = claims.as_ref().map(|c| c.0.sub.as_str());
    let response = state
        .publication_service
        .get_publication(&id, user_id)
        .await?
        .ok_or_else(|| AppError::not_found("Publication"))?;

    Ok(AgentResponse::success_with_request_id(
        response,
        request_id.map(|Extension(request_id)| request_id),
    ))
}

// ==================== Articles ====================

/// 列出文章查询参数
#[derive(Debug, Deserialize)]
pub struct ListArticlesQuery {
    #[serde(flatten)]
    pub query: ArticleQuery,
    
    /// 按出版物筛选
    pub publication_id: Option<String>,
}

/// 列出文章响应
#[derive(Debug, Serialize)]
pub struct ListArticlesResponse {
    pub articles: Vec<ArticleListItem>,
    pub total: i64,
    pub page: usize,
    pub per_page: usize,
}

/// 列出文章处理器
pub async fn list_articles(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ListArticlesQuery>,
    _claims: Option<Extension<AgentClaims>>,
    request_id: Option<Extension<RequestId>>,
) -> Result<AgentResponse<ListArticlesResponse>, AppError> {
    let mut article_query = query.query;

    if article_query.status.is_none() {
        article_query.status = Some("published".to_string());
    }

    if let Some(pub_id) = query.publication_id {
        article_query.publication = Some(pub_id);
    }

    let result = state
        .article_service
        .get_articles(article_query)
        .await?;

    Ok(AgentResponse::success_with_request_id(
        ListArticlesResponse {
            articles: result.data,
            total: result.total as i64,
            page: result.page,
            per_page: result.per_page,
        },
        request_id.map(|Extension(request_id)| request_id),
    ))
}

/// 获取单个文章处理器
pub async fn get_article(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    claims: Option<Extension<AgentClaims>>,
    request_id: Option<Extension<RequestId>>,
) -> Result<AgentResponse<ArticleResponse>, AppError> {
    let user_id = claims.as_ref().map(|c| c.0.sub.clone());

    let article = state
        .article_service
        .get_article_with_details(&id, user_id.as_deref())
        .await?;

    Ok(AgentResponse::success_with_request_id(
        article.ok_or_else(|| AppError::not_found("Article"))?,
        request_id.map(|Extension(request_id)| request_id),
    ))
}

// ==================== Search ====================

/// 搜索查询参数
#[derive(Debug, Deserialize)]
pub struct SearchQueryParams {
    /// 搜索关键词
    pub q: String,
    
    /// 搜索类型：article, publication, all
    #[serde(default = "default_search_type")]
    pub r#type: String,
    
    /// 分页
    #[serde(default = "default_page")]
    pub page: usize,
    
    /// 每页数量
    #[serde(default = "default_per_page")]
    pub per_page: usize,
    
    /// 按出版物筛选
    pub publication_id: Option<String>,
    
    /// 按作者筛选
    pub author_id: Option<String>,
    
    /// 按标签筛选
    pub tag: Option<String>,
}

fn default_search_type() -> String {
    "all".to_string()
}

fn default_page() -> usize {
    1
}

fn default_per_page() -> usize {
    20
}

/// 搜索结果项
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum SearchResultItem {
    Article(ArticleListItem),
    Publication(PublicationListItem),
}

/// 搜索响应
#[derive(Debug, Serialize)]
pub struct SearchResponse {
    pub results: Vec<SearchResultItem>,
    pub total: i64,
    pub page: usize,
    pub per_page: usize,
    pub query: String,
}

/// 搜索处理器
pub async fn search(
    State(state): State<Arc<AppState>>,
    Query(query): Query<SearchQueryParams>,
    claims: Option<Extension<AgentClaims>>,
    request_id: Option<Extension<RequestId>>,
) -> Result<AgentResponse<SearchResponse>, AppError> {
    // 检查搜索权限
    if let Some(Extension(ref claims)) = claims {
        if !has_scope(claims, AgentScope::Search) {
            // 没有 search scope 但有 read scope 也可以搜索
            if !has_scope(claims, AgentScope::ArticleRead) {
                return Ok(AgentResponse::error_with_request_id(
                    "forbidden",
                    "Missing required scope: blog:search:read or blog:article:read",
                    request_id.clone().map(|Extension(request_id)| request_id),
                ));
            }
        }
    }
    
    let per_page = query.per_page.min(100);
    
    // 使用现有的 search service
    let search_query = crate::models::search::SearchQuery {
        q: query.q.clone(),
        search_type: Some(match query.r#type.as_str() {
            "article" => crate::models::search::SearchType::Articles,
            "publication" => crate::models::search::SearchType::Publications,
            _ => crate::models::search::SearchType::All,
        }),
        page: Some(query.page as i32),
        limit: Some(per_page as i32),
    };
    
    let search_results = state
        .search_service
        .search(search_query)
        .await?;
    
    // 转换搜索结果
    let mut results: Vec<SearchResultItem> = vec![];
    
    // 转换文章搜索结果
    for article in search_results.articles {
        results.push(SearchResultItem::Article(ArticleListItem {
            id: article.id,
            title: article.title,
            subtitle: None,
            slug: article.slug,
            excerpt: article.excerpt,
            cover_image_url: article.cover_image_url,
            author: crate::models::article::AuthorInfo {
                id: String::new(),
                username: article.author_username.clone(),
                display_name: article.author_name.clone(),
                avatar_url: None,
                is_verified: false,
            },
            publication: None,
            status: crate::models::article::ArticleStatus::Published,
            is_paid_content: false,
            is_featured: false,
            reading_time: article.reading_time,
            view_count: 0,
            clap_count: article.clap_count,
            comment_count: article.comment_count,
            tags: article.tags.into_iter().map(|name| crate::models::article::TagInfo {
                id: String::new(),
                name,
                slug: String::new(),
            }).collect(),
            created_at: article.published_at,
            published_at: Some(article.published_at),
        }));
    }
    
    // 转换出版物搜索结果
    for pub_result in search_results.publications {
        results.push(SearchResultItem::Publication(PublicationListItem {
            id: pub_result.id,
            name: pub_result.name,
            slug: pub_result.slug,
            description: pub_result.description,
            tagline: pub_result.tagline,
            logo_url: pub_result.logo_url,
            cover_image_url: None,
            member_count: pub_result.member_count,
            article_count: pub_result.article_count,
            follower_count: pub_result.follower_count,
            is_verified: false,
            created_at: chrono::Utc::now(),
        }));
    }
    
    Ok(AgentResponse::success_with_request_id(
        SearchResponse {
            results,
            total: search_results.total_results,
            page: query.page,
            per_page,
            query: query.q,
        },
        request_id.map(|Extension(request_id)| request_id),
    ))
}

// ==================== Comments ====================

/// 列出评论查询参数
#[derive(Debug, Deserialize)]
pub struct ListCommentsQuery {
    /// 文章 ID
    pub article_id: String,
    
    /// 分页
    #[serde(default = "default_page")]
    pub page: usize,
    
    /// 每页数量
    #[serde(default = "default_per_page")]
    pub per_page: usize,
}

/// 列出评论响应
#[derive(Debug, Serialize)]
pub struct ListCommentsResponse {
    pub comments: Vec<CommentWithAuthor>,
    pub total: i64,
    pub page: usize,
    pub per_page: usize,
}

/// 列出评论处理器
pub async fn list_comments(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ListCommentsQuery>,
    claims: Option<Extension<AgentClaims>>,
    request_id: Option<Extension<RequestId>>,
) -> Result<AgentResponse<ListCommentsResponse>, AppError> {
    let user_id = claims.as_ref().map(|c| c.0.sub.clone());
    
    let comments = state
        .comment_service
        .get_article_comments(&query.article_id, user_id.as_deref())
        .await?;
    
    let total = comments.len() as i64;
    
    Ok(AgentResponse::success_with_request_id(
        ListCommentsResponse {
            comments,
            total,
            page: query.page,
            per_page: query.per_page.min(100),
        },
        request_id.map(|Extension(request_id)| request_id),
    ))
}

/// 创建评论处理器
pub async fn create_comment(
    State(state): State<Arc<AppState>>,
    claims: Extension<AgentClaims>,
    request_id: Option<Extension<RequestId>>,
    axum::Json(body): axum::Json<crate::models::comment::CreateCommentRequest>,
) -> Result<AgentResponse<Comment>, AppError> {
    // 检查写权限
    if !has_scope(&claims.0, AgentScope::CommentWrite) {
        return Ok(AgentResponse::error_with_request_id(
            "forbidden",
            "Missing required scope: blog:comment:write",
            request_id.clone().map(|Extension(request_id)| request_id),
        ));
    }
    
    let comment = state
        .comment_service
        .create_comment(&claims.0.sub, body)
        .await?;
    
    Ok(AgentResponse::success_with_request_id(
        comment,
        request_id.map(|Extension(request_id)| request_id),
    ))
}

// Note: Notification endpoints removed - requires additional service methods
// To be implemented when notification service supports:
// - get_user_notifications(user_id, limit)
// - get_unread_count(user_id)
// - mark_all_as_read(user_id)
