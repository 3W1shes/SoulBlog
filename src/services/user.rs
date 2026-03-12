use crate::{
    error::{AppError, Result},
    models::user::*,
    services::Database,
};
use chrono::Utc;
use serde_json::{json, Value};
use std::sync::Arc;
use soulcore::prelude::Thing;
use surrealdb::types::ToSql;
use tracing::{debug, error, info};
use uuid::Uuid;
use validator::Validate;

/// 用户服务，处理用户相关的业务逻辑
#[derive(Clone)]
pub struct UserService {
    db: Arc<Database>,
}

impl UserService {
    fn first_profile_from_vec(rows: Vec<UserProfile>) -> Option<UserProfile> {
        rows.into_iter().next()
    }

    /// 创建新的用户服务实例
    pub async fn new(db: Arc<Database>) -> Result<Self> {
        Ok(Self { db })
    }

    /// 创建新用户资料
    pub async fn create_profile(&self, user_id: &str, email: &str) -> Result<UserProfile> {
        debug!("Creating new user profile for user: {}", user_id);

        // 检查是否已有资料
        if let Some(existing) = self.get_profile_by_user_id(user_id).await? {
            return Ok(existing);
        }

        // 从邮箱生成用户名
        let mut base_username = email
            .split('@')
            .next()
            .unwrap_or("user")
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '_' || *c == '-')
            .collect::<String>()
            .to_lowercase();

        // 确保用户名至少有3个字符
        if base_username.len() < 3 {
            base_username = format!("user_{}", base_username);
        }

        let original_username = if base_username.is_empty() {
            format!("user{}", Uuid::new_v4().simple())
        } else {
            base_username
        };

        // 生成唯一用户名
        let generated_profile_id = Uuid::new_v4().to_string();

        let mut profile = UserProfile {
            id: Thing::new("user_profile", generated_profile_id.clone()),
            user_id: user_id.to_string(),
            username: original_username.clone(),
            display_name: original_username.clone(),
            email: Some(email.to_string()), // 添加邮箱字段
            email_verified: Some(false),    // 默认未验证，稍后从Rainbow-Auth获取真实状态
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
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        // 确保用户名唯一
        let mut counter = 1;
        while self.is_username_taken(&profile.username).await? {
            profile.username = format!("{}_{}", original_username, counter);
            counter += 1;

            if counter > 100 {
                return Err(AppError::Internal(
                    "Failed to generate unique username".to_string(),
                ));
            }
        }

        // 创建用户资料，使用 SurrealDB 的 time::now() 函数
        let create_query = format!(
            r#"
            CREATE user_profile CONTENT {{
                id: "{}",
                user_id: "{}",
                username: "{}",
                display_name: "{}",
                email: {},
                email_verified: {},
                bio: {},
                avatar_url: {},
                cover_image_url: {},
                website: {},
                location: {},
                twitter_username: {},
                github_username: {},
                linkedin_url: {},
                facebook_url: {},
                stripe_customer_id: NONE,
                stripe_account_id: NONE,
                follower_count: {},
                following_count: {},
                article_count: {},
                total_claps_received: {},
                is_verified: {},
                is_suspended: {},
                created_at: time::now(),
                updated_at: time::now()
            }}
            RETURN *, meta::id(id) AS id
            "#,
            generated_profile_id,
            profile.user_id,
            profile.username,
            profile.display_name,
            profile
                .email
                .as_ref()
                .map(|s| format!("\"{}\"", s))
                .unwrap_or("NONE".to_string()),
            profile.email_verified.unwrap_or(false),
            profile
                .bio
                .as_ref()
                .map(|s| format!("\"{}\"", s))
                .unwrap_or("NONE".to_string()),
            profile
                .avatar_url
                .as_ref()
                .map(|s| format!("\"{}\"", s))
                .unwrap_or("NONE".to_string()),
            profile
                .cover_image_url
                .as_ref()
                .map(|s| format!("\"{}\"", s))
                .unwrap_or("NONE".to_string()),
            profile
                .website
                .as_ref()
                .map(|s| format!("\"{}\"", s))
                .unwrap_or("NONE".to_string()),
            profile
                .location
                .as_ref()
                .map(|s| format!("\"{}\"", s))
                .unwrap_or("NONE".to_string()),
            profile
                .twitter_username
                .as_ref()
                .map(|s| format!("\"{}\"", s))
                .unwrap_or("NONE".to_string()),
            profile
                .github_username
                .as_ref()
                .map(|s| format!("\"{}\"", s))
                .unwrap_or("NONE".to_string()),
            profile
                .linkedin_url
                .as_ref()
                .map(|s| format!("\"{}\"", s))
                .unwrap_or("NONE".to_string()),
            profile
                .facebook_url
                .as_ref()
                .map(|s| format!("\"{}\"", s))
                .unwrap_or("NONE".to_string()),
            profile.follower_count,
            profile.following_count,
            profile.article_count,
            profile.total_claps_received,
            profile.is_verified,
            profile.is_suspended,
        );

        let mut response = self.db.query(&create_query).await?;
        let created_profile = Self::first_profile_from_vec(response.take::<Vec<UserProfile>>(0)?);

        let Some(created_profile) = created_profile else {
            error!("No user profile was created for user: {}", user_id);
            return Err(AppError::Internal(
                "Failed to create user profile".to_string(),
            ));
        };
        info!(
            "Created new user profile for user: {} with username: {}",
            user_id, created_profile.username
        );

        Ok(created_profile)
    }

