# NoteLume AI → LogLume 移植：盘点 + 方案

## Context

用户要把 NoteLume 的 AI 能力移植到 LogLume（日志 / 报文工具），但要求先拆清楚、落到日志场景、凭据与出网边界明确，再在独立分支实施。本文档先给出 NoteLume 的完整盘点（第一部分报告），再给可执行方案与分块验证。

约束（用户原话要点）：
- 不硬编码任何 key；不悄悄外发用户内容。
- 未配 key / 未启用时功能**显式不可用并给引导**，不能静默失败。
- 默认关闭，用户显式开启后才联网。
- 场景必须落到“选中日志行解释、错误模式归纳、按查询条件总结”，不是纯聊天框。
- 独立分支，先不合 main；每完成一块各报一次，没做/跑不通/只有 UI 没后端的必须点明。

---

# 第一部分：NoteLume AI 盘点（证据）

源码：`C:\Users\86153\AppData\Local\Temp\NoteLume-fresh\NoteLume-master`

## 1. 功能清单与入口

| 入口 | 位置 | 说明 |
| --- | --- | --- |
| 侧栏「AI」页签 | `src/main.js:528`、`src/main.js:4007`（`Mod-Shift-a`） | `createAiPanel`（`src/ai-panel.js`）挂到 `#ai-panel` |
| 选区 AI | `src/main.js:1119-1157`、`src/main.js:2210`、`src/main.js:4591` | 右键 / 命令面板；任务 `polish / continue / translate / summarize / custom`（`src/ai-tasks.js:10`） |
| AI 设置 | `src/main.js:1099/1124/5351` | `createAiSettings`（`src/ai-settings.js`） |
| 语音输入 | `src/ai-panel.js` 麦克风 | Rust 录音 + 服务商 ASR / Windows 原生听写（`src-tauri/src/voice.rs`）——**本次不移植** |

功能：
- 侧栏对话：每轮发送「当前文档上下文（6000 字，`ai-tasks.js:63-76`）+ 全库检索片段（5 篇 / 6000 字 / 单条 400 字，`ai-tasks.js:422-484`）+ 全部历史」。见 `ai-panel.js:429-500`。
- Ask / Agent 两模式。Agent 走 OpenAI function calling，只读工具 `search_vault / read_note / list_notes / read_current_doc / query_tag`，最多 8 轮（`src/ai-agent.js:13-70, 180-271`），回答可回退。
- 选区任务：流式浮层（`src/ai-inline.js:40-230`），完成后可替换选区 / 插入下方 / 复制。
- 附件图片（≤4 张）、技能注入、上下文用量圆环、模型胶囊切换。

流式与历史：
- 有流式。Rust 逐行解析 SSE，事件 `nl:ai-event`，`kind = chunk | reasoning | done | error`（`src-tauri/src/ai.rs:42-56, 876-903`）；`ai_chat_cancel(reqId)` 中止（`ai.rs:161-173`）。
- 历史只在内存（换文档不清，`ai-panel.js:111`）；可把会话存档到 `<vault>/.notelume-ai/chats/<id>.json`（`ai-panel.js:1436-1482`，Rust `ai_chat_save/read` 见 `src-tauri/src/lib.rs:3118-3129`）。无云端历史。

## 2. 模型与协议

- 只支持 **OpenAI 兼容** `POST {base}/chat/completions`；无 Anthropic / 原生 Gemini SDK。
- 预设：DeepSeek / 智谱 / Kimi / OpenAI / 本机 Ollama（`ai-tasks.js:260-266`）；可自定义 http(s) base url（上限 300 字符）。
- 请求体：`{model, messages, stream:true, temperature?, tools?}`（`ai.rs:772-795`）；`Authorization: Bearer <key>`，key 为空时不带（本机 Ollama）。
- 流式解析：按字节缓冲逐行取 `data:`；`choices[0].delta.content`（兼容忽略 stream 的 `message.content`）；`reasoning / reasoning_content / thinking` 单独推事件；`tool_calls` 按 index 累积；`data: [DONE]` 结束；流内 `{"error":...}` 直接报错（`ai.rs:583-633, 676-723`）。
- 错误体提取：`error.message`（字符串/对象）、顶层 `message`/`msg` + `code`，否则原文截断 200/300 字符（`ai.rs:637-674`）。
- 模型列表 `GET {base}/models`，8s 超时，过滤 embedding/rerank/tts 等非对话模型（`ai.rs:186-232`、`ai-tasks.js:543-561`）。
- 超时：对话整体 300s；**无重试、无 429 专门处理**（429 只是 HTTP 错误文本）。

## 3. 设置与凭据

