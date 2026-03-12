use crate::{
    error::{AppError, Result},
    models::{
        publication::*,
        article::{Article, ArticleListItem, ArticleStatus},
    },
    services::Database,
    utils::slug,
};
use chrono::Utc;
use serde_json::{json, Value};
use std::sync::Arc;
use tracing::{debug, info, warn};
use uuid::Uuid;
use validator::Validate;

#[derive(Clone)]
pub struct PublicationService {
    db: Arc<Database>,
}

impl PublicationService {
    pub async fn new(db: Arc<Database>) -> Result<Self> {
        Ok(Self { db })
    }

    /// 创建出版物
    pub async fn create_publication(
        &self,
        owner_id: &str,
        request: CreatePublicationRequest,
    ) -> Result<Publication> {
        debug!("Creating publication: {} for user: {}", request.name, owner_id);

        request.validate().map_err(|e| AppError::ValidatorError(e))?;

        // 生成唯一slug
        let slug = self.generate_unique_slug(&request.name).await?;

        // 使用 SQL 显式创建记录并用 time::now() 设置时间字段，避免 SurrealDB 类型不匹配
        let id = Uuid::new_v4().to_string();
        // 校验并规范 homepage_layout，限定为 ['grid', 'list', 'magazine']，默认使用 'grid'
        let homepage_layout = match request.homepage_layout {
            Some(layout) => {
                let l = layout.to_lowercase();
                let allowed = ["grid", "list", "magazine"];
                if !allowed.contains(&l.as_str()) {
                    return Err(AppError::BadRequest(
                        "homepage_layout 必须是 'grid' | 'list' | 'magazine'".into(),
                    ));
                }
                l
            }
            None => "grid".to_string(),
        };
        let theme_color = request
            .theme_color
            .unwrap_or_else(|| "#1a1a1a".to_string());
        let sql_str = |s: &str| -> String { format!("'{}'", s.replace('\'', "''")) };
        let sql_opt_str = |v: &Option<String>| -> String {
            match v {
                Some(s) => sql_str(s),
                None => "NONE".to_string(),
            }
        };

        let create_sql = format!(
            r#"
            CREATE publication:`{}` SET
                name = $name,
                slug = $slug,
                description = {},
                tagline = {},
                logo_url = {},
                cover_image_url = {},
                owner_id = $owner_id,
                homepage_layout = $homepage_layout,
                theme_color = $theme_color,
                custom_domain = {},
                member_count = 1,
                article_count = 0,
                follower_count = 0,
                is_verified = false,
                is_suspended = false,
                created_at = time::now(),
                updated_at = time::now()
            RETURN type::string(id) AS id,
                   name, slug, description, tagline, logo_url, cover_image_url,
                   owner_id, homepage_layout, theme_color, custom_domain,
                   member_count, article_count, follower_count, is_verified, is_suspended,
                   created_at, updated_at
            "#,
            id,
            sql_opt_str(&request.description),
            sql_opt_str(&request.tagline),
            sql_opt_str(&request.logo_url),
            sql_opt_str(&request.cover_image_url),
            sql_opt_str(&request.custom_domain),
        );

        let params = json!({
            "id": id,
            "name": request.name,
            "slug": slug,
            "owner_id": owner_id,
            "homepage_layout": homepage_layout,
            "theme_color": theme_color,
        });

        let mut resp = self.db.query_with_params(&create_sql, params).await?;
        let mut created_vec: Vec<Publication> = resp.take(0)?;
        if created_vec.is_empty() {
            // Fallback: query by id if CREATE response is unexpectedly empty.
            let fallback_sql = format!(
                r#"
                SELECT type::string(id) AS id,
                       name, slug, description, tagline, logo_url, cover_image_url,
                       owner_id, homepage_layout, theme_color, custom_domain,
                       member_count, article_count, follower_count, is_verified, is_suspended,
                       created_at, updated_at
                FROM publication
                WHERE id = type::record("publication", "{}")
                LIMIT 1
                "#,
                id
            );
            let mut fallback_resp = self.db.query(&fallback_sql).await?;
            created_vec = fallback_resp.take(0)?;
        }
        let created_publication = created_vec
            .pop()
            .ok_or_else(|| AppError::internal("Failed to create publication"))?;

        // 添加创建者为Owner
        self.add_member_internal(&created_publication.id, owner_id, MemberRole::Owner, owner_id).await?;

        info!("Created publication: {} ({})", created_publication.name, created_publication.id);
        Ok(created_publication)
    }

