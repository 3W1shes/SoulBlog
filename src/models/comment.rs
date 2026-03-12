use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use validator::Validate;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Comment {
    #[serde(with = "crate::utils::serde_helpers::thing_id")]
    pub id: String,
    #[serde(with = "crate::utils::serde_helpers::thing_id")]
    pub article_id: String,
    #[serde(with = "crate::utils::serde_helpers::thing_id")]
    pub author_id: String,
    #[serde(default, with = "crate::utils::serde_helpers::thing_id_option")]
    pub parent_id: Option<String>,
    pub content: String,
    pub is_author_response: bool,
    pub clap_count: i64,
    pub is_edited: bool,
    pub is_deleted: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(default)]
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommentWithAuthor {
    #[serde(flatten)]
    pub comment: Comment,
    pub author_name: String,
    pub author_username: String,
    pub author_avatar: Option<String>,
    pub user_has_clapped: bool,
    pub replies: Vec<CommentWithAuthor>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct CreateCommentRequest {
    #[validate(length(min = 1, max = 200000))]
    pub article_id: String,
    pub parent_id: Option<String>,
    #[validate(length(min = 1, max = 10000))]
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct UpdateCommentRequest {
    #[validate(length(min = 1, max = 10000))]
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommentClap {
    #[serde(with = "crate::utils::serde_helpers::thing_id")]
    pub id: String,
    #[serde(with = "crate::utils::serde_helpers::thing_id")]
    pub user_id: String,
    #[serde(with = "crate::utils::serde_helpers::thing_id")]
    pub comment_id: String,
    pub created_at: DateTime<Utc>,
}
