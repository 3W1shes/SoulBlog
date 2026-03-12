-- Rainbow-Blog Database Schema
-- 基于 Medium 风格的博客系统数据库架构
-- 使用 SurrealDB 语法

-- =====================================
-- 用户扩展表（基于 Rainbow-Auth）
-- =====================================

-- 用户资料扩展表
DEFINE TABLE user_profile SCHEMAFULL;
DEFINE FIELD user_id ON user_profile TYPE string ASSERT $value != NONE; -- Rainbow-Auth 用户ID
DEFINE FIELD username ON user_profile TYPE string ASSERT $value != NONE AND string::len($value) >= 3 AND string::len($value) <= 30;
DEFINE FIELD display_name ON user_profile TYPE string ASSERT $value != NONE AND string::len($value) <= 50;
DEFINE FIELD bio ON user_profile TYPE option<string> ASSERT $value = NONE OR string::len($value) <= 160;
DEFINE FIELD avatar_url ON user_profile TYPE option<string>;
DEFINE FIELD cover_image_url ON user_profile TYPE option<string>;
DEFINE FIELD website ON user_profile TYPE option<string>;
DEFINE FIELD location ON user_profile TYPE option<string>;
DEFINE FIELD twitter_username ON user_profile TYPE option<string>;
DEFINE FIELD github_username ON user_profile TYPE option<string>;
DEFINE FIELD linkedin_url ON user_profile TYPE option<string>;
DEFINE FIELD facebook_url ON user_profile TYPE option<string>;
DEFINE FIELD stripe_customer_id ON user_profile TYPE option<string>;
DEFINE FIELD stripe_account_id ON user_profile TYPE option<string>;
DEFINE FIELD follower_count ON user_profile TYPE number DEFAULT 0;
DEFINE FIELD following_count ON user_profile TYPE number DEFAULT 0;
DEFINE FIELD article_count ON user_profile TYPE number DEFAULT 0;
DEFINE FIELD total_claps_received ON user_profile TYPE number DEFAULT 0;
DEFINE FIELD is_verified ON user_profile TYPE bool DEFAULT false;
DEFINE FIELD is_suspended ON user_profile TYPE bool DEFAULT false;
DEFINE FIELD created_at ON user_profile TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at ON user_profile TYPE datetime DEFAULT time::now();

-- 用户资料索引
DEFINE INDEX user_profile_user_id_idx ON user_profile COLUMNS user_id UNIQUE;
DEFINE INDEX user_profile_username_idx ON user_profile COLUMNS username UNIQUE;
DEFINE INDEX user_profile_verified_idx ON user_profile COLUMNS is_verified;
DEFINE INDEX user_profile_stripe_customer_idx ON user_profile COLUMNS stripe_customer_id UNIQUE;
DEFINE INDEX user_profile_stripe_account_idx ON user_profile COLUMNS stripe_account_id UNIQUE;

-- =====================================
-- 核心内容表
-- =====================================

-- 文章表
DEFINE TABLE article SCHEMAFULL;
DEFINE FIELD title ON article TYPE string ASSERT $value != NONE AND string::len($value) > 0 AND string::len($value) <= 150;
DEFINE FIELD subtitle ON article TYPE option<string> ASSERT $value = NONE OR string::len($value) <= 200;
DEFINE FIELD slug ON article TYPE string ASSERT $value != NONE AND string::len($value) > 0 AND string::len($value) <= 200;
DEFINE FIELD content ON article TYPE string DEFAULT "";
DEFINE FIELD content_html ON article TYPE string DEFAULT ""; -- 渲染后的HTML
DEFINE FIELD excerpt ON article TYPE option<string> ASSERT $value = NONE OR string::len($value) <= 300;
DEFINE FIELD cover_image_url ON article TYPE option<string>;
DEFINE FIELD author_id ON article TYPE string ASSERT $value != NONE;
DEFINE FIELD publication_id ON article TYPE option<record<publication>>;
DEFINE FIELD series_id ON article TYPE option<record<series>>;
DEFINE FIELD series_order ON article TYPE option<number>;
DEFINE FIELD status ON article TYPE string DEFAULT "draft" ASSERT $value INSIDE ["draft", "published", "unlisted", "archived"];
DEFINE FIELD is_paid_content ON article TYPE bool DEFAULT false;
DEFINE FIELD is_featured ON article TYPE bool DEFAULT false;
DEFINE FIELD reading_time ON article TYPE number DEFAULT 0; -- 预计阅读时间（分钟）
DEFINE FIELD word_count ON article TYPE number DEFAULT 0;
DEFINE FIELD view_count ON article TYPE number DEFAULT 0;
DEFINE FIELD clap_count ON article TYPE number DEFAULT 0;
DEFINE FIELD comment_count ON article TYPE number DEFAULT 0;
DEFINE FIELD bookmark_count ON article TYPE number DEFAULT 0;
DEFINE FIELD share_count ON article TYPE number DEFAULT 0;
DEFINE FIELD seo_title ON article TYPE option<string>;
DEFINE FIELD seo_description ON article TYPE option<string>;
DEFINE FIELD seo_keywords ON article TYPE array<string> DEFAULT [];
DEFINE FIELD metadata ON article TYPE object DEFAULT {};
DEFINE FIELD created_at ON article TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at ON article TYPE datetime DEFAULT time::now();
DEFINE FIELD published_at ON article TYPE option<datetime>;
DEFINE FIELD last_edited_at ON article TYPE option<datetime>;
DEFINE FIELD is_deleted ON article TYPE bool DEFAULT false;
DEFINE FIELD deleted_at ON article TYPE option<datetime>;

-- 文章索引
DEFINE INDEX article_slug_idx ON article COLUMNS slug UNIQUE;
DEFINE INDEX article_author_idx ON article COLUMNS author_id;
DEFINE INDEX article_publication_idx ON article COLUMNS publication_id;
DEFINE INDEX article_series_idx ON article COLUMNS series_id;
DEFINE INDEX article_status_idx ON article COLUMNS status;
DEFINE INDEX article_published_idx ON article COLUMNS published_at;
DEFINE INDEX article_featured_idx ON article COLUMNS is_featured;
DEFINE INDEX article_deleted_idx ON article COLUMNS is_deleted;

-- 文章版本历史表
DEFINE TABLE article_version SCHEMAFULL;
DEFINE FIELD article_id ON article_version TYPE record<article> ASSERT $value != NONE;
DEFINE FIELD version_number ON article_version TYPE number ASSERT $value != NONE AND $value > 0;
DEFINE FIELD title ON article_version TYPE string ASSERT $value != NONE;
DEFINE FIELD subtitle ON article_version TYPE option<string>;
DEFINE FIELD content ON article_version TYPE string DEFAULT "";
DEFINE FIELD content_html ON article_version TYPE string DEFAULT "";
DEFINE FIELD change_summary ON article_version TYPE option<string>;
DEFINE FIELD author_id ON article_version TYPE string ASSERT $value != NONE;
DEFINE FIELD created_at ON article_version TYPE datetime DEFAULT time::now();

-- 版本索引
DEFINE INDEX article_version_article_idx ON article_version COLUMNS article_id;
DEFINE INDEX article_version_number_idx ON article_version COLUMNS article_id, version_number UNIQUE;

