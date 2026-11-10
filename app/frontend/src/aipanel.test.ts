// @vitest-environment jsdom
// AI section UI contract: disabled guidance, one continuous transcript across
// scenario modes, pre-send egress notice, streaming, stop, collapse. IPC is
// mocked — the wire itself is covered by ai.rs `stream_tests`.
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AiConfigView, AiMessage, AiStreamEvent } from "./ipc";

const h = vi.hoisted(() => ({
  listeners: [] as ((ev: Record<string, unknown>) => void)[],
  stream: vi.fn((_args?: unknown) => Promise.resolve()),
  cancel: vi.fn((_id?: number) => Promise.resolve(true)),
  saveText: vi.fn((_path?: string, _text?: string) => Promise.resolve()),
}));

vi.mock("./ipc", () => ({
  aiChatStream: (args: unknown) => h.stream(args as never),
  aiChatCancel: (id: number) => h.cancel(id),
  aiSaveText: (path: string, text: string) => h.saveText(path, text as never),
  listenAiEvents: (cb: (ev: Record<string, unknown>) => void) => {
    h.listeners.push(cb);
    return Promise.resolve(() => {});
  },
  ipcErrorZh: (code: string | null, msg: string | null) => `${code}:${msg}`,
  IpcError: class IpcError extends Error {
    zh = "";
  },
}));

import { AiPanel, type AiScenario } from "./aipanel";

function cfg(enabled: boolean): AiConfigView {
  return {
    enabled,
    baseUrl: "http://127.0.0.1:11434/v1",
    model: "local-model",
    temperature: 0.2,
    hasKey: false,
    local: true,
  };
}

function setup(enabled = true) {
  const runScenario = vi.fn<(kind: AiScenario, note?: string) => void>();
  const panel = new AiPanel({
    getConfig: () => cfg(enabled),
    openSettings: () => {},
    runScenario,
  });
  document.body.append(panel.root);
  return { panel, runScenario };
}

function send(panel: AiPanel): HTMLButtonElement {
  return panel.root.querySelector(".ai-send") as HTMLButtonElement;
}

function type(panel: AiPanel, text: string): void {
  const input = panel.root.querySelector(".ai-input-box") as HTMLTextAreaElement;
  input.value = text;
  input.dispatchEvent(new Event("input"));
}

function emit(ev: AiStreamEvent): void {
  h.listeners[0](ev as unknown as Record<string, unknown>);
}

beforeEach(() => {
  h.listeners.length = 0;
  h.stream.mockClear();
  h.cancel.mockClear();
  h.saveText.mockClear();
});

afterEach(() => {
  document.body.innerHTML = "";
});

describe("AI panel · disabled state", () => {
  it("shows guidance and disables every egress control", () => {
    const { panel } = setup(false);
    expect(panel.root.querySelector(".ai-badge")?.textContent).toBe("未启用");
    expect((panel.root.querySelector(".ai-empty") as HTMLElement).hidden).toBe(false);
    expect(panel.root.querySelector(".ai-status")?.textContent).toContain("默认关闭");
    expect((panel.root.querySelector(".ai-input-box") as HTMLTextAreaElement).disabled).toBe(true);
    expect((panel.root.querySelector(".ai-mode") as HTMLSelectElement).disabled).toBe(true);
    expect(send(panel).disabled).toBe(true);
  });

  it("renders a blocked reason in the transcript without any request", () => {
    const { panel } = setup(false);
    panel.reportBlocked("AI 未启用：请先在 AI 设置中开启并填写接口");
    expect(panel.root.querySelector(".ai-status")?.textContent).toContain("AI 未启用");
    expect(panel.root.querySelector(".ai-msg-error")).toBeNull();
    expect(h.stream).not.toHaveBeenCalled();
  });
});

