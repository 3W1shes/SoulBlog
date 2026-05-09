use crate::error::{AppError, Result};
use crate::services::database::Database;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use tracing::{info, warn};

/// 站点全局配置（单行表 site_config:main）
/// Single 模式：installed/owner 是核心；mode 锁为 "single"
/// Platform 模式：作为平台元信息使用，mode = "platform"
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SiteConfig {
    #[serde(default)]
    pub installed: bool,
    #[serde(default)]
    pub mode: String,
    #[serde(default)]
    pub site_name: String,
    #[serde(default)]
    pub site_description: Option<String>,
    #[serde(default)]
    pub site_logo: Option<String>,
    #[serde(default)]
    pub site_favicon: Option<String>,
    #[serde(default = "default_locale")]
    pub locale: String,
    #[serde(default)]
    pub theme_color: Option<String>,
    #[serde(default)]
    pub owner_user_id: Option<String>,
    #[serde(default)]
    pub allow_register: bool,
    #[serde(default = "default_true")]
    pub allow_comments: bool,
    #[serde(default = "default_true")]
    pub seo_robots: bool,
    #[serde(default)]
    pub footer_text: Option<String>,
    #[serde(default)]
    pub icp_text: Option<String>,
    #[serde(default)]
    pub created_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub updated_at: Option<DateTime<Utc>>,
}

fn default_locale() -> String {
    "zh-CN".to_string()
}

fn default_true() -> bool {
    true
}

impl Default for SiteConfig {
    fn default() -> Self {
        Self {
            installed: false,
            mode: "single".to_string(),
            site_name: "My Blog".to_string(),
            site_description: None,
            site_logo: None,
            site_favicon: None,
            locale: "zh-CN".to_string(),
            theme_color: Some("#111827".to_string()),
            owner_user_id: None,
            allow_register: false,
            allow_comments: true,
            seo_robots: true,
            footer_text: None,
            icp_text: None,
            created_at: None,
            updated_at: None,
        }
    }
}

#[derive(Clone)]
pub struct SiteConfigService {
    db: Arc<Database>,
}

impl SiteConfigService {
    pub async fn new(db: Arc<Database>) -> Result<Self> {
        Ok(Self { db })
    }

    pub async fn get(&self) -> Result<Option<SiteConfig>> {
        let mut resp = self.db.query("SELECT * FROM site_config:main").await?;
        let v: Vec<SiteConfig> = resp.take(0).unwrap_or_default();
        Ok(v.into_iter().next())
    }

    pub async fn is_installed(&self) -> Result<bool> {
        Ok(self.get().await?.map(|c| c.installed).unwrap_or(false))
    }

    pub async fn install(
        &self,
        mode: &str,
        site_name: &str,
        site_description: Option<String>,
        locale: &str,
        owner_user_id: &str,
    ) -> Result<SiteConfig> {
        if self.is_installed().await? {
            return Err(AppError::BadRequest("Site already installed".to_string()));
        }

        let now = Utc::now();
        let cfg = SiteConfig {
            installed: true,
            mode: mode.to_string(),
            site_name: site_name.to_string(),
            site_description,
            site_logo: None,
            site_favicon: None,
            locale: locale.to_string(),
            theme_color: Some("#111827".to_string()),
            owner_user_id: Some(owner_user_id.to_string()),
            allow_register: mode == "platform",
            allow_comments: true,
            seo_robots: true,
            footer_text: None,
            icp_text: None,
            created_at: Some(now),
            updated_at: Some(now),
        };

        let create_sql = "CREATE site_config:main CONTENT $data";
        if let Err(e) = self
            .db
            .query_with_params(create_sql, json!({ "data": cfg.clone() }))
            .await
        {
            warn!("CREATE site_config failed (will try UPSERT): {}", e);
            let upsert_sql = "UPSERT site_config:main CONTENT $data";
            self.db
                .query_with_params(upsert_sql, json!({ "data": cfg.clone() }))
                .await?;
        }

        info!("Site installed in {} mode, owner={}", mode, owner_user_id);
        Ok(cfg)
    }

    pub async fn update(&self, updates: Value) -> Result<SiteConfig> {
        let mut updates = updates;
        if let Some(obj) = updates.as_object_mut() {
            obj.insert("updated_at".to_string(), json!(Utc::now()));
        }
        let mut resp = self
            .db
            .query_with_params(
                "UPDATE site_config:main MERGE $updates RETURN AFTER",
                json!({ "updates": updates }),
            )
            .await?;
        let v: Vec<SiteConfig> = resp.take(0).unwrap_or_default();
        v.into_iter()
            .next()
            .ok_or_else(|| AppError::NotFound("site_config not found".to_string()))
    }

    /// 检查 user 是否为站点 owner（admin）
    pub async fn is_admin(&self, user_id: &str) -> Result<bool> {
        let cfg = self.get().await?;
        Ok(cfg
            .and_then(|c| c.owner_user_id)
            .map(|owner| owner == user_id)
            .unwrap_or(false))
    }
}
