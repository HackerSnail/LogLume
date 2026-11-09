//! AI section of the right inspector (NoteLume parity: one dock, one
//! conversation). The three log actions are modes of the same composer; their
//! inputs (selected lines / query conditions / scan summary) are rendered as
//! that turn's user message. Switching modes never clears the transcript.
//!
//! The panel is deliberately honest about state: disabled AI shows a guidance
//! card (never a silent no-op), every send displays how much text is about to
//! leave the machine and to which host, and stop/cancel is always reachable.
import { save } from "@tauri-apps/plugin-dialog";
import {
  aiChatCancel,
  aiChatStream,
  aiSaveText,
  ipcErrorZh,
  IpcError,
  listenAiEvents,
  type AiConfigView,
  type AiMessage,
  type AiStreamEvent,
} from "./ipc";
import type { AiContext } from "./aitasks";

/** Log-scenario actions offered by the composer's mode selector. */
export type AiScenario = "explain" | "errors" | "query";
export type AiMode = "chat" | AiScenario;

export interface AiPanelDeps {
  getConfig: () => AiConfigView | null;
  openSettings: () => void;
  /** Main owns data access; it builds the turn and calls `runScenario`. */
  runScenario: (kind: AiScenario, note?: string) => void;
}

/** Neutral system prompt for plain follow-up turns. */
const CHAT_SYSTEM =
  "你是 LogLume 的日志分析助手。只依据用户在本轮及历史消息里提供的日志内容回答；" +
  "不要编造日志中不存在的错误码、时间或行号。区分事实与推测，引用行号时写成 L<数字>。用中文回答。";

const PLACEHOLDERS: Record<AiMode, string> = {
  chat: "询问这段日志…（Enter 发送，Shift+Enter 换行）",
  explain: "解释选中的日志行…（可补充要求，Enter 发送）",
  errors: "归纳错误模式…（可补充要求，Enter 发送）",
  query: "总结当前查询结果…（可补充要求，Enter 发送）",
};

function el<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  className = "",
  text = "",
): HTMLElementTagNameMap[K] {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (text) node.textContent = text;
  return node;
}

function hostOf(baseUrl: string): string {
  try {
    return new URL(baseUrl).host;
  } catch {
    return baseUrl;
  }
}

function errText(err: unknown): string {
  if (err instanceof IpcError) return err.zh;
  return String(err);
}

export class AiPanel {
  readonly root: HTMLElement;
  private readonly bodyEl: HTMLElement;
  private readonly logEl: HTMLElement;
  private readonly statusEl: HTMLElement;
  private readonly noticeEl: HTMLElement;
  private readonly badgeEl: HTMLElement;
  private readonly emptyEl: HTMLElement;
  private readonly inputEl: HTMLTextAreaElement;
  private readonly modeEl: HTMLSelectElement;
  private readonly modelEl: HTMLButtonElement;
  private readonly sendBtn: HTMLButtonElement;
  private readonly stopBtn: HTMLButtonElement;
  private readonly foldBtn: HTMLButtonElement;
  /** Conversation history as user/assistant turns (system prompts are per turn). */
  private history: AiMessage[] = [];
  private active: {
    reqId: number;
    raw: string;
    textEl: HTMLElement;
    reasonEl: HTMLElement;
    cancelled: boolean;
    userTurn: AiMessage | null;
  } | null = null;
  private seq = 0;
  private busy = false;
  private expanded = true;
  /** Transient status line ("正在扫描…"); cleared when a turn starts/ends. */
  private note = "";

