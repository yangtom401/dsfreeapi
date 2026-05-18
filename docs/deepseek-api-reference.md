# DeepSeek 端点详情

### Base URLs
- `https://chat.deepseek.com/api/v0` — 所有 API 端点
- `https://fe-static.deepseek.com` — WASM 文件下载

### 统一 Headers
- 所有请求: `User-Agent` 必填（WAF 绕过）
- 鉴权请求: `Authorization: Bearer <token>`
- PoW 请求: `X-Ds-Pow-Response: <base64>`
- **`X-Client-Version: 2.0.0`** — 本文档对应的客户端版本（如发现行为不一致，请先检查此版本号）

### 错误响应格式
- **字段缺失** → HTTP 422: `{"detail":[{"loc":"body.<field>"}]}`
- **Token 无效** → HTTP 200: `{"code":40003,"msg":"Authorization Failed (invalid token)","data":null}`
- **业务错误** → HTTP 200: `{"code":0,"data":{"biz_code":<N>,"biz_msg":"<msg>","biz_data":null}}`
- **登录失败** → HTTP 200: `{"code":0,"data":{"biz_code":2,"biz_msg":"PASSWORD_OR_USER_NAME_IS_WRONG"}}`

### PoW target_path 映射
| 端点 | target_path |
|------|-------------|
| completion | `/api/v0/chat/completion` |
| edit_message | `/api/v0/chat/edit_message` |
| upload_file | `/api/v0/file/upload_file` |



## 0. login
- url: https://chat.deepseek.com/api/v0/users/login
- Request Header:
  - `User-Agent`: 必填（WAF 绕过，值需像真实浏览器 UA）
  - `Content-Type: application/json`: 可选（HTTP 库自动设置时不需要）
- Request Payload:
```json
{
  "email": null,
  "mobile": "[phone_number]",
  "password": "<password>",
  "area_code": "+86",
  "device_id": "[任意base64或空字符串，但字段不能省略]",
  "os": "web"
}
```
  - `email` / `mobile`: 二选一，另一个传 null
  - `device_id`: 必填字段（省略 → 422），但值可为空或随机
  - `os`: 必填（省略 → 422），固定 `"web"`
- Response:
```json
{
    "code": 0,
    "msg": "",
    "data": {
        "biz_code": 0,
        "biz_msg": "",
        "biz_data": {
            "code": 0,
            "msg": "",
            "user": {
                "id": "test",
                "token": "api-token",
                "email": "te****t@mails.tsinghua.edu.cn",
                "mobile_number": "999******99",
                "area_code": "+86",
                "status": 0,
                "id_profile": {
                    "provider": "WECHAT",
                    "id": "test",
                    "name": "test",
                    "picture": "https://static.deepseek.com/user-avatar/test",
                    "locale": "zh_CN",
                    "email": null
                },
                "id_profiles": [
                    {
                        "provider": "WECHAT",
                        "id": "test",
                        "name": "test",
                        "picture": "https://static.deepseek.com/user-avatar/test",
                        "locale": "zh_CN",
                        "email": null
                    }
                ],
                "chat": {
                    "is_muted": 0,
                    "mute_until": null
                },
                "has_legacy_chat_history": false,
                "need_birthday": false
            }
        }
    }
}
```
- 关键字段: `data.biz_data.user.token`（后续所有请求的 Bearer token）
- 错误响应: `biz_code=2, biz_msg="PASSWORD_OR_USER_NAME_IS_WRONG"`



## 1. create
- url: https://chat.deepseek.com/api/v0/chat_session/create
- Request Header:
  - `Authorization: Bearer <token>`
  - `User-Agent`: 必填（统一保留，WAF 绕过）
- Request Payload: `{}`
- Response:
```json
{
    "code": 0,
    "msg": "",
    "data": {
        "biz_code": 0,
        "biz_msg": "",
        "biz_data": {
            "chat_session": {
                "id": "e6795fb3-272f-4782-87cf-6d6140b5bf76",
                "seq_id": 197895830,
                "agent": "chat",
                "model_type": "default",
                "title": null,
                "title_type": "WIP",
                "version": 0,
                "current_message_id": null,
                "pinned": false,
                "inserted_at": 1775732630.005,
                "updated_at": 1775732630.005
            },
            "ttl_seconds": 259200
        }
    }
}
```
- 关键字段: `data.biz_data.chat_session.id`（后续 completion 用的 `chat_session_id`）
- 注意: `chat_session` 内嵌对象包含完整 session 信息


