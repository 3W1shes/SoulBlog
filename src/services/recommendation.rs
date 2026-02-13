use crate::{
    error::{AppError, Result},
    models::{
        recommendation::*,
        article::{Article, ArticleListItem, ArticleStatus, AuthorInfo, PublicationInfo, TagInfo},
        user::UserProfile,
        follow::Follow,
        clap::Clap,
        bookmark::Bookmark,
        comment::Comment,
        tag::Tag,
    },
    services::Database,
};
use crate::services::article::normalize_surreal_json;
use std::sync::Arc;
use std::collections::HashMap;
use chrono::{Duration, Utc};
use serde::de::DeserializeOwned;
use serde_json::{json, Value as JsonValue};
use tracing::{debug, info, warn};
use uuid::Uuid;
use surrealdb::Value as SurrealValue;

#[derive(Clone)]
pub struct RecommendationService {
    db: Arc<Database>,
}

fn surreal_value_to_json_list(raw: SurrealValue) -> Result<Vec<JsonValue>> {
    let raw_json = serde_json::to_value(raw)?;
    let list_json = normalize_surreal_json(raw_json);
    let items: Vec<JsonValue> = serde_json::from_value(list_json)?;
    Ok(items)
}

fn surreal_value_to_t_list<T: DeserializeOwned>(raw: SurrealValue) -> Result<Vec<T>> {
    let items = surreal_value_to_json_list(raw)?;
    let mut out = Vec::new();
    for item in items {
        match serde_json::from_value::<T>(item) {
            Ok(value) => out.push(value),
            Err(err) => {
                warn!("Skipping item due to deserialization error: {}", err);
            }
        }
    }
    Ok(out)
}

impl RecommendationService {
    pub async fn new(db: Arc<Database>) -> Result<Self> {
        Ok(Self { db })
    }

    /// 获取用户推荐文章
    pub async fn get_recommendations(
        &self,
        request: RecommendationRequest,
    ) -> Result<RecommendationResult> {
        debug!("Getting recommendations with request: {:?}", request);

        let user_id = request.user_id.as_deref();
        let limit = request.limit.unwrap_or(10);
        let algorithm = request.algorithm.clone().unwrap_or(RecommendationAlgorithm::Hybrid);

        let articles = match algorithm {
            RecommendationAlgorithm::ContentBased => {
                self.content_based_recommendations(user_id, limit, &request).await?
            }
            RecommendationAlgorithm::CollaborativeFiltering => {
                self.collaborative_filtering_recommendations(user_id, limit, &request).await?
            }
            RecommendationAlgorithm::Hybrid => {
                self.hybrid_recommendations(user_id, limit, &request).await?
            }
            RecommendationAlgorithm::Trending => {
                self.trending_recommendations(limit, &request).await?
            }
            RecommendationAlgorithm::Following => {
                self.following_recommendations(user_id, limit, &request).await?
            }
        };

        let total = articles.len();
        Ok(RecommendationResult {
            articles,
            total,
            algorithm_used: format!("{:?}", algorithm),
            generated_at: Utc::now(),
        })
    }

    /// 基于内容的推荐
    async fn content_based_recommendations(
        &self,
        user_id: Option<&str>,
        limit: usize,
        request: &RecommendationRequest,
    ) -> Result<Vec<RecommendedArticle>> {
        debug!("Generating content-based recommendations for user: {:?}", user_id);

        if let Some(uid) = user_id {
            // 获取用户的兴趣标签
            let user_tags = self.get_user_preferred_tags(uid).await?;
            let user_authors = self.get_user_preferred_authors(uid).await?;
            
            // 基于用户兴趣推荐
            self.recommend_by_user_preferences(uid, &user_tags, &user_authors, limit, request).await
        } else {
            // 匿名用户推荐热门内容
            self.trending_recommendations(limit, request).await
        }
    }

    /// 协同过滤推荐
    async fn collaborative_filtering_recommendations(
        &self,
        user_id: Option<&str>,
        limit: usize,
        request: &RecommendationRequest,
    ) -> Result<Vec<RecommendedArticle>> {
        debug!("Generating collaborative filtering recommendations for user: {:?}", user_id);

        if let Some(uid) = user_id {
            // 找到相似用户
            let similar_users = self.find_similar_users(uid).await?;
            
            // 基于相似用户的喜好推荐
            self.recommend_by_similar_users(uid, &similar_users, limit, request).await
        } else {
            // 匿名用户无法使用协同过滤，回退到热门推荐
            self.trending_recommendations(limit, request).await
        }
    }

