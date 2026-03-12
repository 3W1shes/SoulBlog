use serde::{Deserialize, Serialize};
use std::env;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    // Server configuration
    pub server_host: String,
    pub server_port: u16,
    pub environment: String,
    pub log_level: String,

    // Database configuration
    pub database_url: String,
    pub database_namespace: String,
    pub database_name: String,
    pub database_username: String,
    pub database_password: String,

    // Authentication configuration
    pub auth_service_url: String,
    pub auth_service_token: String,
    pub jwt_secret: String,
    pub jwt_expiry: String,
    pub jwt_refresh_expiry: String,

    // Redis configuration
    pub redis_url: Option<String>,
    pub cache_ttl: u64,

    // Storage configuration
    pub storage_type: String,
    pub s3_endpoint: Option<String>,
    pub s3_bucket: String,
    pub s3_region: String,
    pub s3_access_key: String,
    pub s3_secret_key: String,
    pub s3_use_path_style: bool,
    pub max_upload_size: u64,

    // Email configuration
    pub smtp_host: String,
    pub smtp_port: u16,
    pub smtp_username: String,
    pub smtp_password: String,
    pub smtp_from_name: String,
    pub smtp_from_email: String,

    // Frontend URLs
    pub frontend_url: String,
    pub password_reset_url: String,

    // Content settings
    pub max_article_length: usize,
    pub max_comment_length: usize,
    pub max_bio_length: usize,
    pub default_articles_per_page: usize,
    pub default_comments_per_page: usize,

    // Feature flags
    pub enable_registrations: bool,
    pub enable_comments: bool,
    pub enable_subscriptions: bool,
    pub enable_publications: bool,
    pub enable_email_notifications: bool,
    pub enable_recommendation_tasks: bool,

    // Rate limiting
    pub rate_limit_requests: u32,
    pub rate_limit_window: u64,

    // Search configuration
    pub search_min_length: usize,
    pub search_max_results: usize,

    // Image processing
    pub image_max_width: u32,
    pub image_max_height: u32,
    pub image_quality: u8,
    pub allowed_image_types: String,

    // Recommendation engine
    pub recommendation_batch_size: usize,
    pub recommendation_update_interval: u64,

    // CORS configuration
    pub cors_allowed_origins: String,

    // Monitoring
    pub metrics_enabled: bool,
    pub metrics_port: u16,

    // Stripe payment configuration
    pub stripe_secret_key: Option<String>,
    pub stripe_publishable_key: Option<String>,
    pub stripe_webhook_secret: Option<String>,

    // Domain configuration
    pub base_domain: Option<String>,
    pub ssl_provider_endpoint: Option<String>,
    pub ssl_provider_api_key: Option<String>,
    pub auto_provision_ssl: Option<bool>,
    pub ssl_webhook_url: Option<String>,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        Ok(Config {
            server_host: env::var("SERVER_HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
            server_port: env::var("SERVER_PORT")
                .unwrap_or_else(|_| "3000".to_string())
                .parse()?,
            environment: env::var("ENVIRONMENT").unwrap_or_else(|_| "development".to_string()),
            log_level: env::var("LOG_LEVEL").unwrap_or_else(|_| "info".to_string()),

            database_url: env::var("DATABASE_URL")
                .unwrap_or_else(|_| "http://localhost:8000".to_string()),
            database_namespace: env::var("DATABASE_NAMESPACE")
                .unwrap_or_else(|_| "rainbow".to_string()),
            database_name: env::var("DATABASE_NAME")
                .unwrap_or_else(|_| "blog".to_string()),
            database_username: env::var("DATABASE_USERNAME")
                .unwrap_or_else(|_| "root".to_string()),
            database_password: env::var("DATABASE_PASSWORD")
                .unwrap_or_else(|_| "root".to_string()),

            auth_service_url: env::var("AUTH_SERVICE_URL")
                .unwrap_or_else(|_| "http://localhost:8080".to_string()),
            auth_service_token: env::var("AUTH_SERVICE_TOKEN")
                .unwrap_or_else(|_| "default-token".to_string()),
            jwt_secret: env::var("JWT_SECRET")
                .expect("JWT_SECRET must be set"),
            jwt_expiry: env::var("JWT_EXPIRY")
                .unwrap_or_else(|_| "7d".to_string()),
            jwt_refresh_expiry: env::var("JWT_REFRESH_EXPIRY")
                .unwrap_or_else(|_| "30d".to_string()),

            redis_url: env::var("REDIS_URL").ok(),
            cache_ttl: env::var("CACHE_TTL")
                .unwrap_or_else(|_| "3600".to_string())
                .parse()?,

            storage_type: env::var("STORAGE_TYPE")
                .unwrap_or_else(|_| "s3".to_string()),
            s3_endpoint: env::var("S3_ENDPOINT").ok(),
            s3_bucket: env::var("S3_BUCKET")
                .unwrap_or_else(|_| "rainbow-blog".to_string()),
            s3_region: env::var("S3_REGION")
                .unwrap_or_else(|_| "us-east-1".to_string()),
            s3_access_key: env::var("S3_ACCESS_KEY")
                .unwrap_or_else(|_| "minioadmin".to_string()),
            s3_secret_key: env::var("S3_SECRET_KEY")
                .unwrap_or_else(|_| "minioadmin".to_string()),
            s3_use_path_style: env::var("S3_USE_PATH_STYLE")
                .unwrap_or_else(|_| "false".to_string())
                .parse()?,
            max_upload_size: env::var("MAX_UPLOAD_SIZE")
                .unwrap_or_else(|_| "52428800".to_string())
                .parse()?,

            smtp_host: env::var("SMTP_HOST")
                .unwrap_or_else(|_| "localhost".to_string()),
            smtp_port: env::var("SMTP_PORT")
                .unwrap_or_else(|_| "587".to_string())
                .parse()?,
            smtp_username: env::var("SMTP_USERNAME")
                .unwrap_or_default(),
            smtp_password: env::var("SMTP_PASSWORD")
                .unwrap_or_default(),
            smtp_from_name: env::var("SMTP_FROM_NAME")
                .unwrap_or_else(|_| "Rainbow Blog".to_string()),
            smtp_from_email: env::var("SMTP_FROM_EMAIL")
                .unwrap_or_else(|_| "noreply@rainbow-blog.com".to_string()),

            frontend_url: env::var("FRONTEND_URL")
                .unwrap_or_else(|_| "http://localhost:3001".to_string()),
            password_reset_url: env::var("PASSWORD_RESET_URL")
                .unwrap_or_else(|_| "http://localhost:3001/reset-password".to_string()),

            max_article_length: env::var("MAX_ARTICLE_LENGTH")
                .unwrap_or_else(|_| "50000".to_string())
                .parse()?,
            max_comment_length: env::var("MAX_COMMENT_LENGTH")
                .unwrap_or_else(|_| "5000".to_string())
                .parse()?,
            max_bio_length: env::var("MAX_BIO_LENGTH")
                .unwrap_or_else(|_| "160".to_string())
                .parse()?,
            default_articles_per_page: env::var("DEFAULT_ARTICLES_PER_PAGE")
                .unwrap_or_else(|_| "20".to_string())
                .parse()?,
            default_comments_per_page: env::var("DEFAULT_COMMENTS_PER_PAGE")
                .unwrap_or_else(|_| "50".to_string())
                .parse()?,

            enable_registrations: env::var("ENABLE_REGISTRATIONS")
                .unwrap_or_else(|_| "true".to_string())
                .parse()?,
            enable_comments: env::var("ENABLE_COMMENTS")
                .unwrap_or_else(|_| "true".to_string())
                .parse()?,
            enable_subscriptions: env::var("ENABLE_SUBSCRIPTIONS")
                .unwrap_or_else(|_| "false".to_string())
                .parse()?,
            enable_publications: env::var("ENABLE_PUBLICATIONS")
                .unwrap_or_else(|_| "true".to_string())
                .parse()?,
            enable_email_notifications: env::var("ENABLE_EMAIL_NOTIFICATIONS")
                .unwrap_or_else(|_| "true".to_string())
                .parse()?,
            enable_recommendation_tasks: env::var("ENABLE_RECOMMENDATION_TASKS")
                .unwrap_or_else(|_| "false".to_string())
                .parse()?,

            rate_limit_requests: env::var("RATE_LIMIT_REQUESTS")
                .unwrap_or_else(|_| "100".to_string())
                .parse()?,
            rate_limit_window: env::var("RATE_LIMIT_WINDOW")
                .unwrap_or_else(|_| "60".to_string())
                .parse()?,

            search_min_length: env::var("SEARCH_MIN_LENGTH")
                .unwrap_or_else(|_| "2".to_string())
                .parse()?,
            search_max_results: env::var("SEARCH_MAX_RESULTS")
                .unwrap_or_else(|_| "100".to_string())
                .parse()?,

            image_max_width: env::var("IMAGE_MAX_WIDTH")
                .unwrap_or_else(|_| "2000".to_string())
                .parse()?,
            image_max_height: env::var("IMAGE_MAX_HEIGHT")
                .unwrap_or_else(|_| "2000".to_string())
                .parse()?,
            image_quality: env::var("IMAGE_QUALITY")
                .unwrap_or_else(|_| "85".to_string())
                .parse()?,
            allowed_image_types: env::var("ALLOWED_IMAGE_TYPES")
                .unwrap_or_else(|_| "image/jpeg,image/png,image/gif,image/webp".to_string()),

            recommendation_batch_size: env::var("RECOMMENDATION_BATCH_SIZE")
                .unwrap_or_else(|_| "10".to_string())
                .parse()?,
            recommendation_update_interval: env::var("RECOMMENDATION_UPDATE_INTERVAL")
                .unwrap_or_else(|_| "3600".to_string())
                .parse()?,

            cors_allowed_origins: env::var("CORS_ALLOWED_ORIGINS")
                .unwrap_or_else(|_| "http://localhost:3001".to_string()),

            metrics_enabled: env::var("METRICS_ENABLED")
                .unwrap_or_else(|_| "false".to_string())
                .parse()?,
            metrics_port: env::var("METRICS_PORT")
                .unwrap_or_else(|_| "9090".to_string())
                .parse()?,

            stripe_secret_key: env::var("STRIPE_SECRET_KEY").ok(),
            stripe_publishable_key: env::var("STRIPE_PUBLISHABLE_KEY").ok(),
            stripe_webhook_secret: env::var("STRIPE_WEBHOOK_SECRET").ok(),

            base_domain: env::var("BASE_DOMAIN").ok(),
            ssl_provider_endpoint: env::var("SSL_PROVIDER_ENDPOINT").ok(),
            ssl_provider_api_key: env::var("SSL_PROVIDER_API_KEY").ok(),
            auto_provision_ssl: env::var("AUTO_PROVISION_SSL")
                .ok()
                .and_then(|s| s.parse().ok()),
            ssl_webhook_url: env::var("SSL_WEBHOOK_URL").ok(),
        })
    }

    pub fn is_production(&self) -> bool {
        self.environment == "production"
    }

    pub fn is_development(&self) -> bool {
        self.environment == "development"
    }
}