  constructor(private readonly deps: AiPanelDeps) {
    this.root = el("section", "ai-panel");
    this.root.id = "ai-panel";

    const head = el("div", "ai-head");
    this.foldBtn = el("button", "ai-fold", "▾");
    this.foldBtn.type = "button";
    this.foldBtn.title = "折叠 / 展开 AI";
    this.foldBtn.setAttribute("aria-expanded", "true");
    this.foldBtn.addEventListener("click", () => this.setExpanded(!this.expanded));
    const mark = el("span", "ai-mark", "✦");
    mark.setAttribute("aria-hidden", "true");
    const title = el("span", "ai-title", "AI");
    this.badgeEl = el("span", "ai-badge", "未启用");
    const spacer = el("span", "ai-spacer");
    const newBtn = el("button", "ai-icon", "+");
    newBtn.type = "button";
    newBtn.title = "新对话（清空当前上下文）";
    newBtn.setAttribute("aria-label", "新对话");
    newBtn.addEventListener("click", () => this.newChat());
    const settingsBtn = el("button", "ai-icon", "⚙");
    settingsBtn.type = "button";
    settingsBtn.title = "AI 设置…";
    settingsBtn.setAttribute("aria-label", "AI 设置");
    settingsBtn.addEventListener("click", () => this.deps.openSettings());
    head.append(mark, title, this.badgeEl, newBtn, settingsBtn, spacer, this.foldBtn);

    this.statusEl = el("div", "ai-status", "");
    this.noticeEl = el("div", "ai-notice", "");
    this.logEl = el("div", "ai-log");
    this.emptyEl = el("div", "ai-empty");
    const emptyTitle = el("div", "ai-empty-title", "针对当前日志提问");
    const emptyBody = el(
      "div",
      "ai-empty-body",
      "解释选中行、归纳错误模式、总结查询结果，都是这条对话里的动作：在下方选好模式后发送即可。",
    );
    const goSettings = el("button", "ai-primary", "前往 AI 设置…");
    goSettings.type = "button";
    goSettings.addEventListener("click", () => this.deps.openSettings());
    this.emptyEl.append(emptyTitle, emptyBody, goSettings);
    this.logEl.append(this.emptyEl);

    const compose = el("div", "ai-compose");
    this.inputEl = el("textarea", "ai-input-box");
    this.inputEl.rows = 3;
    this.inputEl.placeholder = PLACEHOLDERS.chat;
    this.inputEl.addEventListener("input", () => this.refresh());
    this.inputEl.addEventListener("keydown", (e) => {
      if (e.key !== "Enter" || e.shiftKey) return;
      if (e.isComposing || e.keyCode === 229) return; // IME candidate window
      e.preventDefault();
      this.sendAction();
    });

    const bar = el("div", "ai-bar");
    this.modeEl = el("select", "ai-mode");
    this.modeEl.title = "选择本轮动作";
    for (const [value, label] of [
      ["chat", "对话"],
      ["explain", "解释选中行"],
      ["errors", "归纳错误模式"],
      ["query", "总结查询结果"],
    ] as const) {
      const opt = document.createElement("option");
      opt.value = value;
      opt.textContent = label;
      this.modeEl.append(opt);
    }
    this.modeEl.addEventListener("change", () => {
      this.inputEl.placeholder = PLACEHOLDERS[this.modeEl.value as AiMode] ?? PLACEHOLDERS.chat;
      this.refresh();
    });
    this.modelEl = el("button", "ai-model", "未配置");
    this.modelEl.type = "button";
    this.modelEl.title = "AI 设置…";
    this.modelEl.addEventListener("click", () => this.deps.openSettings());

    this.stopBtn = el("button", "ai-stop", "■");
    this.stopBtn.type = "button";
    this.stopBtn.title = "停止";
    this.stopBtn.hidden = true;
    this.stopBtn.addEventListener("click", () => void this.cancel());
    this.sendBtn = el("button", "ai-send", "↑");
    this.sendBtn.type = "button";
    this.sendBtn.title = "发送（Enter）";
    this.sendBtn.addEventListener("click", () => this.sendAction());
    const barSpacer = el("span", "ai-spacer");
    bar.append(this.modeEl, this.modelEl, barSpacer, this.stopBtn, this.sendBtn);
    compose.append(this.inputEl, bar);

    this.bodyEl = el("div", "ai-body");
    this.bodyEl.append(this.statusEl, this.noticeEl, this.logEl, compose);
    this.root.append(head, this.bodyEl);

    void listenAiEvents((ev) => this.onEvent(ev));
    this.refresh();
  }

  get isExpanded(): boolean {
    return this.expanded;
  }

  /** True while a stream is in flight (main refuses to queue another turn). */
  get isBusy(): boolean {
    return this.busy;
  }

  /** Expand/collapse the section body (the header stays reachable). */
  setExpanded(on: boolean): void {
    this.expanded = on;
    this.root.classList.toggle("ai-collapsed", !on);
    this.foldBtn.textContent = on ? "▾" : "▸";
    this.foldBtn.setAttribute("aria-expanded", String(on));
    if (on) this.inputEl.focus();
  }

  focusInput(): void {
    this.setExpanded(true);
    this.inputEl.focus();
  }

  /** Transient line above the transcript (e.g. "正在扫描错误行…"). */
  setStatus(text: string): void {
    this.note = text;
    this.syncStatus();
  }