## 2. get_wasm_file
- url: https://fe-static.deepseek.com/chat/static/sha3_wasm_bg.7b9ca65ddd.wasm
- Request Header: 无需鉴权，无需 User-Agent，直链 GET 即可
- Request Payload: GET 操作，无
- Response: 26612 bytes，`Content-Type: application/wasm`，标准 WASM 格式（`\x00asm` magic number）
- 注意: URL 中的 hash 部分 `7b9ca65ddd` 可能会变，建议可配置



## 3. create_pow_challenge
- url: https://chat.deepseek.com/api/v0/chat/create_pow_challenge
- Request Header:
  - `Authorization: Bearer <token>`
  - `User-Agent`: 必填（WAF 绕过，省略 → 429）
- Request Payload: `{"target_path": "/api/v0/chat/completion"}`
- Response:
```json
{
    "code": 0,
    "msg": "",
    "data": {
        "biz_code": 0,
        "biz_msg": "",
        "biz_data": {
            "challenge": {
                "algorithm": "DeepSeekHashV1",
                "challenge": "7ffc9d19b6eed96a6fca68f8ffe30ee61035d4959e4180f187bf85b356016a96",
                "salt": "3bde54628ea8413fee87",
                "signature": "ce4678cf7a1290c2a7ac88c4195a5b8497e5fc4b0e8044e804f5a6f3af6fe462",
                "difficulty": 144000,
                "expire_at": 1775380966945,
                "expire_after": 300000,
                "target_path": "/api/v0/chat/completion"
            }
        }
    }
}
```
- 关键字段: `challenge`（哈希输入前缀）、`salt`（拼接用）、`difficulty`（目标阈值）、`expire_at`（过期时间戳 ms）
- `algorithm`: 固定 `"DeepSeekHashV1"`
- `expire_after`: 300000ms = 5 分钟有效期



## 4. completion
- url: https://chat.deepseek.com/api/v0/chat/completion
- Request Header:
  - `Authorization: Bearer <token>`
  - `User-Agent`: 必填
  - `X-Ds-Pow-Response`: 必填（base64 编码的 PoW 响应，**每次请求必须重新计算**）
- Request Payload:
```json
{
    "chat_session_id": "<来自 create 端点的 id>",
    "parent_message_id": null,
    "model_type": "default",
    "prompt": "你好",
    "ref_file_ids": ["file-xxx"],
    "thinking_enabled": true,
    "search_enabled": true,
    "preempt": false
}
```
- `model_type`: `"expert"` (默认) | `"default"` | 等
- `ref_file_ids`: 上传文件后返回的文件 ID 数组，会话级别记忆，后续 `edit_message` 无需重复传入
- `preempt`: 预占模式（目前网页端未使用），默认 false
- Response: `text/event-stream` SSE 流

### SSE 事件格式

**1. `ready` — 会话就绪**
```
event: ready
data: {"request_message_id":1,"response_message_id":2,"model_type":"expert"}
```

**注意**: `ready` 后通常紧跟一个 `event: update_session`，这是正常的会话更新时间，不要误认为流结束。

**2. `update_session` — 会话更新**
```
event: update_session
data: {"updated_at":1775386361.526172}
```

**3. 增量内容 — 操作符格式**

所有增量更新使用统一的数据格式，通过 `"p"`（路径）和 `"o"`（操作符）组合：

- **`"p"` 路径 + `"v"` 值**（SET 操作，替换字段值）:
  ```
  data: {"p":"response/status","v":"FINISHED"}
  ```

- **`"p"` + `"o":"APPEND"` + `"v"` 值**（追加到字段）:
  ```
  data: {"p":"response/fragments/-1/content","o":"APPEND","v":"，"}
  ```

