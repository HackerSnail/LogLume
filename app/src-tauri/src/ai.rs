//! AI assist, block 1: the `[ai]` section of `config.toml` and its two IPC
//! commands. The API key never travels back to the front end — `AiConfigView`
//! only carries `hasKey`; streaming (block 2) will read the key from here.
//!
//! Everything below the commands is a pure string transform so the config
//! rules (defaults, validation, preserving other sections) are unit-testable.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

use crate::dto::IpcError;

/// Default sampling temperature. Logs reward low variance; the UI can change it.
pub const DEFAULT_TEMPERATURE: f64 = 0.2;
const BASE_URL_MAX: usize = 300;
const MODEL_MAX: usize = 120;
const KEY_MAX: usize = 4096;

/// Parsed `[ai]` section. `api_key` is the only secret and stays Rust-side.
#[derive(Debug, Clone, PartialEq)]
pub struct AiConfig {
    pub enabled: bool,
    pub base_url: String,
    pub model: String,
    pub api_key: String,
    pub temperature: f64,
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            base_url: String::new(),
            model: String::new(),
            api_key: String::new(),
            temperature: DEFAULT_TEMPERATURE,
        }
    }
}

/// What the front end is allowed to see. Deliberately no `apiKey`.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiConfigView {
    pub enabled: bool,
    pub base_url: String,
    pub model: String,
    pub temperature: f64,
    pub has_key: bool,
    /// True for loopback endpoints (Ollama etc.) — the UI says "内容不出本机".
    pub local: bool,
}

impl AiConfigView {
    pub fn from_config(c: &AiConfig) -> Self {
        Self {
            enabled: c.enabled,
            base_url: c.base_url.clone(),
            model: c.model.clone(),
            temperature: c.temperature,
            has_key: !c.api_key.is_empty(),
            local: is_loopback(&c.base_url),
        }
    }
}

/// Parse the `[ai]` table. Missing table / wrong types fall back to defaults
/// (the file is user-editable, so leniency beats refusing to boot).
pub fn read_ai_config(text: &str) -> AiConfig {
    let mut cfg = AiConfig::default();
    let Ok(root) = toml::from_str::<toml::Table>(text) else {
        return cfg;
    };
    let Some(table) = root.get("ai").and_then(|v| v.as_table()) else {
        return cfg;
    };
    if let Some(v) = table.get("enabled").and_then(|v| v.as_bool()) {
        cfg.enabled = v;
    }
    if let Some(v) = table.get("base_url").and_then(|v| v.as_str()) {
        cfg.base_url = v.trim().to_string();
    }
    if let Some(v) = table.get("model").and_then(|v| v.as_str()) {
        cfg.model = v.trim().to_string();
    }
    if let Some(v) = table.get("api_key").and_then(|v| v.as_str()) {
        cfg.api_key = v.to_string();
    }
    if let Some(v) = table.get("temperature").and_then(|v| v.as_float()) {
        cfg.temperature = v;
    }
    cfg
}

/// Rewrite `config.toml` with the given `[ai]` table, preserving every other
/// section (same approach as `keymap.rs`). An empty key drops the field instead
/// of writing `api_key = ""`.
pub fn write_ai_config(text: &str, cfg: &AiConfig) -> String {
    let mut root: toml::Table = toml::from_str(text).unwrap_or_default();
    let mut table = toml::Table::new();
    table.insert("enabled".to_string(), toml::Value::Boolean(cfg.enabled));
    table.insert(
        "base_url".to_string(),
        toml::Value::String(cfg.base_url.clone()),
    );
    table.insert("model".to_string(), toml::Value::String(cfg.model.clone()));
    if !cfg.api_key.is_empty() {
        table.insert(
            "api_key".to_string(),
            toml::Value::String(cfg.api_key.clone()),
        );
    }
    table.insert(
        "temperature".to_string(),
        toml::Value::Float(cfg.temperature),
    );
    root.insert("ai".to_string(), toml::Value::Table(table));
    toml::to_string_pretty(&root).unwrap_or_else(|_| text.to_string())
}

/// Split `http(s)://host[:port][/path]` into the lowercased host. No `url`
/// dependency; the base URL is validated before it ever reaches a request.
pub fn host_of(base_url: &str) -> Option<String> {
    let rest = base_url
        .strip_prefix("http://")
        .or_else(|| base_url.strip_prefix("https://"))?;
    let authority = rest.split('/').next().unwrap_or("");
    let host = match authority.rsplit_once('@') {
        Some((_, h)) => h,
        None => authority,
    };
    let host = if host.starts_with('[') {
        // IPv6 literal: keep the bracket form, drop the port after `]`.
        host.split(']').next().map(|h| format!("{h}]")).unwrap_or_default()
    } else {
        host.split(':').next().unwrap_or("").to_string()
    };
    if host.is_empty() {
        None
    } else {
        Some(host.to_ascii_lowercase())
    }
}

/// Loopback detection for the "内容不出本机" copy and the no-key allowance.
pub fn is_loopback(base_url: &str) -> bool {
    match host_of(base_url).as_deref() {
        Some("localhost") | Some("127.0.0.1") | Some("[::1]") => true,
        Some(h) => h.starts_with("127."),
        None => false,
    }
}