    /// 获取出版物详情
    pub async fn get_publication(
        &self,
        slug: &str,
        user_id: Option<&str>,
    ) -> Result<Option<PublicationResponse>> {
        debug!("Getting publication: {}", slug);

        // 使用显式查询并将 id 转换为字符串，避免 Surreal record -> String 反序列化问题
        let query = r#"
            SELECT 
                type::string(id) AS id,
                name, slug, description, tagline, logo_url, cover_image_url,
                owner_id, homepage_layout, theme_color, custom_domain,
                member_count, article_count, follower_count,
                is_verified, is_suspended,
                created_at, updated_at
            FROM publication
            WHERE slug = $slug
            LIMIT 1
        "#;

        let mut resp = self.db.query_with_params(query, json!({"slug": slug})).await?;
        let mut items: Vec<Publication> = resp.take(0)?;
        let publication = match items.pop() {
            Some(p) if !p.is_suspended => p,
            Some(_) => return Ok(None), // 被暂停的出版物不可见
            None => return Ok(None),
        };

        // 获取用户相关信息
        let (is_member, member_role, is_following) = if let Some(uid) = user_id {
            let member_info = self.get_member_info(&publication.id, uid).await?;
            let following = self.is_following_publication(&publication.id, uid).await?;
            (
                member_info.is_some(),
                member_info.map(|m| m.role),
                following,
            )
        } else {
            (false, None, false)
        };

        // 获取最近文章
        let recent_articles = self.get_publication_recent_articles(&publication.id, 5).await?;

        let response = PublicationResponse {
            publication,
            is_member,
            member_role,
            is_following,
            recent_articles,
        };

        Ok(Some(response))
    }

    /// 更新出版物
    pub async fn update_publication(
        &self,
        publication_id: &str,
        user_id: &str,
        request: UpdatePublicationRequest,
    ) -> Result<Publication> {
        debug!("Updating publication: {} by user: {}", publication_id, user_id);

        request.validate().map_err(|e| AppError::ValidatorError(e))?;

        // 检查权限
        self.check_permission(publication_id, user_id, "publication.manage_settings").await?;

        let mut publication: Publication = self.db.get_by_id("publication", publication_id).await?
            .ok_or_else(|| AppError::NotFound("Publication not found".to_string()))?;

        // 更新字段
        if let Some(name) = request.name {
            if name != publication.name {
                publication.name = name;
                publication.slug = self.generate_unique_slug(&publication.name).await?;
            }
        }
        
        if let Some(description) = request.description {
            publication.description = Some(description);
        }
        
        if let Some(tagline) = request.tagline {
            publication.tagline = Some(tagline);
        }
        
        if let Some(logo_url) = request.logo_url {
            publication.logo_url = Some(logo_url);
        }
        
        if let Some(cover_image_url) = request.cover_image_url {
            publication.cover_image_url = Some(cover_image_url);
        }
        
        if let Some(homepage_layout) = request.homepage_layout {
            let l = homepage_layout.to_lowercase();
            let allowed = ["grid", "list", "magazine"];
            if !allowed.contains(&l.as_str()) {
                return Err(AppError::BadRequest(
                    "homepage_layout 必须是 'grid' | 'list' | 'magazine'".into(),
                ));
            }
            publication.homepage_layout = l;
        }
        
        if let Some(theme_color) = request.theme_color {
            publication.theme_color = theme_color;
        }
        
        if let Some(custom_domain) = request.custom_domain {
            publication.custom_domain = Some(custom_domain);
        }

        publication.updated_at = Utc::now();

        let updated: Publication = self.db.update_by_id("publication", publication_id, publication).await?
            .ok_or_else(|| AppError::internal("Failed to update publication"))?;

        Ok(updated)
    }

    /// 删除出版物
    pub async fn delete_publication(
        &self,
        publication_id: &str,
        user_id: &str,
    ) -> Result<()> {
        debug!("Deleting publication: {} by user: {}", publication_id, user_id);

        // 只有Owner可以删除出版物
        let member = self.get_member_info(publication_id, user_id).await?
            .ok_or_else(|| AppError::forbidden("You are not a member of this publication"))?;

        if member.role != MemberRole::Owner {
            return Err(AppError::forbidden("Only publication owner can delete the publication"));
        }

        // 软删除：设置为暂停状态
        let updates = json!({
            "is_suspended": true,
            "updated_at": Utc::now()
        });

        self.db.update_by_id_with_json::<Value>("publication", publication_id, updates).await?;

        info!("Deleted publication: {}", publication_id);
        Ok(())
    }

