// @vitest-environment node
// Contract test: every item in the Rust native macOS menu must have a handler
// in main.ts, and every menu-specific switch case must still exist in Rust.
// Sources are imported as raw text via Vite (`?raw`), so no Node types or
// Tauri/AppKit runtime are needed — this works on every platform.
import { describe, expect, it } from "vitest";
import rustMenu from "../../src-tauri/src/menu.rs?raw";
import mainTs from "./main.ts?raw";

const ids = (text: string, re: RegExp): string[] =>
  [...text.matchAll(re)].map((m) => m[1]);

const menuIds = ids(rustMenu, /"menu:([a-z0-9-]+)"/g);
const submenus = ids(rustMenu, /SubmenuBuilder::new\(app, "([^"]+)"\)/g);

const defsBlock = mainTs.slice(
  mainTs.indexOf("const ACTION_DEFS"),
  mainTs.indexOf("function actionTitle"),
);
const actionIds = ids(defsBlock, /id: "([a-z0-9-]+)"/g);

const dispatchBlock = mainTs.slice(
  mainTs.indexOf("function runMenuAction"),
  mainTs.indexOf("/** Files from the initial command line"),
);
const cases = ids(dispatchBlock, /case "([a-z0-9-]+)"/g);

describe("native menu contract", () => {
  it("keeps the seven Chinese menu categories in order", () => {
    expect(submenus).toEqual([
      "LogLume",
      "文件",
      "编辑",
      "查看",
      "数据",
      "设置",
      "窗口",
    ]);
  });

  it("gives every menu item a unique id", () => {
    expect(menuIds.length).toBeGreaterThanOrEqual(30);
    expect(new Set(menuIds).size).toBe(menuIds.length);
  });

  it("routes every Rust menu item to ACTION_DEFS or a switch case", () => {
    const handled = new Set([...actionIds, ...cases]);
    expect(menuIds.filter((id) => !handled.has(id))).toEqual([]);
  });

  it("has no switch case for a menu item that no longer exists", () => {
    expect(cases.filter((id) => !menuIds.includes(id))).toEqual([]);
  });

  it("shares the keymap action ids with the native menu", () => {
    expect(actionIds.filter((id) => !menuIds.includes(id))).toEqual([]);
  });
});
