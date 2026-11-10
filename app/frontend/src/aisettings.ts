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
const PRESETS: { id: string; label: string; baseUrl: string; model: string }[] = [
  {
    id: "deepseek",
    label: "DeepSeek",
    baseUrl: "https://api.deepseek.com/v1",
    model: "deepseek-chat",
  },
  {
    id: "zhipu",
    label: "智谱",
    baseUrl: "https://open.bigmodel.cn/api/paas/v4",
    model: "glm-4-flash",
  },
  {
    id: "kimi",
    label: "Kimi",
    baseUrl: "https://api.moonshot.cn/v1",
    model: "moonshot-v1-8k",
  },
  {
    id: "openai",
    label: "OpenAI",
    baseUrl: "https://api.openai.com/v1",
    model: "gpt-4o-mini",
  },
  {
    id: "ollama",
    label: "本机 Ollama",
    baseUrl: "http://127.0.0.1:11434/v1",
    model: "qwen2.5:7b",
  },
];

/** Not a preset: any endpoint that matches no preset above (NoteLume parity). */
const CUSTOM_ID = "custom";

/** Scheme + host check for a non-empty base URL. Empty input has no message. */
function baseUrlProblem(raw: string): string {
  if (!raw) return "启用 AI 前需要填写接口地址。";
  if (!/^https?:\/\//.test(raw)) {
    return "接口地址必须以 http:// 或 https:// 开头。";
  }
  if (/\s/.test(raw)) return "接口地址不能包含空格。";
  let url: URL;
  try {
    url = new URL(raw);
  } catch {
    return "接口地址格式无效，请检查主机名与端口（例如 https://api.example.com/v1）。";
  }
  if (!url.hostname) return "接口地址缺少主机名。";
  return "";
}

