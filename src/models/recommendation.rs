use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use crate::models::article::ArticleListItem;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecommendationResult {
    pub articles: Vec<RecommendedArticle>,
    pub total: usize,
    pub algorithm_used: String,
    pub generated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecommendedArticle {
    #[serde(flatten)]
    pub article: ArticleListItem,
    pub score: f64,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInteraction {
    pub id: String,
    pub user_id: String,
    pub article_id: String,
    pub interaction_type: String,
    pub weight: f64,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InteractionType {
    View,
    Clap,
    Comment,
    Bookmark,
    Share,
    ReadComplete,
}

impl InteractionType {
    pub fn as_str(&self) -> &'static str {
        match self {
            InteractionType::View => "View",
            InteractionType::Clap => "Clap",
            InteractionType::Comment => "Comment",
            InteractionType::Bookmark => "Bookmark",
            InteractionType::Share => "Share",
            InteractionType::ReadComplete => "ReadComplete",
        }
    }

    pub fn default_weight(&self) -> f64 {
        match self {
            InteractionType::View => 1.0,
            InteractionType::Clap => 3.0,
            InteractionType::Comment => 5.0,
            InteractionType::Bookmark => 4.0,
            InteractionType::Share => 6.0,
            InteractionType::ReadComplete => 8.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentVector {
    pub article_id: String,
    pub tags: Vec<String>,
    pub author_id: String,
    pub reading_time: i32,
    pub word_count: i32,
    pub clap_ratio: f64,
    pub comment_ratio: f64,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserProfile {
    pub user_id: String,
    pub preferred_tags: Vec<TagPreference>,
    pub preferred_authors: Vec<AuthorPreference>,
    pub avg_reading_time: f64,
    pub total_interactions: i64,
    pub last_updated: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagPreference {
    pub tag_id: String,
    pub tag_name: String,
    pub weight: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthorPreference {
    pub author_id: String,
    pub weight: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrendingMetrics {
    pub article_id: String,
    pub views_24h: i64,
    pub views_7d: i64,
    pub claps_24h: i64,
    pub claps_7d: i64,
    pub comments_24h: i64,
    pub comments_7d: i64,
    pub trending_score: f64,
    pub calculated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RecommendationRequest {
    pub user_id: Option<String>,
    pub limit: Option<usize>,
    pub exclude_read: Option<bool>,
    pub algorithm: Option<RecommendationAlgorithm>,
    pub tags: Option<Vec<String>>,
    pub authors: Option<Vec<String>>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum RecommendationAlgorithm {
    #[serde(rename = "content_based")]
    ContentBased,
    #[serde(rename = "collaborative_filtering")]
    CollaborativeFiltering,
    #[serde(rename = "hybrid")]
    Hybrid,
    #[serde(rename = "trending")]
    Trending,
    #[serde(rename = "following")]
    Following,
}

impl Default for RecommendationRequest {
    fn default() -> Self {
        Self {
            user_id: None,
            limit: Some(10),
            exclude_read: Some(true),
            algorithm: Some(RecommendationAlgorithm::Hybrid),
            tags: None,
            authors: None,
        }
    }
}