-- 文章系列表
DEFINE TABLE series SCHEMAFULL;
DEFINE FIELD title ON series TYPE string ASSERT $value != NONE AND string::len($value) <= 100;
DEFINE FIELD description ON series TYPE option<string>;
DEFINE FIELD slug ON series TYPE string ASSERT $value != NONE;
DEFINE FIELD author_id ON series TYPE string ASSERT $value != NONE;
DEFINE FIELD cover_image_url ON series TYPE option<string>;
DEFINE FIELD article_count ON series TYPE number DEFAULT 0;
DEFINE FIELD is_completed ON series TYPE bool DEFAULT false;
DEFINE FIELD created_at ON series TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at ON series TYPE datetime DEFAULT time::now();

-- 系列索引
DEFINE INDEX series_slug_idx ON series COLUMNS author_id, slug UNIQUE;
DEFINE INDEX series_author_idx ON series COLUMNS author_id;

-- =====================================
-- 标签系统
-- =====================================

-- 标签表
DEFINE TABLE tag SCHEMAFULL;
DEFINE FIELD name ON tag TYPE string ASSERT $value != NONE AND string::len($value) > 0 AND string::len($value) <= 50;
DEFINE FIELD slug ON tag TYPE string ASSERT $value != NONE;
DEFINE FIELD description ON tag TYPE option<string>;
DEFINE FIELD follower_count ON tag TYPE number DEFAULT 0;
DEFINE FIELD article_count ON tag TYPE number DEFAULT 0;
DEFINE FIELD is_featured ON tag TYPE bool DEFAULT false;
DEFINE FIELD created_at ON tag TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at ON tag TYPE datetime DEFAULT time::now();

-- 标签索引
DEFINE INDEX tag_slug_idx ON tag COLUMNS slug UNIQUE;
DEFINE INDEX tag_name_idx ON tag COLUMNS name UNIQUE;
DEFINE INDEX tag_featured_idx ON tag COLUMNS is_featured;

-- 文章标签关联表
DEFINE TABLE article_tag SCHEMAFULL;
DEFINE FIELD article_id ON article_tag TYPE record<article> ASSERT $value != NONE;
DEFINE FIELD tag_id ON article_tag TYPE record<tag> ASSERT $value != NONE;
DEFINE FIELD created_at ON article_tag TYPE datetime DEFAULT time::now();

-- 文章标签索引
DEFINE INDEX article_tag_unique_idx ON article_tag COLUMNS article_id, tag_id UNIQUE;
DEFINE INDEX article_tag_article_idx ON article_tag COLUMNS article_id;
DEFINE INDEX article_tag_tag_idx ON article_tag COLUMNS tag_id;

-- 用户关注标签表
DEFINE TABLE user_tag_follow SCHEMAFULL;
DEFINE FIELD user_id ON user_tag_follow TYPE string ASSERT $value != NONE;
DEFINE FIELD tag_id ON user_tag_follow TYPE record<tag> ASSERT $value != NONE;
DEFINE FIELD created_at ON user_tag_follow TYPE datetime DEFAULT time::now();

-- 用户标签关注索引
DEFINE INDEX user_tag_follow_unique_idx ON user_tag_follow COLUMNS user_id, tag_id UNIQUE;
DEFINE INDEX user_tag_follow_user_idx ON user_tag_follow COLUMNS user_id;
DEFINE INDEX user_tag_follow_tag_idx ON user_tag_follow COLUMNS tag_id;

-- =====================================
-- 互动系统
-- =====================================

-- 点赞表（Claps）
DEFINE TABLE clap SCHEMAFULL;
DEFINE FIELD user_id ON clap TYPE string ASSERT $value != NONE;
DEFINE FIELD article_id ON clap TYPE record<article> ASSERT $value != NONE;
DEFINE FIELD count ON clap TYPE number DEFAULT 1 ASSERT $value >= 1 AND $value <= 50; -- Medium限制最多50次
DEFINE FIELD created_at ON clap TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at ON clap TYPE datetime DEFAULT time::now();

-- 点赞索引
DEFINE INDEX clap_unique_idx ON clap COLUMNS user_id, article_id UNIQUE;
DEFINE INDEX clap_article_idx ON clap COLUMNS article_id;
DEFINE INDEX clap_user_idx ON clap COLUMNS user_id;

-- 评论表
DEFINE TABLE comment SCHEMAFULL;
DEFINE FIELD article_id ON comment TYPE string ASSERT $value != NONE;
DEFINE FIELD parent_id ON comment TYPE option<string>;
DEFINE FIELD author_id ON comment TYPE string ASSERT $value != NONE;
DEFINE FIELD content ON comment TYPE string ASSERT $value != NONE AND string::len($value) > 0;
DEFINE FIELD is_author_response ON comment TYPE bool DEFAULT false; -- 作者回复标记
DEFINE FIELD clap_count ON comment TYPE number DEFAULT 0;
DEFINE FIELD is_edited ON comment TYPE bool DEFAULT false;
DEFINE FIELD is_deleted ON comment TYPE bool DEFAULT false;
DEFINE FIELD created_at ON comment TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at ON comment TYPE datetime DEFAULT time::now();
DEFINE FIELD deleted_at ON comment TYPE option<datetime>;

-- 评论索引
DEFINE INDEX comment_article_idx ON comment COLUMNS article_id;
DEFINE INDEX comment_parent_idx ON comment COLUMNS parent_id;
DEFINE INDEX comment_author_idx ON comment COLUMNS author_id;
DEFINE INDEX comment_deleted_idx ON comment COLUMNS is_deleted;

-- 评论点赞表
DEFINE TABLE comment_clap SCHEMAFULL;
DEFINE FIELD user_id ON comment_clap TYPE string ASSERT $value != NONE;
DEFINE FIELD comment_id ON comment_clap TYPE record<comment> ASSERT $value != NONE;
DEFINE FIELD created_at ON comment_clap TYPE datetime DEFAULT time::now();

-- 评论点赞索引
DEFINE INDEX comment_clap_unique_idx ON comment_clap COLUMNS user_id, comment_id UNIQUE;
DEFINE INDEX comment_clap_comment_idx ON comment_clap COLUMNS comment_id;

-- 书签表
DEFINE TABLE bookmark SCHEMAFULL;
DEFINE FIELD user_id ON bookmark TYPE string ASSERT $value != NONE;
DEFINE FIELD article_id ON bookmark TYPE record<article> ASSERT $value != NONE;
DEFINE FIELD note ON bookmark TYPE option<string>; -- 私人笔记
DEFINE FIELD created_at ON bookmark TYPE datetime DEFAULT time::now();

-- 书签索引
DEFINE INDEX bookmark_unique_idx ON bookmark COLUMNS user_id, article_id UNIQUE;
DEFINE INDEX bookmark_user_idx ON bookmark COLUMNS user_id;
DEFINE INDEX bookmark_article_idx ON bookmark COLUMNS article_id;

-- 高亮表
DEFINE TABLE highlight SCHEMAFULL;
DEFINE FIELD user_id ON highlight TYPE string ASSERT $value != NONE;
DEFINE FIELD article_id ON highlight TYPE record<article> ASSERT $value != NONE;
DEFINE FIELD text ON highlight TYPE string ASSERT $value != NONE;
DEFINE FIELD start_offset ON highlight TYPE number ASSERT $value >= 0;
DEFINE FIELD end_offset ON highlight TYPE number ASSERT $value > 0;
DEFINE FIELD note ON highlight TYPE option<string>;
DEFINE FIELD is_private ON highlight TYPE bool DEFAULT true;
DEFINE FIELD created_at ON highlight TYPE datetime DEFAULT time::now();