- **`"p"` + `"o":"SET"` + `"v"` 值**（显式设置字段值，用于数值）:
  ```
  data: {"p":"response/fragments/-1/elapsed_secs","o":"SET","v":0.95}
  ```

- **`"p"` + `"o":"BATCH"` + `"v"` 数组**（批量更新多个字段）:
  ```
  data: {"p":"response","o":"BATCH","v":[{"p":"accumulated_token_usage","v":41},{"p":"quasi_status","v":"FINISHED"}]}
  ```

- **纯 `"v"` 值**（继续追加到上一 `"p"` 路径）:
  ```
  data: {"v":"用户"}
  ```

- **完整 JSON 对象**（初始状态快照，无 `"p"`）:
  ```
  data: {"v":{"response":{"message_id":2,"parent_id":1,"model":"","role":"ASSISTANT","fragments":[{"id":2,"type":"RESPONSE","content":"Hello"}]}}}
  ```

### Delta 解析算法

来自 `chat.deepseek.com` 前端 JS 源码，完整的 delta 解析逻辑用于处理所有增量更新事件：

```javascript
class DeltaParser {
    constructor() {
        this.op = "SET";   // 默认操作符
        this.path = "";    // 默认路径
    }

    parse(event) {
        // path/op 跨事件持久化：后续事件可省略 p/o 字段
        let op  = this.op  = event.o ?? this.op;
        let path = this.path = event.p ?? this.path;

        // 非 BATCH：返回单条操作
        if (op !== "BATCH")
            return [{ path, op, value: event.v }];

        // BATCH：分解数组中的每一项
        let subParser = new DeltaParser;
        let results = [];
        for (let item of event.v) {
            let sub = subParser.parse(item);
            // 父路径前置：子项的 p 是相对于父路径的
            for (let s of sub)
                s.path = (path ? path + "/" : "") + s.path;
            results.push(...sub);
        }
        return results;
    }
}
```

**关键规则：**

| 规则 | 说明 |
|------|------|
| **`p` 和 `o` 跨事件持久化** | 后续事件可省略 `p`/`o`，沿用上一事件的值（bare `v` 和 bare `v` 数组均适用） |
| **`o` 默认值为 `"SET"`** | 无 `o` 字段的事件（如初始 snapshot）使用 SET 语义替换空路径 |
| **`APPEND` 对字符串 = `+=`** | 纯增量追加，不存在 snapshot 替换语义 |
| **`BATCH` 递归分解** | 创建子解析器递归处理 `v` 数组，子项的 `p` 前置父路径 |
| **操作类型只有 3 种** | `SET`（替换）、`APPEND`（追加）、`BATCH`（批量），无其他操作符 |

**这一算法对应的状态更新引擎（`rm` 类）：**

```javascript
switch (op) {
case "SET":
    target[resolvePath(lastPart)] = value;    // 直接赋值
    break;
case "APPEND":
    if (typeof value === "string")
        target[resolvePath(lastPart)] += value;  // 字符串拼接
    else if (Array.isArray(value))
        // 数组合并（push 或 splice 到负索引位置）
    break;
}
```

**对解析器实现的指导：**
1. 维护 `current_path` 和 `current_op` 状态变量，跨事件持久化
2. BATCH 事件：递归分解数组项，子项路径前置父路径
3. `response/fragments/-1` 级别的 BATCH：子项更新 fragment 子字段（content、references 等），路径前置为 `response/fragments/-1/{sub_path}`
4. 内容字段只做 `+=`，不需要 snapshot 检测或重复抑制

### SSE 流状态路径（`response/` 下的动态字段）

**重要：内容通过 `fragments` 数组组织，使用 `-1` 索引访问最后一个 fragment**

