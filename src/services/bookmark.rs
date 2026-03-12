use crate::{
    error::{AppError, Result},
    models::{bookmark::*, article::Article},
    services::Database,
};
use chrono::Utc;
use serde_json::{json, Value};
use std::sync::Arc;
use tracing::{debug, info};
use validator::Validate;
use uuid::Uuid;

#[derive(Clone)]
pub struct BookmarkService {
    db: Arc<Database>,
}

impl BookmarkService {
    pub async fn new(db: Arc<Database>) -> Result<Self> {
        Ok(Self { db })
    }

    pub async fn create_bookmark(
        &self,
        user_id: &str,
        request: CreateBookmarkRequest,
    ) -> Result<Bookmark> {
        debug!("Creating bookmark for article: {} by user: {}", request.article_id, user_id);

        request
            .validate()
            .map_err(|e| AppError::ValidatorError(e))?;

        // Verify article exists and is published
        let article: Article = self
            .db
            .get_by_id("article", &request.article_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Article not found".to_string()))?;

        if article.status != crate::models::article::ArticleStatus::Published {
            return Err(AppError::forbidden(
                "Cannot bookmark unpublished articles",
            ));
        }

        // Extract pure article uuid for record matching/writing.
        let pure_article_id = if request.article_id.starts_with("article:") {
            &request.article_id[8..]
        } else {
            &request.article_id
        };

        // Check if bookmark already exists (compare using string form of record id)
        let query = r#"
            SELECT * FROM bookmark 
            WHERE user_id = $user_id 
            AND article_id = type::record("article", $article_id)
        "#;

        let mut response = self.db.query_with_params(query, json!({
            "user_id": user_id,
            "article_id": pure_article_id
        })).await?;

        let existing_vals: Vec<serde_json::Value> = response.take(0)?;
        if let Some(existing) = existing_vals.into_iter().next() {
            // Idempotent create: if already bookmarked, return existing bookmark.
            return Self::parse_bookmark_value(existing);
        }

        // Create bookmark using SQL to set article_id as a record(article)
        let bookmark_id = Uuid::new_v4().to_string();
        let note_clause = match &request.note {
            Some(n) if !n.is_empty() => format!(", note = '{}'", n.replace("'", "''")),
            _ => String::new(),
        };

        let query = format!(
            "CREATE bookmark:`{}` SET user_id = '{}', article_id = article:`{}`{}, created_at = time::now()",
            bookmark_id,
            user_id,
            pure_article_id,
            note_clause
        );

        let mut response = match self.db.storage.query(&query).await {
            Ok(resp) => resp,
            Err(e) => {
                let msg = e.to_string();
                if msg.contains("bookmark_unique_idx") || msg.contains("already contains") {
                    // Concurrent duplicate create: return existing bookmark as success.
                    let mut dup_resp = self.db.query_with_params(
                        r#"
                            SELECT * FROM bookmark 
                            WHERE user_id = $user_id 
                            AND article_id = type::record("article", $article_id)
                            LIMIT 1
                        "#,
                        json!({
                            "user_id": user_id,
                            "article_id": pure_article_id
                        }),
                    ).await?;
                    let existing_vals: Vec<serde_json::Value> = dup_resp.take(0)?;
                    if let Some(existing) = existing_vals.into_iter().next() {
                        return Self::parse_bookmark_value(existing);
                    }
                    return Err(AppError::Conflict("Article is already bookmarked".to_string()));
                }
                return Err(e.into());
            }
        };
        let results: Vec<serde_json::Value> = response.take(0)?;
        let mut created_val = results.into_iter().next().ok_or_else(|| AppError::internal("Failed to create bookmark"))?;

        // Normalize id and article_id to string form for our model
        if let Some(id_obj) = created_val.get("id").and_then(|v| v.as_object()) {
            if let Some(id_inner) = id_obj.get("id").and_then(|v| v.as_object()) {
                if let Some(id_str) = id_inner.get("String").and_then(|v| v.as_str()) {
                    created_val["id"] = serde_json::json!(format!("bookmark:{}", id_str));
                }
            }
        }
        if let Some(a_obj) = created_val.get("article_id").and_then(|v| v.as_object()) {
            if let Some(id_inner) = a_obj.get("id").and_then(|v| v.as_object()) {
                if let Some(id_str) = id_inner.get("String").and_then(|v| v.as_str()) {
                    created_val["article_id"] = serde_json::json!(format!("article:{}", id_str));
                }
            }
        }

        let created: Bookmark = Self::parse_bookmark_value(created_val)?;

        // Update article bookmark count
        self.update_article_bookmark_count(&created.article_id).await?;

        Ok(created)
    }

    fn parse_bookmark_value(mut v: Value) -> Result<Bookmark> {
        // Normalize id
        if let Some(id_obj) = v.get("id").and_then(|x| x.as_object()) {
            if let Some(id_inner) = id_obj.get("id").and_then(|x| x.as_object()) {
                if let Some(id_str) = id_inner.get("String").and_then(|x| x.as_str()) {
                    v["id"] = json!(format!("bookmark:{}", id_str));
                }
            } else if let Some(record_id) = id_obj.get("RecordId").and_then(|x| x.as_object()) {
                if let Some(id_str) = record_id.get("key").and_then(|x| x.as_str()) {
                    v["id"] = json!(format!("bookmark:{}", id_str));
                }
            }
        }

        // Normalize article_id
        if let Some(a_obj) = v.get("article_id").and_then(|x| x.as_object()) {
            if let Some(id_inner) = a_obj.get("id").and_then(|x| x.as_object()) {
                if let Some(id_str) = id_inner.get("String").and_then(|x| x.as_str()) {
                    v["article_id"] = json!(format!("article:{}", id_str));
                }
            } else if let Some(record_id) = a_obj.get("RecordId").and_then(|x| x.as_object()) {
                if let Some(id_str) = record_id.get("key").and_then(|x| x.as_str()) {
                    v["article_id"] = json!(format!("article:{}", id_str));
                }
            }
        }

        // Normalize user_id if stored as record
        if let Some(u_obj) = v.get("user_id").and_then(|x| x.as_object()) {
            if let Some(id_inner) = u_obj.get("id").and_then(|x| x.as_object()) {
                if let Some(id_str) = id_inner.get("String").and_then(|x| x.as_str()) {
                    v["user_id"] = json!(id_str);
                }
            } else if let Some(record_id) = u_obj.get("RecordId").and_then(|x| x.as_object()) {
                if let Some(id_str) = record_id.get("key").and_then(|x| x.as_str()) {
                    v["user_id"] = json!(id_str);
                }
            }
        }

        serde_json::from_value(v)
            .map_err(|e| AppError::internal(&format!("Failed to deserialize bookmark: {}", e)))
    }

    pub async fn get_user_bookmarks(
        &self,
        user_id: &str,
        page: Option<i32>,
        limit: Option<i32>,
    ) -> Result<Vec<BookmarkWithArticle>> {
        debug!("Getting bookmarks for user: {}", user_id);

        let page = page.unwrap_or(1).max(1);
        let limit = limit.unwrap_or(20).min(100);
        let offset = (page - 1) * limit;

        // 使用 FETCH 直接拉取 record(article) 的详情，避免 ID 字符串格式差异导致的匹配问题
        let list_query = r#"
            SELECT id, user_id, article_id, type::string(article_id) AS article_id_str, note, created_at
            FROM bookmark
            WHERE user_id = $user_id
            ORDER BY created_at DESC
            LIMIT $limit START $offset
            FETCH article_id
        "#;

        let mut response = self.db.query_with_params(list_query, json!({
            "user_id": user_id,
            "limit": limit,
            "offset": offset
        })).await?;

        let raw_list: Vec<Value> = response.take(0)?;
        let mut result = Vec::new();

        for mut b in raw_list {
            // 规范化 bookmark.id
            if let Some(id_obj) = b.get("id").and_then(|v| v.as_object()) {
                if let Some(id_inner) = id_obj.get("id").and_then(|v| v.as_object()) {
                    if let Some(id_str) = id_inner.get("String").and_then(|v| v.as_str()) {
                        b["id"] = json!(format!("bookmark:{}", id_str));
                    }
                }
            }

            // 获取已 FETCH 的文章对象，或在缺失时回退到二次查询
            let (article, article_id) = if let Some(Value::Object(_)) = b.get("article_id") {
                let article = b.get("article_id").unwrap();
                let article_id = b.get("article_id_str").and_then(|v| v.as_str()).unwrap_or("").to_string();
                (article.clone(), article_id)
            } else {
                // 回退路径：article_id 不是 record，被存为字符串，使用字符串查询文章
                let mut aid = b.get("article_id_str")
                    .and_then(|v| v.as_str())
                    .or_else(|| b.get("article_id").and_then(|v| v.as_str()))
                    .unwrap_or("")
                    .to_string();
                // 规范化形如 article:⟨uuid⟩
                if aid.contains('⟨') && aid.contains('⟩') {
                    if let (Some(s), Some(e)) = (aid.find('⟨'), aid.find('⟩')) {
                        let uuid = &aid[s + '⟨'.len_utf8()..e];
                        aid = format!("article:{}", uuid);
                    }
                }
                let mut a_resp = self.db.query_with_params(
                    "SELECT title, slug, excerpt, cover_image_url, reading_time, author_id FROM article WHERE type::string(id) = $id",
                    json!({ "id": aid })
                ).await?;
                let a_rows: Vec<Value> = a_resp.take(0)?;
                let article = match a_rows.into_iter().next() {
                    Some(v) => v,
                    None => continue,
                };
                (article, aid)
            };
            // 拿到作者ID
            let author_id = article.get("author_id").and_then(|v| v.as_str()).unwrap_or("").to_string();

            // 查询作者信息
            let mut u_resp = self.db.query_with_params(
                "SELECT display_name, username FROM user_profile WHERE user_id = $uid",
                json!({ "uid": author_id })
            ).await?;
            let u_rows: Vec<Value> = u_resp.take(0)?;
            let user = u_rows.first().cloned().unwrap_or(json!({"display_name": "", "username": ""}));

            let item = BookmarkWithArticle {
                bookmark: serde_json::from_value::<Bookmark>(json!({
                    "id": b.get("id").cloned().unwrap_or(json!("")),
                    "user_id": b.get("user_id").cloned().unwrap_or(json!("")),
                    "article_id": article_id,
                    "note": b.get("note").cloned().unwrap_or(json!(null)),
                    "created_at": b.get("created_at").cloned().unwrap_or(json!(Utc::now())),
                }))
                .map_err(|e| AppError::internal(&format!("Failed to parse bookmark: {}", e)))?,
                article_title: article.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                article_slug: article.get("slug").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                article_excerpt: article.get("excerpt").and_then(|v| v.as_str()).map(|s| s.to_string()),
                article_cover_image: article.get("cover_image_url").and_then(|v| v.as_str()).map(|s| s.to_string()),
                article_reading_time: article.get("reading_time").and_then(|v| v.as_i64()).unwrap_or(0) as i32,
                author_name: user.get("display_name").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                author_username: user.get("username").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            };

            result.push(item);
        }

        Ok(result)
    }

    pub async fn get_bookmark(&self, bookmark_id: &str, user_id: &str) -> Result<Option<Bookmark>> {
        let raw: Option<Value> = self.db.get_by_id("bookmark", bookmark_id).await?;

        let mut v = match raw {
            None => return Ok(None),
            Some(v) => v,
        };

        // Normalize id and article_id
        if let Some(id_obj) = v.get("id").and_then(|x| x.as_object()) {
            if let Some(id_inner) = id_obj.get("id").and_then(|x| x.as_object()) {
                if let Some(id_str) = id_inner.get("String").and_then(|x| x.as_str()) {
                    v["id"] = json!(format!("bookmark:{}", id_str));
                }
            }
        }
        if let Some(a_obj) = v.get("article_id").and_then(|x| x.as_object()) {
            if let Some(id_inner) = a_obj.get("id").and_then(|x| x.as_object()) {
                if let Some(id_str) = id_inner.get("String").and_then(|x| x.as_str()) {
                    v["article_id"] = json!(format!("article:{}", id_str));
                }
            }
        }

        let b: Bookmark = serde_json::from_value(v)
            .map_err(|e| AppError::internal(&format!("Failed to deserialize bookmark: {}", e)))?;

        if b.user_id != user_id {
            return Ok(None);
        }

        Ok(Some(b))
    }

    pub async fn update_bookmark(
        &self,
        bookmark_id: &str,
        user_id: &str,
        request: UpdateBookmarkRequest,
    ) -> Result<Bookmark> {
        request
            .validate()
            .map_err(|e| AppError::ValidatorError(e))?;

        let mut v: Value = self
            .db
            .get_by_id("bookmark", bookmark_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Bookmark not found".to_string()))?;

        // Normalize for deserialization
        if let Some(id_obj) = v.get("id").and_then(|x| x.as_object()) {
            if let Some(id_inner) = id_obj.get("id").and_then(|x| x.as_object()) {
                if let Some(id_str) = id_inner.get("String").and_then(|x| x.as_str()) {
                    v["id"] = json!(format!("bookmark:{}", id_str));
                }
            }
        }
        if let Some(a_obj) = v.get("article_id").and_then(|x| x.as_object()) {
            if let Some(id_inner) = a_obj.get("id").and_then(|x| x.as_object()) {
                if let Some(id_str) = id_inner.get("String").and_then(|x| x.as_str()) {
                    v["article_id"] = json!(format!("article:{}", id_str));
                }
            }
        }

        let bookmark: Bookmark = serde_json::from_value(v)
            .map_err(|e| AppError::internal(&format!("Failed to deserialize bookmark: {}", e)))?;

        if bookmark.user_id != user_id {
            return Err(AppError::forbidden(
                "You can only update your own bookmarks",
            ));
        }

        let updates = json!({
            "note": request.note,
        });

        let mut updated_val: Value = self
            .db
            .update_by_id_with_json("bookmark", bookmark_id, updates)
            .await?
            .ok_or_else(|| AppError::internal("Failed to update bookmark"))?;

        // Normalize values
        if let Some(id_obj) = updated_val.get("id").and_then(|x| x.as_object()) {
            if let Some(id_inner) = id_obj.get("id").and_then(|x| x.as_object()) {
                if let Some(id_str) = id_inner.get("String").and_then(|x| x.as_str()) {
                    updated_val["id"] = json!(format!("bookmark:{}", id_str));
                }
            }
        }
        if let Some(a_obj) = updated_val.get("article_id").and_then(|x| x.as_object()) {
            if let Some(id_inner) = a_obj.get("id").and_then(|x| x.as_object()) {
                if let Some(id_str) = id_inner.get("String").and_then(|x| x.as_str()) {
                    updated_val["article_id"] = json!(format!("article:{}", id_str));
                }
            }
        }

        let updated: Bookmark = serde_json::from_value(updated_val)
            .map_err(|e| AppError::internal(&format!("Failed to deserialize bookmark: {}", e)))?;

        Ok(updated)
    }

    pub async fn delete_bookmark(&self, bookmark_id: &str, user_id: &str) -> Result<()> {
        let mut v: Value = self
            .db
            .get_by_id("bookmark", bookmark_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Bookmark not found".to_string()))?;

        if let Some(id_obj) = v.get("id").and_then(|x| x.as_object()) {
            if let Some(id_inner) = id_obj.get("id").and_then(|x| x.as_object()) {
                if let Some(id_str) = id_inner.get("String").and_then(|x| x.as_str()) {
                    v["id"] = json!(format!("bookmark:{}", id_str));
                }
            }
        }
        if let Some(a_obj) = v.get("article_id").and_then(|x| x.as_object()) {
            if let Some(id_inner) = a_obj.get("id").and_then(|x| x.as_object()) {
                if let Some(id_str) = id_inner.get("String").and_then(|x| x.as_str()) {
                    v["article_id"] = json!(format!("article:{}", id_str));
                }
            }
        }

        let bookmark: Bookmark = serde_json::from_value(v)
            .map_err(|e| AppError::internal(&format!("Failed to deserialize bookmark: {}", e)))?;

        if bookmark.user_id != user_id {
            return Err(AppError::forbidden(
                "You can only delete your own bookmarks",
            ));
        }

        self.db.delete_by_id("bookmark", bookmark_id).await?;

        // Update article bookmark count
        self.update_article_bookmark_count(&bookmark.article_id).await?;

        Ok(())
    }

    pub async fn delete_bookmark_by_article(
        &self,
        article_id: &str,
        user_id: &str,
    ) -> Result<()> {
        debug!("Deleting bookmark for article: {} by user: {}", article_id, user_id);

        let query = r#"
            DELETE bookmark 
            WHERE user_id = $user_id 
            AND type::string(article_id) = $article_id
        "#;

        self.db.query_with_params(query, json!({
            "user_id": user_id,
            "article_id": article_id
        })).await?;

        // Update article bookmark count
        self.update_article_bookmark_count(article_id).await?;

        Ok(())
    }

    pub async fn is_bookmarked(&self, article_id: &str, user_id: &str) -> Result<bool> {
        let query = r#"
            SELECT count() as count 
            FROM bookmark 
            WHERE user_id = $user_id 
            AND type::string(article_id) = $article_id
        "#;

        let mut response = self.db.query_with_params(query, json!({
            "user_id": user_id,
            "article_id": article_id
        })).await?;
        
        let result: Vec<Value> = response.take(0)?;

        let count = result
            .first()
            .and_then(|v| v.get("count"))
            .and_then(|v| v.as_i64())
            .unwrap_or(0);

        Ok(count > 0)
    }

    async fn update_article_bookmark_count(&self, article_id: &str) -> Result<()> {
        let query = r#"
            UPDATE article
            SET bookmark_count = count((SELECT * FROM bookmark WHERE type::string(article_id) = $article_id))
            WHERE type::string(id) = $article_id;
        "#;

        self.db.query_with_params(query, json!({
            "article_id": article_id
        })).await?;

        Ok(())
    }
}