-- 高亮索引
DEFINE INDEX highlight_user_article_idx ON highlight COLUMNS user_id, article_id;
DEFINE INDEX highlight_article_idx ON highlight COLUMNS article_id;
DEFINE INDEX highlight_private_idx ON highlight COLUMNS is_private;

-- =====================================
-- 社交系统
-- =====================================

-- 关注表
DEFINE TABLE follow SCHEMAFULL;
DEFINE FIELD follower_id ON follow TYPE string ASSERT $value != NONE;
DEFINE FIELD following_id ON follow TYPE string ASSERT $value != NONE;
DEFINE FIELD created_at ON follow TYPE datetime DEFAULT time::now();

-- 关注索引
DEFINE INDEX follow_unique_idx ON follow COLUMNS follower_id, following_id UNIQUE;
DEFINE INDEX follow_follower_idx ON follow COLUMNS follower_id;
DEFINE INDEX follow_following_idx ON follow COLUMNS following_id;

-- =====================================
-- 出版物系统
-- =====================================

-- 出版物表
DEFINE TABLE publication SCHEMAFULL;
DEFINE FIELD name ON publication TYPE string ASSERT $value != NONE AND string::len($value) <= 100;
DEFINE FIELD slug ON publication TYPE string ASSERT $value != NONE;
DEFINE FIELD description ON publication TYPE option<string>;
DEFINE FIELD tagline ON publication TYPE option<string> ASSERT $value = NONE OR string::len($value) <= 100;
DEFINE FIELD logo_url ON publication TYPE option<string>;
DEFINE FIELD cover_image_url ON publication TYPE option<string>;
DEFINE FIELD owner_id ON publication TYPE string ASSERT $value != NONE;
DEFINE FIELD homepage_layout ON publication TYPE string DEFAULT "grid" ASSERT $value INSIDE ["grid", "list", "magazine"];
DEFINE FIELD theme_color ON publication TYPE string DEFAULT "#000000";
DEFINE FIELD custom_domain ON publication TYPE option<string>;
DEFINE FIELD google_analytics_id ON publication TYPE option<string>;
DEFINE FIELD twitter_username ON publication TYPE option<string>;
DEFINE FIELD facebook_page_url ON publication TYPE option<string>;
DEFINE FIELD instagram_username ON publication TYPE option<string>;
DEFINE FIELD member_count ON publication TYPE number DEFAULT 0;
DEFINE FIELD article_count ON publication TYPE number DEFAULT 0;
DEFINE FIELD follower_count ON publication TYPE number DEFAULT 0;
DEFINE FIELD is_verified ON publication TYPE bool DEFAULT false;
DEFINE FIELD is_suspended ON publication TYPE bool DEFAULT false;
DEFINE FIELD created_at ON publication TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at ON publication TYPE datetime DEFAULT time::now();

-- 出版物索引
DEFINE INDEX publication_slug_idx ON publication COLUMNS slug UNIQUE;
DEFINE INDEX publication_owner_idx ON publication COLUMNS owner_id;
DEFINE INDEX publication_domain_idx ON publication COLUMNS custom_domain;
DEFINE INDEX publication_verified_idx ON publication COLUMNS is_verified;

-- 出版物成员表
DEFINE TABLE publication_member SCHEMAFULL;
DEFINE FIELD publication_id ON publication_member TYPE record<publication> ASSERT $value != NONE;
DEFINE FIELD user_id ON publication_member TYPE string ASSERT $value != NONE;
DEFINE FIELD role ON publication_member TYPE string DEFAULT "writer" ASSERT $value INSIDE ["owner", "editor", "writer"];
DEFINE FIELD permissions ON publication_member TYPE array<string> DEFAULT ["article.write"];
DEFINE FIELD invited_by ON publication_member TYPE string ASSERT $value != NONE;
-- 兼容后端模型，增加 joined_at 与 is_active 字段
DEFINE FIELD joined_at ON publication_member TYPE datetime DEFAULT time::now();
DEFINE FIELD is_active ON publication_member TYPE bool DEFAULT true;
DEFINE FIELD accepted_at ON publication_member TYPE option<datetime>;
DEFINE FIELD created_at ON publication_member TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at ON publication_member TYPE datetime DEFAULT time::now();

-- 出版物成员索引
DEFINE INDEX publication_member_unique_idx ON publication_member COLUMNS publication_id, user_id UNIQUE;
DEFINE INDEX publication_member_publication_idx ON publication_member COLUMNS publication_id;
DEFINE INDEX publication_member_user_idx ON publication_member COLUMNS user_id;

-- 出版物关注表
DEFINE TABLE publication_follow SCHEMAFULL;
DEFINE FIELD user_id ON publication_follow TYPE string ASSERT $value != NONE;
DEFINE FIELD publication_id ON publication_follow TYPE record<publication> ASSERT $value != NONE;
DEFINE FIELD created_at ON publication_follow TYPE datetime DEFAULT time::now();

-- 出版物关注索引
DEFINE INDEX publication_follow_unique_idx ON publication_follow COLUMNS user_id, publication_id UNIQUE;
DEFINE INDEX publication_follow_user_idx ON publication_follow COLUMNS user_id;
DEFINE INDEX publication_follow_publication_idx ON publication_follow COLUMNS publication_id;

-- =====================================
-- 订阅和付费系统
-- =====================================

-- 订阅计划表
DEFINE TABLE subscription_plan SCHEMAFULL;
DEFINE FIELD creator_id ON subscription_plan TYPE string ASSERT $value != NONE; -- 创作者ID
DEFINE FIELD name ON subscription_plan TYPE string ASSERT $value != NONE;
DEFINE FIELD description ON subscription_plan TYPE option<string>;
DEFINE FIELD price ON subscription_plan TYPE number ASSERT $value >= 0; -- 月费（美分）
DEFINE FIELD currency ON subscription_plan TYPE string DEFAULT "USD";
DEFINE FIELD stripe_price_id ON subscription_plan TYPE option<string>;
DEFINE FIELD benefits ON subscription_plan TYPE array<string> DEFAULT [];
DEFINE FIELD is_active ON subscription_plan TYPE bool DEFAULT true;
DEFINE FIELD created_at ON subscription_plan TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at ON subscription_plan TYPE datetime DEFAULT time::now();

-- 订阅计划索引
DEFINE INDEX subscription_plan_creator_idx ON subscription_plan COLUMNS creator_id;
DEFINE INDEX subscription_plan_active_idx ON subscription_plan COLUMNS is_active;

-- 用户订阅表
DEFINE TABLE subscription SCHEMAFULL;
DEFINE FIELD subscriber_id ON subscription TYPE string ASSERT $value != NONE;
DEFINE FIELD plan_id ON subscription TYPE record<subscription_plan> ASSERT $value != NONE;
DEFINE FIELD creator_id ON subscription TYPE string ASSERT $value != NONE;
DEFINE FIELD status ON subscription TYPE string DEFAULT "active" ASSERT $value INSIDE ["active", "canceled", "expired", "past_due"];
DEFINE FIELD started_at ON subscription TYPE datetime DEFAULT time::now();
DEFINE FIELD current_period_end ON subscription TYPE datetime DEFAULT time::now();
DEFINE FIELD canceled_at ON subscription TYPE option<datetime>;
DEFINE FIELD stripe_subscription_id ON subscription TYPE option<string>; -- 支付平台ID
DEFINE FIELD stripe_subscription_record_id ON subscription TYPE option<string>; -- 内部 Stripe 订阅记录ID
DEFINE FIELD created_at ON subscription TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at ON subscription TYPE datetime DEFAULT time::now();