| 路径 / 字段 | 说明 |
|------|------|
| `response/fragments/-1/content` | 最后一个 fragment 的内容（APPEND 或 SET）|
| `response/fragments/-1/elapsed_secs` | 思考/搜索耗时（秒），仅 THINK 类型 |
| `response/fragments/-1/status` | fragment 状态（`WIP` → `FINISHED`），用于 TOOL_SEARCH/TOOL_OPEN |
| `response/fragments/-{n}/status` | 负索引标记任意 fragment 完成（批量并行标记）|
| `response/conversation_mode` | 会话模式：`"DEFAULT"` 或 `"DEEP_SEARCH"` |
| `response/has_pending_fragment` | `bool`，后台有 fragment 正在处理时 true |
| `response/search_status` | `"SEARCHING"` → `"FINISHED"` |
| `response/search_results` | 搜索结果数组 `{url, title, snippet}` |
| `response/accumulated_token_usage` | token 用量累计 |
| `response/quasi_status` | BATCH 内的结束信号，值：`"FINISHED"` 或 `"INCOMPLETE"` |
| `response/status` | 主状态 `WIP` → `FINISHED` 或 `INCOMPLETE` |

### Fragment 结构

```typescript
{
  id: number,                    // fragment 序号
  type: "THINK" | "RESPONSE"     // 基本类型
      | "TOOL_SEARCH"            // 搜索查询（含 queries + results 子字段）
      | "TOOL_OPEN"              // 打开链接（含 result + reference 子字段）
      | "TIP",                   // 提示条（含 style + hide_on_wip 子字段）
  content: string | null,        // 文本内容（TOOL_SEARCH/TOOL_OPEN 为 null）
  elapsed_secs?: number,         // THINK 类型：思考耗时（秒）
  status?: "WIP" | "FINISHED",   // TOOL_SEARCH/TOOL_OPEN 的完成状态
  queries?: Array<{ query: string }>,  // TOOL_SEARCH：多个搜索词
  results?: Array<{              // TOOL_SEARCH：搜索结果列表
    url: string,
    title: string,
    snippet: string,
    published_at?: number,
    site_icon?: string,
    site_name?: string,
    query_indexes?: number[],
  }>,
  result?: {                     // TOOL_OPEN：单个链接的内容
    url: string,
    title: string,
    snippet: string,
    published_at?: number,
    site_icon?: string,
    site_name?: string,
    query_indexes?: number[],
  },
  reference?: {                  // TOOL_OPEN：关联的搜索
    id: number,
    type: "TOOL_SEARCH",
  },
  style?: "WARNING",             // TIP 类型：样式
  hide_on_wip?: boolean,          // TIP 类型：流式进行中时隐藏
  references?: Array<{           // RESPONSE 内容中的引用关联
    id: number,
    type: "TOOL_SEARCH" | "TOOL_OPEN",
  }>,
  stage_id: number               // 阶段 ID
}
```

### 内容引用标记

在 RESPONSE 类型 fragment 的 content 中，DeepSeek 使用 `[reference:N]` 标记引用搜索结果或打开的链接。引用标记通过 fragment 级别的 BATCH 操作注入：

```
# 对当前 fragment 同时追加引用标记和设置 references 字段
data: {"p":"response/fragments/-1","o":"BATCH","v":[
  {"p":"content","o":"APPEND","v":"[reference:0]"},
  {"p":"references","o":"SET","v":[{"id":3,"type":"TOOL_SEARCH"}]}
]}

# 延续上一路径的 BATCH 操作（bare v 数组）
data: {"v":[{"p":"content","o":"APPEND","v":"[reference:1]"},{"p":"references","v":[{"id":5,"type":"TOOL_OPEN"}]}]}
```

### 思考内容 vs 实际输出的区分方法

**核心规则：通过 `fragments[].type` 字段区分**

```
type == "THINK"     → 思考内容（仅 thinking=ON 时出现）
type == "RESPONSE"  → 实际输出内容
```

**流的阶段顺序（thinking=ON, search=ON，含搜索和 fetch）：**

> search 相关步骤仅在 DeepSeek 判定需要联网搜索时才会出现。DEEP_SEARCH 模式下会触发多轮 THINK → TOOL_SEARCH → TOOL_OPEN → THINK 循环。