/// Validate a prospective config. Enabling requires an endpoint + model, and a
/// key unless the endpoint is loopback (local Ollama has none).
pub fn validate_config(cfg: &AiConfig) -> Result<(), IpcError> {
    if cfg.base_url.len() > BASE_URL_MAX {
        return Err(IpcError::new(
            "ai_bad_url",
            format!("接口地址过长（上限 {BASE_URL_MAX} 字符）"),
        ));
    }
    if cfg.base_url.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err(IpcError::new("ai_bad_url", "接口地址不能包含空白或控制字符"));
    }
    if !cfg.base_url.is_empty() && host_of(&cfg.base_url).is_none() {
        return Err(IpcError::new(
            "ai_bad_url",
            "接口地址必须以 http:// 或 https:// 开头",
        ));
    }
    if cfg.model.len() > MODEL_MAX {
        return Err(IpcError::new(
            "invalid_argument",
            format!("模型名过长（上限 {MODEL_MAX} 字符）"),
        ));
    }
    if cfg.model.chars().any(|c| c.is_control()) {
        return Err(IpcError::new("invalid_argument", "模型名不能包含控制字符"));
    }
    if !(0.0..=2.0).contains(&cfg.temperature) || cfg.temperature.is_nan() {
        return Err(IpcError::new(
            "invalid_argument",
            "温度必须在 0 到 2 之间",
        ));
    }
    if cfg.api_key.len() > KEY_MAX {
        return Err(IpcError::new("invalid_argument", "Key 过长"));
    }
    if cfg.enabled {
        if cfg.base_url.is_empty() {
            return Err(IpcError::new("ai_not_configured", "启用 AI 前需要填写接口地址"));
        }
        if cfg.model.is_empty() {
            return Err(IpcError::new("ai_not_configured", "启用 AI 前需要填写模型名"));
        }
        if cfg.api_key.is_empty() && !is_loopback(&cfg.base_url) {
            return Err(IpcError::new(
                "ai_not_configured",
                "启用 AI 前需要填写 API Key（本机回环地址可不填）",
            ));
        }
    }
    Ok(())
}

pub(crate) fn load_config(app: &AppHandle) -> Result<AiConfig, IpcError> {
    Ok(read_ai_config(&crate::commands::read_config_text(app)?))
}

// ---- block 2: OpenAI-compatible streaming ------------------------------------
//
// Nothing below runs until the user has enabled AI *and* filled endpoint /
// model / key (unless the endpoint is loopback). The command refuses with
// `ai_disabled` / `ai_not_configured` before opening a socket, so a disabled
// install cannot leak anything by accident.

/// One chat message. Roles are restricted by `validate_messages`.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AiMessage {
    pub role: String,
    pub content: String,
}

/// One streaming event on the `ll:ai-event` channel.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AiStreamEvent {
    pub req_id: u64,
    /// `chunk` | `reasoning` | `done` | `error`.
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delta: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl AiStreamEvent {
    fn delta(req_id: u64, kind: &str, text: String) -> Self {
        Self {
            req_id,
            kind: kind.to_string(),
            delta: Some(text),
            code: None,
            error: None,
        }
    }
    fn done(req_id: u64) -> Self {
        Self {
            req_id,
            kind: "done".to_string(),
            delta: None,
            code: None,
            error: None,
        }
    }
    fn failed(req_id: u64, code: &str, message: String) -> Self {
        Self {
            req_id,
            kind: "error".to_string(),
            delta: None,
            code: Some(code.to_string()),
            error: Some(message),
        }
    }
}

/// One parsed SSE line. Pure so the protocol quirks are unit-testable.
#[derive(Debug, PartialEq)]
pub enum SseOutcome {
    Delta {
        content: Option<String>,
        reasoning: Option<String>,
    },
    Done,
    Error {
        code: Option<String>,
        message: String,
    },
    Ignore,
}

/// Strip `data:` and parse one SSE line. Comments, blank lines, events other
/// than `data`, and non-JSON payloads are ignored (forward compatibility).
pub fn parse_sse_line(line: &str) -> SseOutcome {
    let line = line.trim_end_matches(['\r', '\n']).trim();
    if line.is_empty() || line.starts_with(':') {
        return SseOutcome::Ignore;
    }
    let Some(payload) = line.strip_prefix("data:") else {
        return SseOutcome::Ignore;
    };
    let payload = payload.trim();
    if payload.is_empty() {
        return SseOutcome::Ignore;
    }
    if payload == "[DONE]" {
        return SseOutcome::Done;
    }
    let Ok(value) = serde_json::from_str::<serde_json::Value>(payload) else {
        return SseOutcome::Ignore;
    };
    if let Some(err) = value.get("error") {
        if !err.is_null() {
            return SseOutcome::Error {
                code: err
                    .get("code")
                    .and_then(|c| c.as_str())
                    .map(str::to_string),
                message: error_text(err),
            };
        }
    }
    let choice = value.get("choices").and_then(|c| c.get(0));
    let delta = choice
        .and_then(|c| c.get("delta"))
        .or_else(|| choice.and_then(|c| c.get("message")));
    let (content, reasoning) = match delta {
        Some(d) => (
            value_text(d.get("content")),
            value_text(d.get("reasoning_content"))
                .or_else(|| value_text(d.get("reasoning")))
                .or_else(|| value_text(d.get("thinking"))),
        ),
        None => (None, None),
    };
    if content.is_none() && reasoning.is_none() {
        SseOutcome::Ignore
    } else {
        SseOutcome::Delta { content, reasoning }
    }
}

