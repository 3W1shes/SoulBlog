# SoulBlog Agent API v1

> 面向 OpenClaw / Agent 调用方的 **最小能力面接口**。

## 1. 当前结论

截至当前工作树，Agent 主线已经明确为 `/agent/v1`：

- 主线入口：`/agent/v1`
- 单一实现：`src/agent/router.rs`
- 挂载位置：`src/main.rs`

也就是说：

> **`/agent/v1` 才是对 OpenClaw / 外部 agent 的稳定契约。**

## 2. 设计目标

v1 解决这些事情：

1. 发现出版物
2. 读取出版物详情
3. 发现文章
4. 读取文章详情
5. 搜索内容
6. 阅读评论
7. 发表评论（认证）
8. 获取通知（认证）

额外只保留一个健康检查端点，方便外部接入做存活探测与能力发现。

因此 v1 默认服务于：

- 聊天问答前的知识检索
- 内容导航 / 引用
- Agent 拉取上下文后再总结
- OpenClaw 最小能力接入
- 轻量级内容交互（评论、通知）

## 3. Base URL

```text
{SOULBLOG_BASE_URL}/agent/v1
```

示例：

```text
http://localhost:3000/agent/v1
```

## 4. 认证与访问模型

受保护接口使用：

```http
Authorization: Bearer <jwt>
```

当前实现中的访问模型是：

- `system.health`：公开可访问
- `publication.list` / `publication.get`：公开可访问
- `article.list` / `article.get`：公开可访问（只返回已发布文章）
- `search`：可选认证
- `comment.list`：公开可访问
- `comment.create`：必须认证，需要 `blog:comment:write` scope
- `notification.list` / `notification.read`：必须认证

这意味着：

- v1 不是"完全公开知识库接口"
- 也不是"管理后台直通接口"
- 它只是把现有能力收束成更适合 agent 消费的最小面

### 4.1 JWT Claims

```json
{
  "sub": "user_id",
  "role": "admin",
  "permissions": ["blog:article:read", "blog:comment:write"],
  "session_id": "session_xxx",
  "exp": 1234567890,
  "iat": 1234567800
}
```

### 4.2 Scope 列表

| Scope | 说明 | 对应能力 |
|---|---|---|
| `blog:system:health` | 系统健康 | health |
| `blog:publication:read` | 读取出版物 | list/get publications |
| `blog:publication:write` | 写入出版物 | create/update/delete |
| `blog:article:read` | 读取文章 | list/get articles |
| `blog:article:write` | 写入文章 | create/update/delete |
| `blog:comment:read` | 读取评论 | list comments |
| `blog:comment:write` | 写入评论 | create comment |
| `blog:search:read` | 搜索 | search |
| `blog:notification:read` | 读取通知 | list notifications |
| `blog:notification:write` | 写入通知 | mark as read |

## 5. 响应风格

当前 `/agent/v1` 采用 agent 专用 envelope：

成功响应：

```json
{
  "ok": true,
  "data": { ... },
  "request_id": "agv1-1742350000000-1a2b"
}
```

失败响应：

```json
{
  "ok": false,
  "error": {
    "code": "not_found",
    "message": "Article not found",
    "details": null
  },
  "request_id": "agv1-1742350000000-3c4d"
}
```

错误代码映射：

| HTTP Status | Error Code | 说明 |
|---|---|---|
| 400 | `bad_request` | 参数错误 |
| 401 | `unauthorized` | 未认证 |
| 403 | `forbidden` | 无权限 |
| 404 | `not_found` | 资源不存在 |
| 409 | `conflict` | 资源冲突 |
| 429 | `too_many_requests` | 限流 |
| 502 | `bad_gateway` | 上游错误 |
| 503 | `service_unavailable` | 服务不可用 |
| 500 | `internal_error` | 内部错误 |

## 6. v1 实际开放能力

### 6.1 健康检查

```http
GET /agent/v1/system/health
```

#### 响应示例

```json
{
  "ok": true,
  "data": {
    "status": "ok",
    "version": "0.1.0",
    "capabilities": [
      "system.health",
      "publication.list",
      "publication.get",
      "article.list",
      "article.get",
      "search",
      "comment.list",
      "comment.create"
    ]
  },
  "request_id": "agv1-1742350000000-1a2b"
}
```

### 6.2 列出出版物

```http
GET /agent/v1/publications?page=1&limit=20&search=tech&sort=popular
```

#### 查询参数

| 参数 | 类型 | 默认值 | 说明 |
|---|---|---|---|
| `page` | integer | 1 | 页码 |
| `limit` | integer | 20 | 每页数量，最大 100 |
| `search` | string | - | 搜索关键词 |
| `sort` | string | `popular` | 排序方式 |
| `verified_only` | boolean | false | 只显示认证出版物 |

#### 响应示例