- 配置写 `%APPDATA%\<identifier>\settings.json` 的 `ai` 段：`baseUrl / model / contextTokens / asrModel / toolsCap / keys`（`src-tauri/src/lib.rs:4364-4406`）。
- API Key **明文**存 `ai.keys`，按接口地址的 `host:port` 分格；切服务商互不影响（`ai-settings.js:55-94`、`ai-tasks.js:98-145`）。无系统钥匙串、无加密。
- Rust 不存配置；每次请求由前端把 key 传进 IPC（`ai.rs:28-40`、`tauri-bridge.js:604-628`）。
- 多 provider：预设 + 自定义地址 + 每 host 一份 key；不是并发多套配置。
- 失败呈现：对话气泡内错误行 / 浮层状态行 + 重试；未配置时文案 `ai.needConfig` 并打开设置；限流无专门文案。

## 4. 依赖

- 前端：`markdown-it`（消息渲染，`html:false` 防注入）、自有 i18n/icons；无 AI SDK、无 Node 代理。
- Rust：`reqwest 0.13`（`json / native-tls / multipart`，`src-tauri/Cargo.toml:48-51`）+ `tauri::async_runtime`；SSE 全部手写解析。
- 无独立 Node 侧代理；无 LangChain 等框架。

---

# 第二部分：移植方案

## 5. 能原样搬 vs 必须改造

**可原样搬（改命名/错误码）**
- SSE 解析与错误提取：`chat_url` / `models_url` / `sse_data_payload` / `extract_delta_content` / `extract_delta_reasoning` / `extract_error_body` / `truncate_utf8` / `build_chat_body`（`ai.rs`）。
- 流式事件驱动模型：reqId + 事件 + cancel。
- 设置面板的交互骨架（地址 / 模型 / Key / 测试连接 / 出机说明）。
- 预设与 `/models` 过滤、非对话模型提示。

**必须改造**
- 上下文：Markdown 文档 → 日志行集合。禁止整文件读入；按“选中行 / 查询命中 / 错误行采样”采集并强制截断。
- 历史：本轮只做**内存历史**（关闭面板即清），不写 vault；不搬会话存档。
- Agent / 工具循环：不搬（LogLume 无 vault / note 语义），本轮不做 function calling。
- 写回：不自动改日志文件。结果只预览、复制、另存为 `.md`。
- 凭据：Key 只进不出——`ai_config_get` 只回 `hasKey`，Key 永不回到前端；默认关闭。
- 选区：只读大文件视图当前没有行选择（`listview.ts:1-17` 明确“no cursor”），需补最小行选择，否则“选中行解释”只覆盖编辑模式。

## 6. 场景落点（三个真实动作）

1. **解释选中日志行**
   - 编辑模式：CodeMirror `state.selection.main` 的 `sliceDoc`。
   - 只读大文件：ListView 新增点击 / Shift+点击行选择（原行号区间），用 `docLinesAt` 取文本。
   - 提示词：日志行 + 前后各 3 行上下文；要求识别时间戳 / 级别 / 线程 / 错误码 / 可能原因 / 下一步排查。输出中文。
2. **错误模式归纳**
   - 用现有 `search_run` 扫错误关键词（默认 `error|warn|fail|exception|异常|错误|失败`，可改成用户输入），最多取 200 条命中；`doc_lines_at` 取文本。
   - 输出：错误类型分布、重复模式、时间/线程聚集、每类 1-2 条代表行、建议排查顺序。
   - UI 必须显示“基于 N 行（已截断 / 未截断）”。
3. **按查询条件总结**
   - 复用查询面板最后一次 Apply 的条件（`main.ts:3711-3784` 的 `conditions / caseSensitive / sel`）与命中行（`queryHits`），最多 200 行 + 40k 字符。
   - 输出：命中量、时间范围、级别/来源分布、典型样例、异常聚集点。

每次发送前在面板顶部显示出机提示：`将发送 N 行 / 约 X 字符 → host`；本机回环地址显示“内容不出本机”。

## 7. 凭据与默认关闭（硬约束）

- 配置存 `%APPDATA%\<identifier>\LumeLog\config.toml` 的 `[ai]` 段：
  ```toml
  [ai]
  enabled = false
  base_url = ""
  model = ""
  api_key = ""
  temperature = 0.2
  ```
- `[ai] enabled` 默认 `false`。所有出网命令先校验：未启用 → `[ai_disabled]`；无地址/模型 → `[ai_not_configured]`；未勾选“我了解内容会发往上述地址”不允许开启。
- Key 明文存本机 config.toml（用户要求盘点里说明；不伪称钥匙串）。设置页固定显示：“Key 明文保存在本机 config.toml，仅当前用户配置目录；不会发往你填写的接口地址以外的任何地方。”
- Key 不回传前端：`ai_config_get` 只给 `hasKey`；`ai_config_set` 的 `apiKey: null` 表示保持、`""` 表示清除。
- 前端没有任何默认 key / 示例 key；测试只用假 key。
- 失败显式：设置页“测试连接”显示 HTTP 状态与错误体；对话面板错误行 + 重试；401/403 → `ai_auth`，429 → `ai_rate_limited`，网络 → `ai_network`。

## 8. 新增 Rust 命令 / 前端模块 / 配置项

