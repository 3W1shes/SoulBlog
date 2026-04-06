# SoulBlog × OpenClaw 集成说明

> 这份文档回答的是当前问题：
>
> **SoulBlog 如何接入 OpenClaw，让 Agent 可以操作博客系统。**

## 1. 先说结论

当前仓库已实现 **Agent API v1**，面向 OpenClaw / Agent 的维护主线为：

- **主线入口：`/agent/v1`**
- **单一实现：`src/agent/router.rs`**
- **认证方式：JWT Bearer Token**
- **响应格式：统一 AgentResponse 信封**

因此集成口径应统一为：

> OpenClaw 新接入一律以 **`/agent/v1`** 为准。

## 2. 为什么要这样设计

直接把 SoulBlog 整套 `/api/blog/*` 业务接口原样暴露给 agent，会有几个问题：

1. **能力面过宽**：写操作、管理动作会混进来
2. **契约不稳定**：业务 REST 的字段和路由是为产品前后端服务，不一定适合 agent 长期消费
3. **权限心智混乱**：前端场景、后端业务场景、agent 场景混在一起
4. **审计边界不清**：难以明确"哪些是 agent 被允许做的事"

而 `/agent/v1` 做的事情很清楚：

- 把读写能力分离，读操作公开，写操作需要认证
- 用 capability/scope 语义而不是整套业务语义对外表达
- 统一响应格式，便于 Agent 处理

## 3. 当前真实可接入的能力

以 `src/agent/router.rs` 当前实现为准，OpenClaw 当前应该只把下面这些视为正式接入面。

### 3.1 健康检查

```http
GET /agent/v1/system/health
```

用途：

- 存活检查
- 能力发现
- 接入连通性验证

说明：

- 这是轻量健康探针
- 无需认证
- 返回当前支持的 capability 列表

### 3.2 出版物列表

```http
GET /agent/v1/publications?page=1&limit=20&search=keyword&sort=popular
```

用途：

- 列出博客平台上的出版物
- 作为内容发现入口

查询参数：

- `page`: 页码，默认 1
- `limit`: 每页数量，默认 20，最大 100
- `search`: 搜索关键词
- `sort`: 排序方式 (`newest`, `oldest`, `popular`, `alphabetical`)

### 3.3 单出版物详情

```http
GET /agent/v1/publications/{id}
```

用途：

- 读取某个出版物详情
- 获取该出版物的最新文章列表

关键点：

- 使用 `id` 而不是 `slug`
- 返回包含出版物信息、成员状态、最新文章

### 3.4 文章列表

```http
GET /agent/v1/articles?page=1&limit=20&publication_id=xxx&sort=popular
```

用途：

- 列出平台上的文章
- 可按出版物、作者、标签筛选

查询参数：

- `page`: 页码，默认 1
- `limit`: 每页数量，默认 20，最大 100
- `publication_id`: 按出版物筛选
- `author`: 按作者筛选
- `tag`: 按标签筛选
- `featured`: 是否只显示精选文章
- `search`: 搜索关键词
- `sort`: 排序方式 (`newest`, `oldest`, `popular`, `trending`)

关键点：

- 默认只返回已发布文章
- 列表项不包含完整正文

### 3.5 单文章读取

```http
GET /agent/v1/articles/{id}
```

用途：

- 拉取单篇文章详情
- 供总结、引用、回答前上下文补充

关键点：

- 使用 `id` 定位文章
- 返回完整文章内容（包括 HTML 渲染后的正文）

### 3.6 搜索

```http
GET /agent/v1/search?q=关键词&type=article&page=1&per_page=20
```

用途：

- 关键词搜索文章和出版物
- 支持多类型搜索

查询参数：

- `q` (必填): 搜索关键词
- `type`: 搜索类型 (`article`, `publication`, `all`)，默认 `all`
- `page`: 页码，默认 1
- `per_page`: 每页数量，默认 20，最大 100
- `publication_id`: 按出版物筛选
- `author_id`: 按作者筛选
- `tag`: 按标签筛选

### 3.7 评论列表

```http
GET /agent/v1/comments?article_id=xxx&page=1&per_page=20
```

用途：

- 列出文章下的评论

查询参数：

- `article_id` (必填): 文章 ID
- `page`: 页码，默认 1
- `per_page`: 每页数量，默认 20

### 3.8 发表评论（需要认证）

```http
POST /agent/v1/comments
Authorization: Bearer <JWT>
Content-Type: application/json

{
  "article_id": "文章ID",
  "content": "评论内容",
  "parent_id": "父评论ID（可选，用于回复）"
}
```

用途：

- 在文章下发表评论
- 可以回复其他评论

所需 Scope：`blog:comment:write`