    /// 获取出版物列表
    pub async fn get_publications(&self, query: PublicationQuery) -> Result<crate::services::database::PaginatedResult<PublicationListItem>> {
        debug!("Getting publications with query: {:?}", query);

        let page = query.page.unwrap_or(1);
        let limit = query.limit.unwrap_or(20);
        let offset = (page - 1) * limit;

        let mut conditions = vec!["is_suspended = false".to_string()];

        if let Some(true) = query.verified_only {
            conditions.push("is_verified = true".to_string());
        }

        if let Some(search) = &query.search {
            conditions.push("(name ~ $search OR description ~ $search)".to_string());
        }

        let where_clause = conditions.join(" AND ");

        let order_by = match query.sort.as_deref() {
            Some("newest") => "created_at DESC",
            Some("oldest") => "created_at ASC",
            Some("alphabetical") => "name ASC",
            _ => "follower_count DESC, article_count DESC", // popular
        };

        let count_query = format!("SELECT count() AS total FROM publication WHERE {}", where_clause);
        let data_query = format!(
            "SELECT 
                type::string(id) AS id, 
                name, slug, description, tagline, logo_url, cover_image_url, 
                member_count, article_count, follower_count, is_verified, created_at 
             FROM publication 
             WHERE {} 
             ORDER BY {} 
             LIMIT $limit START $offset",
            where_clause, order_by
        );

        let mut params = json!({
            "limit": limit,
            "offset": offset
        });

        if let Some(search) = &query.search {
            params["search"] = json!(search);
        }

        // 获取总数
        let mut count_response = self.db.query_with_params(&count_query, &params).await?;
        let total = if let Ok(Some(result)) = count_response.take::<Option<Value>>(0) {
            result.get("total").and_then(|v| v.as_i64()).unwrap_or(0) as usize
        } else { 0 };

        // 获取数据
        let mut data_response = self.db.query_with_params(&data_query, params).await?;
        let publications: Vec<PublicationListItem> = data_response.take(0)?;

        Ok(crate::services::database::PaginatedResult {
            data: publications,
            total,
            page,
            per_page: limit,
            total_pages: (total + limit - 1) / limit,
        })
    }

    /// 获取出版物文章
    pub async fn get_publication_articles(
        &self,
        publication_id: &str,
        page: usize,
        limit: usize,
    ) -> Result<crate::services::database::PaginatedResult<ArticleListItem>> {
        debug!("Getting articles for publication: {}", publication_id);

        let offset = (page - 1) * limit;

        let count_query = r#"
            SELECT count() AS total 
            FROM article 
            WHERE publication_id = $publication_id 
            AND status = 'published' 
            AND is_deleted = false
        "#;

        let data_query = r#"
            SELECT id, title, subtitle, slug, excerpt, cover_image_url, author_id, 
                   reading_time, view_count, clap_count, comment_count, bookmark_count,
                   tags, created_at, published_at
            FROM article 
            WHERE publication_id = $publication_id 
            AND status = 'published' 
            AND is_deleted = false
            ORDER BY published_at DESC, created_at DESC
            LIMIT $limit START $offset
        "#;

        let params = json!({
            "publication_id": publication_id,
            "limit": limit,
            "offset": offset
        });

        let mut count_response = self.db.query_with_params(count_query, &params).await?;
        let total = if let Ok(Some(result)) = count_response.take::<Option<Value>>(0) {
            result.get("total").and_then(|v| v.as_i64()).unwrap_or(0) as usize
        } else { 0 };

        let mut data_response = self.db.query_with_params(data_query, params).await?;
        let articles: Vec<ArticleListItem> = data_response.take(0)?;

        Ok(crate::services::database::PaginatedResult {
            data: articles,
            total,
            page,
            per_page: limit,
            total_pages: (total + limit - 1) / limit,
        })
    }

    /// 添加成员
    pub async fn add_member(
        &self,
        publication_id: &str,
        requester_id: &str,
        request: AddMemberRequest,
    ) -> Result<PublicationMember> {
        debug!("Adding member {} to publication: {}", request.user_id, publication_id);

        request.validate().map_err(|e| AppError::ValidatorError(e))?;

        // 检查权限
        self.check_permission(publication_id, requester_id, "publication.manage_members").await?;

        // 检查用户是否已经是成员
        if self.get_member_info(publication_id, &request.user_id).await?.is_some() {
            return Err(AppError::Conflict("User is already a member".to_string()));
        }

        let member = self.add_member_internal(publication_id, &request.user_id, request.role, requester_id).await?;

        // 更新成员数量
        self.update_member_count(publication_id).await?;

        Ok(member)
    }