**Rust：新增 `app/src-tauri/src/ai.rs`**
- `ai_config_get(app) -> AiConfigView { enabled, baseUrl, model, temperature, hasKey, local }`
- `ai_config_set(app, { enabled, baseUrl, model, apiKey: Option<String>, temperature }) -> AiConfigView`
  - 校验 http(s)、长度上限；`enabled=true` 时必须有合法 baseUrl + model。
- `ai_models_list(app) -> Vec<String>`（用已存配置，8s 超时）
- `ai_chat_stream(app, reqId, messages, temperature?) -> Result<(), IpcError>`（从配置取 baseUrl/model/key；逐行 SSE；事件 `ll:ai-event`）
- `ai_chat_cancel(reqId) -> bool`
- 纯函数（可单测）：`read_ai_config` / `write_ai_config`（toml 字符串变换，保留其他段，参考 `keymap.rs:29-55`）、`chat_url` / `models_url` / `sse_data_payload` / `extract_delta_content` / `extract_delta_reasoning` / `extract_error_body` / `truncate_utf8` / `status_code`。
- `commands.rs`：`read_config_text` / `write_config_text` 改 `pub(crate)`；`lib.rs` 注册 5 个命令 + `mod ai`。
- `Cargo.toml`：`reqwest = { version = "0.13", default-features = false, features = ["json", "native-tls"] }`（Cargo.lock 已有 0.13.5）。

**前端**
- `src/ipc.ts`：新增 `AiConfigView`、`AiMessage`、`AiStreamEvent` 类型；`aiConfigGet/Set`、`aiModelsList`、`aiChatStream`、`aiChatCancel`、`aiEventListen`；新增错误码 `ai_disabled / ai_not_configured / ai_bad_url / ai_auth / ai_rate_limited / ai_http / ai_network` 及中文模板。
- `src/aitasks.ts`（纯逻辑，vitest）：三种场景的 messages 拼装、`MAX_LINES=200` / `MAX_CHARS=40000` 截断、`estimateChars`、出机提示文案、`stripFences`。
- `src/aisettings.ts`：设置弹层（开关 / 预设 / 地址 / 模型（含 `/models` 候选）/ Key（写入但不回显）/ 温度 / 测试连接 / 明文与出机说明）。参考 `keymap-ui.ts` 的模态结构。
- `src/aipanel.ts`：右侧 AI 面板（消息区 + 输入框 + 停止 + 复制 + 另存 .md + 三个场景按钮），`createAiPanel(deps)`。
- `src/main.ts`：工具栏 AI 按钮 + 命令面板条目（设置 / 对话 / 解释选中 / 归纳错误 / 总结查询）+ 快捷键；接入选区、查询状态、`docLinesAt`。
- `src/listview.ts`：只读视图最小行选择（click / Shift+click，`selectedLines()`），配套 CSS 与纯函数测试。
- `src/styles.css`：面板 / 设置 / 行选中样式，沿用现有 CSS 变量与暗色主题。

## 9. 分块实施与验证（独立分支 `feat/ai-assist`）

**Block 1：设置与凭据**
- Rust `ai.rs` 配置读写 + 校验 + 单测；`ai_config_get/set`；注册。
- 前端 `aisettings.ts` + ipc 包装 + 错误码；未启用时工具栏/命令面板条目仍在，但点开只显示“AI 未启用”与“去设置”。
- 验证：`cargo test --workspace`（配置读写/坏值/保留其他段）、`cargo check --workspace`、`npm test`、`npm run build`；真实窗口：默认关闭 → 开启被未勾选/无地址拦下 → 保存后重开窗口配置仍在 → Key 不回显。

**Block 2：基础对话 / 流式**
- Rust `ai_chat_stream / cancel / models_list` + SSE 单测。
- 前端 `aipanel.ts` 对话 + 设置里的“测试连接 / 拉模型”。
- 验证：本地 mock SSE 服务（临时脚本，非仓库内）覆盖增量、`[DONE]`、401、429、流内 error、取消；四项命令；真实窗口发一句话看增量与停止。**不接真实服务商，报告里写明。**

**Block 3：日志场景动作**
- `aitasks.ts` 三套提示词 + 截断；`listview.ts` 行选择；`main.ts` 三个入口与出机提示；错误扫描走 `search_run` + `doc_lines_at`；查询总结复用 `queryHits`。
- 验证：三套提示词的纯逻辑测试；真实窗口用示例日志走“选中行解释 / 错误归纳 / 查询总结”，断言发往 mock 的请求体行数与截断提示；`cargo test`、`npm test`、`npm run build`。

## 10. 不做 / 限制（报告里要明说）

- 不做语音输入、Agent 工具循环、技能扫描、会话存档、图片附件、Ollama 拉取。
- 不做真实服务商端到端验证（无可用 key）；只做本地 mock SSE。
- 只读大文件视图的行选择是最小实现（单行 / Shift 区间），不做多段选择、键盘扩展。
- 错误扫描最多 200 行 / 40k 字符；超出部分明确标注“已截断”，不声称全量分析。
- Key 为明文本机配置，不是系统钥匙串加密。
