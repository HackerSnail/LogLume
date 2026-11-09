//! AI settings modal (opt-in). AI ships disabled; this dialog is the only place
//! that turns it on. The API key is write-only from the webview's perspective:
//! `ai_config_get` reports `hasKey`, never the key itself.
import { aiConfigGet, aiConfigSet, aiModelsList, IpcError, type AiConfigView } from "./ipc";

export interface AiSettingsOpts {
  /** Called with the freshly saved view so the caller can update its cache. */
  onSaved: (cfg: AiConfigView) => void;
  onClose: () => void;
}

/** Presets are endpoint + model only — never keys. */
const PRESETS: { label: string; baseUrl: string; model: string }[] = [
  { label: "DeepSeek", baseUrl: "https://api.deepseek.com/v1", model: "deepseek-chat" },
  {
    label: "智谱",
    baseUrl: "https://open.bigmodel.cn/api/paas/v4",
    model: "glm-4-flash",
  },
  { label: "Kimi", baseUrl: "https://api.moonshot.cn/v1", model: "moonshot-v1-8k" },
  { label: "OpenAI", baseUrl: "https://api.openai.com/v1", model: "gpt-4o-mini" },
  {
    label: "本机 Ollama",
    baseUrl: "http://127.0.0.1:11434/v1",
    model: "qwen2.5:7b",
  },
];

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

function errText(err: unknown): string {
  return err instanceof IpcError ? err.zh : String(err);
}