    /// 更新成员
    pub async fn update_member(
        &self,
        publication_id: &str,
        member_user_id: &str,
        requester_id: &str,
        request: UpdateMemberRequest,
    ) -> Result<PublicationMember> {
        debug!("Updating member {} in publication: {}", member_user_id, publication_id);

        request.validate().map_err(|e| AppError::ValidatorError(e))?;

        // 检查权限
        self.check_permission(publication_id, requester_id, "publication.manage_members").await?;

        let mut member = self.get_member_info(publication_id, member_user_id).await?
            .ok_or_else(|| AppError::NotFound("Member not found".to_string()))?;

        // 不能修改Owner的角色（除非Owner自己修改）
        if member.role == MemberRole::Owner && requester_id != member_user_id {
            return Err(AppError::forbidden("Cannot modify owner role"));
        }

        if let Some(role) = request.role {
            member.role = role;
            member.permissions = member.role.default_permissions();
        }

        if let Some(permissions) = request.permissions {
            member.permissions = permissions;
        }

        if let Some(is_active) = request.is_active {
            member.is_active = is_active;
        }

        let member_id = member.id.clone();
        let updated: PublicationMember = self.db.update_by_id("publication_member", &member_id, member).await?
            .ok_or_else(|| AppError::internal("Failed to update member"))?;

        Ok(updated)
    }

    /// 移除成员
    pub async fn remove_member(
        &self,
        publication_id: &str,
        member_user_id: &str,
        requester_id: &str,
    ) -> Result<()> {
        debug!("Removing member {} from publication: {}", member_user_id, publication_id);

        // 检查权限
        self.check_permission(publication_id, requester_id, "publication.manage_members").await?;

        let member = self.get_member_info(publication_id, member_user_id).await?
            .ok_or_else(|| AppError::NotFound("Member not found".to_string()))?;

        // 不能移除Owner（除非Owner自己离开）
        if member.role == MemberRole::Owner && requester_id != member_user_id {
            return Err(AppError::forbidden("Cannot remove owner"));
        }

        // 如果是Owner要离开，检查是否有其他Owner
        if member.role == MemberRole::Owner && requester_id == member_user_id {
            let other_owners_count = self.count_members_by_role(publication_id, MemberRole::Owner).await?;
            if other_owners_count <= 1 {
                return Err(AppError::bad_request("Publication must have at least one owner"));
            }
        }

        self.db.delete_by_id("publication_member", &member.id).await?;

        // 更新成员数量
        self.update_member_count(publication_id).await?;

        info!("Removed member {} from publication: {}", member_user_id, publication_id);
        Ok(())
    }

    /// 获取出版物成员列表
    pub async fn get_members(
        &self,
        publication_id: &str,
        page: usize,
        limit: usize,
    ) -> Result<crate::services::database::PaginatedResult<PublicationMember>> {
        debug!("Getting members for publication: {}", publication_id);

        let offset = (page - 1) * limit;

        let count_query = r#"
            SELECT count() AS total 
            FROM publication_member 
            WHERE publication_id = $publication_id 
            AND is_active = true
        "#;

        let data_query = r#"
            SELECT * FROM publication_member 
            WHERE publication_id = $publication_id 
            AND is_active = true
            ORDER BY joined_at ASC
            LIMIT $limit START $offset
        "#;

        let params = json!({
            "publication_id": publication_id,
            "limit": limit,
            "offset": offset
        });

        let mut count_response = self.db.query_with_params(count_query, &params).await?;
        let total = if let Ok(Some(result)) = count_response.take::<Option<Value>>(0) {
            result.get("total").and_then(|v| v.as_i64()).unwrap_or(0) as usize
        } else { 0 };

        let mut data_response = self.db.query_with_params(data_query, params).await?;
        let members: Vec<PublicationMember> = data_response.take(0)?;

        Ok(crate::services::database::PaginatedResult {
            data: members,
            total,
            page,
            per_page: limit,
            total_pages: (total + limit - 1) / limit,
        })
    }