-- 订阅索引
DEFINE INDEX subscription_subscriber_idx ON subscription COLUMNS subscriber_id;
DEFINE INDEX subscription_creator_idx ON subscription COLUMNS creator_id;
DEFINE INDEX subscription_status_idx ON subscription COLUMNS status;
DEFINE INDEX subscription_stripe_idx ON subscription COLUMNS stripe_subscription_id;
DEFINE INDEX subscription_stripe_record_idx ON subscription COLUMNS stripe_subscription_record_id;

-- =====================================
-- 第四阶段：会员和付费系统扩展
-- =====================================

-- 付费文章访问记录表
DEFINE TABLE paid_content_access SCHEMAFULL;
DEFINE FIELD user_id ON paid_content_access TYPE string ASSERT $value != NONE;
DEFINE FIELD article_id ON paid_content_access TYPE record<article> ASSERT $value != NONE;
DEFINE FIELD access_type ON paid_content_access TYPE string ASSERT $value INSIDE ["subscription", "one_time_purchase"];
DEFINE FIELD payment_id ON paid_content_access TYPE option<string>; -- 关联支付记录ID
DEFINE FIELD expires_at ON paid_content_access TYPE option<datetime>; -- 访问过期时间
DEFINE FIELD created_at ON paid_content_access TYPE datetime DEFAULT time::now();

-- 付费内容访问索引
DEFINE INDEX paid_content_access_user_article_idx ON paid_content_access COLUMNS user_id, article_id;
DEFINE INDEX paid_content_access_user_idx ON paid_content_access COLUMNS user_id;
DEFINE INDEX paid_content_access_article_idx ON paid_content_access COLUMNS article_id;

-- 一次性购买记录表
DEFINE TABLE one_time_purchase SCHEMAFULL;
DEFINE FIELD buyer_id ON one_time_purchase TYPE string ASSERT $value != NONE;
DEFINE FIELD article_id ON one_time_purchase TYPE record<article> ASSERT $value != NONE;
DEFINE FIELD creator_id ON one_time_purchase TYPE string ASSERT $value != NONE;
DEFINE FIELD amount ON one_time_purchase TYPE number ASSERT $value > 0; -- 金额（美分）
DEFINE FIELD currency ON one_time_purchase TYPE string DEFAULT "USD";
DEFINE FIELD payment_status ON one_time_purchase TYPE string DEFAULT "pending" ASSERT $value INSIDE ["pending", "completed", "failed", "refunded"];
DEFINE FIELD stripe_payment_intent_id ON one_time_purchase TYPE option<string>;
DEFINE FIELD completed_at ON one_time_purchase TYPE option<datetime>;
DEFINE FIELD created_at ON one_time_purchase TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at ON one_time_purchase TYPE datetime DEFAULT time::now();

-- 一次性购买索引
DEFINE INDEX one_time_purchase_buyer_idx ON one_time_purchase COLUMNS buyer_id;
DEFINE INDEX one_time_purchase_article_idx ON one_time_purchase COLUMNS article_id;
DEFINE INDEX one_time_purchase_creator_idx ON one_time_purchase COLUMNS creator_id;
DEFINE INDEX one_time_purchase_status_idx ON one_time_purchase COLUMNS payment_status;

-- 作者收益记录表
DEFINE TABLE creator_earning SCHEMAFULL;
DEFINE FIELD creator_id ON creator_earning TYPE string ASSERT $value != NONE;
DEFINE FIELD source_type ON creator_earning TYPE string ASSERT $value INSIDE ["subscription", "one_time_purchase", "tip"];
DEFINE FIELD source_id ON creator_earning TYPE string ASSERT $value != NONE; -- 来源记录ID
DEFINE FIELD gross_amount ON creator_earning TYPE number ASSERT $value > 0; -- 总金额
DEFINE FIELD platform_fee ON creator_earning TYPE number DEFAULT 0; -- 平台费用
DEFINE FIELD processing_fee ON creator_earning TYPE number DEFAULT 0; -- 处理费用
DEFINE FIELD net_amount ON creator_earning TYPE number ASSERT $value >= 0; -- 净收入
DEFINE FIELD currency ON creator_earning TYPE string DEFAULT "USD";
DEFINE FIELD payout_status ON creator_earning TYPE string DEFAULT "pending" ASSERT $value INSIDE ["pending", "paid", "failed"];
DEFINE FIELD payout_date ON creator_earning TYPE option<datetime>;
DEFINE FIELD article_id ON creator_earning TYPE option<record<article>>; -- 关联文章（如果适用）
DEFINE FIELD created_at ON creator_earning TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at ON creator_earning TYPE datetime DEFAULT time::now();

-- 收益记录索引
DEFINE INDEX creator_earning_creator_idx ON creator_earning COLUMNS creator_id;
DEFINE INDEX creator_earning_source_idx ON creator_earning COLUMNS source_type, source_id;
DEFINE INDEX creator_earning_status_idx ON creator_earning COLUMNS payout_status;
DEFINE INDEX creator_earning_date_idx ON creator_earning COLUMNS created_at;

-- 收益汇总表（按月统计）
DEFINE TABLE creator_earning_summary SCHEMAFULL;
DEFINE FIELD creator_id ON creator_earning_summary TYPE string ASSERT $value != NONE;
DEFINE FIELD year ON creator_earning_summary TYPE number ASSERT $value > 0;
DEFINE FIELD month ON creator_earning_summary TYPE number ASSERT $value >= 1 AND $value <= 12;
DEFINE FIELD total_gross ON creator_earning_summary TYPE number DEFAULT 0;
DEFINE FIELD total_platform_fee ON creator_earning_summary TYPE number DEFAULT 0;
DEFINE FIELD total_processing_fee ON creator_earning_summary TYPE number DEFAULT 0;
DEFINE FIELD total_net ON creator_earning_summary TYPE number DEFAULT 0;
DEFINE FIELD subscription_earnings ON creator_earning_summary TYPE number DEFAULT 0;
DEFINE FIELD purchase_earnings ON creator_earning_summary TYPE number DEFAULT 0;
DEFINE FIELD tip_earnings ON creator_earning_summary TYPE number DEFAULT 0;
DEFINE FIELD currency ON creator_earning_summary TYPE string DEFAULT "USD";
DEFINE FIELD created_at ON creator_earning_summary TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at ON creator_earning_summary TYPE datetime DEFAULT time::now();

-- 收益汇总索引
DEFINE INDEX creator_earning_summary_creator_period_idx ON creator_earning_summary COLUMNS creator_id, year, month UNIQUE;
DEFINE INDEX creator_earning_summary_period_idx ON creator_earning_summary COLUMNS year, month;

-- =====================================
-- Stripe 集成系统
-- =====================================

-- Stripe客户表
DEFINE TABLE stripe_customer SCHEMAFULL;
DEFINE FIELD user_id ON stripe_customer TYPE string ASSERT $value != NONE;
DEFINE FIELD stripe_customer_id ON stripe_customer TYPE string ASSERT $value != NONE;
DEFINE FIELD email ON stripe_customer TYPE string ASSERT $value != NONE;
DEFINE FIELD name ON stripe_customer TYPE option<string>;
DEFINE FIELD default_payment_method ON stripe_customer TYPE option<string>;
DEFINE FIELD created_at ON stripe_customer TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at ON stripe_customer TYPE datetime DEFAULT time::now();

