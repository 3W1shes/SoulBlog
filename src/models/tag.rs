use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use validator::Validate;
use crate::utils::serde_helpers::{loose_datetime_now, loose_i64, thing_id};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tag {
    #[serde(with = "thing_id")]
    pub id: String,
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    #[serde(default, deserialize_with = "loose_i64::deserialize")]
    pub follower_count: i64,
    #[serde(default, deserialize_with = "loose_i64::deserialize")]
    pub article_count: i64,
    pub is_featured: bool,
    #[serde(default = "Utc::now", deserialize_with = "loose_datetime_now::deserialize")]
    pub created_at: DateTime<Utc>,
    #[serde(default = "Utc::now", deserialize_with = "loose_datetime_now::deserialize")]
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct CreateTagRequest {
    #[validate(length(min = 1, max = 50))]
    pub name: String,
    #[validate(length(max = 200))]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct UpdateTagRequest {
    #[validate(length(min = 1, max = 50))]
    pub name: Option<String>,
    #[validate(length(max = 200))]
    pub description: Option<String>,
    pub is_featured: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArticleTag {
    #[serde(with = "thing_id")]
    pub id: String,
    #[serde(with = "thing_id")]
    pub article_id: String,
    #[serde(with = "thing_id")]
    pub tag_id: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserTagFollow {
    #[serde(with = "thing_id")]
    pub id: String,
    #[serde(with = "thing_id")]
    pub user_id: String,
    #[serde(with = "thing_id")]
    pub tag_id: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagWithFollowStatus {
    #[serde(flatten)]
    pub tag: Tag,
    pub is_following: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagQuery {
    pub search: Option<String>,
    pub featured_only: Option<bool>,
    pub sort_by: Option<String>, // popular, name, created_at
    pub page: Option<i32>,
    pub limit: Option<i32>,
}
