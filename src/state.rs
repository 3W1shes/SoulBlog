use crate::{
    config::Config,
    services::{
        database::Database,
        auth::AuthService,
        article::ArticleService,
        user::UserService,
        comment::CommentService,
        notification::NotificationService,
        search::SearchService,
        media::MediaService,
        recommendation::RecommendationService,
        publication::PublicationService,
        bookmark::BookmarkService,
        follow::FollowService,
        tag::TagService,
        series::SeriesService,
        analytics::AnalyticsService,
        subscription::SubscriptionService,
        payment::PaymentService,
        revenue::RevenueService,
        stripe::StripeService,
        websocket::WebSocketService,
        realtime::RealtimeService,
        domain::{DomainService, DomainConfig},
        site_config::SiteConfigService,
    },
};

/// 应用程序的共享状态
/// 包含所有服务和配置的引用
#[derive(Clone)]
pub struct AppState {
    /// 应用配置
    pub config: Config,
    
    /// 数据库连接
    pub db: Database,
    
    /// 认证服务
    pub auth_service: AuthService,
    
    /// 文章服务
    pub article_service: ArticleService,
    
    /// 用户服务
    pub user_service: UserService,
    
    /// 评论服务
    pub comment_service: CommentService,
    
    /// 通知服务
    pub notification_service: NotificationService,
    
    /// 搜索服务
    pub search_service: SearchService,
    
    /// 媒体服务
    pub media_service: MediaService,
    
    /// 推荐服务
    pub recommendation_service: RecommendationService,
    
    /// 出版物服务
    pub publication_service: PublicationService,
    
    /// 书签服务
    pub bookmark_service: BookmarkService,
    
    /// 关注服务
    pub follow_service: FollowService,
    
    /// 标签服务
    pub tag_service: TagService,
    
    /// 系列服务
    pub series_service: SeriesService,
    
    /// 统计分析服务
    pub analytics_service: AnalyticsService,
    
    /// 订阅服务
    pub subscription_service: SubscriptionService,
    
    /// 付费内容服务
    pub payment_service: PaymentService,
    
    /// 收益管理服务
    pub revenue_service: RevenueService,
    
    /// Stripe支付服务
    pub stripe_service: StripeService,
    
    /// WebSocket实时通知服务
    pub websocket_service: WebSocketService,
    
    /// 实时通知集成服务
    pub realtime_service: RealtimeService,
    
    /// 域名管理服务
    pub domain_service: DomainService,

    /// 站点配置服务（单/平台模式共享，存站点信息+installed标记）
    pub site_config_service: SiteConfigService,
}

impl Default for AppState {
    fn default() -> Self {
        panic!("AppState should not be constructed with default values - use proper initialization");
    }
}

impl AppState {
    /// 检查功能是否启用
    pub fn is_feature_enabled(&self, feature: &str) -> bool {
        match feature {
            "registrations" => self.config.enable_registrations,
            "comments" => self.config.enable_comments,
            "subscriptions" => self.config.enable_subscriptions,
            "publications" => self.config.enable_publications,
            "email_notifications" => self.config.enable_email_notifications,
            _ => false,
        }
    }
    
    /// 获取分页配置
    pub fn get_page_size(&self, resource_type: &str) -> usize {
        match resource_type {
            "articles" => self.config.default_articles_per_page,
            "comments" => self.config.default_comments_per_page,
            _ => 20, // 默认每页20条
        }
    }
    
    /// 检查是否为生产环境
    pub fn is_production(&self) -> bool {
        self.config.is_production()
    }
    
    /// 检查是否为开发环境
    pub fn is_development(&self) -> bool {
        self.config.is_development()
    }
}