-- Stripe客户索引
DEFINE INDEX stripe_customer_user_idx ON stripe_customer COLUMNS user_id UNIQUE;
DEFINE INDEX stripe_customer_stripe_id_idx ON stripe_customer COLUMNS stripe_customer_id UNIQUE;

-- Stripe支付意图表
DEFINE TABLE payment_intent SCHEMAFULL;
DEFINE FIELD stripe_payment_intent_id ON payment_intent TYPE string ASSERT $value != NONE;
DEFINE FIELD user_id ON payment_intent TYPE string ASSERT $value != NONE;
DEFINE FIELD amount ON payment_intent TYPE number ASSERT $value > 0;
DEFINE FIELD currency ON payment_intent TYPE string DEFAULT "USD";
DEFINE FIELD status ON payment_intent TYPE string ASSERT $value INSIDE ["requires_payment_method", "requires_confirmation", "requires_action", "processing", "requires_capture", "succeeded", "canceled"];
DEFINE FIELD mode ON payment_intent TYPE string DEFAULT "payment" ASSERT $value INSIDE ["payment", "setup"];
DEFINE FIELD payment_method_id ON payment_intent TYPE option<string>;
DEFINE FIELD article_id ON payment_intent TYPE option<record<article>>;
DEFINE FIELD metadata ON payment_intent TYPE object DEFAULT {};
DEFINE FIELD created_at ON payment_intent TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at ON payment_intent TYPE datetime DEFAULT time::now();

-- 支付意图索引
DEFINE INDEX payment_intent_stripe_id_idx ON payment_intent COLUMNS stripe_payment_intent_id UNIQUE;
DEFINE INDEX payment_intent_user_idx ON payment_intent COLUMNS user_id;
DEFINE INDEX payment_intent_status_idx ON payment_intent COLUMNS status;

-- Stripe订阅表
DEFINE TABLE stripe_subscription SCHEMAFULL;
DEFINE FIELD subscription_id ON stripe_subscription TYPE string ASSERT $value != NONE; -- 内部订阅ID
DEFINE FIELD stripe_subscription_id ON stripe_subscription TYPE string ASSERT $value != NONE;
DEFINE FIELD stripe_customer_id ON stripe_subscription TYPE string ASSERT $value != NONE;
DEFINE FIELD stripe_price_id ON stripe_subscription TYPE string ASSERT $value != NONE;
DEFINE FIELD status ON stripe_subscription TYPE string ASSERT $value INSIDE ["active", "past_due", "unpaid", "canceled", "incomplete", "incomplete_expired", "trialing", "paused"];
DEFINE FIELD current_period_start ON stripe_subscription TYPE datetime;
DEFINE FIELD current_period_end ON stripe_subscription TYPE datetime;
DEFINE FIELD cancel_at_period_end ON stripe_subscription TYPE bool DEFAULT false;
DEFINE FIELD canceled_at ON stripe_subscription TYPE option<datetime>;
DEFINE FIELD trial_start ON stripe_subscription TYPE option<datetime>;
DEFINE FIELD trial_end ON stripe_subscription TYPE option<datetime>;
DEFINE FIELD created_at ON stripe_subscription TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at ON stripe_subscription TYPE datetime DEFAULT time::now();

-- Stripe订阅索引
DEFINE INDEX stripe_subscription_stripe_id_idx ON stripe_subscription COLUMNS stripe_subscription_id UNIQUE;
DEFINE INDEX stripe_subscription_internal_id_idx ON stripe_subscription COLUMNS subscription_id;
DEFINE INDEX stripe_subscription_customer_idx ON stripe_subscription COLUMNS stripe_customer_id;
DEFINE INDEX stripe_subscription_status_idx ON stripe_subscription COLUMNS status;

-- Stripe支付方式表
DEFINE TABLE stripe_payment_method SCHEMAFULL;
DEFINE FIELD user_id ON stripe_payment_method TYPE string ASSERT $value != NONE;
DEFINE FIELD stripe_payment_method_id ON stripe_payment_method TYPE string ASSERT $value != NONE;
DEFINE FIELD payment_method_type ON stripe_payment_method TYPE string ASSERT $value INSIDE ["card", "bank_account", "alipay", "wechat"];
DEFINE FIELD card_brand ON stripe_payment_method TYPE option<string>;
DEFINE FIELD card_last4 ON stripe_payment_method TYPE option<string>;
DEFINE FIELD card_exp_month ON stripe_payment_method TYPE option<number>;
DEFINE FIELD card_exp_year ON stripe_payment_method TYPE option<number>;
DEFINE FIELD is_default ON stripe_payment_method TYPE bool DEFAULT false;
DEFINE FIELD created_at ON stripe_payment_method TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at ON stripe_payment_method TYPE datetime DEFAULT time::now();

DEFINE INDEX stripe_payment_method_user_idx ON stripe_payment_method COLUMNS user_id;
DEFINE INDEX stripe_payment_method_stripe_id_idx ON stripe_payment_method COLUMNS stripe_payment_method_id UNIQUE;

-- Stripe Connect 账户表
DEFINE TABLE connect_account SCHEMAFULL;
DEFINE FIELD user_id ON connect_account TYPE string ASSERT $value != NONE;
DEFINE FIELD stripe_account_id ON connect_account TYPE string ASSERT $value != NONE;
DEFINE FIELD account_type ON connect_account TYPE string ASSERT $value INSIDE ["express", "standard", "custom"];
DEFINE FIELD country ON connect_account TYPE string ASSERT $value != NONE;
DEFINE FIELD currency ON connect_account TYPE string DEFAULT "USD";
DEFINE FIELD details_submitted ON connect_account TYPE bool DEFAULT false;
DEFINE FIELD charges_enabled ON connect_account TYPE bool DEFAULT false;
DEFINE FIELD payouts_enabled ON connect_account TYPE bool DEFAULT false;
DEFINE FIELD requirements ON connect_account TYPE object DEFAULT {};
DEFINE FIELD created_at ON connect_account TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at ON connect_account TYPE datetime DEFAULT time::now();

DEFINE INDEX connect_account_user_idx ON connect_account COLUMNS user_id;
DEFINE INDEX connect_account_stripe_id_idx ON connect_account COLUMNS stripe_account_id UNIQUE;

-- Webhook事件表
DEFINE TABLE webhook_event SCHEMAFULL;
DEFINE FIELD stripe_event_id ON webhook_event TYPE string ASSERT $value != NONE;
DEFINE FIELD event_type ON webhook_event TYPE string ASSERT $value != NONE;
DEFINE FIELD processed ON webhook_event TYPE bool DEFAULT false;
DEFINE FIELD processed_at ON webhook_event TYPE option<datetime>;
DEFINE FIELD data ON webhook_event TYPE object;
DEFINE FIELD processing_summary ON webhook_event TYPE option<object>;
DEFINE FIELD created_at ON webhook_event TYPE datetime DEFAULT time::now();

-- Webhook事件索引
DEFINE INDEX webhook_event_stripe_id_idx ON webhook_event COLUMNS stripe_event_id UNIQUE;
DEFINE INDEX webhook_event_type_idx ON webhook_event COLUMNS event_type;
DEFINE INDEX webhook_event_processed_idx ON webhook_event COLUMNS processed;

