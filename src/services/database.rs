use crate::config::Config;
use crate::error::{AppError, Result};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use std::fmt::Debug;
use soulcore::prelude::*;
use soulcore::engines::storage::StorageEngine;
use surrealdb::Response;
use surrealdb::sql::Thing;
use tracing::{info, error, debug};

/// 数据库服务
#[derive(Clone)]
pub struct Database {
    pub storage: Arc<StorageEngine>,
    pub config: Config,
}

impl Database {
    /// 创建新的数据库实例
    pub async fn new(config: &Config) -> Result<Self> {
        info!("Initializing database connection to {}", config.database_url);
        
        // 创建存储配置
        let storage_config = StorageConfig {
            connection_mode: ConnectionMode::Http,
            url: config.database_url.clone(),
            username: config.database_username.clone(),
            password: config.database_password.clone(),
            namespace: config.database_namespace.clone(),
            database: config.database_name.clone(),
            pool_size: 10,
            ..Default::default()
        };

        // 使用SoulCoreBuilder创建storage engine
        let soulcore = SoulCoreBuilder::new()
            .with_storage_config(storage_config)
            .build()
            .await
            .map_err(|e| AppError::from(e))?;

        let storage = soulcore.storage().clone();

        Ok(Self {
            storage,
            config: config.clone(),
        })
    }

    /// 验证数据库连接
    pub async fn verify_connection(&self) -> Result<()> {
        // 尝试执行一个简单的查询来验证连接
        match self.storage.query("INFO FOR DB").await {
            Ok(_) => {
                info!("Database connection verified successfully");
                Ok(())
            }
            Err(e) => {
                error!("Failed to verify database connection: {}", e);
                Err(AppError::from(e))
            }
        }
    }

    /// 使用查询构建器创建查询
    pub fn query_builder(&self) -> QueryBuilder {
        self.storage.query_builder()
    }
    
    /// 执行原始SQL查询
    pub async fn query(&self, sql: &str) -> Result<Response> {
        self.storage.query(sql)
            .await
            .map_err(|e| AppError::from(e))
    }

    /// 执行带参数的查询
    pub async fn query_with_params<P>(&self, sql: &str, params: P) -> Result<Response>
    where
        P: Serialize,
    {
        let params = serde_json::to_value(params)
            .map_err(|e| AppError::Internal(e.to_string()))?;
        self.storage.query_with_params(sql, params)
            .await
            .map_err(|e| AppError::from(e))
    }

    /// 创建记录
    pub async fn create<T>(&self, table: &str, data: T) -> Result<T>
    where
        T: Serialize + for<'de> Deserialize<'de> + Send + Sync + Clone + Debug + 'static,
    {
        // 使用 storage 的原生 create 方法
        let results = self.storage.create(table, data)
            .await
            .map_err(|e| AppError::from(e))?;
        
        results.into_iter().next()
            .ok_or_else(|| AppError::Internal("Failed to create record".to_string()))
    }

    /// 选择记录
    pub async fn select<T>(&self, resource: &str) -> Result<Vec<T>>
    where
        T: for<'de> Deserialize<'de> + Send + Sync + Debug,
    {
        self.storage.select(resource)
            .await
            .map_err(|e| AppError::from(e))
    }

    /// 更新记录
    pub async fn update<T>(&self, thing: Thing, data: T) -> Result<Option<T>>
    where
        T: Serialize + for<'de> Deserialize<'de> + Send + Sync + Debug + 'static,
    {
        self.storage.update(thing, data)
            .await
            .map_err(|e| AppError::from(e))
    }

    /// 删除记录
    pub async fn delete(&self, thing: Thing) -> Result<()> {
        let _: Option<serde_json::Value> = self.storage.delete(thing)
            .await
            .map_err(|e| AppError::from(e))?;
        Ok(())
    }

    /// 通过ID删除记录
    pub async fn delete_by_id(&self, table: &str, id: &str) -> Result<()> {
        let prefix = format!("{}:", table);
        let pure_id = if id.starts_with(&prefix) { &id[prefix.len()..] } else { id };
        let thing = Thing::from((table, pure_id));
        self.delete(thing).await
    }

    /// 通过ID获取单个记录
    pub async fn get_by_id<T>(&self, table: &str, id: &str) -> Result<Option<T>>
    where
        T: for<'de> Deserialize<'de> + Send + Sync + Debug,
    {
        // 获取纯 ID（不带 table 前缀）
        let prefix = format!("{}:", table);
        let pure_id = if id.starts_with(&prefix) {
            &id[prefix.len()..]
        } else {
            id
        };
        
        // 使用反引号包裹 ID 以避免解析问题（与 article.rs 保持一致）
        let query = format!("SELECT * FROM {}:`{}`", table, pure_id);
        debug!("Executing query: {}", query);
        
        let mut response = self.storage.query(&query).await?;
        let results: Vec<T> = response.take(0)?;
        Ok(results.into_iter().next())
    }

    /// 通过ID更新记录
    pub async fn update_by_id<T>(&self, table: &str, id: &str, data: T) -> Result<Option<T>>
    where
        T: Serialize + for<'de> Deserialize<'de> + Send + Sync + Debug + 'static,
    {
        let prefix = format!("{}:", table);
        let pure_id = if id.starts_with(&prefix) { &id[prefix.len()..] } else { id };
        let thing = Thing::from((table, pure_id));
        self.update(thing, data).await
    }

    /// 通过ID使用JSON数据更新记录并返回指定类型
    pub async fn update_by_id_with_json<T>(&self, table: &str, id: &str, updates: serde_json::Value) -> Result<Option<T>>
    where
        T: for<'de> Deserialize<'de> + Send + Sync + Debug,
    {
        // 获取纯 ID（不带 table 前缀），并用反引号包裹，兼容包含连字符的 ID
        let prefix = format!("{}:", table);
        let pure_id = if id.starts_with(&prefix) { &id[prefix.len()..] } else { id };
        let query = format!("UPDATE {}:`{}` MERGE $updates RETURN *", table, pure_id);
        let mut response = self.query_with_params(&query, json!({"updates": updates})).await?;
        let results: Vec<T> = response.take(0)?;
        Ok(results.into_iter().next())
    }

    /// 查找单个记录
    pub async fn find_one<T>(&self, table: &str, field: &str, value: &str) -> Result<Option<T>>
    where
        T: for<'de> Deserialize<'de> + Send + Sync + Clone + Debug,
    {
        self.storage.find_one(table, field, value)
            .await
            .map_err(|e| AppError::from(e))
    }

    /// 开始事务
    pub async fn begin_transaction(&self) -> Result<Transaction> {
        self.storage.begin_transaction()
            .await
            .map_err(|e| AppError::from(e))
    }
}

/// 分页结果结构
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PaginatedResult<T> {
    pub data: Vec<T>,
    pub total: usize,
    pub page: usize,
    pub per_page: usize,
    pub total_pages: usize,
}

// 为了向后兼容，提供ClientWrapper别名
pub type ClientWrapper = Database;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_database_connection() {
        let config = Config::default();
        let db = Database::new(&config).await;
        assert!(db.is_ok());
    }
}