    /// 根据用户ID获取用户资料
    pub async fn get_profile_by_user_id(&self, user_id: &str) -> Result<Option<UserProfile>> {
        debug!("Getting user profile by user_id: {}", user_id);

        let query = "SELECT *, meta::id(id) AS id FROM user_profile WHERE user_id = $user_id LIMIT 1";
        let mut response = self
            .db
            .query_with_params(query, json!({ "user_id": user_id }))
            .await?;
        Ok(Self::first_profile_from_vec(response.take::<Vec<UserProfile>>(0)?))
    }

    /// 统计用户已发布且未删除的文章数量
    pub async fn count_published_articles(&self, user_id: &str) -> Result<i64> {
        let query = r#"
            SELECT count() AS count 
            FROM article 
            WHERE author_id = $user_id 
            AND is_deleted = false 
            AND status = 'published'
        "#;
        let mut resp = self
            .db
            .query_with_params(
                query,
                json!({
                    "user_id": user_id
                }),
            )
            .await?;
        if let Ok(Some(row)) = resp.take::<Option<Value>>(0) {
            Ok(row.get("count").and_then(|v| v.as_i64()).unwrap_or(0))
        } else {
            Ok(0)
        }
    }

    /// 统计粉丝数量（关注该用户的人）
    pub async fn count_followers(&self, user_id: &str) -> Result<i64> {
        let query = r#"
            SELECT count() AS count 
            FROM follow 
            WHERE following_id = $user_id
        "#;
        let mut resp = self
            .db
            .query_with_params(
                query,
                json!({
                    "user_id": user_id
                }),
            )
            .await?;
        if let Ok(Some(row)) = resp.take::<Option<Value>>(0) {
            Ok(row.get("count").and_then(|v| v.as_i64()).unwrap_or(0))
        } else {
            Ok(0)
        }
    }

    /// 统计关注数量（该用户关注了多少人）
    pub async fn count_following(&self, user_id: &str) -> Result<i64> {
        let query = r#"
            SELECT count() AS count 
            FROM follow 
            WHERE follower_id = $user_id
        "#;
        let mut resp = self
            .db
            .query_with_params(
                query,
                json!({
                    "user_id": user_id
                }),
            )
            .await?;
        if let Ok(Some(row)) = resp.take::<Option<Value>>(0) {
            Ok(row.get("count").and_then(|v| v.as_i64()).unwrap_or(0))
        } else {
            Ok(0)
        }
    }

    /// 根据用户名获取用户资料
    pub async fn get_profile_by_username(&self, username: &str) -> Result<Option<UserProfile>> {
        debug!("Getting user profile by username: {}", username);

        self.db.find_one("user_profile", "username", username).await
    }