-- =====================================
-- WebSocket 实时通知系统
-- =====================================

-- WebSocket连接表
DEFINE TABLE websocket_connection SCHEMAFULL;
DEFINE FIELD connection_id ON websocket_connection TYPE string ASSERT $value != NONE;
DEFINE FIELD user_id ON websocket_connection TYPE string ASSERT $value != NONE;
DEFINE FIELD connected_at ON websocket_connection TYPE datetime DEFAULT time::now();
DEFINE FIELD last_ping_at ON websocket_connection TYPE datetime DEFAULT time::now();
DEFINE FIELD user_agent ON websocket_connection TYPE option<string>;
DEFINE FIELD ip_address ON websocket_connection TYPE option<string>;
DEFINE FIELD is_active ON websocket_connection TYPE bool DEFAULT true;

-- WebSocket连接索引
DEFINE INDEX websocket_connection_id_idx ON websocket_connection COLUMNS connection_id UNIQUE;
DEFINE INDEX websocket_connection_user_idx ON websocket_connection COLUMNS user_id;
DEFINE INDEX websocket_connection_active_idx ON websocket_connection COLUMNS is_active;

-- 通知配置表
DEFINE TABLE notification_config SCHEMAFULL;
DEFINE FIELD user_id ON notification_config TYPE string ASSERT $value != NONE;
DEFINE FIELD email_notifications ON notification_config TYPE bool DEFAULT true;
DEFINE FIELD push_notifications ON notification_config TYPE bool DEFAULT true;
DEFINE FIELD websocket_notifications ON notification_config TYPE bool DEFAULT true;
DEFINE FIELD notification_types ON notification_config TYPE array<string> DEFAULT ["new_article", "new_comment", "new_follower", "article_clap", "subscription_update", "payment_update"];
DEFINE FIELD quiet_hours_start ON notification_config TYPE option<string>; -- "22:00"格式
DEFINE FIELD quiet_hours_end ON notification_config TYPE option<string>; -- "08:00"格式
DEFINE FIELD timezone ON notification_config TYPE string DEFAULT "UTC";
DEFINE FIELD created_at ON notification_config TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at ON notification_config TYPE datetime DEFAULT time::now();

-- 通知配置索引
DEFINE INDEX notification_config_user_idx ON notification_config COLUMNS user_id UNIQUE;

-- =====================================
-- 域名绑定系统
-- =====================================

-- 出版物域名表
DEFINE TABLE publication_domain SCHEMAFULL;
DEFINE FIELD publication_id ON publication_domain TYPE record<publication> ASSERT $value != NONE;
DEFINE FIELD domain_type ON publication_domain TYPE string ASSERT $value INSIDE ["subdomain", "custom"];
DEFINE FIELD subdomain ON publication_domain TYPE option<string>; -- 子域名（不含主域名）
DEFINE FIELD custom_domain ON publication_domain TYPE option<string>; -- 完整自定义域名
DEFINE FIELD status ON publication_domain TYPE string DEFAULT "pending" ASSERT $value INSIDE ["pending", "verifying", "active", "failed"];
DEFINE FIELD ssl_status ON publication_domain TYPE string DEFAULT "none" ASSERT $value INSIDE ["none", "pending", "active", "expired", "failed"];
DEFINE FIELD is_primary ON publication_domain TYPE bool DEFAULT false; -- 是否为主域名
DEFINE FIELD verification_token ON publication_domain TYPE option<string>; -- DNS验证令牌
DEFINE FIELD ssl_expires_at ON publication_domain TYPE option<datetime>; -- SSL证书过期时间
DEFINE FIELD verified_at ON publication_domain TYPE option<datetime>;
DEFINE FIELD created_at ON publication_domain TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at ON publication_domain TYPE datetime DEFAULT time::now();

-- 域名索引
DEFINE INDEX publication_domain_publication_idx ON publication_domain COLUMNS publication_id;
DEFINE INDEX publication_domain_subdomain_idx ON publication_domain COLUMNS subdomain UNIQUE;
DEFINE INDEX publication_domain_custom_domain_idx ON publication_domain COLUMNS custom_domain UNIQUE;
DEFINE INDEX publication_domain_status_idx ON publication_domain COLUMNS status;
DEFINE INDEX publication_domain_ssl_status_idx ON publication_domain COLUMNS ssl_status;
DEFINE INDEX publication_domain_primary_idx ON publication_domain COLUMNS is_primary;

-- 域名验证记录表
DEFINE TABLE domain_verification_record SCHEMAFULL;
DEFINE FIELD domain_id ON domain_verification_record TYPE record<publication_domain> ASSERT $value != NONE;
DEFINE FIELD record_type ON domain_verification_record TYPE string ASSERT $value INSIDE ["TXT", "CNAME", "A"];
DEFINE FIELD record_name ON domain_verification_record TYPE string ASSERT $value != NONE; -- DNS记录名称
DEFINE FIELD record_value ON domain_verification_record TYPE string ASSERT $value != NONE; -- DNS记录值
DEFINE FIELD purpose ON domain_verification_record TYPE string ASSERT $value INSIDE ["ownership", "routing", "ssl"];
DEFINE FIELD is_verified ON domain_verification_record TYPE bool DEFAULT false;
DEFINE FIELD verified_at ON domain_verification_record TYPE option<datetime>;
DEFINE FIELD last_check_at ON domain_verification_record TYPE option<datetime>;
DEFINE FIELD created_at ON domain_verification_record TYPE datetime DEFAULT time::now();

-- 验证记录索引
DEFINE INDEX domain_verification_record_domain_idx ON domain_verification_record COLUMNS domain_id;
DEFINE INDEX domain_verification_record_purpose_idx ON domain_verification_record COLUMNS purpose;
DEFINE INDEX domain_verification_record_verified_idx ON domain_verification_record COLUMNS is_verified;

-- SSL证书信息表
DEFINE TABLE ssl_certificate_info SCHEMAFULL;
DEFINE FIELD domain_id ON ssl_certificate_info TYPE record<publication_domain> ASSERT $value != NONE;
DEFINE FIELD provider ON ssl_certificate_info TYPE string; -- SSL证书提供商
DEFINE FIELD certificate_id ON ssl_certificate_info TYPE string; -- 证书ID
DEFINE FIELD issued_at ON ssl_certificate_info TYPE datetime;
DEFINE FIELD expires_at ON ssl_certificate_info TYPE datetime;
DEFINE FIELD auto_renew ON ssl_certificate_info TYPE bool DEFAULT true;
DEFINE FIELD renewal_attempts ON ssl_certificate_info TYPE number DEFAULT 0;
DEFINE FIELD last_renewal_attempt ON ssl_certificate_info TYPE option<datetime>;
DEFINE FIELD status ON ssl_certificate_info TYPE string ASSERT $value INSIDE ["active", "expired", "failed", "pending"];
DEFINE FIELD created_at ON ssl_certificate_info TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at ON ssl_certificate_info TYPE datetime DEFAULT time::now();

-- SSL证书索引
DEFINE INDEX ssl_certificate_info_domain_idx ON ssl_certificate_info COLUMNS domain_id UNIQUE;
DEFINE INDEX ssl_certificate_info_expires_idx ON ssl_certificate_info COLUMNS expires_at;
DEFINE INDEX ssl_certificate_info_status_idx ON ssl_certificate_info COLUMNS status;
DEFINE INDEX ssl_certificate_info_auto_renew_idx ON ssl_certificate_info COLUMNS auto_renew;