    /// 关注出版物
    pub async fn follow_publication(
        &self,
        publication_id: &str,
        user_id: &str,
    ) -> Result<()> {
        debug!("User {} following publication: {}", user_id, publication_id);

        // 检查出版物是否存在
        let _publication: Publication = self.db.get_by_id("publication", publication_id).await?
            .ok_or_else(|| AppError::NotFound("Publication not found".to_string()))?;

        // 检查是否已经关注
        if self.is_following_publication(publication_id, user_id).await? {
            return Err(AppError::Conflict("Already following this publication".to_string()));
        }

        let follow = PublicationFollow {
            id: Uuid::new_v4().to_string(),
            user_id: user_id.to_string(),
            publication_id: publication_id.to_string(),
            created_at: Utc::now(),
        };

        self.db.create("publication_follow", follow).await?;

        // 更新关注者计数
        self.update_follower_count(publication_id).await?;

        info!("User {} now following publication: {}", user_id, publication_id);
        Ok(())
    }

    /// 取消关注出版物
    pub async fn unfollow_publication(
        &self,
        publication_id: &str,
        user_id: &str,
    ) -> Result<()> {
        debug!("User {} unfollowing publication: {}", user_id, publication_id);

        let query = r#"
            DELETE publication_follow 
            WHERE user_id = $user_id 
            AND publication_id = $publication_id
        "#;

        self.db.query_with_params(query, json!({
            "user_id": user_id,
            "publication_id": publication_id
        })).await?;

        // 更新关注者计数
        self.update_follower_count(publication_id).await?;

        info!("User {} unfollowed publication: {}", user_id, publication_id);
        Ok(())
    }

    // Helper methods

    async fn generate_unique_slug(&self, name: &str) -> Result<String> {
        let base_slug = slug::generate_slug(name);
        let mut slug = base_slug.clone();
        let mut counter = 1;

        while self.db.find_one::<Value>("publication", "slug", &slug).await?.is_some() {
            slug = format!("{}-{}", base_slug, counter);
            counter += 1;

            if counter > 100 {
                return Err(AppError::internal("Failed to generate unique slug"));
            }
        }

        Ok(slug)
    }

    async fn add_member_internal(
        &self,
        publication_id: &str,
        user_id: &str,
        role: MemberRole,
        invited_by: &str,
    ) -> Result<PublicationMember> {
        // 使用 SQL 显式设置 joined_at 为 time::now()，避免时间类型不匹配
        let id = Uuid::new_v4().to_string();
        let permissions = role.default_permissions();
        let role_str = match role {
            MemberRole::Owner => "owner",
            MemberRole::Editor => "editor",
            MemberRole::Writer => "writer",
            MemberRole::Contributor => "writer",
        };

        let sql = r#"
            CREATE publication_member CONTENT {
                id: type::record("publication_member", $id),
                publication_id: type::record("publication", string::split($publication_id, ':')[1] ?: $publication_id),
                user_id: $user_id,
                role: $role,
                permissions: $permissions,
                invited_by: $invited_by
            }
            RETURN 
                type::string(id) AS id,
                type::string(publication_id) AS publication_id,
                user_id,
                IF role = 'owner' THEN 'Owner' ELSE IF role = 'editor' THEN 'Editor' ELSE 'Writer' END AS role,
                permissions,
                joined_at,
                is_active;
        "#;

        let params = json!({
            "id": id,
            "publication_id": publication_id,
            "user_id": user_id,
            "role": role_str,
            "permissions": permissions,
            "invited_by": invited_by,
        });

        let mut resp = self.db.query_with_params(sql, params).await?;
        // 单条 CREATE ... RETURN 语句，数据在索引 0
        let mut created_vec: Vec<PublicationMember> = resp.take(0)?;
        let created = created_vec
            .pop()
            .ok_or_else(|| AppError::internal("Failed to create publication member"))?;
        Ok(created)
    }

    async fn get_member_info(
        &self,
        publication_id: &str,
        user_id: &str,
    ) -> Result<Option<PublicationMember>> {
        let query = r#"
            SELECT * FROM publication_member 
            WHERE publication_id = $publication_id 
            AND user_id = $user_id 
            AND is_active = true
            LIMIT 1
        "#;

        let mut response = self.db.query_with_params(query, json!({
            "publication_id": publication_id,
            "user_id": user_id
        })).await?;

        let members: Vec<PublicationMember> = response.take(0)?;
        Ok(members.into_iter().next())
    }

    async fn check_permission(
        &self,
        publication_id: &str,
        user_id: &str,
        permission: &str,
    ) -> Result<()> {
        let member = self.get_member_info(publication_id, user_id).await?
            .ok_or_else(|| AppError::forbidden("You are not a member of this publication"))?;

        if !member.permissions.contains(&permission.to_string()) {
            return Err(AppError::forbidden(&format!("Permission '{}' required", permission)));
        }

        Ok(())
    }