```
 1. SNAPSHOT    → {"v":{"response":{..., "fragments":[{"type":"THINK","content":""}]}}}
 2. THINKING    → p=response/fragments/-1/content, o="APPEND", v="..."
 3. (可选)      → p=response/conversation_mode, v="DEEP_SEARCH"
 4. THINK END   → p=response/fragments/-1/elapsed_secs, o="SET", v=...
 5. TOOL_SEARCH → p=response, o="BATCH", v=[{"p":"fragments","o":"APPEND","v":[
                    {"id":N,"type":"TOOL_SEARCH","queries":[...]}]},...]
 6. SEARCH      → p=response/fragments/-1/results, o="SET", v=[...] (大量结果)
 7. SEARCH END  → p=response/fragments/-1/status, v="FINISHED"
 8. THINK(2)    → BATCH APPEND 新 THINK fragment（评估搜索结果）
 9. THINKING(2) → p=response/fragments/-1/content, o="APPEND", v="..."
10. THINK END(2)→ p=response/fragments/-1/elapsed_secs, o="SET", v=...
11. TOOL_OPEN   → BATCH APPEND 多个 TOOL_OPEN fragment（批量打开链接）
12. OPEN END    → p=response/fragments/-{n}/status, v="FINISHED"（批量标记）
13. THINK(3)    → BATCH APPEND 新 THINK fragment（整理信息准备回答）
14. THINKING(3) → p=response/fragments/-1/content, o="APPEND", v="..."
15. THINK END(3)→ p=response/fragments/-1/elapsed_secs, o="SET", v=...
16. RESPONSE    → p=response/fragments, o="APPEND", v=[{"type":"RESPONSE","content":"..."}]
17. CONTENT     → p=response/fragments/-1/content, v="..." (继续追加，可能不带 o)
18. REFERENCE   → p=response/fragments/-1, o="BATCH", v=[{注入 reference}, {设置 references}]
19. CONTENT     → p=response/fragments/-1/content, o="APPEND", v="..." (继续追加)
20. TIP         → p=response/fragments, v=[{"type":"TIP","style":"WARNING",...}]
21. BATCH       → p=response, o="BATCH", v=[{accumulated_token_usage},{quasi_status}]
22. DONE        → p=response/status, o="SET", v="FINISHED"（正常完成）
                  → p=response/status, o="SET", v="INCOMPLETE"（手动中断）
```

> 注意：TOOL_OPEN 的结束通过批量负索引 status 更新来标记（如 `fragments/-7/status`），而非逐个等待。
> `event: finish` 通常不出现。`update_session`、`title`、`close` 在 status=FINISHED 后出现。

**流的阶段顺序（thinking=OFF, search=OFF）：**

```
1. SNAPSHOT    → {"v":{"response":{..., "fragments":[{"type":"RESPONSE","content":""}]}}}
2. CONTENT     → p=response/fragments/-1/content, o="APPEND", v="..."
3. BATCH       → p=response, o="BATCH", v=[{accumulated_token_usage},{quasi_status}]
4. DONE        → p=response/status, o="SET", v="FINISHED"
```

**实现要点：** BATCH 事件递归分解后得到若干 SET/APPEND 操作，每条操作在消息对象上执行路径解析并更新对应字段。内容类字段始终使用 APPEND（`+=`）。

**4. `hint` — 服务端提示/错误（必处理）**
```
event: hint
data: {"type":"error","content":"Content is too long. Please shorten it and try again.","clear_response":true,"finish_reason":"input_exceeds_limit"}
```
- `type`: `"error"` 表示错误提示，其他值（如 `"info"`）可忽略
- `content`: 人类可读的提示信息
- `clear_response`: 为 true 时表示已有输出应被清除
- `finish_reason`: `"input_exceeds_limit"`（输入超长）、`"rate_limit_reached"`（限流）等

**注意**: hint 事件通常出现在 `ready` 后不久。流处理器应在收到 hint 后主动终止流并返回错误（如 `Overloaded` 或 `BadRequest`），而非继续等待后续事件。`ready` 和 `hint` 之间可能有 `update_session` 事件。

### 流结束序列（含中断）

终态有两种：`FINISHED`（正常完成）和 `INCOMPLETE`（手动中断/异常终止）。序列如下：

