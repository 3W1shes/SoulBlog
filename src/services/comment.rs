use crate::{
    error::{AppError, Result},
    models::comment::*,
    models::article::Article,
    services::Database,
    services::article::normalize_surreal_json,
};
use chrono::Utc;
use serde_json::{json, Value};
use std::sync::Arc;
use std::collections::HashMap;
use soulcore::prelude::Thing;
use surrealdb::types::Value as SurrealValue;
use tracing::{debug, error, info};
use validator::Validate;
use uuid::Uuid;
use serde::{Deserialize, Serialize};

// 用于数据库插入的评论结构体（不包含时间戳字段，让数据库自动设置）
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CommentInsert {
    id: String,
    article_id: String,
    author_id: String,
    parent_id: Option<String>,
    content: String,
    is_author_response: bool,
    clap_count: i64,
    is_edited: bool,
    is_deleted: bool,
    deleted_at: Option<String>,
}

#[derive(Clone)]
pub struct CommentService {
    db: Arc<Database>,
}

impl CommentService {
    pub async fn new(db: Arc<Database>) -> Result<Self> {
        Ok(Self { db })
    }


    pub async fn create_comment(
        &self,
        user_id: &str,
        request: CreateCommentRequest,
    ) -> Result<Comment> {
        debug!("Creating comment for article: {}", request.article_id);
        info!("Received article_id: '{}'", request.article_id);

        request
            .validate()
            .map_err(|e| AppError::ValidatorError(e))?;

        // Verify article exists and is published
        let article = self.get_article_for_comment(&request.article_id).await?;

        if article.status != crate::models::article::ArticleStatus::Published {
            return Err(AppError::forbidden(
                "Cannot comment on unpublished articles",
            ));
        }

        // Verify parent comment exists if provided
        // Note: SurrealDB may return Thing objects for `id`, which don't deserialize into String directly.
        // Use a generic JSON value for existence checks to avoid id deserialization issues.
        if let Some(parent_id) = &request.parent_id {
            let parent: Option<serde_json::Value> = self.db.get_by_id("comment", parent_id).await?;
            match parent {
                None => {
                    return Err(AppError::NotFound("Parent comment not found".to_string()));
                }
                Some(p) => {
                    // Disallow replying to deleted comments
                    if p.get("is_deleted").and_then(|v| v.as_bool()).unwrap_or(false) {
                        return Err(AppError::NotFound("Parent comment not found".to_string()));
                    }
                }
            }
        }

        // Check if this is an author response
        let is_author_response = article.author_id == user_id;

        let comment_id = Uuid::new_v4().to_string();

        // 使用 CREATE 语句创建评论，让数据库自动设置时间戳
        let parent_id_clause = request.parent_id.as_ref()
            .map(|p| format!(", parent_id = '{}'", p))
            .unwrap_or_else(|| String::new());
            
        let query = format!(
            "CREATE comment:`{}` SET article_id = '{}', author_id = '{}'{}, content = '{}', is_author_response = {}, clap_count = 0, is_edited = false, is_deleted = false, deleted_at = NONE, created_at = time::now(), updated_at = time::now()",
            comment_id,
            request.article_id,
            user_id,
            parent_id_clause,
            request.content.replace("'", "''"), // 转义单引号
            is_author_response
        );
        
        debug!("Creating comment with query: {}", query);
        
        // 执行 CREATE 语句
        let mut response = self.db.storage.query(&query).await?;

        // SurrealDB 返回的是 Value（可能包含 Thing/Array/Object 等）
        let raw: SurrealValue = response.take(0)?;
        let raw_json = serde_json::to_value(raw)
            .map_err(|e| AppError::Serialization(e))?;
        let list_json = normalize_surreal_json(raw_json);

        // 规范化后应为数组
        let results: Vec<serde_json::Value> = serde_json::from_value(list_json)
            .map_err(|e| AppError::Serialization(e))?;
        debug!("Query results: {:?}", results);

        // 从数组中取出第一个元素
        let mut created_value = results
            .into_iter()
            .next()
            .ok_or_else(|| AppError::Internal("Failed to create comment - no results returned".to_string()))?;

        // Some records may omit optional fields entirely; normalize them before deserialization.
        if created_value.get("parent_id").is_none() {
            created_value["parent_id"] = Value::Null;
        }
            
        // 处理 SurrealDB 的 Thing 格式的 ID
        if let Some(id_obj) = created_value.get("id").and_then(|v| v.as_object()) {
            if let Some(id_inner) = id_obj.get("id").and_then(|v| v.as_object()) {
                if let Some(id_str) = id_inner.get("String").and_then(|v| v.as_str()) {
                    // 将 id 替换为格式化的字符串
                    created_value["id"] = json!(format!("comment:{}", id_str));
                }
            }
        }

        // 兜底：如果数据库未返回 created_at/updated_at，则补上当前时间
        if created_value.get("created_at").is_none() {
            created_value["created_at"] = json!(Utc::now());
        }
        if created_value.get("updated_at").is_none() {
            created_value["updated_at"] = json!(Utc::now());
        }
        
        debug!("Processed comment value: {:?}", created_value);
            
        let created: Comment = serde_json::from_value(created_value)
            .map_err(|e| AppError::Internal(format!("Failed to deserialize comment: {}", e)))?;

        // Update article comment count
        self.update_article_comment_count(&request.article_id).await?;

        Ok(created)
    }

