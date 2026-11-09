// Generic in-app text prompt (NoteLume's `prompt-dialog.js`): one modal with
// title / message / fields / optional extra controls, Enter to confirm, Esc to
// cancel, per-field inline errors and a locked state while submitting.
//
// `window.prompt` is unreliable inside the Tauri webview, so every text input
// (new branch, remote URL, …) goes through this.

export interface PromptField {
  key: string;
  label?: string;
  placeholder?: string;
  value?: string;
  mono?: boolean;
  /** Return an error message for inline display, or null when the value is ok. */
  validate?: (value: string, all: Record<string, string>) => string | null;
}

export interface PromptOptions {
  title: string;
  message?: string;
  fields: PromptField[];
  /**
   * Extra controls (selects, checkboxes) rendered above the button row.
   * Return a getter for whatever the caller needs back; the dialog only
   * treats it as opaque data.
   */
  extra?: (body: HTMLElement) => (() => unknown) | void;
  confirmLabel?: string;
  cancelLabel?: string;
  /**
   * Runs after local validation passes. Throwing keeps the dialog open and
   * shows the thrown message inline, so backend rejections never close the
   * form or lose what the user typed.
   */
  onSubmit?: (values: Record<string, string>, extra: unknown) => Promise<void>;
}

function div(cls: string): HTMLDivElement {
  const n = document.createElement("div");
  n.className = cls;
  return n;
}

/** Resolves with the field values on confirm, or null when cancelled. */
export function promptDialog(options: PromptOptions): Promise<Record<string, string> | null> {
  return new Promise((resolve) => {
    const previous = document.activeElement as HTMLElement | null;
    const overlay = div("prompt-overlay");
    const box = div("prompt-box");
    const titleId = `prompt-title-${Math.random().toString(36).slice(2, 8)}`;
    box.setAttribute("role", "dialog");
    box.setAttribute("aria-modal", "true");
    box.setAttribute("aria-labelledby", titleId);

    const title = div("prompt-title");
    title.id = titleId;
    title.textContent = options.title;
    box.append(title);

    if (options.message) {
      const msg = div("prompt-msg");
      msg.textContent = options.message;
      box.append(msg);
    }

    const body = div("prompt-body");
    const inputs = new Map<string, HTMLInputElement>();
    const errors = new Map<string, HTMLElement>();
    for (const field of options.fields) {
      const wrap = div("prompt-field");
      if (field.label) {
        const label = document.createElement("label");
        label.className = "prompt-label";
        label.textContent = field.label;
        wrap.append(label);
      }
      const input = document.createElement("input");
      input.type = "text";
      input.className = field.mono ? "prompt-input prompt-mono" : "prompt-input";
      input.value = field.value ?? "";
      input.placeholder = field.placeholder ?? "";
      input.spellcheck = false;
      input.autocomplete = "off";
      const err = div("prompt-err");
      err.hidden = true;
      input.addEventListener("input", () => {
        input.classList.remove("invalid");
        err.hidden = true;
      });
      wrap.append(input, err);
      body.append(wrap);
      inputs.set(field.key, input);
      errors.set(field.key, err);
    }

    let extraValue: (() => unknown) | null = null;
    if (options.extra) {
      const slot = div("prompt-extra");
      body.append(slot);
      extraValue = options.extra(slot) ?? null;
    }
    box.append(body);

    const formErr = div("prompt-form-err");
    formErr.hidden = true;
    box.append(formErr);

    const row = div("prompt-row");
    const cancel = document.createElement("button");
    cancel.type = "button";
    cancel.className = "prompt-btn";
    cancel.textContent = options.cancelLabel ?? "取消";
    const confirm = document.createElement("button");
    confirm.type = "button";
    confirm.className = "prompt-btn primary";
    confirm.textContent = options.confirmLabel ?? "确定";
    row.append(cancel, confirm);
    box.append(row);

    overlay.append(box);
    document.body.append(overlay);

    let done = false;
    let busy = false;

    const close = (value: Record<string, string> | null): void => {
      if (done) return;
      done = true;
      document.removeEventListener("keydown", onKey, true);
      overlay.remove();
      previous?.focus?.();
      resolve(value);
    };

    const readValues = (): Record<string, string> => {
      const out: Record<string, string> = {};
      for (const [key, input] of inputs) out[key] = input.value.trim();
      return out;
    };

    const setBusy = (on: boolean): void => {
      busy = on;
      confirm.disabled = on;
      cancel.disabled = on;
      for (const input of inputs.values()) input.disabled = on;
      confirm.textContent = on ? "处理中…" : options.confirmLabel ?? "确定";
      box.classList.toggle("busy", on);
    };

    const showFieldError = (key: string, message: string): void => {
      const input = inputs.get(key);
      const err = errors.get(key);
      if (!input || !err) return;
      err.textContent = message;
      err.hidden = false;
      input.classList.add("invalid");
      input.focus();
    };

    const submit = async (): Promise<void> => {
      if (busy || done) return;
      formErr.hidden = true;
      const values = readValues();
      for (const field of options.fields) {
        const message = field.validate?.(values[field.key] ?? "", values) ?? null;
        if (message) {
          showFieldError(field.key, message);
          return;
        }
      }
      if (!options.onSubmit) {
        close(values);
        return;
      }
      setBusy(true);
      try {
        await options.onSubmit(values, extraValue?.());
        close(values);
      } catch (err) {
        setBusy(false);
        formErr.textContent = err instanceof Error ? err.message : String(err);
        formErr.hidden = false;
        box.querySelector<HTMLInputElement>(".prompt-input")?.focus();
      }
    };

    function onKey(e: KeyboardEvent): void {
      if (e.key === "Escape") {
        e.preventDefault();
        e.stopPropagation();
        if (!busy) close(null);
        return;
      }
      if (e.key === "Enter" && !e.isComposing) {
        e.preventDefault();
        e.stopPropagation();
        void submit();
      }
    }

    confirm.addEventListener("click", () => void submit());
    cancel.addEventListener("click", () => close(null));
    overlay.addEventListener("pointerdown", (e) => {
      if (e.target === overlay && !busy) close(null);
    });
    document.addEventListener("keydown", onKey, true);

    const first = options.fields.length > 0 ? inputs.get(options.fields[0].key) : null;
    first?.focus();
    first?.select();
  });
}