export function openAiSettingsModal(opts: AiSettingsOpts): void {
  if (document.querySelector(".ai-modal")) return;

  const overlay = el("div", "keymap-modal ai-modal");
  overlay.setAttribute("role", "dialog");
  overlay.setAttribute("aria-modal", "true");
  const box = el("div", "keymap-box ai-box");
  const title = el("div", "keymap-title", "AI 设置");
  const note = el(
    "div",
    "keymap-note",
    "AI 默认关闭。只有在这里显式开启后，AI 面板才会向你填写的接口地址发送内容；未开启、未配置或缺少 Key 时，相关操作会直接提示，不会静默失败。",
  );
  const form = el("div", "ai-form");
  const status = el("div", "keymap-status", "");
  const foot = el("div", "keymap-foot");
  const appRoot = document.getElementById("app");

  let cfg: AiConfigView | null = null;
  let saving = false;

  const close = (): void => {
    if (saving) return;
    overlay.remove();
    appRoot?.removeAttribute("inert");
    document.removeEventListener("keydown", onKey, true);
    opts.onClose();
  };

  const onKey = (e: KeyboardEvent): void => {
    if (e.key === "Escape" && !saving) {
      e.preventDefault();
      e.stopPropagation();
      close();
    }
  };

  // Field widgets are created once, then filled from `cfg` after the fetch.
  const enabledChk = el("input");
  enabledChk.type = "checkbox";
  enabledChk.id = "ai-enabled";
  const enabledRow = el("label", "ai-toggle");
  enabledRow.append(enabledChk, el("span", "", "启用 AI（默认关闭）"));

  const presetRow = el("div", "ai-presets");
  const baseInput = el("input", "ai-input");
  baseInput.type = "text";
  baseInput.placeholder = "https://api.example.com/v1";
  baseInput.spellcheck = false;
  const modelInput = el("input", "ai-input");
  modelInput.type = "text";
  modelInput.placeholder = "模型名，如 deepseek-chat";
  modelInput.spellcheck = false;
  modelInput.setAttribute("list", "ai-models");
  const modelList = el("datalist");
  modelList.id = "ai-models";
  const tempInput = el("input", "ai-input ai-temp");
  tempInput.type = "number";
  tempInput.min = "0";
  tempInput.max = "2";
  tempInput.step = "0.1";
  tempInput.value = "0.2";

  const keyInput = el("input", "ai-input");
  keyInput.type = "password";
  keyInput.autocomplete = "off";
  keyInput.spellcheck = false;
  const keyHint = el("div", "ai-sub");
  const clearKey = el("input");
  clearKey.type = "checkbox";
  const clearRow = el("label", "ai-clear");
  clearRow.append(clearKey, el("span", "", "清除已保存的 Key"));

  const ack = el("input");
  ack.type = "checkbox";
  ack.id = "ai-ack";
  const ackRow = el("label", "ai-ack");
  ackRow.append(ack, el("span", "", "我了解：发送的日志内容会发往上面填写的接口地址"));

  const baseField = el("label", "ai-field");
  baseField.append(el("span", "ai-label", "接口地址（OpenAI 兼容）"), baseInput);
  const testBtn = el("button", "ai-preset ai-test", "测试连接 / 拉取模型…");
  testBtn.type = "button";
  const modelField = el("label", "ai-field");
  modelField.append(el("span", "ai-label", "模型（可从测试连接的结果选择）"), modelInput, modelList, testBtn);
  const tempField = el("label", "ai-field");
  tempField.append(el("span", "ai-label", "温度（0–2）"), tempInput);
  const keyField = el("label", "ai-field");
  keyField.append(el("span", "ai-label", "API Key（只写，不回显）"), keyInput, keyHint, clearRow);

  const warn = el(
    "div",
    "ai-warn",
    "凭据说明：Key 以明文保存在本机 config.toml（当前用户的配置目录），不会发往你填写的接口地址以外的任何地方；本机回环地址不填 Key 也可用。",
  );
  const target = el("div", "ai-sub", "");

  for (const p of PRESETS) {
    const b = el("button", "ai-preset", p.label);
    b.type = "button";
    b.addEventListener("click", () => {
      baseInput.value = p.baseUrl;
      modelInput.value = p.model;
      target.textContent = p.baseUrl;
    });
    presetRow.append(b);
  }

  const saveBtn = el("button", "keymap-close", "保存");
  const cancelBtn = el("button", "keymap-reset-all", "取消");
  cancelBtn.addEventListener("click", () => close());

  const refreshKeyHint = (): void => {
    const typed = keyInput.value.trim();
    if (typed) {
      keyHint.textContent = "将替换已保存的 Key（不会回显）。";
    } else if (clearKey.checked) {
      keyHint.textContent = "保存后会删除已保存的 Key。";
    } else if (cfg?.hasKey) {
      keyHint.textContent = "已保存 Key：留空保持不变。";
    } else {
      keyHint.textContent = "尚未保存 Key。";
    }
  };
  keyInput.addEventListener("input", refreshKeyHint);
  clearKey.addEventListener("change", refreshKeyHint);
  baseInput.addEventListener("input", () => {
    target.textContent = baseInput.value.trim();
  });

  const validate = (): string => {
    const base = baseInput.value.trim();
    const model = modelInput.value.trim();
    const temp = Number(tempInput.value);
    const enabling = enabledChk.checked && !cfg?.enabled;
    if (enabling && !ack.checked) {
      return "开启 AI 前请先勾选“我了解内容会发往上述地址”。";
    }
    if (enabledChk.checked) {
      if (!base) return "启用 AI 前需要填写接口地址。";
      if (!/^https?:\/\/\S+$/i.test(base)) return "接口地址必须以 http:// 或 https:// 开头。";
      if (!model) return "启用 AI 前需要填写模型名。";
      const local = /^https?:\/\/(127\.|localhost|\[::1\])/i.test(base);
      const hasKey = cfg?.hasKey || keyInput.value.trim().length > 0;
      if (!local && (!hasKey || clearKey.checked)) {
        return "这个接口不是本机地址，需要填写 API Key。";
      }
    }
    if (!Number.isFinite(temp) || temp < 0 || temp > 2) return "温度必须在 0 到 2 之间。";
    return "";
  };

  const doSave = async (): Promise<boolean> => {
    if (saving || !cfg) return false;
    const problem = validate();
    if (problem) {
      status.textContent = problem;
      return false;
    }
    saving = true;
    saveBtn.disabled = true;
    cancelBtn.disabled = true;
    status.textContent = "保存中…";
    const typed = keyInput.value.trim();
    const apiKey = typed !== "" ? typed : clearKey.checked ? "" : null;
    try {
      const next = await aiConfigSet({
        enabled: enabledChk.checked,
        baseUrl: baseInput.value.trim(),
        model: modelInput.value.trim(),
        apiKey,
        temperature: Number(tempInput.value),
      });
      cfg = next;
      keyInput.value = "";
      clearKey.checked = false;
      refreshKeyHint();
      status.textContent = next.enabled ? "已保存并启用。" : "已保存（未启用）。";
      opts.onSaved(next);
      return true;
    } catch (err) {
      status.textContent = errText(err);
      return false;
    } finally {
      saving = false;
      saveBtn.disabled = false;
      cancelBtn.disabled = false;
    }
  };
  saveBtn.addEventListener("click", () => void doSave());

  // "Test" is an egress action too: it saves (so Rust sees the same config) and
  // only runs while the enable box is checked. It never sends log content.
  testBtn.addEventListener("click", () => {
    void (async () => {
      if (!enabledChk.checked) {
        status.textContent = "测试连接也会联网：请先勾选启用并保存。";
        return;
      }
      if (!(await doSave())) return;
      status.textContent = "正在连接…";
      testBtn.disabled = true;
      try {
        const ids = await aiModelsList();
        modelList.textContent = "";
        for (const id of ids.slice(0, 200)) {
          const opt = el("option");
          opt.value = id;
          modelList.append(opt);
        }
        status.textContent =
          ids.length === 0
            ? "连接成功，但接口没有返回可用的对话模型。"
            : `连接成功：${ids.length} 个对话模型，可在模型框中选择。`;
      } catch (err) {
        status.textContent = errText(err);
      } finally {
        testBtn.disabled = false;
      }
    })();
  });

  foot.append(cancelBtn, saveBtn);
  form.append(
    enabledRow,
    presetRow,
    baseField,
    modelField,
    tempField,
    keyField,
    ackRow,
    warn,
    target,
  );
  box.append(title, note, form, status, foot);
  overlay.append(box);
  overlay.addEventListener("mousedown", (e) => {
    if (e.target === overlay) close();
  });
  document.body.append(overlay);
  appRoot?.setAttribute("inert", "");
  document.addEventListener("keydown", onKey, true);

  status.textContent = "读取设置…";
  void (async () => {
    try {
      cfg = await aiConfigGet();
    } catch (err) {
      status.textContent = errText(err);
      saveBtn.disabled = true;
      return;
    }
    enabledChk.checked = cfg.enabled;
    baseInput.value = cfg.baseUrl;
    modelInput.value = cfg.model;
    tempInput.value = String(cfg.temperature);
    ack.checked = cfg.enabled; // already opted in: no need to re-confirm
    target.textContent = cfg.baseUrl;
    refreshKeyHint();
    status.textContent = cfg.enabled ? "" : "AI 当前未启用。";
    saveBtn.disabled = false;
  })();
  saveBtn.disabled = true;
  cancelBtn.focus();
}