/// Extract text from a delta field: a string, or an array of `{text}` parts
/// (some OpenAI-compatible gateways stream content as parts).
fn value_text(v: Option<&serde_json::Value>) -> Option<String> {
    match v? {
        serde_json::Value::String(s) if !s.is_empty() => Some(s.clone()),
        serde_json::Value::Array(items) => {
            let text: String = items
                .iter()
                .filter_map(|it| it.get("text").and_then(|t| t.as_str()))
                .collect();
            if text.is_empty() {
                None
            } else {
                Some(text)
            }
        }
        _ => None,
    }
}

/// Human text for an error node (string or object with `message` / `code`).
pub fn error_text(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => truncate_utf8(s, 300),
        serde_json::Value::Object(_) => {
            let msg = v
                .get("message")
                .and_then(|m| error_text_opt(m))
                .or_else(|| v.get("msg").and_then(|m| error_text_opt(m)))
                .unwrap_or_default();
            let code = v
                .get("code")
                .and_then(|c| {
                    c.as_str()
                        .map(str::to_string)
                        .or_else(|| c.as_u64().map(|n| n.to_string()))
                })
                .unwrap_or_default();
            let out = if msg.is_empty() {
                code
            } else if code.is_empty() {
                msg
            } else {
                format!("{msg}（{code}）")
            };
            truncate_utf8(out.trim(), 300)
        }
        _ => truncate_utf8(&v.to_string(), 300),
    }
}

fn error_text_opt(v: &serde_json::Value) -> Option<String> {
    match v {
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Object(_) => {
            let s = error_text(v);
            if s.is_empty() {
                None
            } else {
                Some(s)
            }
        }
        _ => None,
    }
}

/// Readable error body for a non-2xx response: the provider's message when it
/// has one, otherwise the raw body, always capped.
pub fn extract_error_body(body: &str) -> String {
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(body) {
        if let Some(err) = v.get("error") {
            let text = error_text(err);
            if !text.is_empty() {
                return text;
            }
        }
        for key in ["message", "msg", "detail"] {
            if let Some(s) = v.get(key).and_then(|x| x.as_str()) {
                if !s.trim().is_empty() {
                    return truncate_utf8(s.trim(), 300);
                }
            }
        }
    }
    truncate_utf8(body.trim(), 300)
}

/// Byte-cap that never splits a UTF-8 code point.
pub fn truncate_utf8(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    let mut end = max;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    s[..end].to_string()
}

/// Stable IPC code for an HTTP status.
pub fn http_error_code(status: u16) -> &'static str {
    match status {
        401 | 403 => "ai_auth",
        429 => "ai_rate_limited",
        _ => "ai_http",
    }
}

/// Endpoint root: strip a terminal `/chat/completions` or `/models`, then add
/// `/v1` when the path has no version segment yet. Presets all end in `/v1`.
pub fn api_root(base: &str) -> String {
    let mut b = base.trim().trim_end_matches('/').to_string();
    for suffix in ["/chat/completions", "/models"] {
        if let Some(prefix) = b.strip_suffix(suffix) {
            b = prefix.trim_end_matches('/').to_string();
        }
    }
    if b.ends_with("/v1") || b.contains("/v1/") {
        b
    } else {
        format!("{b}/v1")
    }
}

pub fn chat_url(base: &str) -> String {
    format!("{}/chat/completions", api_root(base))
}

pub fn models_url(base: &str) -> String {
    format!("{}/models", api_root(base))
}

/// Only chat-capable model ids are offered (embeddings/tts/rerank are noise).
pub fn is_chat_model(id: &str) -> bool {
    let l = id.to_ascii_lowercase();
    ![
        "embedding",
        "embed",
        "rerank",
        "tts",
        "whisper",
        "dall-e",
        "moderation",
        "image",
        "audio",
        "speech",
        "transcribe",
    ]
    .iter()
    .any(|k| l.contains(k))
}

fn cancels() -> &'static Mutex<HashMap<u64, Arc<AtomicBool>>> {
    static MAP: OnceLock<Mutex<HashMap<u64, Arc<AtomicBool>>>> = OnceLock::new();
    MAP.get_or_init(|| Mutex::new(HashMap::new()))
}

fn register_cancel(req_id: u64) -> Arc<AtomicBool> {
    let flag = Arc::new(AtomicBool::new(false));
    if let Ok(mut m) = cancels().lock() {
        m.insert(req_id, flag.clone());
    }
    flag
}

fn clear_cancel(req_id: u64) {
    if let Ok(mut m) = cancels().lock() {
        m.remove(&req_id);
    }
}

fn client() -> &'static reqwest::Client {
    static C: OnceLock<reqwest::Client> = OnceLock::new();
    C.get_or_init(|| {
        reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(300))
            .build()
            .unwrap_or_default()
    })
}