  /** Re-read the config (after the settings modal saves) / UI state. */
  refresh(): void {
    const cfg = this.deps.getConfig();
    const enabled = cfg?.enabled === true;
    this.badgeEl.textContent = enabled ? cfg?.model || "已启用" : "未启用";
    this.badgeEl.classList.toggle("on", enabled);
    this.badgeEl.title = enabled ? cfg?.baseUrl ?? "" : "";
    this.modelEl.textContent = enabled ? cfg?.model || "已配置" : "未配置";
    this.modelEl.title = enabled ? `${cfg?.model ?? ""} · ${cfg?.baseUrl ?? ""}` : "AI 设置…";
    this.emptyEl.hidden = enabled || this.history.length > 0;
    this.inputEl.disabled = !enabled || this.busy;
    this.modeEl.disabled = !enabled || this.busy;
    this.sendBtn.disabled =
      !enabled || this.busy || (this.modeEl.value === "chat" && this.inputEl.value.trim() === "");
    this.stopBtn.hidden = !this.busy;
    this.syncStatus();
  }

  private syncStatus(): void {
    const enabled = this.deps.getConfig()?.enabled === true;
    this.statusEl.textContent = this.busy
      ? "正在回复…"
      : this.note
        ? this.note
        : enabled
          ? ""
          : "AI 未启用（默认关闭）。请先在设置中开启并配置，之后才会联网。";
  }

  /** Show a locally detected blocker in the transcript (never a silent no-op). */
  reportBlocked(message: string): void {
    this.setStatus(message);
    if (this.deps.getConfig()?.enabled !== true) {
      this.emptyEl.hidden = false;
      return;
    }
    this.emptyEl.hidden = true;
    const bubble = el("div", "ai-msg ai-msg-error", message);
    this.logEl.append(bubble);
    this.scroll();
  }

  /** Clear the conversation explicitly (the header's "+"). */
  newChat(): void {
    if (this.busy) return;
    this.history = [];
    for (const child of [...this.logEl.children]) {
      if (child !== this.emptyEl) child.remove();
    }
    this.note = "";
    this.noticeEl.textContent = "";
    this.refresh();
    this.inputEl.focus();
  }

  /**
   * Run a prepared scenario as the next turn: the scenario's user message
   * (excerpt, conditions, scan summary) is shown verbatim, and prior turns stay
   * in the request so mode switches keep the conversation coherent.
   */
  runScenario(ctx: AiContext): void {
    const messages = ctx.messages;
    const sys = messages[0]?.role === "system" ? messages[0] : null;
    const turns = sys ? messages.slice(1) : messages;
    const userTurn = [...turns].reverse().find((m) => m.role === "user") ?? null;
    if (!userTurn) return;
    const request: AiMessage[] = [
      sys ?? { role: "system", content: CHAT_SYSTEM },
      ...this.history,
      ...turns,
    ];
    this.inputEl.value = "";
    this.start(request, ctx.label, userTurn);
  }

  private sendAction(): void {
    const mode = this.modeEl.value as AiMode;
    if (mode === "chat") {
      void this.sendChat();
      return;
    }
    if (this.busy) return;
    if (this.deps.getConfig()?.enabled !== true) {
      this.refresh();
      return;
    }
    const note = this.inputEl.value.trim();
    this.note = "";
    this.deps.runScenario(mode, note || undefined);
  }

  private async sendChat(): Promise<void> {
    const text = this.inputEl.value.trim();
    if (!text || this.busy) return;
    const cfg = this.deps.getConfig();
    if (!cfg?.enabled) {
      this.refresh();
      return;
    }
    this.inputEl.value = "";
    const userTurn: AiMessage = { role: "user", content: text };
    const request: AiMessage[] = [
      { role: "system", content: CHAT_SYSTEM },
      ...this.history,
      userTurn,
    ];
    this.start(request, `约 ${text.length} 字符`, userTurn);
  }