    async fn get_article_for_comment(&self, article_id: &str) -> Result<Article> {
        let pure_id = normalize_record_id(article_id, "article");
        let query = format!("SELECT * FROM article:`{}`", pure_id);
        debug!("Executing query: {}", query);

        let mut response = self.db.storage.query(&query).await?;
        let raw: SurrealValue = response.take(0)?;
        let raw_json = serde_json::to_value(raw)
            .map_err(|e| AppError::Serialization(e))?;
        let list_json = normalize_surreal_json(raw_json);
        let mut articles: Vec<Article> = serde_json::from_value(list_json)
            .map_err(|e| AppError::Serialization(e))?;

        articles
            .into_iter()
            .next()
            .ok_or_else(|| AppError::NotFound("Article not found".to_string()))
    }

    pub async fn get_comment(&self, comment_id: &str) -> Result<Option<Comment>> {
        let comment: Option<Comment> = self.db.get_by_id("comment", comment_id).await?;
        
        if let Some(ref c) = comment {
            if c.is_deleted {
                return Ok(None);
            }
        }
        
        Ok(comment)
    }

    pub async fn get_article_comments(
        &self,
        article_id: &str,
        user_id: Option<&str>,
    ) -> Result<Vec<CommentWithAuthor>> {
        debug!("Getting comments for article: {}", article_id);

        let query = r#"
            SELECT * FROM comment 
            WHERE article_id = $article_id 
            AND is_deleted = false 
            ORDER BY created_at DESC
        "#;

        let mut response = self.db.query_with_params(query, json!({
            "article_id": article_id
        })).await?;

        // SurrealDB 返回 Value，先做规范化再解析
        let raw: SurrealValue = response.take(0)?;
        let raw_json = serde_json::to_value(raw)
            .map_err(|e| AppError::Serialization(e))?;
        let list_json = normalize_surreal_json(raw_json);
        let raw_comments: Vec<serde_json::Value> = match list_json {
            Value::Array(arr) => arr,
            other => vec![other],
        };

        info!("Got {} raw comments from database", raw_comments.len());
        
        // Process each comment to fix the ID format
        let mut processed_comments = Vec::new();
        for mut comment_value in raw_comments {
            if comment_value.get("parent_id").is_none() {
                comment_value["parent_id"] = Value::Null;
            }

            // Process the main comment ID
            if let Some(id_value) = comment_value.get("id") {
                if let Some(id_str) = id_value.as_str() {
                    // Handle special bracket format: comment:⟨uuid⟩
                    if id_str.contains("⟨") && id_str.contains("⟩") {
                        if let Some(start) = id_str.find("⟨") {
                            if let Some(end) = id_str.find("⟩") {
                                let uuid = &id_str[start + 3..end];
                                comment_value["id"] = json!(format!("comment:{}", uuid));
                            }
                        }
                    }
                } else if let Some(id_obj) = id_value.as_object() {
                    // Handle SurrealDB Thing format: {"tb": "comment", "id": {"String": "uuid"}}
                    if let Some(id_inner) = id_obj.get("id").and_then(|v| v.as_object()) {
                        if let Some(id_str) = id_inner.get("String").and_then(|v| v.as_str()) {
                            comment_value["id"] = json!(format!("comment:{}", id_str));
                        }
                    }
                }
            }
            
            // Also process parent_id if it exists
            if let Some(parent_id_value) = comment_value.get_mut("parent_id") {
                if let Some(parent_str) = parent_id_value.as_str() {
                    if parent_str.contains("⟨") && parent_str.contains("⟩") {
                        if let Some(start) = parent_str.find("⟨") {
                            if let Some(end) = parent_str.find("⟩") {
                                let uuid = &parent_str[start + 3..end];
                                *parent_id_value = json!(format!("comment:{}", uuid));
                            }
                        }
                    }
                } else if let Some(parent_obj) = parent_id_value.as_object() {
                    // Handle SurrealDB Thing format for parent_id
                    if let Some(id_inner) = parent_obj.get("id").and_then(|v| v.as_object()) {
                        if let Some(id_str) = id_inner.get("String").and_then(|v| v.as_str()) {
                            *parent_id_value = json!(format!("comment:{}", id_str));
                        }
                    }
                }
            }
            
            match serde_json::from_value::<Comment>(comment_value.clone()) {
                Ok(comment) => processed_comments.push(comment),
                Err(e) => {
                    error!("Failed to deserialize comment: {}, raw value: {:?}", e, comment_value);
                    return Err(AppError::Internal(format!("Failed to deserialize comment: {}", e)));
                }
            }
        }
        
        info!("Successfully processed {} comments", processed_comments.len());

        // Build comment tree
        let mut comment_tree = self.build_comment_tree(processed_comments, user_id).await?;
        
        Ok(comment_tree)
    }