## 4. OpenClaw 侧推荐接法

推荐把 SoulBlog 当成一个 **可读可写的博客内容源**。

推荐结构：

```text
OpenClaw
  -> SoulBlog adapter / tool definitions
    -> /agent/v1
```

这一层 adapter 的职责：

1. 固定允许调用的 capability 对应路径
2. 注入 JWT token（需要时）
3. 控制返回字段与正文长度
4. 统一错误翻译
5. 补审计日志

## 5. 建议给 OpenClaw 的工具集合

如果只保留最小能力面，建议 OpenClaw 侧工具名收敛为下面 8 个：

1. `soulblog_health` - 健康检查
2. `soulblog_list_publications` - 列出出版物
3. `soulblog_get_publication` - 获取出版物详情
4. `soulblog_list_articles` - 列出文章
5. `soulblog_get_article` - 获取文章详情
6. `soulblog_search` - 搜索内容
7. `soulblog_list_comments` - 列出评论
8. `soulblog_create_comment` - 发表评论

推荐映射如下：

| Tool | Method | `/agent/v1` path | 认证 | Scope |
|---|---|---|---|---|
| `soulblog_health` | GET | `/system/health` | 否 | - |
| `soulblog_list_publications` | GET | `/publications` | 否 | - |
| `soulblog_get_publication` | GET | `/publications/{id}` | 否 | - |
| `soulblog_list_articles` | GET | `/articles` | 否 | - |
| `soulblog_get_article` | GET | `/articles/{id}` | 否 | - |
| `soulblog_search` | GET | `/search?q=...` | 可选 | `blog:search:read` |
| `soulblog_list_comments` | GET | `/comments?article_id=...` | 否 | - |
| `soulblog_create_comment` | POST | `/comments` | 是 | `blog:comment:write` |

## 6. 最小能力面与 notIncluded

当前主线下，最重要的是边界清楚。

### 6.1 included

当前 `/agent/v1` 包含：

- 健康检查
- 出版物发现与读取
- 文章发现与读取
- 搜索
- 评论读取
- 发表评论（认证）

### 6.2 notIncluded

下面这些都 **不属于** 当前 OpenClaw 主线：

- 创建/编辑/删除文章
- 创建/编辑/删除出版物
- 管理出版物成员
- 用户管理
- 支付相关操作
- 订阅管理
- 统计分析
- 文件上传

这部分边界要写死，不要在适配层里"顺手多接一点"。

## 7. 风险边界

### 7.1 内容安全

文章内容中可能出现：

- 恶意 prompt
- 误导性信息
- 过期内容

这些都应被当作 **待判断的内容**，不是高优先级系统命令。

### 7.2 正文读取要防止过量注入

单篇文章正文可能很长，也可能夹带脏内容。

建议：

- 默认截断
- 大文档分段读取或摘要化
- 输出回答时优先引用而不是整段转储

### 7.3 评论内容审核

发表评论时：

- 应经过内容审核
- 避免垃圾评论
- 避免恶意链接

## 8. 认证与权限建议

### 8.1 用独立服务账号

不要直接拿管理员 token 对接 OpenClaw。

推荐：

- 独立 service account
- 只赋予必要的 scope
- 按出版物做 allowlist / 白名单

### 8.2 最小权限原则

推荐的 JWT permissions：

```json
[
  "blog:publication:read",
  "blog:article:read",
  "blog:search:read",
  "blog:comment:read",
  "blog:comment:write"
]
```

### 8.3 审计最少记录这些信息

- session / channel / requester
- tool 名称
- 上游 path
- article_id / publication_id
- 是否读取全文
- HTTP 状态码
- request_id

## 9. 联调建议

推荐按这个顺序联调：

1. `GET /agent/v1/system/health` - 确认服务正常
2. `GET /agent/v1/publications` - 测试公开读取
3. `GET /agent/v1/articles` - 测试文章列表
4. `GET /agent/v1/articles/{id}` - 测试单文章读取
5. `GET /agent/v1/search?q=xxx` - 测试搜索
6. `POST /agent/v1/comments` - 测试认证写入（需要 JWT）

## 10. 相关文件

- `docs/agent_api_v1.md` - Agent API 详细文档
- `docs/examples/openclaw_tool_catalog.json` - OpenClaw 工具目录
- `src/agent/router.rs` - Agent 路由实现
- `src/agent/handlers.rs` - Agent 处理器实现
- `src/agent/auth.rs` - Agent 认证与权限
- `src/agent/response.rs` - 统一响应格式

## 11. 一句话版本

**从 OpenClaw 调 SoulBlog，直接走 `/agent/v1/*` + Bearer JWT；仓库已具备最小 agent contract，支持读取文章、发表评论等核心能力。**