```json
{
  "ok": true,
  "data": {
    "publications": [
      {
        "id": "publication:uuid",
        "name": "Tech Blog",
        "slug": "tech-blog",
        "description": "Technology and programming",
        "tagline": "Code, Create, Share",
        "logo_url": "https://example.com/logo.png",
        "cover_image_url": "https://example.com/cover.png",
        "member_count": 10,
        "article_count": 150,
        "follower_count": 5000,
        "is_verified": true,
        "created_at": "2024-01-01T00:00:00Z"
      }
    ],
    "total": 100,
    "page": 1,
    "per_page": 20
  },
  "request_id": "agv1-1742350000000-2b3c"
}
```

### 6.3 获取单个出版物

```http
GET /agent/v1/publications/{id}
```

#### 响应示例

```json
{
  "ok": true,
  "data": {
    "id": "publication:uuid",
    "name": "Tech Blog",
    "slug": "tech-blog",
    "description": "Technology and programming",
    ...
    "is_member": false,
    "member_role": null,
    "is_following": false,
    "recent_articles": [...]
  },
  "request_id": "agv1-1742350000000-3c4d"
}
```

### 6.4 列出文章

```http
GET /agent/v1/articles?page=1&limit=20&publication_id=xxx&sort=newest
```

#### 查询参数

| 参数 | 类型 | 默认值 | 说明 |
|---|---|---|---|
| `page` | integer | 1 | 页码 |
| `limit` | integer | 20 | 每页数量，最大 100 |
| `publication_id` | string | - | 按出版物筛选 |
| `author` | string | - | 按作者筛选 |
| `tag` | string | - | 按标签筛选 |
| `featured` | boolean | - | 只显示精选文章 |
| `search` | string | - | 搜索关键词 |
| `sort` | string | `newest` | 排序方式 |

#### 响应示例

```json
{
  "ok": true,
  "data": {
    "articles": [
      {
        "id": "article:uuid",
        "title": "Getting Started with Rust",
        "subtitle": "A beginner's guide",
        "slug": "getting-started-with-rust",
        "excerpt": "Rust is a systems programming language...",
        "cover_image_url": "https://example.com/cover.png",
        "author": {
          "id": "user:uuid",
          "username": "johndoe",
          "display_name": "John Doe",
          "avatar_url": "https://example.com/avatar.png",
          "is_verified": true
        },
        "publication": {
          "id": "publication:uuid",
          "name": "Tech Blog",
          "slug": "tech-blog",
          "logo_url": "https://example.com/logo.png"
        },
        "status": "published",
        "is_paid_content": false,
        "is_featured": true,
        "reading_time": 5,
        "view_count": 1000,
        "clap_count": 50,
        "comment_count": 10,
        "tags": [...],
        "created_at": "2024-01-01T00:00:00Z",
        "published_at": "2024-01-01T00:00:00Z"
      }
    ],
    "total": 1000,
    "page": 1,
    "per_page": 20
  },
  "request_id": "agv1-1742350000000-4d5e"
}
```

### 6.5 获取单篇文章

```http
GET /agent/v1/articles/{id}
```

#### 响应示例

```json
{
  "ok": true,
  "data": {
    "id": "article:uuid",
    "title": "Getting Started with Rust",
    "subtitle": "A beginner's guide",
    "slug": "getting-started-with-rust",
    "content": "# Getting Started...",
    "content_html": "<h1>Getting Started...</h1>",
    "excerpt": "Rust is a systems programming language...",
    "cover_image_url": "https://example.com/cover.png",
    "author": {...},
    "publication": {...},
    "series": null,
    "status": "published",
    "is_paid_content": false,
    "is_featured": true,
    "reading_time": 5,
    "word_count": 1200,
    "view_count": 1000,
    "clap_count": 50,
    "comment_count": 10,
    "bookmark_count": 20,
    "tags": [...],
    "seo_title": "...",
    "seo_description": "...",
    "seo_keywords": [...],
    "created_at": "2024-01-01T00:00:00Z",
    "updated_at": "2024-01-02T00:00:00Z",
    "published_at": "2024-01-01T00:00:00Z",
    "is_bookmarked": false,
    "is_clapped": false,
    "user_clap_count": 0
  },
  "request_id": "agv1-1742350000000-5e6f"
}
```

### 6.6 搜索

```http
GET /agent/v1/search?q=rust&type=article&page=1&per_page=20
```

#### 查询参数

| 参数 | 类型 | 默认值 | 说明 |
|---|---|---|---|
| `q` (必填) | string | - | 搜索关键词 |
| `type` | string | `all` | 搜索类型：article, publication, all |
| `page` | integer | 1 | 页码 |
| `per_page` | integer | 20 | 每页数量，最大 100 |
| `publication_id` | string | - | 按出版物筛选 |
| `author_id` | string | - | 按作者筛选 |
| `tag` | string | - | 按标签筛选 |

#### 响应示例

```json
{
  "ok": true,
  "data": {
    "results": [
      {
        "id": "article:uuid",
        "title": "Getting Started with Rust",
        ...
      }
    ],
    "total": 50,
    "page": 1,
    "per_page": 20,
    "query": "rust"
  },
  "request_id": "agv1-1742350000000-6f7g"
}
```

### 6.7 列出评论

```http
GET /agent/v1/comments?article_id=xxx&page=1&per_page=20
```

#### 响应示例