    /// 混合推荐
    async fn hybrid_recommendations(
        &self,
        user_id: Option<&str>,
        limit: usize,
        request: &RecommendationRequest,
    ) -> Result<Vec<RecommendedArticle>> {
        debug!("Generating hybrid recommendations for user: {:?}", user_id);

        if let Some(uid) = user_id {
            let half_limit = limit / 2;
            
            // 获取内容推荐和协同过滤推荐
            let mut content_recs = self.content_based_recommendations(Some(uid), half_limit, request).await?;
            let mut collab_recs = self.collaborative_filtering_recommendations(Some(uid), half_limit, request).await?;
            
            // 合并和去重
            content_recs.append(&mut collab_recs);
            self.deduplicate_and_rank(content_recs, limit)
        } else {
            self.trending_recommendations(limit, request).await
        }
    }

    /// 热门推荐
    async fn trending_recommendations(
        &self,
        limit: usize,
        request: &RecommendationRequest,
    ) -> Result<Vec<RecommendedArticle>> {
        debug!("Generating trending recommendations");

        let mut query = r#"
            SELECT *, 
                (clap_count * 0.3 + view_count * 0.1 + comment_count * 0.4 + bookmark_count * 0.2) as trending_score
            FROM article 
            WHERE status = 'published' 
            AND is_deleted = false
        "#.to_string();

        let mut params = json!({
            "limit": limit
        });

        // 添加过滤条件
        if let Some(tags) = &request.tags {
            query.push_str(" AND (");
            for (i, tag) in tags.iter().enumerate() {
                if i > 0 { query.push_str(" OR "); }
                query.push_str(&format!("$tag_{} IN tags", i));
                params[format!("tag_{}", i)] = json!(tag);
            }
            query.push_str(")");
        }

        if let Some(authors) = &request.authors {
            query.push_str(" AND author_id IN $authors");
            params["authors"] = json!(authors);
        }

        // 过滤最近7天的文章以获得真正的"热门"
        let week_ago = (Utc::now() - Duration::days(7)).to_rfc3339();
        query.push_str(&format!(" AND created_at >= d'{}'", week_ago));

        query.push_str(" ORDER BY trending_score DESC, created_at DESC LIMIT $limit");

        let mut response = self.db.query_with_params(&query, params).await?;
        let raw: SurrealValue = response.take(0)?;
        let articles: Vec<JsonValue> = surreal_value_to_json_list(raw)?;

        let mut recommendations = Vec::new();
        for (i, article_data) in articles.iter().enumerate() {
            let normalized = normalize_surreal_json(article_data.clone());
            if let Ok(article) = serde_json::from_value::<Article>(normalized) {
                let list_item = self.article_to_list_item(&article).await?;

                let score = article_data.get("trending_score")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0);

                recommendations.push(RecommendedArticle {
                    article: list_item,
                    score,
                    reason: "热门文章".to_string(),
                });
            }
        }

