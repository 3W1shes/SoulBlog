# SoulBlog 部署指南

SoulBlog 同一份源码支持两种部署模式，**部署时通过编译特性选择，安装后不可切换**：

| 模式 | 适用场景 | 编译特性 |
|---|---|---|
| **Single（单用户博客）** | 个人博客：站长一人写作，访客可注册评论 | `--no-default-features --features single` |
| **Platform（平台博客）** | 多用户多博客：每人可创建出版物、订阅、付费 | `--features platform`（默认） |

## 1. 准备环境

- Rust 1.75+
- SurrealDB 3.0
- 前端构建：`dx`（Dioxus CLI 0.7）

```bash
cargo install dioxus-cli --version 0.7
```

## 2. 启动 SurrealDB（持久化模式）

```bash
surreal start --user root --pass root --bind 0.0.0.0:8000 \
  rocksdb:./data/surrealdb
```

## 3. 配置 .env

复制 `.env.example` 为 `.env`，按需修改：

```env
SERVER_HOST=0.0.0.0
SERVER_PORT=3002
DATABASE_URL=ws://localhost:8000
DATABASE_NAMESPACE=soulblog
DATABASE_NAME=blog
JWT_SECRET=请改成 64 位随机串
FRONTEND_URL=http://localhost:5183
CORS_ALLOWED_ORIGINS=http://localhost:5183,http://localhost:8080
```

## 4. 选择模式编译

### Single 模式（单用户博客）

后端：
```bash
cargo build --release --no-default-features --features single
./target/release/rainbow-blog
```

前端：
```bash
cd ../SoulBlogFront
dx build --release --no-default-features --features single
# 或开发模式：
dx serve --no-default-features --features single
```

**Single 模式启动后**：
- 访问 `http://localhost:5183/` 自动跳转 `/install`
- 完成安装向导（站点名 + 站长账号）
- 安装完成后 `/install` 永久不可访问
- 站长进入 `/admin` 管理文章、评论、用户、站点设置

### Platform 模式（多用户多博客）

后端：
```bash
cargo build --release            # 默认就是 platform
./target/release/rainbow-blog
```

前端：
```bash
cd ../SoulBlogFront
dx serve                          # 默认 platform
```

**Platform 模式启动后**：
- 访问 `http://localhost:5183/` 进入平台首页
- 注册即可创建个人 publication
- 多人协作、订阅、付费、域名绑定均启用

## 5. 部署模式不可切换

**重要**：换版本 = 重装。不支持运行时或安装后切换。

如需从 Single 切到 Platform（或反向）：
1. 备份数据库
2. 用站长后台导出文章为 ZIP（功能在路线图中）
3. 卸载 + 清空数据库
4. 用目标模式重新编译并安装
5. 走完安装向导
6. 导入 ZIP

## 6. 关键 API 端点

### 共享（两模式都有）
- `GET /api/blog/site/status` — 安装状态、当前模式
- `POST /api/blog/site/install` — 一次性安装（首次启动）
- `GET /api/blog/site/config` — 公开品牌信息
- `GET /api/blog/site/admin/config` — 站长完整配置（admin only）
- `PUT /api/blog/site/admin/config` — 更新配置（admin only）
- `POST /api/blog/auth/login | register | logout`
- `GET /api/blog/articles` — 文章列表
- `GET /api/blog/articles/:slug` — 文章详情
- `POST /api/blog/articles/create` — 创建文章
- `POST /api/blog/articles/by-id/:id/publish` — 发布
- `GET /api/blog/admin/overview` — 后台总览（admin only）
- `GET /api/blog/admin/articles | comments | users` — 管理接口
- `POST /api/blog/ai/generate | improve | suggest-title | suggest-tags` — AI 创作
- `GET /api/blog/ai/config` — 用户 AI 配置

### Platform 专属
- `GET /api/blog/publications` — 出版物列表
- `POST /api/blog/publications` — 创建出版物
- `GET /api/blog/subscriptions` — 订阅
- `POST /api/blog/payments/*` — Stripe 付费
- `GET /api/blog/domains/*` — 域名绑定

## 7. 常见排错

**Q: 启动后 /api/blog/site/status 返回 500**
A: 确认 SurrealDB 已启动且 schema 初始化无报错。检查日志中的 `Schema initialized` 消息。

**Q: 安装向导提示 "Site already installed"**
A: 数据库里已有 `site_config:main` 记录。如需重置：
```bash
surreal sql --conn ws://localhost:8000 --user root --pass root \
  --ns soulblog --db blog --pretty
DELETE site_config:main;
```

**Q: 进入 /admin 跳到首页**
A: 当前用户不是 site_config.owner_user_id。只有安装时创建的站长账号能访问 /admin。

**Q: AI 生成无响应**
A: 先到 `/ai-settings` 配置 provider（Anthropic/OpenAI）和 API key。