/// Gate every network call: disabled or incomplete config never opens a socket.
fn ensure_usable(cfg: &AiConfig) -> Result<(), IpcError> {
    if !cfg.enabled {
        return Err(IpcError::new(
            "ai_disabled",
            "AI 未启用，请先在 AI 设置中开启",
        ));
    }
    validate_config(cfg)
}

fn validate_messages(messages: &[AiMessage]) -> Result<(), IpcError> {
    if messages.is_empty() {
        return Err(IpcError::new("invalid_argument", "消息不能为空"));
    }
    if messages.len() > 64 {
        return Err(IpcError::new("invalid_argument", "单次消息条数过多（上限 64）"));
    }
    let mut total = 0usize;
    for m in messages {
        if !matches!(m.role.as_str(), "system" | "user" | "assistant") {
            return Err(IpcError::new(
                "invalid_argument",
                format!("不支持的消息角色：{}", m.role),
            ));
        }
        if m.content.trim().is_empty() {
            return Err(IpcError::new("invalid_argument", "消息内容不能为空"));
        }
        total += m.content.len();
    }
    if total > 400_000 {
        return Err(IpcError::new(
            "invalid_argument",
            "单次发送内容过大（上限 400000 字节），请缩小选区或样例数",
        ));
    }
    Ok(())
}

/// Ask an in-flight stream to stop. Returns false for an unknown id.
#[tauri::command]
pub fn ai_chat_cancel(req_id: u64) -> bool {
    match cancels().lock().ok().and_then(|m| m.get(&req_id).cloned()) {
        Some(flag) => {
            flag.store(true, Ordering::SeqCst);
            true
        }
        None => false,
    }
}

/// Stream one completion. Pre-flight failures reject the invoke; remote
/// failures arrive as a terminal `error` event so the UI has one code path.
#[tauri::command]
pub async fn ai_chat_stream(
    app: AppHandle,
    req_id: u64,
    messages: Vec<AiMessage>,
    temperature: Option<f64>,
) -> Result<(), IpcError> {
    let cfg = load_config(&app)?;
    ensure_usable(&cfg)?;
    validate_messages(&messages)?;
    let temp = temperature.unwrap_or(cfg.temperature);
    if !(0.0..=2.0).contains(&temp) || temp.is_nan() {
        return Err(IpcError::new("invalid_argument", "温度必须在 0 到 2 之间"));
    }
    let cancel = register_cancel(req_id);
    // The sink is injected so the real socket path can be exercised in tests
    // against a local mock server (see `mod stream_tests`).
    let emit = |ev: AiStreamEvent| {
        let _ = app.emit("ll:ai-event", ev);
    };
    stream_chat(&emit, &cfg, req_id, messages, temp, &cancel).await;
    clear_cancel(req_id);
    Ok(())
}

async fn stream_chat(
    emit: &(dyn Fn(AiStreamEvent) + Sync),
    cfg: &AiConfig,
    req_id: u64,
    messages: Vec<AiMessage>,
    temperature: f64,
    cancel: &Arc<AtomicBool>,
) {
    let body = serde_json::json!({
        "model": cfg.model,
        "messages": messages,
        "stream": true,
        "temperature": temperature,
    });
    let mut request = client()
        .post(chat_url(&cfg.base_url))
        .header("Accept", "text/event-stream")
        .json(&body);
    if !cfg.api_key.is_empty() {
        request = request.bearer_auth(&cfg.api_key);
    }
    let mut response = match request.send().await {
        Ok(r) => r,
        Err(e) => {
            emit(AiStreamEvent::failed(req_id, "ai_network", e.to_string()));
            return;
        }
    };
    let status = response.status();
    if !status.is_success() {
        let text = response.text().await.unwrap_or_default();
        emit(AiStreamEvent::failed(
            req_id,
            http_error_code(status.as_u16()),
            format!("HTTP {}：{}", status.as_u16(), extract_error_body(&text)),
        ));
        return;
    }

    let mut buf: Vec<u8> = Vec::new();
    loop {
        if cancel.load(Ordering::SeqCst) {
            emit(AiStreamEvent::done(req_id));
            return;
        }
        match response.chunk().await {
            Ok(Some(chunk)) => {
                buf.extend_from_slice(&chunk);
                while let Some(pos) = buf.iter().position(|b| *b == b'\n') {
                    let line: Vec<u8> = buf.drain(..=pos).collect();
                    let text = String::from_utf8_lossy(&line);
                    if let Some(terminal) = handle_sse_line(emit, req_id, &text) {
                        if terminal {
                            return;
                        }
                    }
                }
            }
            Ok(None) => break,
            Err(e) => {
                emit(AiStreamEvent::failed(req_id, "ai_network", e.to_string()));
                return;
            }
        }
    }
    if !buf.is_empty() {
        let text = String::from_utf8_lossy(&buf);
        if handle_sse_line(emit, req_id, &text) == Some(true) {
            return;
        }
    }
    emit(AiStreamEvent::done(req_id));
}

