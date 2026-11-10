// @vitest-environment jsdom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { clampMenuPosition, closeContextMenu, showContextMenu } from "./contextmenu";

beforeEach(() => {
  closeContextMenu();
  document.body.textContent = "";
  vi.restoreAllMocks();
});

describe("context menu", () => {
  it("keeps the menu inside the viewport with a 4px gutter", () => {
    expect(clampMenuPosition(5, 5, 100, 80, 800, 600)).toEqual({ left: 5, top: 5 });
    expect(clampMenuPosition(900, 700, 100, 80, 800, 600)).toEqual({ left: 696, top: 516 });
    expect(clampMenuPosition(-20, -20, 100, 80, 800, 600)).toEqual({ left: 4, top: 4 });
  });

  it("renders items and runs the clicked action", () => {
    const run = vi.fn();
    showContextMenu(10, 20, [{ label: "复制", hint: "Ctrl+C", run }]);
    const item = document.querySelector<HTMLButtonElement>(".ctx-menu-item");
    expect(item).not.toBeNull();
    expect(item!.textContent).toContain("复制");
    expect(item!.textContent).toContain("Ctrl+C");
    item!.click();
    expect(run).toHaveBeenCalledTimes(1);
    expect(document.querySelector(".ctx-menu")).toBeNull();
  });

  it("does not run disabled items", () => {
    const run = vi.fn();
    showContextMenu(10, 20, [{ label: "粘贴", disabled: true, run }]);
    const item = document.querySelector<HTMLButtonElement>(".ctx-menu-item")!;
    expect(item.disabled).toBe(true);
    item.click();
    expect(run).not.toHaveBeenCalled();
  });

  it("closes on Escape and outside mousedown", () => {
    showContextMenu(10, 20, [{ label: "A", run: () => {} }]);
    window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape" }));
    expect(document.querySelector(".ctx-menu")).toBeNull();

    showContextMenu(10, 20, [{ label: "A", run: () => {} }]);
    document.body.dispatchEvent(new MouseEvent("mousedown", { bubbles: true }));
    expect(document.querySelector(".ctx-menu")).toBeNull();
  });

  it("opens a hover submenu with its own items", () => {
    showContextMenu(10, 20, [
      { label: "行操作", items: [{ label: "排序", run: () => {} }] },
    ]);
    const parent = document.querySelector<HTMLButtonElement>(".ctx-menu-parent")!;
    expect(parent.textContent).toContain("行操作");
    parent.dispatchEvent(new MouseEvent("mouseenter"));
    const sub = document.querySelector<HTMLElement>(".ctx-menu-sub");
    expect(sub).not.toBeNull();
    expect(sub!.textContent).toContain("排序");
  });
});