/** Hostname used for the loopback no-key allowance (mirrors `is_loopback`). */
function hostnameOf(raw: string): string {
  try {
    return new URL(raw).hostname.toLowerCase();
  } catch {
    return "";
  }
}

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

  // Inline, per-field errors (specific copy; never a silent preset fallback).
  const baseErr = el("div", "ai-err ai-err-base");
  baseErr.setAttribute("role", "alert");
  const modelErr = el("div", "ai-err ai-err-model");
  modelErr.setAttribute("role", "alert");
  const tempErr = el("div", "ai-err ai-err-temp");
  tempErr.setAttribute("role", "alert");

  const baseField = el("label", "ai-field");
  baseField.append(el("span", "ai-label", "接口地址（OpenAI 兼容）"), baseInput, baseErr);
  const testBtn = el("button", "ai-preset ai-test", "测试连接 / 拉取模型…");
  testBtn.type = "button";
  const modelField = el("label", "ai-field");
  modelField.append(
    el("span", "ai-label", "模型（可手填，也可从测试连接的结果选择）"),
    modelInput,
    modelList,
    testBtn,
    el("div", "ai-sub", "模型列表拉取失败或尚未配置时，直接手填模型名即可。"),
    modelErr,
  );
  const tempField = el("label", "ai-field");
  tempField.append(el("span", "ai-label", "温度（0–2）"), tempInput, tempErr);
  const keyField = el("label", "ai-field");
  keyField.append(el("span", "ai-label", "API Key（只写，不回显）"), keyInput, keyHint, clearRow);

  const warn = el(
    "div",
    "ai-warn",
    "凭据说明：Key 以明文保存在本机 config.toml（当前用户的配置目录），不会发往你填写的接口地址以外的任何地方；本机回环地址不填 Key 也可用。",
  );
  const target = el("div", "ai-sub", "");
  const presetBtns: { id: string; btn: HTMLButtonElement }[] = [];
  /** Values typed while "自定义" was selected; restored when returning to it. */
  let customMemory: { baseUrl: string; model: string } | null = null;

  const activePresetId = (base: string): string => {
    const v = base.trim();
    return PRESETS.find((p) => p.baseUrl === v)?.id ?? CUSTOM_ID;
  };

  const syncPresetActive = (): void => {
    const id = activePresetId(baseInput.value);
    for (const { id: btnId, btn } of presetBtns) {
      const on = btnId === id;
      btn.classList.toggle("active", on);
      btn.setAttribute("aria-pressed", on ? "true" : "false");
    }
  };

  const setErr = (node: HTMLElement, msg: string): void => {
    node.textContent = msg;
  };

  const clearErrors = (): void => {
    setErr(baseErr, "");
    setErr(modelErr, "");
    setErr(tempErr, "");
  };

  const clearModelOptions = (): void => {
    modelList.textContent = "";
  };

  /** Remember custom values before a preset overwrites the two inputs. */
  const rememberCustom = (): void => {
    const base = baseInput.value.trim();
    if (base && activePresetId(base) === CUSTOM_ID) {
      customMemory = { baseUrl: base, model: modelInput.value.trim() };
    }
  };

  const selectPreset = (p: (typeof PRESETS)[number]): void => {
    rememberCustom();
    baseInput.value = p.baseUrl;
    modelInput.value = p.model;
    clearModelOptions();
    clearErrors();
    syncPresetActive();
    target.textContent = p.baseUrl;
    status.textContent = "";
  };

  for (const p of PRESETS) {
    const b = el("button", "ai-preset", p.label);
    b.type = "button";
    b.dataset.preset = p.id;
    b.addEventListener("click", () => selectPreset(p));
    presetBtns.push({ id: p.id, btn: b });
    presetRow.append(b);
  }

  const customBtn = el("button", "ai-preset", "自定义");
  customBtn.type = "button";
  customBtn.dataset.preset = CUSTOM_ID;
  customBtn.title = "任意 OpenAI 兼容接口：自由填写 base URL 与模型名";
  customBtn.addEventListener("click", () => {
    if (activePresetId(baseInput.value) === CUSTOM_ID) {
      // Already custom: keep every typed value, just hand over the cursor.
      baseInput.focus();
      return;
    }
    if (customMemory) {
      baseInput.value = customMemory.baseUrl;
      modelInput.value = customMemory.model;
    } else {
      baseInput.value = "";
      modelInput.value = "";
    }
    clearModelOptions();
    clearErrors();
    syncPresetActive();
    target.textContent = baseInput.value.trim();
    status.textContent = "";
    baseInput.focus();
  });
  presetBtns.push({ id: CUSTOM_ID, btn: customBtn });
  presetRow.append(customBtn);

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
    syncPresetActive();
    setErr(baseErr, "");
    setErr(modelErr, "");
  });
  baseInput.addEventListener("blur", () => {
    const base = baseInput.value.trim();
    setErr(baseErr, base ? baseUrlProblem(base) : "");
  });
  modelInput.addEventListener("input", () => setErr(modelErr, ""));

  interface FieldProblem {
    field: HTMLElement | null;
    msg: string;
  }

  /** Validate the whole form, writing inline errors and returning the first one. */
  const validate = (): FieldProblem => {
    clearErrors();
    const base = baseInput.value.trim();
    const model = modelInput.value.trim();
    const temp = Number(tempInput.value);
    const enabling = enabledChk.checked && !cfg?.enabled;
    if (enabling && !ack.checked) {
      return {
        field: ack,
        msg: "开启 AI 前请先勾选“我了解内容会发往上述地址”。",
      };
    }
    // A non-empty endpoint is always checked, even while AI stays disabled.
    const baseMsg = base || enabledChk.checked ? baseUrlProblem(base) : "";
    if (baseMsg) {
      setErr(baseErr, baseMsg);
      return { field: baseInput, msg: baseMsg };
    }
    if (enabledChk.checked && !model) {
      const msg = "启用 AI 前需要填写模型名（可手填，不必等模型列表）。";
      setErr(modelErr, msg);
      return { field: modelInput, msg };
    }
    if (enabledChk.checked) {
      const host = hostnameOf(base);
      const local =
        host === "localhost" || host === "127.0.0.1" || host === "[::1]" || host.startsWith("127.");
      const hasKey = cfg?.hasKey || keyInput.value.trim().length > 0;
      if (!local && (!hasKey || clearKey.checked)) {
        const msg = "这个接口不是本机地址，需要填写 API Key。";
        return { field: keyInput, msg };
      }
    }
    if (!Number.isFinite(temp) || temp < 0 || temp > 2) {
      const msg = "温度必须在 0 到 2 之间。";
      setErr(tempErr, msg);
      return { field: tempInput, msg };
    }
    return { field: null, msg: "" };
  };

  const doSave = async (): Promise<boolean> => {
    if (saving || !cfg) return false;
    const problem = validate();
    if (problem.msg) {
      status.textContent = problem.msg;
      problem.field?.focus();
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
            ? "连接成功，但接口没有返回可用的对话模型；可直接手填模型名。"
            : `连接成功：${ids.length} 个对话模型，可在模型框中选择。`;
      } catch (err) {
        status.textContent = `${errText(err)}（模型列表不可用，可直接在模型框手填模型名）`;
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
    clearErrors();
    syncPresetActive(); // a saved custom endpoint lights up "自定义", not a preset
    refreshKeyHint();
    status.textContent = cfg.enabled ? "" : "AI 当前未启用。";
    saveBtn.disabled = false;
  })();
  saveBtn.disabled = true;
  cancelBtn.focus();
}