        Ok(recommendations)
    }

    /// 关注用户的文章推荐
    async fn following_recommendations(
        &self,
        user_id: Option<&str>,
        limit: usize,
        request: &RecommendationRequest,
    ) -> Result<Vec<RecommendedArticle>> {
        debug!("Generating following recommendations for user: {:?}", user_id);

        let uid = user_id.ok_or_else(|| AppError::Authentication("User ID required for following recommendations".to_string()))?;

        let query = r#"
            SELECT *
            FROM article
            WHERE author_id IN (
                SELECT following_user_id FROM follow WHERE follower_user_id = $user_id
            )
            AND status = 'published'
            AND is_deleted = false
            ORDER BY created_at DESC
            LIMIT $limit
        "#;

        let mut response = self.db.query_with_params(query, json!({
            "user_id": uid,
            "limit": limit
        })).await?;
        let raw: SurrealValue = response.take(0)?;
        let articles: Vec<JsonValue> = surreal_value_to_json_list(raw)?;
        let mut recommendations = Vec::new();

        for article_data in articles.iter() {
            let normalized = normalize_surreal_json(article_data.clone());
            if let Ok(article) = serde_json::from_value::<Article>(normalized) {
                let list_item = self.article_to_list_item(&article).await?;

                recommendations.push(RecommendedArticle {
                    article: list_item,
                    score: 100.0, // 关注的作者给最高分
                    reason: "来自您关注的作者".to_string(),
                });
            }
        }

        Ok(recommendations)
    }

    /// 获取用户偏好标签
    async fn get_user_preferred_tags(&self, user_id: &str) -> Result<Vec<TagPreference>> {
        // 先获取用户点赞的文章
        let clapped_query = "SELECT article_id FROM clap WHERE user_id = $user_id";
        
        let mut clap_response = self.db.query_with_params(clapped_query, json!({
            "user_id": user_id
        })).await?;
        
        let clapped_raw: SurrealValue = clap_response.take(0)?;
        let clapped_articles: Vec<JsonValue> = surreal_value_to_json_list(clapped_raw)?;
        if clapped_articles.is_empty() {
            return Ok(Vec::new());
        }
        
        // 统计每个标签出现的次数
        let mut tag_weights: HashMap<String, (String, f64)> = HashMap::new();
        
        for clap in clapped_articles {
            if let Some(article_id) = clap.get("article_id").and_then(|v| v.as_str()) {
                // 获取文章的标签
                let tags_query = "SELECT tag_id FROM article_tag WHERE article_id = $article_id";
                
                if let Ok(mut tags_response) = self.db.query_with_params(tags_query, json!({
                    "article_id": article_id
                })).await {
                    if let Ok(tag_relations_raw) = tags_response.take::<SurrealValue>(0) {
                        let tag_relations = surreal_value_to_json_list(tag_relations_raw)?;
                        for rel in tag_relations {
                            if let Some(tag_id) = rel.get("tag_id").and_then(|v| v.as_str()) {
                                // 获取标签信息
                                if let Ok(mut tag_response) = self.db.query(&format!("SELECT * FROM {}", tag_id)).await {
                                    if let Ok(tags_raw) = tag_response.take::<SurrealValue>(0) {
                                        let tags = surreal_value_to_json_list(tags_raw)?;
                                        if let Some(tag) = tags.first() {
                                            if let (Some(id), Some(name)) = (
                                                tag.get("id").and_then(|v| v.as_str()),
                                                tag.get("name").and_then(|v| v.as_str())
                                            ) {
                                                let entry = tag_weights.entry(id.to_string())
                                                    .or_insert((name.to_string(), 0.0));
                                                entry.1 += 1.0;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        
        // 转换为 TagPreference 并排序
        let mut preferences: Vec<TagPreference> = tag_weights.into_iter()
            .map(|(id, (name, weight))| TagPreference {
                tag_id: id,
                tag_name: name,
                weight,
            })
            .collect();
            
        preferences.sort_by(|a, b| b.weight.partial_cmp(&a.weight).unwrap_or(std::cmp::Ordering::Equal));
        preferences.truncate(20);
        
        Ok(preferences)
    }

    /// 获取用户偏好作者
    async fn get_user_preferred_authors(&self, user_id: &str) -> Result<Vec<AuthorPreference>> {
        // 先获取用户点赞的文章，再在 Rust 中聚合作者偏好
        let clapped_query = "SELECT article_id FROM clap WHERE user_id = $user_id";
        let mut clap_response = self.db.query_with_params(clapped_query, json!({
            "user_id": user_id
        })).await?;

        let raw: SurrealValue = clap_response.take(0)?;
        let clapped: Vec<JsonValue> = surreal_value_to_json_list(raw)?;
        let article_ids: Vec<String> = clapped
            .iter()
            .filter_map(|v| v.get("article_id").and_then(|v| v.as_str()).map(|s| s.to_string()))
            .collect();

        if article_ids.is_empty() {
            return Ok(Vec::new());
        }

        let articles_query = r#"
            SELECT author_id
            FROM article
            WHERE id IN $article_ids
        "#;

        let mut response = self.db.query_with_params(articles_query, json!({
            "article_ids": article_ids
        })).await?;

        let raw: SurrealValue = response.take(0)?;
        let results: Vec<JsonValue> = surreal_value_to_json_list(raw)?;

        let mut counts: std::collections::HashMap<String, f64> = std::collections::HashMap::new();
        for result in results {
            if let Some(author_id) = result.get("author_id").and_then(|v| v.as_str()) {
                *counts.entry(author_id.to_string()).or_insert(0.0) += 1.0;
            }
        }

        let mut preferences: Vec<AuthorPreference> = counts
            .into_iter()
            .map(|(author_id, weight)| AuthorPreference { author_id, weight })
            .collect();

        preferences.sort_by(|a, b| b.weight.partial_cmp(&a.weight).unwrap_or(std::cmp::Ordering::Equal));
        preferences.truncate(10);

        Ok(preferences)
    }

    /// 基于用户偏好推荐文章
    async fn recommend_by_user_preferences(
        &self,
        user_id: &str,
        tag_preferences: &[TagPreference],
        author_preferences: &[AuthorPreference],
        limit: usize,
        request: &RecommendationRequest,
    ) -> Result<Vec<RecommendedArticle>> {
        let mut recommendations = Vec::new();

        // 基于标签偏好推荐
        if !tag_preferences.is_empty() {
            let tag_recs = self.recommend_by_tags(user_id, tag_preferences, limit / 2).await?;
            recommendations.extend(tag_recs);
        }

        // 基于作者偏好推荐
        if !author_preferences.is_empty() {
            let author_recs = self.recommend_by_authors(user_id, author_preferences, limit / 2).await?;
            recommendations.extend(author_recs);
        }

        // 去重并排序
        Ok(self.deduplicate_and_rank(recommendations, limit)?)
    }

    /// 基于标签推荐
    async fn recommend_by_tags(
        &self,
        user_id: &str,
        tag_preferences: &[TagPreference],
        limit: usize,
    ) -> Result<Vec<RecommendedArticle>> {
        let tag_ids: Vec<&str> = tag_preferences.iter().map(|t| t.tag_id.as_str()).collect();

        let query = r#"
            SELECT *
            FROM article
            WHERE id IN (
                SELECT article_id FROM article_tag WHERE tag_id IN $tag_ids
            )
            AND status = 'published'
            AND is_deleted = false
            AND author_id != $user_id
            AND id NOT IN (
                SELECT article_id FROM clap WHERE user_id = $user_id
            )
            ORDER BY clap_count DESC, created_at DESC
            LIMIT $limit
        "#;

        let mut response = self.db.query_with_params(query, json!({
            "tag_ids": tag_ids,
            "user_id": user_id,
            "limit": limit
        })).await?;

        let raw: SurrealValue = response.take(0)?;
        let articles: Vec<Article> = surreal_value_to_t_list(raw)?;
        let mut recommendations = Vec::new();

        for article in articles {
            let list_item = self.article_to_list_item(&article).await?;

            recommendations.push(RecommendedArticle {
                article: list_item,
                score: 80.0,
                reason: "基于您的兴趣标签".to_string(),
            });
        }

        Ok(recommendations)
    }

    /// 基于作者推荐
    async fn recommend_by_authors(
        &self,
        user_id: &str,
        author_preferences: &[AuthorPreference],
        limit: usize,
    ) -> Result<Vec<RecommendedArticle>> {
        let author_ids: Vec<&str> = author_preferences.iter().map(|a| a.author_id.as_str()).collect();

        let query = r#"
            SELECT * FROM article
            WHERE author_id IN $author_ids
            AND status = 'published'
            AND is_deleted = false
            AND id NOT IN (
                SELECT article_id FROM clap WHERE user_id = $user_id
            )
            ORDER BY created_at DESC
            LIMIT $limit
        "#;

        let mut response = self.db.query_with_params(query, json!({
            "author_ids": author_ids,
            "user_id": user_id,
            "limit": limit
        })).await?;

        let raw: SurrealValue = response.take(0)?;
        let articles: Vec<Article> = surreal_value_to_t_list(raw)?;
        let mut recommendations = Vec::new();

        for article in articles {
            let list_item = self.article_to_list_item(&article).await?;

            recommendations.push(RecommendedArticle {
                article: list_item,
                score: 90.0,
                reason: "来自您喜欢的作者".to_string(),
            });
        }

        Ok(recommendations)
    }

    /// 找到相似用户
    async fn find_similar_users(&self, user_id: &str) -> Result<Vec<String>> {
        // 简化的相似性计算：基于共同点赞的文章
        // 先获取当前用户点赞的文章，再在 Rust 中聚合相似用户
        let clapped_query = "SELECT article_id FROM clap WHERE user_id = $user_id";
        let mut clap_response = self.db.query_with_params(clapped_query, json!({
            "user_id": user_id
        })).await?;

        let raw: SurrealValue = clap_response.take(0)?;
        let clapped: Vec<JsonValue> = surreal_value_to_json_list(raw)?;
        let article_ids: Vec<String> = clapped
            .iter()
            .filter_map(|v| v.get("article_id").and_then(|v| v.as_str()).map(|s| s.to_string()))
            .collect();

        if article_ids.is_empty() {
            return Ok(Vec::new());
        }

        let others_query = r#"
            SELECT user_id FROM clap
            WHERE article_id IN $article_ids
            AND user_id != $user_id
        "#;

        let mut response = self.db.query_with_params(others_query, json!({
            "article_ids": article_ids,
            "user_id": user_id
        })).await?;

        let raw: SurrealValue = response.take(0)?;
        let results: Vec<JsonValue> = surreal_value_to_json_list(raw)?;

        let mut counts: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
        for result in results {
            if let Some(uid) = result.get("user_id").and_then(|v| v.as_str()) {
                *counts.entry(uid.to_string()).or_insert(0) += 1;
            }
        }

        let mut similar_users: Vec<(String, i64)> = counts.into_iter().collect();
        similar_users.sort_by(|a, b| b.1.cmp(&a.1));
        similar_users.truncate(10);

        Ok(similar_users.into_iter().map(|(uid, _)| uid).collect())
    }

    /// 基于相似用户推荐
    async fn recommend_by_similar_users(
        &self,
        user_id: &str,
        similar_users: &[String],
        limit: usize,
        request: &RecommendationRequest,
    ) -> Result<Vec<RecommendedArticle>> {
        if similar_users.is_empty() {
            return Ok(Vec::new());
        }

        let query = r#"
            SELECT *,
                count((SELECT * FROM clap WHERE article_id = id AND user_id IN $similar_users)) as popularity
            FROM article
            WHERE id IN (
                SELECT article_id FROM clap WHERE user_id IN $similar_users
            )
            AND status = 'published'
            AND is_deleted = false
            AND id NOT IN (
                SELECT article_id FROM clap WHERE user_id = $user_id
            )
            ORDER BY popularity DESC, created_at DESC
            LIMIT $limit
        "#;

        let mut response = self.db.query_with_params(query, json!({
            "similar_users": similar_users,
            "user_id": user_id,
            "limit": limit
        })).await?;

        let raw: SurrealValue = response.take(0)?;
        let articles: Vec<JsonValue> = surreal_value_to_json_list(raw)?;
        let mut recommendations = Vec::new();

        for article_data in articles.iter() {
            let normalized = normalize_surreal_json(article_data.clone());
            if let Ok(article) = serde_json::from_value::<Article>(normalized) {
                let list_item = self.article_to_list_item(&article).await?;

                let popularity = article_data.get("popularity")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0);

                recommendations.push(RecommendedArticle {
                    article: list_item,
                    score: 70.0 + popularity * 5.0,
                    reason: "相似用户喜欢的文章".to_string(),
                });
            }
        }

        Ok(recommendations)
    }

    /// 去重并排序推荐结果
    fn deduplicate_and_rank(
        &self,
        mut recommendations: Vec<RecommendedArticle>,
        limit: usize,
    ) -> Result<Vec<RecommendedArticle>> {
        // 按文章ID去重
        let mut seen = std::collections::HashSet::new();
        recommendations.retain(|rec| seen.insert(rec.article.id.clone()));

        // 按分数排序
        recommendations.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

        // 限制数量
        recommendations.truncate(limit);

        Ok(recommendations)
    }

    /// 记录用户交互
    pub async fn record_interaction(
        &self,
        user_id: &str,
        article_id: &str,
        interaction_type: InteractionType,
    ) -> Result<()> {
        debug!("Recording interaction: {} -> {} ({:?})", user_id, article_id, interaction_type);

        let interaction = UserInteraction {
            id: Uuid::new_v4().to_string(),
            user_id: user_id.to_string(),
            article_id: article_id.to_string(),
            interaction_type: interaction_type.as_str().to_string(),
            weight: interaction_type.default_weight(),
            created_at: Utc::now(),
        };

        let interaction_json = json!({
            "id": interaction.id,
            "user_id": interaction.user_id,
            "article_id": interaction.article_id,
            "interaction_type": interaction.interaction_type,
            "weight": interaction.weight,
            "created_at": interaction.created_at,
        });

        self.db.query_with_params(
            "CREATE user_interaction CONTENT $data RETURN *",
            json!({ "data": interaction_json })
        ).await?;
        Ok(())
    }

    /// 更新推荐系统缓存
    pub async fn update_recommendations(&self) -> Result<()> {
        info!("Starting recommendation system update");

        // 更新热门文章缓存（失败时不阻塞推荐系统）
        if let Err(e) = self.update_trending_cache().await {
            warn!("update_trending_cache failed: {}", e);
        }

        // 计算用户画像（失败时不阻塞推荐系统）
        if let Err(e) = self.update_user_profiles().await {
            warn!("update_user_profiles failed: {}", e);
        }

        // 预计算推荐结果（对活跃用户，失败时不阻塞推荐系统）
        if let Err(e) = self.precompute_recommendations().await {
            warn!("precompute_recommendations failed: {}", e);
        }

        info!("Recommendation system update completed");
        Ok(())
    }

    /// 更新热门文章缓存
    async fn update_trending_cache(&self) -> Result<()> {
        let query = r#"
            SELECT 
                id as article_id,
                view_count,
                clap_count,
                comment_count,
                bookmark_count,
                created_at,
                (
                    view_count * 0.1 + 
                    clap_count * 0.3 + 
                    comment_count * 0.4 + 
                    bookmark_count * 0.2 +
                    IF created_at > $week_ago THEN 20 ELSE 0 END
                ) as trending_score
            FROM article
            WHERE status = 'published' 
            AND is_deleted = false
            ORDER BY trending_score DESC
        "#;

        let week_ago = (Utc::now() - Duration::days(7)).to_rfc3339();
        let query = query.replace("$week_ago", &format!("d'{}'", week_ago));
        let mut response = self.db.query(&query).await?;
        let raw: SurrealValue = response.take(0)?;
        let trending_metrics: Vec<JsonValue> = surreal_value_to_json_list(raw)?;

        // 清理旧的趋势数据
        let yesterday = (Utc::now() - Duration::days(1)).to_rfc3339();
        let delete_query = format!(
            "DELETE trending_metrics WHERE calculated_at < d'{}'",
            yesterday
        );
        self.db.query(&delete_query).await?;

        // 插入新的趋势数据
        for metric in trending_metrics {
            if let Some(article_id) = metric.get("article_id").and_then(|v| v.as_str()) {
                let trending_metric = TrendingMetrics {
                    article_id: article_id.to_string(),
                    views_24h: 0, // 简化版本，可以后续优化
                    views_7d: metric.get("view_count").and_then(|v| v.as_i64()).unwrap_or(0),
                    claps_24h: 0,
                    claps_7d: metric.get("clap_count").and_then(|v| v.as_i64()).unwrap_or(0),
                    comments_24h: 0,
                    comments_7d: metric.get("comment_count").and_then(|v| v.as_i64()).unwrap_or(0),
                    trending_score: metric.get("trending_score").and_then(|v| v.as_f64()).unwrap_or(0.0),
                    calculated_at: Utc::now(),
                };

                let trending_json = json!({
                    "article_id": trending_metric.article_id,
                    "views_24h": trending_metric.views_24h,
                    "views_7d": trending_metric.views_7d,
                    "claps_24h": trending_metric.claps_24h,
                    "claps_7d": trending_metric.claps_7d,
                    "comments_24h": trending_metric.comments_24h,
                    "comments_7d": trending_metric.comments_7d,
                    "trending_score": trending_metric.trending_score,
                    "calculated_at": trending_metric.calculated_at.to_rfc3339(),
                });

                let json_str = serde_json::to_string(&trending_json)
                    .map_err(|e| AppError::Internal(e.to_string()))?;
                let create_query = format!(
                    "CREATE trending_metrics CONTENT {} RETURN *",
                    json_str
                );
                self.db.query(&create_query).await?;
            }
        }

        Ok(())
    }

    /// 更新用户画像
    async fn update_user_profiles(&self) -> Result<()> {
        // 获取活跃用户列表
        let query = r#"
            SELECT user_id
            FROM user_interaction
            WHERE created_at > $week_ago
            GROUP BY user_id
        "#;

        let mut response = self.db.query_with_params(query, json!({
            "week_ago": Utc::now() - Duration::days(7)
        })).await?;

        let raw: SurrealValue = response.take(0)?;
        let user_ids: Vec<JsonValue> = surreal_value_to_json_list(raw)?;

        for user_data in user_ids {
            if let Some(user_id) = user_data.get("user_id").and_then(|v| v.as_str()) {
                let _ = self.build_user_profile(user_id).await; // 忽略单个用户的错误
            }
        }

        Ok(())
    }

    /// 构建用户画像
    async fn build_user_profile(&self, user_id: &str) -> Result<()> {
        let tag_preferences = self.get_user_preferred_tags(user_id).await?;
        let author_preferences = self.get_user_preferred_authors(user_id).await?;

        // 计算平均阅读时间
        let avg_reading_time_query = r#"
            SELECT AVG(a.reading_time) as avg_time
            FROM article a
            JOIN user_interaction ui ON a.id = ui.article_id
            WHERE ui.user_id = $user_id
            AND ui.interaction_type = 'ReadComplete'
        "#;

        let mut response = self.db.query_with_params(avg_reading_time_query, json!({
            "user_id": user_id
        })).await?;

        let raw: SurrealValue = response.take(0)?;
        let avg_time_result: Vec<JsonValue> = surreal_value_to_json_list(raw)?;
        let avg_reading_time = avg_time_result.first()
            .and_then(|v| v.get("avg_time"))
            .and_then(|v| v.as_f64())
            .unwrap_or(5.0);

        // 计算总交互数
        let total_interactions_query = r#"
            SELECT count() as total
            FROM user_interaction
            WHERE user_id = $user_id
        "#;

        let mut response = self.db.query_with_params(total_interactions_query, json!({
            "user_id": user_id
        })).await?;

        let raw: SurrealValue = response.take(0)?;
        let total_result: Vec<JsonValue> = surreal_value_to_json_list(raw)?;
        let total_interactions = total_result.first()
            .and_then(|v| v.get("total"))
            .and_then(|v| v.as_i64())
            .unwrap_or(0);

        let profile = crate::models::recommendation::UserProfile {
            user_id: user_id.to_string(),
            preferred_tags: tag_preferences,
            preferred_authors: author_preferences,
            avg_reading_time,
            total_interactions,
            last_updated: Utc::now(),
        };

        // 删除旧的用户画像
        let delete_query = "DELETE user_profile_recommendation WHERE user_id = $user_id";
        self.db.query_with_params(delete_query, json!({
            "user_id": user_id
        })).await?;

        // 创建新的用户画像
        let profile_json = json!({
            "user_id": profile.user_id,
            "preferred_tags": profile.preferred_tags,
            "preferred_authors": profile.preferred_authors,
            "avg_reading_time": profile.avg_reading_time,
            "total_interactions": profile.total_interactions,
            "last_updated": profile.last_updated,
        });

        self.db.query_with_params(
            "CREATE user_profile_recommendation CONTENT $data RETURN *",
            json!({ "data": profile_json })
        ).await?;

        Ok(())
    }

    /// 预计算推荐结果
    async fn precompute_recommendations(&self) -> Result<()> {
        // 简化版本：只为最活跃的用户预计算
        let active_users_query = r#"
            SELECT user_id, count() as interaction_count
            FROM user_interaction
            WHERE created_at > $week_ago
            GROUP BY user_id
            ORDER BY interaction_count DESC
            LIMIT 100
        "#;

        let mut response = self.db.query_with_params(active_users_query, json!({
            "week_ago": Utc::now() - Duration::days(7)
        })).await?;

        let raw: SurrealValue = response.take(0)?;
        let active_users: Vec<JsonValue> = surreal_value_to_json_list(raw)?;

        for user_data in active_users {
            if let Some(user_id) = user_data.get("user_id").and_then(|v| v.as_str()) {
                let request = RecommendationRequest {
                    user_id: Some(user_id.to_string()),
                    limit: Some(20),
                    exclude_read: Some(true),
                    algorithm: Some(RecommendationAlgorithm::Hybrid),
                    tags: None,
                    authors: None,
                };

                // 预计算并缓存推荐结果
                if let Ok(recommendations) = self.get_recommendations(request).await {
                    // 这里可以将结果存储到缓存表中
                    debug!("Precomputed {} recommendations for user {}", 
                          recommendations.articles.len(), user_id);
                }
            }
        }

        Ok(())
    }

    /// 获取相关文章推荐
    pub async fn get_related_articles(
        &self,
        article_id: &str,
        limit: usize,
    ) -> Result<Vec<RecommendedArticle>> {
        debug!("Getting related articles for article: {}", article_id);

        // 获取目标文章信息
        let article: Article = self.db.get_by_id("article", article_id).await?
            .ok_or_else(|| AppError::NotFound("Article not found".to_string()))?;

        // 基于标签找相关文章
        let query = r#"
            SELECT DISTINCT a.*, COUNT(at1.tag_id) as common_tags
            FROM article a
            JOIN article_tag at1 ON a.id = at1.article_id
            JOIN article_tag at2 ON at1.tag_id = at2.tag_id
            WHERE at2.article_id = $article_id
            AND a.id != $article_id
            AND a.status = 'published'
            AND a.is_deleted = false
            GROUP BY a.id
            ORDER BY common_tags DESC, a.clap_count DESC
            LIMIT $limit
        "#;

        let mut response = self.db.query_with_params(query, json!({
            "article_id": article_id,
            "limit": limit
        })).await?;

        let raw: SurrealValue = response.take(0)?;
        let articles: Vec<JsonValue> = surreal_value_to_json_list(raw)?;
        let mut recommendations = Vec::new();

        for article_data in articles.iter() {
            let normalized = normalize_surreal_json(article_data.clone());
            if let Ok(related_article) = serde_json::from_value::<Article>(normalized) {
                let list_item = self.article_to_list_item(&related_article).await?;

                let common_tags = article_data.get("common_tags")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0);

                recommendations.push(RecommendedArticle {
                    article: list_item,
                    score: common_tags * 10.0 + related_article.clap_count as f64 * 0.1,
                    reason: "相关主题".to_string(),
                });
            }
        }

        Ok(recommendations)
    }

    /// Helper method to convert article data to ArticleListItem
    async fn article_to_list_item(&self, article: &Article) -> Result<ArticleListItem> {
        // Get author info
        let author_query = r#"
            SELECT id, username, display_name, avatar_url, is_verified
            FROM user_profile
            WHERE user_id = $author_id
        "#;
        
        let mut author_response = self.db.query_with_params(author_query, json!({
            "author_id": &article.author_id
        })).await?;
        
        let author_raw: SurrealValue = author_response.take(0)?;
        let author_data: Vec<JsonValue> = surreal_value_to_json_list(author_raw)?;
        let author_info = if let Some(author) = author_data.first() {
            AuthorInfo {
                id: author["id"].as_str().unwrap_or("").to_string(),
                username: author["username"].as_str().unwrap_or("").to_string(),
                display_name: author["display_name"].as_str().unwrap_or("").to_string(),
                avatar_url: author["avatar_url"].as_str().map(String::from),
                is_verified: author["is_verified"].as_bool().unwrap_or(false),
            }
        } else {
            AuthorInfo {
                id: article.author_id.clone(),
                username: "unknown".to_string(),
                display_name: "Unknown Author".to_string(),
                avatar_url: None,
                is_verified: false,
            }
        };
        
        // Get publication info if exists
        let publication_info = if let Some(pub_id) = &article.publication_id {
            let pub_query = r#"
                SELECT id, name, slug, logo_url
                FROM publication
                WHERE id = $publication_id
            "#;
            
            let mut pub_response = self.db.query_with_params(pub_query, json!({
                "publication_id": pub_id
            })).await?;
            
            let pub_raw: SurrealValue = pub_response.take(0)?;
            let pub_data: Vec<JsonValue> = surreal_value_to_json_list(pub_raw)?;
            pub_data.first().map(|p| PublicationInfo {
                id: p["id"].as_str().unwrap_or("").to_string(),
                name: p["name"].as_str().unwrap_or("").to_string(),
                slug: p["slug"].as_str().unwrap_or("").to_string(),
                logo_url: p["logo_url"].as_str().map(String::from),
            })
        } else {
            None
        };
        
        // Get tags info - 先获取article_tag关系，再获取tag详情
        let tag_relations_query = "SELECT tag_id FROM article_tag WHERE article_id = $article_id";
        
        let mut tag_rel_response = self.db.query_with_params(tag_relations_query, json!({
            "article_id": &article.id
        })).await?;
        
        let tag_rel_raw: SurrealValue = tag_rel_response.take(0)?;
        let tag_relations: Vec<JsonValue> = surreal_value_to_json_list(tag_rel_raw)?;
        let mut tags: Vec<TagInfo> = Vec::new();
        
        for rel in tag_relations {
            if let Some(tag_id) = rel.get("tag_id").and_then(|v| v.as_str()) {
                // 获取tag详情
                if let Ok(mut tag_response) = self.db.query(&format!("SELECT * FROM {}", tag_id)).await {
                    if let Ok(tag_raw) = tag_response.take::<SurrealValue>(0) {
                        let tag_values = surreal_value_to_json_list(tag_raw)?;
                        if let Some(tag_value) = tag_values.first() {
                            tags.push(TagInfo {
                                id: tag_value.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                                name: tag_value.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                                slug: tag_value.get("slug").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                            });
                        }
                    }
                }
            }
        }
        
        Ok(ArticleListItem {
            id: article.id.clone(),
            title: article.title.clone(),
            subtitle: article.subtitle.clone(),
            slug: article.slug.clone(),
            excerpt: article.excerpt.clone(),
            cover_image_url: article.cover_image_url.clone(),
            author: author_info,
            publication: publication_info,
            status: article.status.clone(),
            is_paid_content: article.is_paid_content,
            is_featured: article.is_featured,
            reading_time: article.reading_time,
            view_count: article.view_count,
            clap_count: article.clap_count,
            comment_count: article.comment_count,
            tags,
            created_at: article.created_at,
            published_at: article.published_at,
        })
    }
}