    /// 更新用户资料
    pub async fn update_profile(
        &self,
        user_id: &str,
        update_request: UpdateUserProfileRequest,
    ) -> Result<UserProfile> {
        debug!("Updating user profile for user: {}", user_id);

        // 验证输入
        update_request
            .validate()
            .map_err(|e| AppError::ValidatorError(e))?;

        // 获取现有资料
        let mut profile = self
            .get_profile_by_user_id(user_id)
            .await?
            .ok_or_else(|| AppError::NotFound("User profile not found".to_string()))?;

        // 更新字段
        if let Some(display_name) = update_request.display_name {
            profile.display_name = display_name;
        }
        if let Some(bio) = update_request.bio {
            profile.bio = Some(bio);
        }
        if let Some(avatar_url) = update_request.avatar_url {
            profile.avatar_url = Some(avatar_url);
        }
        if let Some(cover_image_url) = update_request.cover_image_url {
            profile.cover_image_url = Some(cover_image_url);
        }
        if let Some(website) = update_request.website {
            profile.website = Some(website);
        }
        if let Some(location) = update_request.location {
            profile.location = Some(location);
        }
        if let Some(twitter_username) = update_request.twitter_username {
            profile.twitter_username = Some(twitter_username);
        }
        if let Some(github_username) = update_request.github_username {
            profile.github_username = Some(github_username);
        }
        if let Some(linkedin_url) = update_request.linkedin_url {
            profile.linkedin_url = Some(linkedin_url);
        }
        if let Some(facebook_url) = update_request.facebook_url {
            profile.facebook_url = Some(facebook_url);
        }

        profile.updated_at = Utc::now();

        // 更新数据库
        let result = self
            .db
            .update(profile.id.clone(), profile)
            .await?
            .ok_or_else(|| AppError::NotFound("Failed to update profile".to_string()))?;

        info!("Updated user profile for user: {}", user_id);

        Ok(result)
    }

    /// 检查用户名是否已被使用
    pub async fn is_username_taken(&self, username: &str) -> Result<bool> {
        let query = "SELECT count() AS count FROM user_profile WHERE username = $username";
        let mut response = self
            .db
            .query_with_params(query, json!({ "username": username }))
            .await?;

        if let Ok(Some(result)) = response.take::<Option<Value>>(0) {
            if let Some(count) = result.get("count").and_then(|v| v.as_i64()) {
                return Ok(count > 0);
            }
        }

        Ok(false)
    }

    /// 获取用户统计信息
    pub async fn get_user_stats(&self, user_id: &str) -> Result<UserActivitySummary> {
        debug!("Getting user statistics for user: {}", user_id);

        // 获取文章统计
        let articles_query = "SELECT count() AS count FROM article WHERE user_id = $user_id";
        let mut articles_response = self
            .db
            .query_with_params(articles_query, json!({ "user_id": user_id }))
            .await?;
        let article_count = if let Ok(Some(result)) = articles_response.take::<Option<Value>>(0) {
            result.get("count").and_then(|v| v.as_i64()).unwrap_or(0)
        } else {
            0
        };

        // 获取评论统计
        let comments_query = "SELECT count() AS count FROM comment WHERE user_id = $user_id";
        let mut comments_response = self
            .db
            .query_with_params(comments_query, json!({ "user_id": user_id }))
            .await?;
        let comment_count = if let Ok(Some(result)) = comments_response.take::<Option<Value>>(0) {
            result.get("count").and_then(|v| v.as_i64()).unwrap_or(0)
        } else {
            0
        };

        // 获取关注者统计
        let followers_query = "SELECT count() AS count FROM follow WHERE following = $user_id";
        let mut followers_response = self
            .db
            .query_with_params(followers_query, json!({ "user_id": user_id }))
            .await?;
        let follower_count = if let Ok(Some(result)) = followers_response.take::<Option<Value>>(0) {
            result.get("count").and_then(|v| v.as_i64()).unwrap_or(0)
        } else {
            0
        };

        // 获取关注统计
        let following_query = "SELECT count() AS count FROM follow WHERE follower = $user_id";
        let mut following_response = self
            .db
            .query_with_params(following_query, json!({ "user_id": user_id }))
            .await?;
        let following_count = if let Ok(Some(result)) = following_response.take::<Option<Value>>(0)
        {
            result.get("count").and_then(|v| v.as_i64()).unwrap_or(0)
        } else {
            0
        };

        // 获取用户给出的拍手数
        let claps_given_query = r#"
            SELECT math::sum(count) as total_claps 
            FROM clap 
            WHERE user_id = $user_id
        "#;

        let mut claps_given_response = self
            .db
            .query_with_params(
                claps_given_query,
                json!({
                    "user_id": user_id
                }),
            )
            .await?;

        let claps_given = if let Ok(Some(result)) = claps_given_response.take::<Option<Value>>(0) {
            result
                .get("total_claps")
                .and_then(|v| v.as_i64())
                .unwrap_or(0)
        } else {
            0
        };

        // 获取用户收到的拍手数（通过用户的文章）
        let claps_received_query = r#"
            SELECT math::sum(count) as total_claps
            FROM clap
            WHERE article_id IN (
                SELECT id FROM article WHERE author_id = $user_id
            )
        "#;

        let mut claps_received_response = self
            .db
            .query_with_params(
                claps_received_query,
                json!({
                    "user_id": user_id
                }),
            )
            .await?;

        let claps_received =
            if let Ok(Some(result)) = claps_received_response.take::<Option<Value>>(0) {
                result
                    .get("total_claps")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0)
            } else {
                0
            };