describe("AI panel · single transcript", () => {
  it("sends a chat turn with the history and announces the egress", () => {
    const { panel } = setup();
    type(panel, "你好");
    expect(send(panel).disabled).toBe(false);
    send(panel).click();

    expect(h.stream).toHaveBeenCalledTimes(1);
    const req = h.stream.mock.calls[0][0] as { reqId: number; messages: AiMessage[] };
    expect(req.messages.map((m) => m.role)).toEqual(["system", "user"]);
    expect(req.messages[1].content).toBe("你好");
    expect(panel.root.querySelector(".ai-notice")?.textContent).toBe(
      "将发送：约 2 字符 → 127.0.0.1:11434",
    );
    expect(panel.isBusy).toBe(true);

    emit({ reqId: req.reqId, kind: "chunk", delta: "你好" } as AiStreamEvent);
    emit({ reqId: req.reqId, kind: "chunk", delta: "呀" } as AiStreamEvent);
    emit({ reqId: req.reqId, kind: "done" } as AiStreamEvent);

    expect(panel.root.querySelector(".ai-msg-assistant .ai-text")?.textContent).toBe("你好呀");
    expect(panel.isBusy).toBe(false);
  });

  it("keeps prior turns when switching to a scenario mode", () => {
    const { panel } = setup();

    // Turn 1: plain chat.
    type(panel, "第一问");
    send(panel).click();
    const first = h.stream.mock.calls[0][0] as { reqId: number; messages: AiMessage[] };
    emit({ reqId: first.reqId, kind: "chunk", delta: "答一" } as AiStreamEvent);
    emit({ reqId: first.reqId, kind: "done" } as AiStreamEvent);

    // Turn 2: scenario, with its input shown as the user message.
    panel.runScenario({
      label: "选中 2 行（约 30 字符）",
      messages: [
        { role: "system", content: "SCENARIO-SYSTEM" },
        { role: "user", content: "L10 …\nL11 …" },
      ],
    });
    expect(h.stream).toHaveBeenCalledTimes(2);
    const second = h.stream.mock.calls[1][0] as { reqId: number; messages: AiMessage[] };
    expect(second.messages.map((m) => m.role)).toEqual([
      "system",
      "user",
      "assistant",
      "user",
    ]);
    expect(second.messages[0].content).toBe("SCENARIO-SYSTEM");
    expect(second.messages[3].content).toContain("L10");
    expect(panel.root.querySelector(".ai-notice")?.textContent).toContain("选中 2 行");
    // History is not cleared by the mode switch.
    expect(panel.root.querySelectorAll(".ai-msg-user")).toHaveLength(2);
  });

  it("mode selector triggers the scenario with an optional note", () => {
    const { panel, runScenario } = setup();
    const mode = panel.root.querySelector(".ai-mode") as HTMLSelectElement;
    mode.value = "explain";
    mode.dispatchEvent(new Event("change"));
    expect(send(panel).disabled).toBe(false);

    send(panel).click();
    expect(runScenario).toHaveBeenLastCalledWith("explain", undefined);

    type(panel, "重点看超时");
    send(panel).click();
    expect(runScenario).toHaveBeenLastCalledWith("explain", "重点看超时");
  });

  it("shows a blocked scenario reason inline", () => {
    const { panel } = setup();
    panel.reportBlocked("请先点击列表中的日志行（Shift+点击可扩选）");
    expect(panel.root.querySelector(".ai-msg-error")?.textContent).toContain("Shift+点击");
    expect(h.stream).not.toHaveBeenCalled();
  });

  it("stop calls ai_chat_cancel", async () => {
    const { panel } = setup();
    type(panel, "hi");
    send(panel).click();
    const stop = panel.root.querySelector(".ai-stop") as HTMLButtonElement;
    expect(stop.hidden).toBe(false);
    stop.click();
    await Promise.resolve();
    await Promise.resolve();
    expect(h.cancel).toHaveBeenCalledWith(1);
  });

  it("right-click offers copy for a turn and save-as for a reply", () => {
    const { panel } = setup();
    type(panel, "问题");
    send(panel).click();
    const req = h.stream.mock.calls[0][0] as { reqId: number };
    emit({ reqId: req.reqId, kind: "chunk", delta: "回答" } as AiStreamEvent);
    emit({ reqId: req.reqId, kind: "done" } as AiStreamEvent);

    const open = (selector: string): string[] => {
      const bubble = panel.root.querySelector(selector) as HTMLElement;
      bubble.dispatchEvent(
        new MouseEvent("contextmenu", { bubbles: true, cancelable: true, clientX: 10, clientY: 10 }),
      );
      const menu = document.querySelector(".ctx-menu");
      const labels = [...(menu?.querySelectorAll(".ctx-menu-label") ?? [])].map(
        (e) => e.textContent ?? "",
      );
      menu?.remove();
      return labels;
    };

    expect(open(".ai-msg-user")).toEqual(["复制此条内容", "复制整段对话"]);
    expect(open(".ai-msg-assistant")).toEqual(["复制此条内容", "复制整段对话", "另存为 .md…"]);
  });

  it("collapses without losing the header", () => {
    const { panel } = setup();
    panel.setExpanded(false);
    expect(panel.isExpanded).toBe(false);
    expect(panel.root.classList.contains("ai-collapsed")).toBe(true);
    panel.setExpanded(true);
    expect(panel.root.classList.contains("ai-collapsed")).toBe(false);
  });

  it("switches copy and disables log-only modes for non-log documents", () => {
    const { panel } = setup(false);
    const title = panel.root.querySelector(".ai-empty-title") as HTMLElement;
    const body = panel.root.querySelector(".ai-empty-body") as HTMLElement;
    const mode = panel.root.querySelector(".ai-mode") as HTMLSelectElement;
    const input = panel.root.querySelector(".ai-input-box") as HTMLTextAreaElement;
    const opt = (value: string): HTMLOptionElement =>
      [...mode.options].find((o) => o.value === value) as HTMLOptionElement;

    expect(title.textContent).toBe("针对当前日志提问");
    expect(opt("explain").textContent).toBe("解释选中行");
    expect(opt("errors").disabled).toBe(false);
    expect(input.placeholder).toContain("询问这段日志");

    panel.setDocKind(false);
    expect(title.textContent).toBe("针对当前文件提问");
    expect(body.textContent).toContain("仅适用于日志");
    expect(opt("explain").textContent).toBe("解释选中内容");
    expect(opt("errors").disabled).toBe(true);
    expect(opt("query").disabled).toBe(true);
    expect(input.placeholder).toContain("询问当前内容");

    mode.value = "explain";
    mode.dispatchEvent(new Event("change"));
    expect(input.placeholder).toContain("解释选中的内容");

    panel.setDocKind(true);
    expect(title.textContent).toBe("针对当前日志提问");
    expect(opt("explain").textContent).toBe("解释选中行");
    expect(opt("errors").disabled).toBe(false);
  });
});
