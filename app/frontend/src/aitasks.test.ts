import { describe, expect, it } from "vitest";
import {
  buildErrorMessages,
  buildExplainMessages,
  buildQueryMessages,
  clampLines,
  ERROR_PATTERN,
  estimateChars,
  MAX_CHARS,
  MAX_LINES,
  type LogLine,
} from "./aitasks";

const lines = (n: number, text = "x"): LogLine[] =>
  Array.from({ length: n }, (_, i) => ({ line: i + 1, text }));

describe("ai context budgeting", () => {
  it("clamps by line count and reports truncation", () => {
    const out = clampLines(lines(MAX_LINES + 10));
    expect(out.lines).toHaveLength(MAX_LINES);
    expect(out.truncated).toBe(true);
    expect(clampLines(lines(3)).truncated).toBe(false);
  });

  it("clamps by character budget without dropping everything", () => {
    const out = clampLines(lines(1000, "y".repeat(200)));
    const chars = out.lines.reduce((n, l) => n + l.text.length + 1, 0);
    expect(chars).toBeLessThanOrEqual(MAX_CHARS);
    expect(out.lines.length).toBeGreaterThan(0);
    expect(out.truncated).toBe(true);
  });

  it("keeps the first line even when it alone exceeds the budget", () => {
    const out = clampLines([{ line: 1, text: "z".repeat(MAX_CHARS + 50) }]);
    expect(out.lines).toHaveLength(1);
  });

  it("estimateChars sums every message", () => {
    expect(
      estimateChars([
        { role: "system", content: "abc" },
        { role: "user", content: "de" },
      ]),
    ).toBe(5);
  });
});

describe("log scenario prompts", () => {
  it("explain includes file, 1-based range and line numbers", () => {
    const ctx = buildExplainMessages({
      path: "C:/logs/app.log",
      from: 12,
      to: 14,
      totalLines: 500,
      lines: [
        { line: 12, text: "2026-11-09 ERROR timeout" },
        { line: 13, text: "retry 1" },
      ],
    });
    const user = ctx.messages[1].content;
    expect(user).toContain("C:/logs/app.log");
    expect(user).toContain("第 12–14 行");
    expect(user).toContain("L12 | 2026-11-09 ERROR timeout");
    expect(ctx.messages[0].role).toBe("system");
    expect(ctx.messages[0].content).toContain("不要编造");
    expect(ctx.label).toContain("字符");
  });

  it("single-line explain uses singular wording", () => {
    const ctx = buildExplainMessages({
      path: "a.log",
      from: 7,
      to: 7,
      totalLines: 7,
      lines: [{ line: 7, text: "boom" }],
    });
    expect(ctx.messages[1].content).toContain("第 7 行（");
  });

  it("error summary discloses the hit count and truncation", () => {
    const ctx = buildErrorMessages({
      path: "app.log",
      pattern: ERROR_PATTERN,
      lines: lines(3, "ERROR x"),
      hitCount: 900,
      totalLines: 1000,
    });
    const user = ctx.messages[1].content;
    expect(user).toContain("命中的不重复行数：900");
    expect(user).toContain("已截断，不是全量");
    expect(ctx.label).toContain("命中 900 行");
    expect(ctx.label).toContain("已截断");
  });

  it("error summary discloses a scan that stopped at the sample cap", () => {
    const ctx = buildErrorMessages({
      path: "app.log",
      pattern: ERROR_PATTERN,
      lines: lines(MAX_LINES, "ERROR x"),
      hitCount: MAX_LINES,
      totalLines: 100_000,
      truncated: true,
    });
    const user = ctx.messages[1].content;
    expect(user).toContain("扫描在样例上限处停止");
    expect(user).toContain("已截断，不是全量");
    expect(ctx.label).toContain("已截断");
  });

  it("query summary carries conditions and hit count", () => {
    const ctx = buildQueryMessages({
      path: "app.log",
      conditions: ['包含 "ERROR"', '排除（正则）"health"'],
      caseSensitive: true,
      hitCount: 42,
      totalLines: 1000,
      lines: lines(2, "ERROR"),
    });
    const user = ctx.messages[1].content;
    expect(user).toContain('包含 "ERROR"；排除（正则）"health"');
    expect(user).toContain("区分大小写");
    expect(user).toContain("命中行数：42");
    expect(ctx.label).toContain("查询命中 42 行");
  });

  it("empty conditions still describe the query as time/line constrained", () => {
    const ctx = buildQueryMessages({
      path: "a.log",
      conditions: [],
      caseSensitive: false,
      hitCount: 1,
      totalLines: 10,
      lines: [{ line: 3, text: "x" }],
    });
    expect(ctx.messages[1].content).toContain("（仅时间/行号约束）");
  });
});