```
# 正常完成
data: {"p":"response","o":"BATCH","v":[{"p":"accumulated_token_usage","v":139},{"p":"quasi_status","v":"FINISHED"}]}
data: {"p":"response/status","o":"SET","v":"FINISHED"}

# 手动中断（可能无 RESPONSE fragment）
data: {"p":"response","o":"BATCH","v":[{"p":"accumulated_token_usage","v":39},{"p":"quasi_status","v":"INCOMPLETE"}]}
data: {"p":"response/status","v":"INCOMPLETE"}                         # 无 o 字段
# elapsed_secs SET 可能在 INCOMPLETE 之后到达

# 后续序列（两者相同）
event: update_session
data: {"updated_at":1778639258.866693}

event: title
data: {"content":"Rust所有权概念解释"}

event: close
data: {"click_behavior":"none","auto_resume":false}
```

- `close`: 会话语义结束信号。`click_behavior` 控制点击行为（`"none"` 或 `"retry"`），`auto_resume` 表示是否可自动恢复
- `title`: 自动生成会话标题，不依赖 thinking/search 开关

**最可靠的结束信号是 `response/status` 变为 `FINISHED` 或 `INCOMPLETE`。** `event: finish` 可能不出现；`title` 和 `close` 也可能不出现。`update_session` 事件可能在流中多次出现。不要依赖事件顺序来判定结束。



## 5. edit_message
- url: https://chat.deepseek.com/api/v0/chat/edit_message
- Request Header:
  - `Authorization: Bearer <token>`
  - `User-Agent`: 必填
  - `X-Ds-Pow-Response`: 必填（每次请求重新计算）
- Request Payload:
```json
{
    "chat_session_id": "<session_id>",
    "message_id": 1,
    "prompt": "test again",
    "search_enabled": true,
    "thinking_enabled": true
}
```
- **注意**: `model_type` 和 `ref_file_ids` 不在 edit_message payload 中——二者在首次 completion 时传入后由 session 级别记忆，后续 edit_message 继承
- `message_id`: 必须已存在（空 session 的 `message_id=1` 会返回 `biz_code=26, "invalid message id"`）
- 编辑后生成新的 `message_id`，需从 SSE `ready` 事件中获取 `response_message_id` 字段用于后续 `stop_stream`
- 实际抓包确认：首次 `edit_message(message_id=1)` 的 `response_message_id=4`（而非 2），后续对话按 `1→4, 3→6, 5→8...` 递增
- Response: 同 `completion`（SSE 流）


## 6. stop_stream
- url: https://chat.deepseek.com/api/v0/chat/stop_stream
- Request Header:
  - `Authorization: Bearer <token>`
  - `User-Agent`: 必填（WAF 绕过）
- Request Payload:
```json
{
    "chat_session_id": "57bf7fb1-5fde-4d21-a08e-5dfa017216d5",
    "message_id": 2
}
```
  - `chat_session_id`: 来自 create 端点的 session ID
  - `message_id`: 要取消的响应消息 ID。编辑请求的 `message_id=1` 对应响应 `message_id=2`，所以停止流固定传 `2`。
- Response:
```json
{"code":0,"msg":"","data":{"biz_code":0,"biz_msg":"","biz_data":null}}
```
- 作用: 取消正在进行的流式输出。客户端断开连接后调用此端点可让 DeepSeek 侧停止继续生成，避免浪费资源。
- 注意: 不需要 PoW header

## 7. delete
- url: https://chat.deepseek.com/api/v0/chat_session/delete
- Request Header:
  - `Authorization: Bearer <token>`
  - `User-Agent`: 必填
- Request Payload: `{"chat_session_id": "<session_id>"}`
- Response:
```json
{"code":0,"msg":"","data":{"biz_code":0,"biz_msg":"","biz_data":null}}
```


## 8. update_title
- url: https://chat.deepseek.com/api/v0/chat_session/update_title
- Request Header:
  - `Authorization: Bearer <token>`
  - `User-Agent`: 必填
- Request Payload:
```json
{
    "chat_session_id": "<session_id>",
    "title": "test"
}
```
- Response:
```json
{
    "code": 0,
    "msg": "",
    "data": {
        "biz_code": 0,
        "biz_msg": "",
        "biz_data": {
            "chat_session_updated_at": 1775382827.122839,
            "title": "test"
        }
    }
}
```
- 错误码: `biz_code=5` → `EMPTY_CHAT_SESSION`（空 session 无法设置标题）；`biz_code=1` → `ILLEGAL_CHAT_SESSION_ID`