```json
{
  "ok": true,
  "data": {
    "comments": [
      {
        "id": "comment:uuid",
        "content": "Great article!",
        "author": {...},
        "parent_id": null,
        "reply_count": 5,
        "clap_count": 10,
        "created_at": "2024-01-01T00:00:00Z"
      }
    ],
    "total": 50,
    "page": 1,
    "per_page": 20
  },
  "request_id": "agv1-1742350000000-7g8h"
}
```

### 6.8 发表评论

```http
POST /agent/v1/comments
Authorization: Bearer <JWT>
Content-Type: application/json

{
  "article_id": "article:uuid",
  "content": "This is a great article! Thanks for sharing.",
  "parent_id": null
}
```

#### 响应示例

```json
{
  "ok": true,
  "data": {
    "id": "comment:uuid",
    "content": "This is a great article! Thanks for sharing.",
    "author": {...},
    "article_id": "article:uuid",
    "parent_id": null,
    "reply_count": 0,
    "clap_count": 0,
    "created_at": "2024-01-01T00:00:00Z"
  },
  "request_id": "agv1-1742350000000-8h9i"
}
```

### 6.9 获取通知

```http
GET /agent/v1/notifications
Authorization: Bearer <JWT>
```

#### 响应示例

```json
{
  "ok": true,
  "data": {
    "notifications": [
      {
        "id": "notification:uuid",
        "type": "comment_reply",
        "title": "New Reply",
        "message": "Someone replied to your comment",
        "data": {...},
        "is_read": false,
        "created_at": "2024-01-01T00:00:00Z"
      }
    ],
    "unread_count": 5,
    "total": 20
  },
  "request_id": "agv1-1742350000000-9i0j"
}
```

### 6.10 标记通知已读

```http
## 7. 最小能力面总结

当前 `/agent/v1` 真实最小能力面：

**公开访问（无需认证）：**

1. `system.health` - 健康检查
2. `publication.list` - 列出出版物
3. `publication.get` - 获取出版物详情
4. `article.list` - 列出文章
5. `article.get` - 获取文章详情
6. `search` - 搜索内容
7. `comment.list` - 列出评论

**需要认证：**

8. `comment.create` - 发表评论

这是一个明确的 **读写分离面**：

- 公开读取：出版物、文章、评论
- 认证写入：评论
- 没有写入文章、管理出版物等敏感操作

## 8. notIncluded / 明确不纳入 v1 的内容

### 8.1 文章写操作

- 创建 / 更新 / 删除文章
- 发布 / 下架文章
- 管理文章状态

### 8.2 出版物管理

- 创建 / 更新 / 删除出版物
- 管理出版物成员
- 管理出版物设置

### 8.3 用户管理

- 用户注册 / 登录
- 用户信息修改
- 密码重置

### 8.4 高级功能

- 支付相关
- 订阅管理
- 收益管理
- 统计分析
- 文件上传

### 8.5 旧 `/api/blog/*` 业务 REST

特别说明：

- `GET /api/blog/articles`
- `POST /api/blog/articles`
- `PUT /api/blog/articles/{id}`
- `DELETE /api/blog/articles/{id}`

这些依然是 SoulBlog 的业务 REST 能力，但 **不再是 Agent API v1 的主契约**。

## 9. 安全与风险边界

### 最小权限原则

推荐为 OpenClaw 单独准备：

- 独立 JWT / service account
- 只读角色（如果只需要读取）
- 按出版物白名单收敛访问范围

### 输出最小化

建议 adapter / tool 层：

- 默认只返回必要字段
- 文章列表不回传长正文
- 正文接口加截断或分段策略
- 搜索结果限制数量与 snippet 长度

### 审计与可回放

建议记录：

- capability
- request_id
- 调用会话 / 调用人
- 命中的 article_id / publication_id
- 是否拉取全文
- 上游 HTTP 状态码

### Prompt 注入边界

SoulBlog 文章内容中可能出现：

- 恶意 prompt
- 误导性 shell 命令
- 过期运维步骤
- "请继续执行……" 之类指令文本

这些都应被当作 **待判断的内容**，不是高优先级系统命令。

## 10. 演进建议

### v1.1 可考虑增加

在不破坏当前主线的前提下，可评估：

- 文章目录树能力
- breadcrumbs 导航能力
- 更精细的 agent DTO
- 针对 OpenClaw 的专用 tool catalog 示例

### 暂不建议做

在没有独立审计、确认流、回滚与更严权限隔离前，不建议把以下内容推进到 v2：

- 自动创建文章
- 自动修改文章
- 自动管理出版物
- 自动执行管理操作

## 11. 与当前代码的一致性说明

本文档基于当前仓库中的实际实现整理，关键依据包括：

- `src/main.rs`：Agent API 挂载点
- `src/agent/router.rs`：Agent 路由定义
- `src/agent/handlers.rs`：Agent 处理器
- `src/agent/auth.rs`：Agent 认证与权限
- `src/agent/response.rs`：Agent 响应格式

若后续 `/agent/v1` 增加 notifications 等新 capability，或参数 / 返回 envelope 发生变化，应优先同步本文件。
