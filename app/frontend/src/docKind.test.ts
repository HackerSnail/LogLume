import { describe, expect, it } from "vitest";
import { isLogDocument } from "./docKind";

describe("isLogDocument", () => {
  it("treats log/text/extension-less files as logs", () => {
    expect(isLogDocument("C:\\logs\\access.log")).toBe(true);
    expect(isLogDocument("/var/log/syslog.txt")).toBe(true);
    expect(isLogDocument("bench.out")).toBe(true);
    expect(isLogDocument("README")).toBe(true);
    expect(isLogDocument("C:\\logs\\APP.LOG")).toBe(true);
  });

  it("treats code, config, data and document files as non-logs", () => {
    expect(isLogDocument("src/main.rs")).toBe(false);
    expect(isLogDocument("app.ts")).toBe(false);
    expect(isLogDocument("data.json")).toBe(false);
    expect(isLogDocument("config.toml")).toBe(false);
    expect(isLogDocument("notes.md")).toBe(false);
    expect(isLogDocument("records.csv")).toBe(false);
    expect(isLogDocument("manual.pdf")).toBe(false);
  });

  it("returns false without an active document", () => {
    expect(isLogDocument(null)).toBe(false);
  });
});