## 9. upload_file
- url: https://chat.deepseek.com/api/v0/file/upload_file
- Request Header:
  - `Authorization: Bearer <token>`
  - `User-Agent`: 必填
  - `X-Ds-Pow-Response`: 必填（target_path 为 `/api/v0/file/upload_file`）
- Request Payload: `multipart/form-data`，字段名 `file`
```
Content-Disposition: form-data; name="file"; filename="test.txt"
Content-Type: text/plain
```
- Response:
```json
{
    "code": 0,
    "msg": "",
    "data": {
        "biz_code": 0,
        "biz_msg": "",
        "biz_data": {
            "id": "file-4387ddbe-efed-4459-83b0-ebb89db61f0f",
            "status": "PENDING",
            "file_name": "控制工程基础习题解= Introduction to control engineering solution to problems exercises ( 第4版 ).pdf",
            "from_share": false,
            "file_size": 36978670,
            "model_kind": "NORMAL",
            "token_usage": null,
            "error_code": null,
            "inserted_at": 1778644590.853,
            "updated_at": 1778644590.853,
            "is_image": false,
            "audit_result": null
        }
    }
}
```
- 关键字段: `data.biz_data.id`（后续 completion 的 `ref_file_ids` 使用）
- 上传后 `status` 为 `PENDING`，需轮询 `fetch_files` 直到 `status=SUCCESS`
- 状态流转: `PENDING` → `PARSING` → `SUCCESS`（或 `FAILED`）



## 10. fetch_files?file_ids=<id>
- url: https://chat.deepseek.com/api/v0/file/fetch_files?file_ids=<id>
- Request Header:
  - `Authorization: Bearer <token>`
  - `User-Agent`: 必填
- Request Payload: 无，GET 操作
- Response（多个可能的状态阶段）:
```json
# 解析中
{
    "code": 0,
    "msg": "",
    "data": {
        "biz_code": 0,
        "biz_msg": "",
        "biz_data": {
            "files": [
                {
                    "id": "file-4387ddbe-efed-4459-83b0-ebb89db61f0f",
                    "status": "PARSING",
                    "file_name": "控制工程基础习题解= Introduction to control engineering solution to problems exercises ( 第4版 ).pdf",
                    "from_share": false,
                    "file_size": 36978670,
                    "model_kind": "NORMAL",
                    "token_usage": null,
                    "error_code": null,
                    "inserted_at": 1778644590.853,
                    "updated_at": 1778644591.307,
                    "is_image": false,
                    "audit_result": null
                }
            ]
        }
    }
}

# 完成
{
    "code": 0,
    "msg": "",
    "data": {
        "biz_code": 0,
        "biz_msg": "",
        "biz_data": {
            "files": [
                {
                    "id": "file-eb550231-5bc4-4cb1-a8d2-49f8fed82247",
                    "status": "SUCCESS",
                    "file_name": "main.js",
                    "from_share": false,
                    "file_size": 2836902,
                    "model_kind": "NORMAL",
                    "token_usage": 619907,
                    "error_code": null,
                    "inserted_at": 1778644547.106,
                    "updated_at": 1778644547.106,
                    "signed_path": "/file?file_id=...",
                    "is_image": false,
                    "audit_result": null
                }
            ]
        }
    }
}
```
- 关键字段: `files[].status` → `SUCCESS` 表示上传完成
- 状态流转: `PENDING` → `PARSING` → `SUCCESS`（前端对 `PENDING` 和 `PARSING` 都视为"处理中"）
- `model_kind`: 文件处理模型类型，`"NORMAL"`（普通文本/PDF）或 `"VISION"`（图片视觉）
- `is_image`: 是否图片文件
- `audit_result`: 审核结果，图片可能为 `"unknown"`（初始）→ `"pass"`（通过）
- `width` / `height`: 图片文件在 SUCCESS 后可能出现，表示图片像素尺寸
- `token_usage`: 文件解析消耗的 token 数（SUCCESS 后才有值）
- `signed_path`: SUCCESS 后可能出现，文件下载路径
