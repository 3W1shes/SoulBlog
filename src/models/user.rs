use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;
use uuid::Uuid;
use validator::Validate;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserProfile {
    pub id: Thing,
    pub user_id: String, // Rainbow-Auth 用户ID
    pub username: String,
    pub display_name: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub email: Option<String>, // 从Rainbow-Auth获取的邮箱（不存储在数据库中）
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub email_verified: Option<bool>, // 从Rainbow-Auth获取的邮箱验证状态（不存储在数据库中）
    #[serde(default)]
    pub bio: Option<String>,
    #[serde(default)]
    pub avatar_url: Option<String>,
    #[serde(default)]
    pub cover_image_url: Option<String>,
    #[serde(default)]
    pub website: Option<String>,
    #[serde(default)]
    pub location: Option<String>,
    #[serde(default)]
    pub twitter_username: Option<String>,
    #[serde(default)]
    pub github_username: Option<String>,
    #[serde(default)]
    pub linkedin_url: Option<String>,
    #[serde(default)]
    pub facebook_url: Option<String>,
    #[serde(default)]
    pub stripe_customer_id: Option<String>,
    #[serde(default)]
    pub stripe_account_id: Option<String>,
    pub follower_count: i64,
    pub following_count: i64,
    pub article_count: i64,
    pub total_claps_received: i64,
    pub is_verified: bool,
    pub is_suspended: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct CreateUserProfileRequest {
    #[validate(length(min = 3, max = 30))]
    pub username: String,

    #[validate(length(min = 1, max = 50))]
    pub display_name: String,

    #[validate(length(max = 160))]
    pub bio: Option<String>,

    #[validate(url)]
    pub website: Option<String>,

    #[validate(length(max = 100))]
    pub location: Option<String>,

    #[validate(length(max = 15))]
    pub twitter_username: Option<String>,

    #[validate(length(max = 39))]
    pub github_username: Option<String>,

    #[validate(url)]
    pub linkedin_url: Option<String>,

    #[validate(url)]
    pub facebook_url: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct UpdateUserProfileRequest {
    #[validate(length(min = 1, max = 50))]
    pub display_name: Option<String>,

    #[validate(length(max = 160))]
    pub bio: Option<String>,

    #[validate(url)]
    pub avatar_url: Option<String>,

    #[validate(url)]
    pub cover_image_url: Option<String>,

    #[validate(url)]
    pub website: Option<String>,

    #[validate(length(max = 100))]
    pub location: Option<String>,

    #[validate(length(max = 15))]
    pub twitter_username: Option<String>,

    #[validate(length(max = 39))]
    pub github_username: Option<String>,

    #[validate(url)]
    pub linkedin_url: Option<String>,

    #[validate(url)]
    pub facebook_url: Option<String>,
}

/// 邮箱更新请求（需要通过Rainbow-Auth验证）
#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct UpdateEmailRequest {
    #[validate(email(message = "Invalid email format"))]
    pub email: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UserProfileResponse {
    pub id: String,
    pub user_id: String,
    pub username: String,
    pub display_name: String,
    pub email: String,        // 包含邮箱信息
    pub email_verified: bool, // 包含邮箱验证状态
    pub bio: Option<String>,
    pub avatar_url: Option<String>,
    pub cover_image_url: Option<String>,
    pub website: Option<String>,
    pub location: Option<String>,
    pub twitter_username: Option<String>,
    pub github_username: Option<String>,
    pub linkedin_url: Option<String>,
    pub facebook_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stripe_customer_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stripe_account_id: Option<String>,
    pub follower_count: i64,
    pub following_count: i64,
    pub article_count: i64,
    pub total_claps_received: i64,
    pub is_verified: bool,
    pub is_suspended: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UserStats {
    pub total_users: i64,
    pub verified_users: i64,
    pub active_users: i64, // 最近30天活跃
    pub new_users_this_month: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UserActivitySummary {
    pub articles_written: i64,
    pub comments_made: i64,
    pub claps_given: i64,
    pub claps_received: i64,
    pub followers: i64,
    pub following: i64,
}

impl UserProfile {
    pub fn new(user_id: String, username: String, display_name: String) -> Self {
        let now = Utc::now();
        Self {
            id: Thing::from(("user_profile".to_string(), Uuid::new_v4().to_string())),
            user_id,
            username,
            display_name,
            email: None,
            email_verified: None,
            bio: None,
            avatar_url: None,
            cover_image_url: None,
            website: None,
            location: None,
            twitter_username: None,
            github_username: None,
            linkedin_url: None,
            facebook_url: None,
            stripe_customer_id: None,
            stripe_account_id: None,
            follower_count: 0,
            following_count: 0,
            article_count: 0,
            total_claps_received: 0,
            is_verified: false,
            is_suspended: false,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn to_response(&self) -> UserProfileResponse {
        UserProfileResponse {
            id: self.id.id.to_string(),
            user_id: self.user_id.clone(),
            username: self.username.clone(),
            display_name: self.display_name.clone(),
            email: self.email.clone().unwrap_or_default(), // 如果没有 email，返回空字符串
            email_verified: self.email_verified.unwrap_or(false),
            bio: self.bio.clone(),
            avatar_url: self.avatar_url.clone(),
            cover_image_url: self.cover_image_url.clone(),
            website: self.website.clone(),
            location: self.location.clone(),
            twitter_username: self.twitter_username.clone(),
            github_username: self.github_username.clone(),
            linkedin_url: self.linkedin_url.clone(),
            facebook_url: self.facebook_url.clone(),
            stripe_customer_id: self.stripe_customer_id.clone(),
            stripe_account_id: self.stripe_account_id.clone(),
            follower_count: self.follower_count,
            following_count: self.following_count,
            article_count: self.article_count,
            total_claps_received: self.total_claps_received,
            is_verified: self.is_verified,
            is_suspended: self.is_suspended,
            created_at: self.created_at,
        }
    }
}

impl From<CreateUserProfileRequest> for UserProfile {
    fn from(req: CreateUserProfileRequest) -> Self {
        let now = Utc::now();
        Self {
            id: Thing::from(("user_profile".to_string(), Uuid::new_v4().to_string())),
            user_id: String::new(), // 将在服务层设置
            username: req.username,
            display_name: req.display_name,
            email: None,          // 将在服务层设置
            email_verified: None, // 默认未验证
            bio: req.bio,
            avatar_url: None,
            cover_image_url: None,
            website: req.website,
            location: req.location,
            twitter_username: req.twitter_username,
            github_username: req.github_username,
            linkedin_url: req.linkedin_url,
            facebook_url: req.facebook_url,
            stripe_customer_id: None,
            stripe_account_id: None,
            follower_count: 0,
            following_count: 0,
            article_count: 0,
            total_claps_received: 0,
            is_verified: false,
            is_suspended: false,
            created_at: now,
            updated_at: now,
        }
    }
}
