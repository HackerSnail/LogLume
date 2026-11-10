// @vitest-environment jsdom
// AI settings contract: custom provider (free base URL + model) sits beside the
// presets, switching never drops what the user typed, and validation is
// specific + inline. IPC is mocked; key handling is pinned by ipc.test.ts.
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AiConfigView } from "./ipc";

const h = vi.hoisted(() => ({
  get: vi.fn<() => Promise<AiConfigView>>(),
  set: vi.fn<(args: Record<string, unknown>) => Promise<AiConfigView>>(),
  models: vi.fn<() => Promise<string[]>>(),
}));

vi.mock("./ipc", () => ({
  aiConfigGet: () => h.get(),
  aiConfigSet: (args: Record<string, unknown>) => h.set(args),
  aiModelsList: () => h.models(),
  IpcError: class IpcError extends Error {
    zh = "";
  },
}));

import { openAiSettingsModal } from "./aisettings";

function cfg(over: Partial<AiConfigView> = {}): AiConfigView {
  return {
    enabled: false,
    baseUrl: "",
    model: "",
    temperature: 0.2,
    hasKey: false,
    local: false,
    ...over,
  };
}

const flush = () => new Promise((r) => setTimeout(r, 0));

function open(initial: AiConfigView = cfg()) {
  h.get.mockResolvedValue(initial);
  const onSaved = vi.fn();
  const onClose = vi.fn();
  openAiSettingsModal({ onSaved, onClose });
  return { onSaved, onClose };
}

const preset = (id: string) =>
  document.querySelector(`[data-preset="${id}"]`) as HTMLButtonElement;
const baseInput = () =>
  document.querySelector('input[placeholder="https://api.example.com/v1"]') as HTMLInputElement;
const modelInput = () =>
  document.querySelector('input[placeholder="模型名，如 deepseek-chat"]') as HTMLInputElement;
const err = (kind: "base" | "model" | "temp") =>
  document.querySelector(`.ai-err-${kind}`) as HTMLElement;
const saveBtn = () => document.querySelector(".keymap-close") as HTMLButtonElement;

function typeInto(input: HTMLInputElement, value: string): void {
  input.value = value;
  input.dispatchEvent(new Event("input"));
}

beforeEach(() => {
  h.get.mockReset();
  h.set.mockReset();
  h.models.mockReset();
  h.set.mockImplementation(async (args) =>
    cfg({
      enabled: Boolean(args.enabled),
      baseUrl: String(args.baseUrl),
      model: String(args.model),
      temperature: Number(args.temperature ?? 0.2),
      hasKey: true,
    }),
  );
});

afterEach(() => {
  document.querySelector(".ai-modal")?.remove();
  document.body.innerHTML = "";
});

describe("AI settings: custom provider", () => {
  it("lists 自定义 beside the presets and keeps typed values across switches", async () => {
    open();
    await flush();

    for (const id of ["deepseek", "zhipu", "kimi", "openai", "ollama", "custom"]) {
      expect(preset(id), id).not.toBeNull();
    }
    expect(preset("custom").classList.contains("active")).toBe(true); // empty URL

    typeInto(baseInput(), "https://gw.example.com/v1");
    typeInto(modelInput(), "my-model");
    expect(preset("custom").classList.contains("active")).toBe(true);

    preset("deepseek").click();
    expect(baseInput().value).toBe("https://api.deepseek.com/v1");
    expect(modelInput().value).toBe("deepseek-chat");
    expect(preset("deepseek").classList.contains("active")).toBe(true);
    expect(preset("custom").classList.contains("active")).toBe(false);

    preset("custom").click();
    expect(baseInput().value).toBe("https://gw.example.com/v1");
    expect(modelInput().value).toBe("my-model");
    expect(preset("custom").classList.contains("active")).toBe(true);
  });

  it("echoes a saved custom endpoint and never renders the key", async () => {
    open(cfg({ baseUrl: "https://gw.example.com/v1", model: "my-model", hasKey: true }));
    await flush();

    expect(baseInput().value).toBe("https://gw.example.com/v1");
    expect(modelInput().value).toBe("my-model");
    expect(preset("custom").classList.contains("active")).toBe(true);
    expect(preset("deepseek").classList.contains("active")).toBe(false);
    expect((document.querySelector('input[type="password"]') as HTMLInputElement).value).toBe("");
    expect(document.querySelector(".ai-clear")!.parentElement!.textContent).toContain(
      "已保存 Key：留空保持不变",
    );
  });

  it("shows specific inline errors instead of silently falling back to a preset", async () => {
    open();
    await flush();
    (document.querySelector("#ai-enabled") as HTMLInputElement).checked = true;
    (document.querySelector("#ai-ack") as HTMLInputElement).checked = true;

    typeInto(baseInput(), "api.example.com/v1");
    typeInto(modelInput(), "my-model");
    saveBtn().click();
    await flush();
    expect(err("base").textContent).toContain("http:// 或 https://");
    expect(h.set).not.toHaveBeenCalled();
    expect(baseInput().value).toBe("api.example.com/v1"); // not rewritten to a preset

    typeInto(baseInput(), "https://");
    saveBtn().click();
    await flush();
    expect(err("base").textContent).toMatch(/格式无效|主机名/);
    expect(h.set).not.toHaveBeenCalled();

    typeInto(baseInput(), "https://gw.example.com/v1");
    typeInto(modelInput(), "");
    saveBtn().click();
    await flush();
    expect(err("model").textContent).toContain("模型名");
    expect(h.set).not.toHaveBeenCalled();
  });

  it("still accepts a manually typed model when the model list fails", async () => {
    open(cfg({ enabled: true, baseUrl: "https://gw.example.com/v1", model: "my-model", hasKey: true }));
    await flush();
    h.models.mockRejectedValue(new Error("网络不通"));

    (document.querySelector(".ai-test") as HTMLButtonElement).click();
    await flush();
    await flush();

    expect(h.models).toHaveBeenCalledTimes(1);
    const status = document.querySelector(".keymap-status") as HTMLElement;
    expect(status.textContent).toContain("手填");
    expect(modelInput().disabled).toBe(false);
  });

  it("saves the custom endpoint/model and keeps an untouched key", async () => {
    const { onSaved } = open();
    await flush();
    typeInto(baseInput(), "https://gw.example.com/v1");
    typeInto(modelInput(), "my-model");

    saveBtn().click();
    await flush();

    expect(h.set).toHaveBeenCalledWith(
      expect.objectContaining({
        baseUrl: "https://gw.example.com/v1",
        model: "my-model",
        apiKey: null,
      }),
    );
    expect(onSaved).toHaveBeenCalled();
  });
});