        Ok(UserActivitySummary {
            articles_written: article_count,
            comments_made: comment_count,
            claps_given,
            claps_received,
            followers: follower_count,
            following: following_count,
        })
    }

    /// 更新最后活跃时间
    pub async fn update_last_active(&self, user_id: &str) -> Result<()> {
        debug!("Updating last active time for user: {}", user_id);

        let update_query = "UPDATE user_profile SET last_active_at = $now WHERE user_id = $user_id";
        self.db
            .query_with_params(
                update_query,
                json!({
                    "user_id": user_id,
                    "now": Utc::now()
                }),
            )
            .await?;

        Ok(())
    }

    /// 获取用户列表（分页）
    pub async fn get_users(
        &self,
        page: usize,
        limit: usize,
        search: Option<String>,
    ) -> Result<crate::services::database::PaginatedResult<UserProfile>> {
        let offset = (page - 1) * limit;

        let (query, params) = if let Some(search_term) = search.clone() {
            let query_str = r#"
                SELECT *, meta::id(id) AS id FROM user_profile 
                WHERE username ~ $search OR display_name ~ $search OR email ~ $search
                ORDER BY created_at DESC
                LIMIT $limit START $offset
            "#;
            (
                query_str,
                json!({
                    "search": search_term,
                    "limit": limit,
                    "offset": offset
                }),
            )
        } else {
            let query_str = r#"
                SELECT *, meta::id(id) AS id FROM user_profile 
                ORDER BY created_at DESC
                LIMIT $limit START $offset
            "#;
            (
                query_str,
                json!({
                    "limit": limit,
                    "offset": offset
                }),
            )
        };

        let mut response = self.db.query_with_params(query, params).await?;
        let profiles: Vec<UserProfile> = response.take(0)?;

        // 获取总数
        let count_query = if search.is_some() {
            "SELECT count() AS total FROM user_profile WHERE username ~ $search OR display_name ~ $search OR email ~ $search"
        } else {
            "SELECT count() AS total FROM user_profile"
        };

        let count_params = if let Some(search_term) = search {
            json!({ "search": search_term })
        } else {
            json!({})
        };

        let mut count_response = self.db.query_with_params(count_query, count_params).await?;
        let total = if let Ok(Some(result)) = count_response.take::<Option<Value>>(0) {
            result.get("total").and_then(|v| v.as_i64()).unwrap_or(0) as usize
        } else {
            0
        };

        Ok(crate::services::database::PaginatedResult {
            data: profiles,
            total,
            page,
            per_page: limit,
            total_pages: (total + limit - 1) / limit,
        })
    }

    /// 搜索用户
    pub async fn search_users(&self, query: &str, limit: usize) -> Result<Vec<UserProfile>> {
        debug!("Searching users with query: {}", query);

        let search_query = r#"
            SELECT *, meta::id(id) AS id FROM user_profile 
            WHERE username ~ $query OR display_name ~ $query OR bio ~ $query
            ORDER BY follower_count DESC
            LIMIT $limit
        "#;

        let mut response = self
            .db
            .query_with_params(
                search_query,
                json!({
                    "query": query,
                    "limit": limit
                }),
            )
            .await?;

        let profiles: Vec<UserProfile> = response.take(0)?;
        Ok(profiles)
    }

    /// 增加关注者数量
    pub async fn increment_follower_count(&self, user_id: &str) -> Result<()> {
        let update_query = "UPDATE user_profile SET follower_count += 1 WHERE user_id = $user_id";
        self.db
            .query_with_params(update_query, json!({ "user_id": user_id }))
            .await?;
        Ok(())
    }

    /// 减少关注者数量
    pub async fn decrement_follower_count(&self, user_id: &str) -> Result<()> {
        let update_query = "UPDATE user_profile SET follower_count -= 1 WHERE user_id = $user_id AND follower_count > 0";
        self.db
            .query_with_params(update_query, json!({ "user_id": user_id }))
            .await?;
        Ok(())
    }

    /// 增加关注数量
    pub async fn increment_following_count(&self, user_id: &str) -> Result<()> {
        let update_query = "UPDATE user_profile SET following_count += 1 WHERE user_id = $user_id";
        self.db
            .query_with_params(update_query, json!({ "user_id": user_id }))
            .await?;
        Ok(())
    }

    /// 减少关注数量
    pub async fn decrement_following_count(&self, user_id: &str) -> Result<()> {
        let update_query = "UPDATE user_profile SET following_count -= 1 WHERE user_id = $user_id AND following_count > 0";
        self.db
            .query_with_params(update_query, json!({ "user_id": user_id }))
            .await?;
        Ok(())
    }

    /// 获取或创建用户资料（从Rainbow-Auth用户信息创建）
    pub async fn get_or_create_profile(
        &self,
        user_id: &str,
        email: &str,
        email_verified: bool,
        username: Option<String>,
        display_name: Option<String>,
    ) -> Result<UserProfile> {
        // 先尝试获取现有资料
        if let Some(mut profile) = self.get_profile_by_user_id(user_id).await? {
            // 不需要更新 email，因为我们不在数据库中存储它
            // email 信息始终从 Rainbow-Auth 获取
            return Ok(profile);
        }

        // 如果不存在，创建新资料
        self.create_profile_with_auth_info(user_id, email, email_verified, username, display_name)
            .await
    }

    /// 使用认证信息创建用户资料
    async fn create_profile_with_auth_info(
        &self,
        user_id: &str,
        email: &str,
        email_verified: bool,
        username: Option<String>,
        display_name: Option<String>,
    ) -> Result<UserProfile> {
        debug!(
            "Creating new user profile for user: {} with email: {}",
            user_id, email
        );

        // 检查是否已有资料
        if let Some(existing) = self.get_profile_by_user_id(user_id).await? {
            return Ok(existing);
        }

        // 使用提供的用户名或从邮箱生成
        let mut base_username = if let Some(username) = username {
            username
        } else {
            email
                .split('@')
                .next()
                .unwrap_or("user")
                .chars()
                .filter(|c| c.is_alphanumeric() || *c == '_' || *c == '-')
                .collect::<String>()
                .to_lowercase()
        };

        // 确保用户名至少有3个字符
        if base_username.len() < 3 {
            base_username = format!("user_{}", base_username);
        }

        let original_username = if base_username.is_empty() {
            format!("user{}", Uuid::new_v4().simple())
        } else {
            base_username
        };

        // 生成唯一用户名
        let mut profile = UserProfile {
            id: Thing::new("user_profile", Uuid::new_v4().to_string()),
            user_id: user_id.to_string(),
            username: original_username.clone(),
            display_name: display_name.unwrap_or_else(|| original_username.clone()),
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
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        // 确保用户名唯一
        profile.username = self.generate_unique_username(&profile.username).await?;

        // 创建用户资料，使用 SurrealDB 的 time::now() 函数处理时间
        let create_query = format!(
            r#"
            CREATE user_profile CONTENT {{
                id: "{}",
                user_id: "{}",
                username: "{}",
                display_name: "{}",
                bio: {},
                avatar_url: {},
                cover_image_url: {},
                website: {},
                location: {},
                twitter_username: {},
                github_username: {},
                linkedin_url: {},
                facebook_url: {},
                stripe_customer_id: NONE,
                stripe_account_id: NONE,
                follower_count: {},
                following_count: {},
                article_count: {},
                total_claps_received: {},
                is_verified: {},
                is_suspended: {},
                created_at: time::now(),
                updated_at: time::now()
            }}
            RETURN *, meta::id(id) AS id
            "#,
            profile.id.key.to_sql(),
            profile.user_id,
            profile.username,
            profile.display_name,
            profile
                .bio
                .as_ref()
                .map(|s| format!("\"{}\"", s))
                .unwrap_or("NONE".to_string()),
            profile
                .avatar_url
                .as_ref()
                .map(|s| format!("\"{}\"", s))
                .unwrap_or("NONE".to_string()),
            profile
                .cover_image_url
                .as_ref()
                .map(|s| format!("\"{}\"", s))
                .unwrap_or("NONE".to_string()),
            profile
                .website
                .as_ref()
                .map(|s| format!("\"{}\"", s))
                .unwrap_or("NONE".to_string()),
            profile
                .location
                .as_ref()
                .map(|s| format!("\"{}\"", s))
                .unwrap_or("NONE".to_string()),
            profile
                .twitter_username
                .as_ref()
                .map(|s| format!("\"{}\"", s))
                .unwrap_or("NONE".to_string()),
            profile
                .github_username
                .as_ref()
                .map(|s| format!("\"{}\"", s))
                .unwrap_or("NONE".to_string()),
            profile
                .linkedin_url
                .as_ref()
                .map(|s| format!("\"{}\"", s))
                .unwrap_or("NONE".to_string()),
            profile
                .facebook_url
                .as_ref()
                .map(|s| format!("\"{}\"", s))
                .unwrap_or("NONE".to_string()),
            profile.follower_count,
            profile.following_count,
            profile.article_count,
            profile.total_claps_received,
            profile.is_verified,
            profile.is_suspended,
        );

        debug!("Creating user profile with query: {}", create_query);

        let mut response = self.db.query(&create_query).await.map_err(|e| {
            error!("Failed to create user profile record: {:?}", e);
            AppError::Internal(format!("Failed to create user profile: {}", e))
        })?;

        let created = Self::first_profile_from_vec(response.take::<Vec<UserProfile>>(0)?);
        if created.is_none() {
            error!("No user profile was created for user: {}", user_id);
            return Err(AppError::Internal(
                "Failed to create user profile".to_string(),
            ));
        }

        let result = created.unwrap();
        debug!("Successfully created user profile with ID: {}", result.id.to_sql());
        info!("Created user profile for user: {}", user_id);

        // 从数据库重新获取创建的记录，确保数据已经持久化
        if let Some(created_profile) = self.get_profile_by_user_id(user_id).await? {
            Ok(created_profile)
        } else {
            error!("Failed to retrieve created profile for user: {}", user_id);
            Err(AppError::Internal(
                "Profile was created but could not be retrieved".to_string(),
            ))
        }
    }

    /// 获取热门用户
    pub async fn get_popular_users(&self, limit: usize) -> Result<Vec<UserProfile>> {
        debug!("Getting popular users with limit: {}", limit);

        let query = r#"
            SELECT *, meta::id(id) AS id FROM user_profile 
            WHERE is_suspended = false
            ORDER BY follower_count DESC, article_count DESC
            LIMIT $limit
        "#;

        let mut response = self
            .db
            .query_with_params(query, json!({ "limit": limit }))
            .await?;
        let profiles: Vec<UserProfile> = response.take(0)?;
        Ok(profiles)
    }

    /// 生成唯一的用户名
    async fn generate_unique_username(&self, base_username: &str) -> Result<String> {
        let mut username = base_username.to_string();
        let mut counter = 1;

        // 检查用户名是否已存在
        while self.username_exists(&username).await? {
            username = format!("{}{}", base_username, counter);
            counter += 1;

            if counter > 1000 {
                return Err(AppError::Internal(
                    "Failed to generate unique username".to_string(),
                ));
            }
        }

        Ok(username)
    }

    /// 检查用户名是否已存在
    async fn username_exists(&self, username: &str) -> Result<bool> {
        let query = "SELECT count() as count FROM user_profile WHERE username = $username";
        let mut response = self
            .db
            .query_with_params(
                query,
                json!({
                    "username": username
                }),
            )
            .await?;

        let result: Vec<serde_json::Value> = response.take(0)?;
        let count = result
            .first()
            .and_then(|v| v.get("count"))
            .and_then(|v| v.as_i64())
            .unwrap_or(0);

        Ok(count > 0)
    }
}