-- =====================================
-- 通知系统
-- =====================================

-- 通知表
DEFINE TABLE notification SCHEMAFULL;
DEFINE FIELD recipient_id ON notification TYPE string ASSERT $value != NONE;
DEFINE FIELD type ON notification TYPE string ASSERT $value INSIDE ["follow", "clap", "comment", "mention", "article_published", "subscription"];
DEFINE FIELD title ON notification TYPE string ASSERT $value != NONE;
DEFINE FIELD message ON notification TYPE string ASSERT $value != NONE;
DEFINE FIELD data ON notification TYPE object DEFAULT {}; -- 相关数据（文章ID、用户ID等）
DEFINE FIELD is_read ON notification TYPE bool DEFAULT false;
DEFINE FIELD read_at ON notification TYPE option<datetime>;
DEFINE FIELD created_at ON notification TYPE datetime DEFAULT time::now();

-- 通知索引
DEFINE INDEX notification_recipient_idx ON notification COLUMNS recipient_id;
DEFINE INDEX notification_recipient_unread_idx ON notification COLUMNS recipient_id, is_read;
DEFINE INDEX notification_created_idx ON notification COLUMNS created_at;

-- =====================================
-- 统计和分析
-- =====================================

-- 文章统计表（按天汇总）
DEFINE TABLE article_stats_daily SCHEMAFULL;
DEFINE FIELD article_id ON article_stats_daily TYPE record<article> ASSERT $value != NONE;
DEFINE FIELD date ON article_stats_daily TYPE datetime ASSERT $value != NONE;
DEFINE FIELD views ON article_stats_daily TYPE number DEFAULT 0;
DEFINE FIELD reads ON article_stats_daily TYPE number DEFAULT 0; -- 完整阅读
DEFINE FIELD claps ON article_stats_daily TYPE number DEFAULT 0;
DEFINE FIELD comments ON article_stats_daily TYPE number DEFAULT 0;
DEFINE FIELD bookmarks ON article_stats_daily TYPE number DEFAULT 0;
DEFINE FIELD shares ON article_stats_daily TYPE number DEFAULT 0;
DEFINE FIELD reading_time_total ON article_stats_daily TYPE number DEFAULT 0; -- 总阅读时间（秒）
DEFINE FIELD created_at ON article_stats_daily TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at ON article_stats_daily TYPE datetime DEFAULT time::now();

-- 文章统计索引
DEFINE INDEX article_stats_daily_unique_idx ON article_stats_daily COLUMNS article_id, date UNIQUE;
DEFINE INDEX article_stats_daily_article_idx ON article_stats_daily COLUMNS article_id;
DEFINE INDEX article_stats_daily_date_idx ON article_stats_daily COLUMNS date;

-- 用户活动日志表
DEFINE TABLE activity_log SCHEMAFULL;
DEFINE FIELD user_id ON activity_log TYPE string ASSERT $value != NONE;
DEFINE FIELD action ON activity_log TYPE string ASSERT $value != NONE;
DEFINE FIELD resource_type ON activity_log TYPE string ASSERT $value INSIDE ["article", "comment", "user", "publication", "tag"];
DEFINE FIELD resource_id ON activity_log TYPE string ASSERT $value != NONE;
DEFINE FIELD ip_address ON activity_log TYPE option<string>;
DEFINE FIELD user_agent ON activity_log TYPE option<string>;
DEFINE FIELD details ON activity_log TYPE object DEFAULT {};
DEFINE FIELD created_at ON activity_log TYPE datetime DEFAULT time::now();

-- 活动日志索引
DEFINE INDEX activity_log_user_idx ON activity_log COLUMNS user_id;
DEFINE INDEX activity_log_resource_idx ON activity_log COLUMNS resource_type, resource_id;
DEFINE INDEX activity_log_created_idx ON activity_log COLUMNS created_at;
DEFINE INDEX activity_log_action_idx ON activity_log COLUMNS action;

-- =====================================
-- 搜索优化
-- =====================================

-- 搜索索引表
DEFINE TABLE search_index SCHEMAFULL;
DEFINE FIELD article_id ON search_index TYPE record<article> ASSERT $value != NONE;
DEFINE FIELD title ON search_index TYPE string;
DEFINE FIELD content ON search_index TYPE string; -- 纯文本内容，用于全文搜索
DEFINE FIELD author_name ON search_index TYPE string;
DEFINE FIELD tags ON search_index TYPE array<string> DEFAULT [];
DEFINE FIELD publication_name ON search_index TYPE option<string>;
DEFINE FIELD is_published ON search_index TYPE bool DEFAULT false;
DEFINE FIELD published_at ON search_index TYPE option<datetime>;
DEFINE FIELD popularity_score ON search_index TYPE number DEFAULT 0; -- 基于互动的流行度分数
DEFINE FIELD updated_at ON search_index TYPE datetime DEFAULT time::now();

-- 搜索索引
DEFINE INDEX search_index_article_idx ON search_index COLUMNS article_id UNIQUE;
DEFINE INDEX search_index_published_idx ON search_index COLUMNS is_published;
DEFINE INDEX search_index_popularity_idx ON search_index COLUMNS popularity_score;

-- =====================================
-- 初始数据
-- =====================================

-- 插入默认标签（热门标签）
INSERT INTO tag (name, slug, description, is_featured) VALUES
    ("Technology", "technology", "Latest in tech and programming", true),
    ("Business", "business", "Business insights and strategies", true),
    ("Health", "health", "Health and wellness topics", true),
    ("Science", "science", "Scientific discoveries and research", true),
    ("Design", "design", "Design trends and inspiration", true),
    ("Politics", "politics", "Political analysis and news", false),
    ("Culture", "culture", "Cultural commentary and arts", false),
    ("Programming", "programming", "Programming tutorials and tips", true),
    ("AI", "ai", "Artificial Intelligence and ML", true),
    ("Startup", "startup", "Startup ecosystem and advice", true);

-- 预留子域名（不可注册）
-- 注意：这些在实际应用中应该通过配置文件或环境变量管理
-- 这里仅作为示例数据

-- =====================================
-- 存储过程和触发器（SurrealDB特有）
-- =====================================

-- 更新用户统计的函数
DEFINE FUNCTION fn::update_user_stats($user_id: string) {
    -- 更新文章数
    LET $article_count = (SELECT count() FROM article WHERE author_id = $user_id AND status = 'published' AND is_deleted = false);
    
    -- 更新关注者数
    LET $follower_count = (SELECT count() FROM follow WHERE following_id = $user_id);
    
    -- 更新关注数
    LET $following_count = (SELECT count() FROM follow WHERE follower_id = $user_id);
    
    -- 更新总获赞数
    LET $total_claps = (SELECT math::sum(clap_count) FROM article WHERE author_id = $user_id);
    
    -- 更新用户资料
    UPDATE user_profile SET 
        article_count = $article_count,
        follower_count = $follower_count,
        following_count = $following_count,
        total_claps_received = $total_claps,
        updated_at = time::now()
    WHERE user_id = $user_id;
};