/// Emit one line's events. `Some(true)` means the stream is finished.
fn handle_sse_line(
    emit: &(dyn Fn(AiStreamEvent) + Sync),
    req_id: u64,
    line: &str,
) -> Option<bool> {
    match parse_sse_line(line) {
        SseOutcome::Delta { content, reasoning } => {
            if let Some(r) = reasoning {
                emit(AiStreamEvent::delta(req_id, "reasoning", r));
            }
            if let Some(c) = content {
                emit(AiStreamEvent::delta(req_id, "chunk", c));
            }
            None
        }
        SseOutcome::Done => {
            emit(AiStreamEvent::done(req_id));
            Some(true)
        }
        SseOutcome::Error { code, message } => {
            emit(AiStreamEvent::failed(
                req_id,
                code.as_deref().unwrap_or("ai_http"),
                message,
            ));
            Some(true)
        }
        SseOutcome::Ignore => None,
    }
}

/// Write an AI reply to a user-picked path (the panel's "另存为 .md"). Capped
/// so a runaway reply cannot be used to write an unbounded file.
#[tauri::command]
pub fn ai_save_text(path: String, text: String) -> Result<(), IpcError> {
    if path.trim().is_empty() {
        return Err(IpcError::new("invalid_argument", "保存路径为空"));
    }
    if text.len() > 2_000_000 {
        return Err(IpcError::new("too_large", "内容超过 2MB，无法另存"));
    }
    std::fs::write(&path, text)
        .map_err(|e| IpcError::new("io", format!("保存失败：{e}")))
}