    pub async fn update_comment(
        &self,
        comment_id: &str,
        user_id: &str,
        request: UpdateCommentRequest,
    ) -> Result<Comment> {
        request
            .validate()
            .map_err(|e| AppError::ValidatorError(e))?;

        let comment: Comment = self
            .db
            .get_by_id("comment", comment_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Comment not found".to_string()))?;

        if comment.is_deleted {
            return Err(AppError::NotFound("Comment not found".to_string()));
        }

        if comment.author_id != user_id {
            return Err(AppError::forbidden(
                "You can only edit your own comments",
            ));
        }

        let updates = json!({
            "content": request.content,
            "is_edited": true,
            "updated_at": Utc::now(),
        });

        let updated: Comment = self.db.update_by_id_with_json("comment", comment_id, updates).await?.ok_or_else(|| AppError::internal("Failed to update comment"))?;
        
        Ok(updated)
    }

    pub async fn delete_comment(&self, comment_id: &str, user_id: &str) -> Result<()> {
        let comment: Comment = self
            .db
            .get_by_id("comment", comment_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Comment not found".to_string()))?;

        if comment.is_deleted {
            return Err(AppError::NotFound("Comment not found".to_string()));
        }

        if comment.author_id != user_id {
            return Err(AppError::forbidden(
                "You can only delete your own comments",
            ));
        }

        let updates = json!({
            "is_deleted": true,
            "deleted_at": Utc::now(),
        });

        self.db.update_by_id_with_json::<Value>("comment", comment_id, updates).await?;

        // Update article comment count
        self.update_article_comment_count(&comment.article_id).await?;

        Ok(())
    }

    pub async fn clap_comment(&self, comment_id: &str, user_id: &str) -> Result<()> {
        let comment: Comment = self
            .db
            .get_by_id("comment", comment_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Comment not found".to_string()))?;

        if comment.is_deleted {
            return Err(AppError::NotFound("Comment not found".to_string()));
        }

        // Check if user has already clapped
        let query = r#"
            SELECT * FROM comment_clap 
            WHERE user_id = $user_id 
            AND comment_id = $comment_id
        "#;
        
        let mut response = self.db.query_with_params(query, json!({
            "user_id": user_id,
            "comment_id": comment_id
        })).await?;
        
        let existing_claps: Vec<CommentClap> = response.take(0)?;
        
        if !existing_claps.is_empty() {
            return Err(AppError::Conflict(
                "You have already clapped this comment".to_string(),
            ));
        }

        // Create clap
        let clap = CommentClap {
            id: Uuid::new_v4().to_string(),
            user_id: user_id.to_string(),
            comment_id: comment_id.to_string(),
            created_at: Utc::now(),
        };

        self.db.create("comment_clap", clap).await?;

        // Update clap count
        self.update_comment_clap_count(comment_id).await?;

        Ok(())
    }

    pub async fn remove_clap(&self, comment_id: &str, user_id: &str) -> Result<()> {
        let query = r#"
            DELETE comment_clap 
            WHERE user_id = $user_id 
            AND comment_id = $comment_id
        "#;

        self.db.query_with_params(query, json!({
            "user_id": user_id,
            "comment_id": comment_id
        })).await?;

        // Update clap count
        self.update_comment_clap_count(comment_id).await?;

        Ok(())
    }

    // Helper methods
    async fn build_comment_tree(
        &self,
        comments: Vec<Comment>,
        user_id: Option<&str>,
    ) -> Result<Vec<CommentWithAuthor>> {
        let mut nodes: HashMap<String, CommentWithAuthor> = HashMap::new();

        // Get all author information
        let author_ids: Vec<&str> = comments.iter().map(|c| c.author_id.as_str()).collect();
        let authors = self.get_authors_info(&author_ids).await?;

        // Get user claps if user is authenticated
        let user_claps = if let Some(uid) = user_id {
            self.get_user_comment_claps(uid, &comments).await?
        } else {
            HashMap::new()
        };

        // Prepare nodes map with empty replies
        for comment in comments.into_iter() {
            let author_info = authors.get(&comment.author_id).cloned().unwrap_or_default();
            let user_has_clapped = user_claps.get(&comment.id).copied().unwrap_or(false);

            nodes.insert(
                comment.id.clone(),
                CommentWithAuthor {
                    comment: comment.clone(),
                    author_name: author_info.0,
                    author_username: author_info.1,
                    author_avatar: author_info.2,
                    user_has_clapped,
                    replies: Vec::new(),
                },
            );
        }

        // Attach children to parents by moving children out of the map
        let ids: Vec<String> = nodes.keys().cloned().collect();
        for id in ids {
            // Clone parent_id to avoid borrow issues
            let parent_id_opt = nodes.get(&id).and_then(|n| n.comment.parent_id.clone());
            if let Some(parent_id) = parent_id_opt {
                if parent_id != id {
                    if let Some(child) = nodes.remove(&id) {
                        if let Some(parent) = nodes.get_mut(&parent_id) {
                            parent.replies.push(child);
                        } else {
                            // Parent not found (shouldn't happen); place back as root
                            nodes.insert(id, child);
                        }
                    }
                }
            }
        }

        // Remaining entries in the map are roots with populated replies
        let mut root_comments: Vec<CommentWithAuthor> = nodes.into_values().collect();

        // Sort roots by creation date (desc)
        root_comments.sort_by(|a, b| b.comment.created_at.cmp(&a.comment.created_at));

        // Recursively sort replies of all nodes by creation date (desc)
        for root in &mut root_comments {
            sort_replies_by_time_desc(root);
        }

        Ok(root_comments)
    }

    async fn get_authors_info(
        &self,
        author_ids: &[&str],
    ) -> Result<HashMap<String, (String, String, Option<String>)>> {
        let mut authors = HashMap::new();
        
        for id in author_ids {
            let query = r#"
                SELECT display_name, username, avatar_url 
                FROM user_profile 
                WHERE user_id = $user_id
            "#;
            
            let mut response = self.db.query_with_params(query, json!({
                "user_id": id
            })).await?;
            let results: Vec<Value> = response.take(0)?;
            
            if let Some(author) = results.first() {
                let display_name = author["display_name"].as_str().unwrap_or("").to_string();
                let username = author["username"].as_str().unwrap_or("").to_string();
                let avatar_url = author["avatar_url"].as_str().map(String::from);
                
                authors.insert(id.to_string(), (display_name, username, avatar_url));
            }
        }

        Ok(authors)
    }

    async fn get_user_comment_claps(
        &self,
        user_id: &str,
        comments: &[Comment],
    ) -> Result<HashMap<String, bool>> {
        let mut claps = HashMap::new();
        let comment_ids: Vec<&str> = comments.iter().map(|c| c.id.as_str()).collect();

        if !comment_ids.is_empty() {
            let query = r#"
                SELECT comment_id 
                FROM comment_clap 
                WHERE user_id = $user_id 
                AND comment_id IN $comment_ids
            "#;

            let mut response = self.db.query_with_params(query, json!({
                "user_id": user_id,
                "comment_ids": comment_ids
            })).await?;

            let results: Vec<Value> = response.take(0)?;
            
            for result in results {
                if let Some(comment_id) = result["comment_id"].as_str() {
                    claps.insert(comment_id.to_string(), true);
                }
            }
        }

        Ok(claps)
    }

    async fn update_article_comment_count(&self, article_id: &str) -> Result<()> {
        // 获取纯 ID（不带 table 前缀）
        let pure_id = if article_id.starts_with("article:") {
            &article_id[8..]
        } else {
            article_id
        };

        // 使用反引号包裹 ID（与 article.rs 保持一致）
        let query = format!(r#"
            LET $count = count((SELECT * FROM comment WHERE article_id = $article_id AND is_deleted = false));
            UPDATE article:`{}` SET comment_count = $count;
        "#, pure_id);

        self.db.query_with_params(&query, json!({
            "article_id": article_id
        })).await?;

        Ok(())
    }

    async fn update_comment_clap_count(&self, comment_id: &str) -> Result<()> {
        let query = r#"
            LET $count = (SELECT count() FROM comment_clap WHERE comment_id = $comment_id);
            UPDATE comment SET clap_count = $count WHERE id = $comment_id;
        "#;

        self.db.query_with_params(query, json!({
            "comment_id": comment_id
        })).await?;

        Ok(())
    }
}

fn normalize_record_id(id: &str, table: &str) -> String {
    let trimmed = id.trim();
    let without_prefix = trimmed
        .strip_prefix(&format!("{}:", table))
        .unwrap_or(trimmed);
    let cleaned = without_prefix.replace('⟨', "").replace('⟩', "");
    cleaned.trim_matches('"').to_string()
}

// Helper: recursively sort replies by created_at desc
fn sort_replies_by_time_desc(node: &mut crate::models::comment::CommentWithAuthor) {
    node.replies.sort_by(|a, b| b.comment.created_at.cmp(&a.comment.created_at));
    for child in &mut node.replies {
        sort_replies_by_time_desc(child);
    }
}