    async fn is_following_publication(
        &self,
        publication_id: &str,
        user_id: &str,
    ) -> Result<bool> {
        let query = r#"
            SELECT count() as count 
            FROM publication_follow 
            WHERE user_id = $user_id 
            AND publication_id = $publication_id
        "#;

        let mut response = self.db.query_with_params(query, json!({
            "user_id": user_id,
            "publication_id": publication_id
        })).await?;

        let result: Vec<Value> = response.take(0)?;
        let count = result.first()
            .and_then(|v| v.get("count"))
            .and_then(|v| v.as_i64())
            .unwrap_or(0);

        Ok(count > 0)
    }

    async fn get_publication_recent_articles(
        &self,
        publication_id: &str,
        limit: usize,
    ) -> Result<Vec<ArticleListItem>> {
        let query = r#"
            SELECT id, title, subtitle, slug, excerpt, cover_image_url, author_id,
                   reading_time, view_count, clap_count, comment_count, bookmark_count,
                   tags, created_at, published_at
            FROM article 
            WHERE publication_id = $publication_id 
            AND status = 'published' 
            AND is_deleted = false
            ORDER BY published_at DESC, created_at DESC
            LIMIT $limit
        "#;

        let mut response = self.db.query_with_params(query, json!({
            "publication_id": publication_id,
            "limit": limit
        })).await?;

        let articles: Vec<ArticleListItem> = response.take(0)?;
        Ok(articles)
    }

    async fn update_member_count(&self, publication_id: &str) -> Result<()> {
        let query = r#"
            LET $count = (SELECT count() FROM publication_member WHERE publication_id = $publication_id AND is_active = true);
            UPDATE publication SET member_count = $count WHERE id = $publication_id;
        "#;

        self.db.query_with_params(query, json!({
            "publication_id": publication_id
        })).await?;

        Ok(())
    }

    async fn update_follower_count(&self, publication_id: &str) -> Result<()> {
        let query = r#"
            LET $count = (SELECT count() FROM publication_follow WHERE publication_id = $publication_id);
            UPDATE publication SET follower_count = $count WHERE id = $publication_id;
        "#;

        self.db.query_with_params(query, json!({
            "publication_id": publication_id
        })).await?;

        Ok(())
    }

    async fn count_members_by_role(
        &self,
        publication_id: &str,
        role: MemberRole,
    ) -> Result<i64> {
        let query = r#"
            SELECT count() as count 
            FROM publication_member 
            WHERE publication_id = $publication_id 
            AND role = $role 
            AND is_active = true
        "#;

        let mut response = self.db.query_with_params(query, json!({
            "publication_id": publication_id,
            "role": format!("{:?}", role)
        })).await?;

        let result: Vec<Value> = response.take(0)?;
        let count = result.first()
            .and_then(|v| v.get("count"))
            .and_then(|v| v.as_i64())
            .unwrap_or(0);

        Ok(count)
    }

    /// 获取出版物的所有成员
    pub async fn get_publication_members(
        &self,
        publication_id: &str,
        role_filter: Option<MemberRole>,
    ) -> Result<Vec<PublicationMember>> {
        debug!("Getting members for publication: {}", publication_id);
        
        let mut query = String::from(r#"
            SELECT * FROM publication_member 
            WHERE publication_id = $publication_id 
            AND is_active = true
        "#);
        
        if role_filter.is_some() {
            query.push_str(" AND role = $role");
        }
        
        query.push_str(" ORDER BY joined_at DESC");
        
        let mut params = json!({
            "publication_id": publication_id
        });
        
        if let Some(role) = role_filter {
            params["role"] = json!(format!("{:?}", role));
        }
        
        let mut response = self.db.query_with_params(&query, params).await?;
        let members: Vec<PublicationMember> = response.take(0)?;
        
        Ok(members)
    }
    
    /// 统计出版物的成员数量
    pub async fn count_publication_members(&self, publication_id: &str) -> Result<usize> {
        debug!("Counting members for publication: {}", publication_id);
        
        let query = r#"
            SELECT count() as total 
            FROM publication_member 
            WHERE publication_id = $publication_id 
            AND is_active = true
        "#;
        
        let mut response = self.db.query_with_params(query, json!({
            "publication_id": publication_id
        })).await?;
        
        let result: Vec<Value> = response.take(0)?;
        let count = result.first()
            .and_then(|v| v.get("total"))
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as usize;
        
        Ok(count)
    }
}