/// `GET /models`, filtered to chat-capable ids (8s timeout). Requires an
/// enabled, complete config — a "test connection" is still an egress action.
#[tauri::command]
pub async fn ai_models_list(app: AppHandle) -> Result<Vec<String>, IpcError> {
    let cfg = load_config(&app)?;
    ensure_usable(&cfg)?;
    let mut request = client()
        .get(models_url(&cfg.base_url))
        .timeout(Duration::from_secs(8));
    if !cfg.api_key.is_empty() {
        request = request.bearer_auth(&cfg.api_key);
    }
    let response = request
        .send()
        .await
        .map_err(|e| IpcError::new("ai_network", e.to_string()))?;
    let status = response.status();
    let text = response.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(IpcError::new(
            http_error_code(status.as_u16()),
            format!("HTTP {}：{}", status.as_u16(), extract_error_body(&text)),
        ));
    }
    let value: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| IpcError::new("ai_http", format!("模型列表不是 JSON：{e}")))?;
    let mut ids: Vec<String> = value
        .get("data")
        .and_then(|d| d.as_array())
        .map(|rows| {
            rows.iter()
                .filter_map(|m| m.get("id").and_then(|i| i.as_str()).map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    ids.retain(|id| is_chat_model(id));
    ids.sort();
    ids.dedup();
    Ok(ids)
}

/// Current AI settings; `hasKey` only, the key itself never crosses IPC.
#[tauri::command]
pub fn ai_config_get(app: AppHandle) -> Result<AiConfigView, IpcError> {
    Ok(AiConfigView::from_config(&load_config(&app)?))
}

/// Save AI settings. `api_key`: `None` keeps the stored key, `Some("")` clears
/// it, `Some(value)` replaces it. The whole table is validated before writing.
#[tauri::command]
pub fn ai_config_set(
    app: AppHandle,
    enabled: bool,
    base_url: String,
    model: String,
    api_key: Option<String>,
    temperature: Option<f64>,
) -> Result<AiConfigView, IpcError> {
    let text = crate::commands::read_config_text(&app)?;
    let current = read_ai_config(&text);
    let mut next = AiConfig {
        enabled,
        base_url: base_url.trim().to_string(),
        model: model.trim().to_string(),
        api_key: match api_key {
            None => current.api_key,
            Some(k) => k.trim().to_string(),
        },
        temperature: temperature.unwrap_or(current.temperature),
    };
    validate_config(&next)?;
    // Never persist a key for a URL that is not the one it was entered for.
    if next.api_key.is_empty() {
        next.api_key = String::new();
    }
    crate::commands::write_config_text(&app, &write_ai_config(&text, &next))?;
    Ok(AiConfigView::from_config(&next))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_off_and_empty() {
        let cfg = read_ai_config("");
        assert!(!cfg.enabled);
        assert!(cfg.base_url.is_empty());
        assert!(cfg.model.is_empty());
        assert!(cfg.api_key.is_empty());
        assert_eq!(cfg.temperature, DEFAULT_TEMPERATURE);
        assert!(!AiConfigView::from_config(&cfg).has_key);
    }

    #[test]
    fn reads_valid_types_and_ignores_bad_ones() {
        let src = r#"
[ai]
enabled = true
base_url = "  https://api.example.com/v1  "
model = "demo"
api_key = "sk-test"
temperature = 0.7
"#;
        let cfg = read_ai_config(src);
        assert!(cfg.enabled);
        assert_eq!(cfg.base_url, "https://api.example.com/v1");
        assert_eq!(cfg.model, "demo");
        assert_eq!(cfg.api_key, "sk-test");
        assert_eq!(cfg.temperature, 0.7);

        let bad = read_ai_config("[ai]\nenabled = \"yes\"\ntemperature = \"hot\"\n");
        assert!(!bad.enabled);
        assert_eq!(bad.temperature, DEFAULT_TEMPERATURE);
    }

    #[test]
    fn write_preserves_other_sections_and_roundtrips() {
        let src = "recovery_enabled = true\n\n[keymap]\nopen = \"Ctrl+O\"\n";
        let cfg = AiConfig {
            enabled: true,
            base_url: "https://api.example.com/v1".into(),
            model: "demo".into(),
            api_key: "sk-test".into(),
            temperature: 0.4,
        };
        let out = write_ai_config(src, &cfg);
        assert_eq!(read_ai_config(&out), cfg);
        let root: toml::Table = toml::from_str(&out).unwrap();
        assert_eq!(
            root.get("recovery_enabled").and_then(|v| v.as_bool()),
            Some(true)
        );
        assert!(root.get("keymap").is_some(), "other table kept: {out}");
    }

    #[test]
    fn empty_key_is_removed_not_written() {
        let cfg = AiConfig {
            base_url: "http://127.0.0.1:11434/v1".into(),
            model: "qwen".into(),
            ..Default::default()
        };
        let out = write_ai_config("", &cfg);
        assert!(!out.contains("api_key"), "empty key must not be stored: {out}");
        assert_eq!(read_ai_config(&out).api_key, "");
    }

    #[test]
    fn loopback_detection_and_host_parsing() {
        assert!(is_loopback("http://127.0.0.1:11434/v1"));
        assert!(is_loopback("https://localhost:8080/v1"));
        assert!(is_loopback("http://[::1]:1234/v1"));
        assert!(!is_loopback("https://api.deepseek.com/v1"));
        assert_eq!(
            host_of("https://user:pw@API.Example.com:8443/v1").as_deref(),
            Some("api.example.com")
        );
        assert_eq!(host_of("ftp://example.com"), None);
    }

    #[test]
    fn enable_requires_endpoint_model_and_key() {
        let base = AiConfig {
            enabled: true,
            ..Default::default()
        };
        assert_eq!(
            validate_config(&base).unwrap_err().code,
            "ai_not_configured"
        );
        let no_model = AiConfig {
            base_url: "https://api.example.com/v1".into(),
            ..base.clone()
        };
        assert_eq!(
            validate_config(&no_model).unwrap_err().code,
            "ai_not_configured"
        );
        let no_key = AiConfig {
            model: "demo".into(),
            ..no_model.clone()
        };
        assert_eq!(validate_config(&no_key).unwrap_err().code, "ai_not_configured");
        let ok = AiConfig {
            api_key: "sk-test".into(),
            ..no_key
        };
        assert!(validate_config(&ok).is_ok());
    }

    #[test]
    fn loopback_enable_needs_no_key() {
        let cfg = AiConfig {
            enabled: true,
            base_url: "http://localhost:11434/v1".into(),
            model: "qwen".into(),
            ..Default::default()
        };
        assert!(validate_config(&cfg).is_ok());
        assert!(AiConfigView::from_config(&cfg).local);
    }

    #[test]
    fn rejects_bad_urls_and_temperature() {
        let bad_url = AiConfig {
            base_url: "api.example.com/v1".into(),
            ..Default::default()
        };
        assert_eq!(validate_config(&bad_url).unwrap_err().code, "ai_bad_url");
        let spaced = AiConfig {
            base_url: "https://api example.com".into(),
            ..Default::default()
        };
        assert_eq!(validate_config(&spaced).unwrap_err().code, "ai_bad_url");
        let hot = AiConfig {
            temperature: 3.0,
            ..Default::default()
        };
        assert_eq!(validate_config(&hot).unwrap_err().code, "invalid_argument");
    }

    #[test]
    fn disabled_config_may_be_partially_filled() {
        let cfg = AiConfig {
            enabled: false,
            base_url: String::new(),
            model: String::new(),
            ..Default::default()
        };
        assert!(validate_config(&cfg).is_ok());
    }

    #[test]
    fn endpoint_urls_are_normalized() {
        assert_eq!(
            chat_url("https://api.deepseek.com/v1"),
            "https://api.deepseek.com/v1/chat/completions"
        );
        assert_eq!(
            chat_url("https://api.deepseek.com/v1/"),
            "https://api.deepseek.com/v1/chat/completions"
        );
        assert_eq!(
            chat_url("https://api.deepseek.com/v1/chat/completions"),
            "https://api.deepseek.com/v1/chat/completions"
        );
        assert_eq!(
            chat_url("http://127.0.0.1:11434"),
            "http://127.0.0.1:11434/v1/chat/completions"
        );
        assert_eq!(
            models_url("https://api.example.com"),
            "https://api.example.com/v1/models"
        );
        assert_eq!(
            models_url("https://gateway.example.com/openai/v1/models"),
            "https://gateway.example.com/openai/v1/models"
        );
    }

    #[test]
    fn parses_delta_content_and_reasoning() {
        let content = r#"data: {"choices":[{"delta":{"content":"hello"}}]}"#;
        assert_eq!(
            parse_sse_line(content),
            SseOutcome::Delta {
                content: Some("hello".into()),
                reasoning: None
            }
        );
        let reasoning =
            r#"data: {"choices":[{"delta":{"reasoning_content":"think"}}]}"#;
        assert_eq!(
            parse_sse_line(reasoning),
            SseOutcome::Delta {
                content: None,
                reasoning: Some("think".into())
            }
        );
        // A gateway that ignored `stream:true` still yields text.
        let plain = r#"data: {"choices":[{"message":{"content":"whole"}}]}"#;
        assert_eq!(
            parse_sse_line(plain),
            SseOutcome::Delta {
                content: Some("whole".into()),
                reasoning: None
            }
        );
        // Content-as-parts gateways.
        let parts =
            r#"data: {"choices":[{"delta":{"content":[{"type":"text","text":"a"},{"type":"text","text":"b"}]}}]}"#;
        assert_eq!(
            parse_sse_line(parts),
            SseOutcome::Delta {
                content: Some("ab".into()),
                reasoning: None
            }
        );
    }

    #[test]
    fn parses_done_comments_and_garbage() {
        assert_eq!(parse_sse_line("data: [DONE]\r\n"), SseOutcome::Done);
        assert_eq!(parse_sse_line(": keep-alive"), SseOutcome::Ignore);
        assert_eq!(parse_sse_line("event: ping"), SseOutcome::Ignore);
        assert_eq!(parse_sse_line("data: not json"), SseOutcome::Ignore);
        assert_eq!(
            parse_sse_line(r#"data: {"choices":[{"delta":{"role":"assistant"}}]}"#),
            SseOutcome::Ignore
        );
        assert_eq!(parse_sse_line(""), SseOutcome::Ignore);
    }

    #[test]
    fn parses_in_stream_error() {
        let line = r#"data: {"error":{"message":"bad key","code":"invalid_api_key"}}"#;
        match parse_sse_line(line) {
            SseOutcome::Error { code, message } => {
                assert_eq!(code.as_deref(), Some("invalid_api_key"));
                assert!(message.contains("bad key"));
            }
            other => panic!("expected error, got {other:?}"),
        }
    }

    #[test]
    fn maps_http_status_to_stable_codes() {
        assert_eq!(http_error_code(401), "ai_auth");
        assert_eq!(http_error_code(403), "ai_auth");
        assert_eq!(http_error_code(429), "ai_rate_limited");
        assert_eq!(http_error_code(500), "ai_http");
    }

    #[test]
    fn extracts_readable_error_bodies() {
        assert_eq!(
            extract_error_body(r#"{"error":{"message":"quota exceeded","type":"x"}}"#),
            "quota exceeded"
        );
        assert_eq!(
            extract_error_body(r#"{"message":"plain msg"}"#),
            "plain msg"
        );
        assert_eq!(extract_error_body("  upstream 500  "), "upstream 500");
        let long = "x".repeat(500);
        assert_eq!(extract_error_body(&long).len(), 300);
    }

    #[test]
    fn truncation_never_splits_multibyte_chars() {
        let s = "日志行";
        assert_eq!(truncate_utf8(s, 4), "日"); // 3-byte boundary
        assert_eq!(truncate_utf8(s, 100), s);
        assert_eq!(truncate_utf8("ab", 0), "");
    }

    #[test]
    fn filters_non_chat_models() {
        assert!(is_chat_model("deepseek-chat"));
        assert!(is_chat_model("gpt-4o-mini"));
        assert!(!is_chat_model("text-embedding-3-small"));
        assert!(!is_chat_model("BAAI/bge-reranker-v2"));
        assert!(!is_chat_model("tts-1"));
    }
}

/// End-to-end wire tests: a real `reqwest` call to a loopback mock server,
/// asserting the events the front end would receive. These cover the part unit
/// tests cannot — TLS-less HTTP setup, chunked SSE decoding, HTTP status
/// mapping and cancellation — without any real provider or API key.
#[cfg(test)]
mod stream_tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread::JoinHandle;

    /// Serialize these tests: they share the global tokio runtime.
    fn guard() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|e| e.into_inner())
    }

    fn cfg_for(base_url: &str) -> AiConfig {
        AiConfig {
            enabled: true,
            base_url: base_url.to_string(),
            model: "test-model".into(),
            api_key: String::new(),
            temperature: DEFAULT_TEMPERATURE,
        }
    }

    fn drain_headers(sock: &mut std::net::TcpStream) {
        let _ = sock.set_read_timeout(Some(Duration::from_secs(5)));
        let mut req = Vec::new();
        let mut b = [0u8; 1024];
        while !req.windows(4).any(|w| w == b"\r\n\r\n") {
            match sock.read(&mut b) {
                Ok(0) | Err(_) => break,
                Ok(n) => req.extend_from_slice(&b[..n]),
            }
        }
    }

    /// Accept one connection, or give up after a deadline so a broken client
    /// fails the test instead of hanging the suite.
    fn accept_one(listener: &TcpListener) -> Option<std::net::TcpStream> {
        listener.set_nonblocking(true).ok()?;
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        while std::time::Instant::now() < deadline {
            if let Ok((sock, _)) = listener.accept() {
                let _ = sock.set_nonblocking(false); // Windows inherits the flag
                return Some(sock);
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        None
    }

    /// One request → chunked SSE made of `parts` (body, delay before sending).
    fn mock_sse(status: &'static str, parts: Vec<(&'static str, u64)>) -> (String, JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let handle = std::thread::spawn(move || {
            let Some(mut sock) = accept_one(&listener) else {
                return;
            };
            drain_headers(&mut sock);
            let _ = write!(
                sock,
                "HTTP/1.1 {status}\r\nContent-Type: text/event-stream\r\n\
                 Transfer-Encoding: chunked\r\nConnection: close\r\n\r\n"
            );
            let _ = sock.flush();
            for (body, delay) in parts {
                if delay > 0 {
                    std::thread::sleep(Duration::from_millis(delay));
                }
                let _ = write!(sock, "{:x}\r\n", body.len());
                let _ = sock.write_all(body.as_bytes());
                let _ = sock.write_all(b"\r\n");
                let _ = sock.flush();
            }
            let _ = sock.write_all(b"0\r\n\r\n");
        });
        (format!("http://{addr}/v1"), handle)
    }

    /// One request → a plain (non-stream) response body.
    fn mock_http(status: &'static str, body: &'static str) -> (String, JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let handle = std::thread::spawn(move || {
            let Some(mut sock) = accept_one(&listener) else {
                return;
            };
            drain_headers(&mut sock);
            let _ = write!(
                sock,
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\n\
                 Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
        });
        (format!("http://{addr}/v1"), handle)
    }

    fn run(
        cfg: &AiConfig,
        cancel: &Arc<AtomicBool>,
        sink: &(dyn Fn(AiStreamEvent) + Sync),
    ) {
        tauri::async_runtime::block_on(stream_chat(
            sink,
            cfg,
            7,
            vec![AiMessage {
                role: "user".into(),
                content: "解释这行日志".into(),
            }],
            DEFAULT_TEMPERATURE,
            cancel,
        ));
    }

    #[test]
    fn streams_deltas_then_done_over_http() {
        let _g = guard();
        let (url, server) = mock_sse(
            "200 OK",
            vec![
                ("data: {\"choices\":[{\"delta\":{\"content\":\"你\"}}]}\n\n", 0),
                ("data: {\"choices\":[{\"delta\":{\"content\":\"好\"}}]}\n\n", 0),
                ("data: [DONE]\n\n", 0),
            ],
        );
        let events = Arc::new(Mutex::new(Vec::new()));
        let sink = {
            let events = events.clone();
            move |ev: AiStreamEvent| events.lock().unwrap().push(ev)
        };
        run(&cfg_for(&url), &Arc::new(AtomicBool::new(false)), &sink);
        server.join().unwrap();
        let got = events.lock().unwrap();
        let text: String = got.iter().filter_map(|e| e.delta.clone()).collect();
        assert_eq!(text, "你好");
        assert_eq!(got.last().unwrap().kind, "done");
        assert!(got.iter().all(|e| e.req_id == 7));
    }

    #[test]
    fn http_401_maps_to_ai_auth() {
        let _g = guard();
        let (url, server) =
            mock_http("401 Unauthorized", r#"{"error":{"message":"bad key"}}"#);
        let events = Arc::new(Mutex::new(Vec::new()));
        let sink = {
            let events = events.clone();
            move |ev: AiStreamEvent| events.lock().unwrap().push(ev)
        };
        run(&cfg_for(&url), &Arc::new(AtomicBool::new(false)), &sink);
        server.join().unwrap();
        let got = events.lock().unwrap();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].kind, "error");
        assert_eq!(got[0].code.as_deref(), Some("ai_auth"));
        assert!(got[0].error.as_deref().unwrap().contains("bad key"));
    }

    #[test]
    fn http_429_maps_to_rate_limited() {
        let _g = guard();
        let (url, server) = mock_http("429 Too Many Requests", r#"{"error":"slow down"}"#);
        let events = Arc::new(Mutex::new(Vec::new()));
        let sink = {
            let events = events.clone();
            move |ev: AiStreamEvent| events.lock().unwrap().push(ev)
        };
        run(&cfg_for(&url), &Arc::new(AtomicBool::new(false)), &sink);
        server.join().unwrap();
        let got = events.lock().unwrap();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].kind, "error");
        assert_eq!(got[0].code.as_deref(), Some("ai_rate_limited"));
    }

    #[test]
    fn cancel_after_first_delta_stops_the_stream() {
        let _g = guard();
        let (url, server) = mock_sse(
            "200 OK",
            vec![
                ("data: {\"choices\":[{\"delta\":{\"content\":\"a\"}}]}\n\n", 0),
                ("data: {\"choices\":[{\"delta\":{\"content\":\"b\"}}]}\n\n", 400),
            ],
        );
        let events = Arc::new(Mutex::new(Vec::new()));
        let cancel = Arc::new(AtomicBool::new(false));
        let sink = {
            let events = events.clone();
            let cancel = cancel.clone();
            move |ev: AiStreamEvent| {
                if ev.kind == "chunk" {
                    cancel.store(true, Ordering::SeqCst); // user pressed 停止
                }
                events.lock().unwrap().push(ev);
            }
        };
        run(&cfg_for(&url), &cancel, &sink);
        server.join().unwrap();
        let got = events.lock().unwrap();
        let text: String = got.iter().filter_map(|e| e.delta.clone()).collect();
        assert_eq!(text, "a"); // the delayed second delta never renders
        assert_eq!(got.last().unwrap().kind, "done");
    }
}