  private start(request: AiMessage[], label: string, userTurn: AiMessage | null): void {
    const cfg = this.deps.getConfig();
    if (!cfg?.enabled || this.busy) {
      this.refresh();
      return;
    }
    this.note = "";
    // Show the actual turn input (scenario excerpts included) as the user bubble.
    if (userTurn) this.pushBubble("user", userTurn.content);

    const bubble = el("div", "ai-msg ai-msg-assistant");
    const reasonEl = el("pre", "ai-reason");
    const reasonBox = el("details", "ai-reason-box");
    reasonBox.append(el("summary", "", "推理过程"), reasonEl);
    reasonBox.hidden = true;
    const textEl = el("div", "ai-text", "");
    const tools = el("div", "ai-msg-tools");
    bubble.append(reasonBox, textEl, tools);
    this.logEl.append(bubble);
    this.scroll();

    const reqId = ++this.seq;
    this.active = { reqId, raw: "", textEl, reasonEl, cancelled: false, userTurn };
    this.busy = true;
    this.noticeEl.textContent = `将发送：${label} → ${hostOf(cfg.baseUrl)}`;
    this.refresh();
    textEl.textContent = "…";

    void aiChatStream({ reqId, messages: request }).catch((err) => {
      if (!this.active || this.active.reqId !== reqId) return;
      this.fail(errText(err));
    });
  }

  private async cancel(): Promise<void> {
    if (!this.active) return;
    this.active.cancelled = true;
    this.stopBtn.disabled = true;
    try {
      await aiChatCancel(this.active.reqId);
    } catch (err) {
      this.note = errText(err);
      this.syncStatus();
    } finally {
      this.stopBtn.disabled = false;
    }
  }

  private onEvent(ev: AiStreamEvent): void {
    const a = this.active;
    if (!a || ev.reqId !== a.reqId) return;
    if (ev.kind === "chunk" && ev.delta) {
      a.raw += ev.delta;
      a.textEl.textContent = a.raw;
      this.scroll();
    } else if (ev.kind === "reasoning" && ev.delta) {
      const box = a.reasonEl.parentElement;
      if (box) box.hidden = false;
      a.reasonEl.textContent += ev.delta;
    } else if (ev.kind === "error") {
      this.fail(ipcErrorZh(ev.code ?? "ai_http", ev.error));
    } else if (ev.kind === "done") {
      this.finish();
    }
  }

  private commitTurn(): void {
    const a = this.active;
    if (!a) return;
    if (a.userTurn) this.history.push(a.userTurn);
    a.userTurn = null;
  }

  private fail(message: string): void {
    const a = this.active;
    if (!a) return;
    this.commitTurn();
    const err = el("div", "ai-msg-error", message);
    a.textEl.parentElement?.append(err);
    if (a.raw.trim() === "") a.textEl.textContent = "";
    this.note = "";
    this.stopActive();
    this.statusEl.textContent = message;
  }

  private finish(): void {
    const a = this.active;
    if (!a) return;
    if (a.cancelled) a.raw += a.raw ? "\n（已停止）" : "（已停止）";
    this.commitTurn();
    if (a.raw.trim() === "") {
      a.textEl.textContent = "（没有返回内容）";
    } else {
      a.textEl.textContent = a.raw;
      this.history.push({ role: "assistant", content: a.raw });
      this.addReplyTools(a.textEl.parentElement as HTMLElement, a.raw);
    }
    this.stopActive();
  }

  private stopActive(): void {
    this.active = null;
    this.busy = false;
    this.note = "";
    this.refresh();
  }

  private addReplyTools(host: HTMLElement, raw: string): void {
    const tools = host.querySelector(".ai-msg-tools");
    if (!tools) return;
    tools.textContent = "";
    const copy = el("button", "ai-ghost", "复制");
    copy.type = "button";
    copy.addEventListener("click", () => {
      void navigator.clipboard
        .writeText(raw)
        .then(() => {
          copy.textContent = "已复制";
        })
        .catch(() => {
          this.note = "复制失败（剪贴板不可用）";
          this.syncStatus();
        });
    });
    const saveBtn = el("button", "ai-ghost", "另存为 .md…");
    saveBtn.type = "button";
    saveBtn.addEventListener("click", () => void this.saveReply(raw));
    tools.append(copy, saveBtn);
  }

  private async saveReply(raw: string): Promise<void> {
    try {
      const path = await save({
        defaultPath: "ai-reply.md",
        filters: [{ name: "Markdown", extensions: ["md"] }],
      });
      if (!path) return;
      await aiSaveText(path, raw);
      this.note = `已保存：${path}`;
      this.syncStatus();
    } catch (err) {
      this.note = errText(err);
      this.syncStatus();
    }
  }

  private pushBubble(role: "user" | "assistant", text: string): HTMLElement {
    const bubble = el("div", `ai-msg ai-msg-${role}`);
    bubble.append(el("div", "ai-text", text));
    this.logEl.append(bubble);
    this.scroll();
    return bubble;
  }

  private scroll(): void {
    this.logEl.scrollTop = this.logEl.scrollHeight;
  }
}