-- 更新文章统计的函数
DEFINE FUNCTION fn::update_article_stats($article_id: record<article>) {
    -- 更新评论数
    LET $comment_count = (SELECT count() FROM comment WHERE article_id = $article_id AND is_deleted = false);
    
    -- 更新书签数
    LET $bookmark_count = (SELECT count() FROM bookmark WHERE article_id = $article_id);
    
    -- 更新点赞总数
    LET $clap_count = (SELECT math::sum(count) FROM clap WHERE article_id = $article_id);
    
    -- 更新文章
    UPDATE article SET 
        comment_count = $comment_count,
        bookmark_count = $bookmark_count,
        clap_count = $clap_count,
        updated_at = time::now()
    WHERE id = $article_id;
};

-- 计算文章流行度分数的函数
DEFINE FUNCTION fn::calculate_popularity_score($article_id: record<article>) {
    LET $article = (SELECT * FROM article WHERE id = $article_id);
    
    -- 基于各种互动指标计算分数
    LET $score = (
        $article.view_count * 0.1 +
        $article.clap_count * 1 +
        $article.comment_count * 2 +
        $article.bookmark_count * 3 +
        $article.share_count * 5
    );
    
    -- 时间衰减因子（新文章获得加成）
    LET $age_days = duration::days(time::now() - $article.published_at);
    LET $time_factor = math::max(0.5, 1 - ($age_days / 30));
    
    RETURN $score * $time_factor;
};

-- 更新创作者收益汇总的函数
DEFINE FUNCTION fn::update_creator_earnings_summary($creator_id: string, $year: number, $month: number) {
    -- 计算指定月份的收益汇总
    LET $earnings = (
        SELECT 
            math::sum(gross_amount) AS total_gross,
            math::sum(platform_fee) AS total_platform_fee,
            math::sum(processing_fee) AS total_processing_fee,
            math::sum(net_amount) AS total_net,
            math::sum(IF source_type = 'subscription' THEN net_amount ELSE 0 END) AS subscription_earnings,
            math::sum(IF source_type = 'one_time_purchase' THEN net_amount ELSE 0 END) AS purchase_earnings,
            math::sum(IF source_type = 'tip' THEN net_amount ELSE 0 END) AS tip_earnings
        FROM creator_earning 
        WHERE creator_id = $creator_id 
        AND time::year(created_at) = $year 
        AND time::month(created_at) = $month
    );
    
    -- 更新或插入汇总记录
    UPDATE creator_earning_summary SET 
        total_gross = $earnings.total_gross,
        total_platform_fee = $earnings.total_platform_fee,
        total_processing_fee = $earnings.total_processing_fee,
        total_net = $earnings.total_net,
        subscription_earnings = $earnings.subscription_earnings,
        purchase_earnings = $earnings.purchase_earnings,
        tip_earnings = $earnings.tip_earnings,
        updated_at = time::now()
    WHERE creator_id = $creator_id AND year = $year AND month = $month;
    
    -- 如果不存在则创建新记录
    IF (SELECT count() FROM creator_earning_summary WHERE creator_id = $creator_id AND year = $year AND month = $month) = 0 {
        INSERT INTO creator_earning_summary {
            creator_id: $creator_id,
            year: $year,
            month: $month,
            total_gross: $earnings.total_gross,
            total_platform_fee: $earnings.total_platform_fee,
            total_processing_fee: $earnings.total_processing_fee,
            total_net: $earnings.total_net,
            subscription_earnings: $earnings.subscription_earnings,
            purchase_earnings: $earnings.purchase_earnings,
            tip_earnings: $earnings.tip_earnings,
            currency: 'USD'
        };
    };
};

-- 检查用户是否有权限访问付费内容的函数
DEFINE FUNCTION fn::check_paid_content_access($user_id: string, $article_id: record<article>) {
    LET $article = (SELECT * FROM article WHERE id = $article_id);
    
    -- 如果不是付费内容，直接返回true
    IF !$article.is_paid_content {
        RETURN true;
    };
    
    -- 检查是否是作者本人
    IF $article.author_id = $user_id {
        RETURN true;
    };
    
    -- 检查订阅状态
    LET $subscription = (
        SELECT * FROM subscription 
        WHERE subscriber_id = $user_id 
        AND creator_id = $article.author_id 
        AND status = 'active'
        AND current_period_end > time::now()
    );
    
    IF count($subscription) > 0 {
        RETURN true;
    };
    
    -- 检查一次性购买
    LET $purchase = (
        SELECT * FROM paid_content_access 
        WHERE user_id = $user_id 
        AND article_id = $article_id
        AND (expires_at IS NULL OR expires_at > time::now())
    );
    
    IF count($purchase) > 0 {
        RETURN true;
    };
    
    RETURN false;
};

-- 计算平台费用的函数
DEFINE FUNCTION fn::calculate_platform_fees($amount: number) {
    LET $platform_fee_rate = 0.10; -- 平台费用 10%
    LET $processing_fee_rate = 0.029; -- 处理费用 2.9%
    
    LET $platform_fee = math::floor($amount * $platform_fee_rate);
    LET $processing_fee = math::floor($amount * $processing_fee_rate);
    LET $net_amount = $amount - $platform_fee - $processing_fee;
    
    RETURN {
        gross_amount: $amount,
        platform_fee: $platform_fee,
        processing_fee: $processing_fee,
        net_amount: $net_amount
    };
};

-- 域名验证状态检查函数
DEFINE FUNCTION fn::check_domain_verification($domain_id: record<publication_domain>) {
    LET $required_records = (
        SELECT * FROM domain_verification_record 
        WHERE domain_id = $domain_id 
        AND purpose IN ['ownership', 'routing']
    );
    
    LET $verified_count = (
        SELECT count() FROM domain_verification_record 
        WHERE domain_id = $domain_id 
        AND purpose IN ['ownership', 'routing'] 
        AND is_verified = true
    );
    
    LET $total_count = count($required_records);
    
    IF $verified_count = $total_count AND $total_count > 0 {
        -- 所有必需记录都已验证，更新域名状态
        UPDATE publication_domain SET 
            status = 'active',
            verified_at = time::now(),
            updated_at = time::now()
        WHERE id = $domain_id;
        
        RETURN 'verified';
    } ELSE IF $verified_count > 0 {
        -- 部分验证
        UPDATE publication_domain SET 
            status = 'verifying',
            updated_at = time::now()
        WHERE id = $domain_id;
        
        RETURN 'partial';
    } ELSE {
        -- 未验证
        UPDATE publication_domain SET 
            status = 'pending',
            updated_at = time::now()
        WHERE id = $domain_id;
        
        RETURN 'pending';
    };
};

-- =====================================
-- 媒体文件系统
-- =====================================

DEFINE TABLE media_file SCHEMAFULL;
DEFINE FIELD user_id ON media_file TYPE string ASSERT $value != NONE;
DEFINE FIELD filename ON media_file TYPE string ASSERT $value != NONE;
DEFINE FIELD original_filename ON media_file TYPE string ASSERT $value != NONE;
DEFINE FIELD content_type ON media_file TYPE string ASSERT $value != NONE;
DEFINE FIELD size ON media_file TYPE number ASSERT $value >= 0;
DEFINE FIELD width ON media_file TYPE option<number>;
DEFINE FIELD height ON media_file TYPE option<number>;
DEFINE FIELD storage_path ON media_file TYPE string ASSERT $value != NONE;
DEFINE FIELD public_url ON media_file TYPE string ASSERT $value != NONE;
DEFINE FIELD created_at ON media_file TYPE datetime DEFAULT time::now();

DEFINE INDEX media_file_user_idx ON media_file COLUMNS user_id;
DEFINE INDEX media_file_created_idx ON media_file COLUMNS created_at;
