// @vitest-environment jsdom
// Generic prompt dialog (`promptdialog.ts`): shell, keyboard, inline errors,
// busy lock, and the throw-keeps-open contract.
import { beforeEach, describe, expect, it, vi } from "vitest";
import { promptDialog } from "./promptdialog";

function settle(): Promise<void> {
  return new Promise((r) => setTimeout(r, 0));
}

function box(): HTMLElement {
  return document.querySelector<HTMLElement>(".prompt-box")!;
}

function input(index = 0): HTMLInputElement {
  return document.querySelectorAll<HTMLInputElement>(".prompt-input")[index];
}

beforeEach(() => {
  document.body.textContent = "";
});

describe("promptDialog", () => {
  it("renders title, message and placeholders, then submits on Enter", async () => {
    const onSubmit = vi.fn(async () => {});
    const p = promptDialog({
      title: "新建分支",
      message: "默认从 HEAD 创建。",
      fields: [{ key: "name", label: "分支名", placeholder: "feat/x", value: "a" }],
      confirmLabel: "创建",
      onSubmit,
    });

    expect(box().querySelector(".prompt-title")?.textContent).toBe("新建分支");
    expect(box().querySelector(".prompt-msg")?.textContent).toBe("默认从 HEAD 创建。");
    expect(box().querySelector(".prompt-label")?.textContent).toBe("分支名");
    expect(input().placeholder).toBe("feat/x");
    expect(document.activeElement).toBe(input());
    expect(box().querySelector<HTMLButtonElement>(".prompt-btn.primary")?.textContent).toBe("创建");

    input().value = "feat/y";
    input().dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true }));

    await expect(p).resolves.toEqual({ name: "feat/y" });
    expect(onSubmit).toHaveBeenCalledWith({ name: "feat/y" }, undefined);
    expect(document.querySelector(".prompt-box")).toBeNull();
  });

  it("cancels on Escape and on an overlay click", async () => {
    const p1 = promptDialog({ title: "t", fields: [{ key: "x" }] });
    document.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape" }));
    await expect(p1).resolves.toBeNull();
    expect(document.querySelector(".prompt-box")).toBeNull();

    const p2 = promptDialog({ title: "t", fields: [{ key: "x" }] });
    const overlay = document.querySelector<HTMLElement>(".prompt-overlay")!;
    overlay.dispatchEvent(new MouseEvent("pointerdown", { bubbles: true }));
    await expect(p2).resolves.toBeNull();
  });

  it("shows inline field errors and does not call submit while invalid", async () => {
    const onSubmit = vi.fn(async () => {});
    const p = promptDialog({
      title: "t",
      fields: [
        { key: "name", validate: (v) => (v === "" ? "请输入名称" : null) },
        { key: "url", validate: () => null },
      ],
      onSubmit,
    });

    box().querySelector<HTMLButtonElement>(".prompt-btn.primary")!.click();
    await settle();

    const err = box().querySelector<HTMLElement>(".prompt-err")!;
    expect(err.textContent).toBe("请输入名称");
    expect(err.hidden).toBe(false);
    expect(input().classList.contains("invalid")).toBe(true);
    expect(document.activeElement).toBe(input());
    expect(onSubmit).not.toHaveBeenCalled();

    // Typing clears the inline error, then the valid value submits.
    input().value = "ok";
    input().dispatchEvent(new Event("input", { bubbles: true }));
    expect(err.hidden).toBe(true);
    expect(input().classList.contains("invalid")).toBe(false);

    input().dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true }));
    await expect(p).resolves.toEqual({ name: "ok", url: "" });
    expect(onSubmit).toHaveBeenCalledOnce();
  });

  it("locks the form while submitting", async () => {
    let release!: () => void;
    const gate = new Promise<void>((r) => {
      release = r;
    });
    const p = promptDialog({ title: "t", fields: [{ key: "x" }], onSubmit: () => gate });

    const confirm = box().querySelector<HTMLButtonElement>(".prompt-btn.primary")!;
    const cancel = box().querySelector<HTMLButtonElement>(".prompt-btn:not(.primary)")!;
    confirm.click();
    await settle();

    expect(confirm.disabled).toBe(true);
    expect(cancel.disabled).toBe(true);
    expect(input().disabled).toBe(true);
    expect(confirm.textContent).toBe("处理中…");

    // Escape is ignored while busy, so a slow write cannot be half-cancelled.
    document.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape" }));
    expect(document.querySelector(".prompt-box")).not.toBeNull();

    release();
    await expect(p).resolves.toEqual({ x: "" });
  });

  it("keeps the dialog open with the thrown message when submit fails", async () => {
    const p = promptDialog({
      title: "t",
      fields: [{ key: "x", value: "typed" }],
      onSubmit: async () => {
        throw new Error("远端地址不合法");
      },
    });

    box().querySelector<HTMLButtonElement>(".prompt-btn.primary")!.click();
    await settle();

    const formErr = box().querySelector<HTMLElement>(".prompt-form-err")!;
    expect(formErr.hidden).toBe(false);
    expect(formErr.textContent).toBe("远端地址不合法");
    expect(input().value).toBe("typed");
    expect(box().querySelector<HTMLButtonElement>(".prompt-btn.primary")!.disabled).toBe(false);

    document.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape" }));
    await expect(p).resolves.toBeNull();
  });
});
