//! Log-scenario prompts and context budgeting for the AI panel. Pure functions
//! only: no DOM, no IPC, no network — so the "what exactly gets sent" rules are
//! unit-testable. Every builder returns the messages plus a human label used by
//! the panel's pre-send notice ("将发送：… → host").
import type { AiMessage } from "./ipc";

export interface AiContext {
  label: string;
  messages: AiMessage[];
}

/** A log line with its 1-based number (as shown in the gutter). */
export interface LogLine {
  line: number;
  text: string;
}

/** Hard caps on what one scenario may send. Truncation is always disclosed. */
export const MAX_LINES = 200;
export const MAX_CHARS = 40_000;

/**
 * Default "looks like an error" matcher for `search_run` (Rust regex syntax).
 * Broad on purpose; the model gets line numbers to verify each claim.
 */
export const ERROR_PATTERN =
  "(?i)(error|warn|fail|exception|panic|fatal|timeout|refused|unreachable|异常|错误|失败|超时)";

const SYSTEM_PROMPT = [
  "你是 LogLume 的日志与报文分析助手。",
  "规则：",
  "1. 只依据用户提供的日志行作答；不要编造未出现的错误码、行号、字段或文件内容。",
  "2. 区分“日志中可见的事实”和“你的推测”，推测必须标注为推测。",
  "3. 引用依据时使用给出的行号（如 L123）。",
  "4. 用中文回答，先给结论，再给依据和下一步排查建议。",
  "5. 不要输出与日志无关的泛泛而谈。",
].join("\n");

const FILE_SYSTEM_PROMPT = [
  "你是 LogLume 的文件内容分析助手。",
  "规则：",
  "1. 只依据用户提供的文件行作答；不要编造未出现的内容、行号或字段。",
  "2. 区分“文件中可见的事实”和“你的推测”，推测必须标注为推测。",
  "3. 引用依据时使用给出的行号（如 L123）。",
  "4. 用中文回答，先给结论，再给依据和下一步建议。",
  "5. 不要输出与文件内容无关的泛泛而谈。",
].join("\n");

export interface Clamped {
  lines: LogLine[];
  /** True when some input lines were dropped (line-count or char budget). */
  truncated: boolean;
}

/**
 * Enforce both the line and character budget on a line excerpt. At least one
 * line is always kept so the model sees the sample it is asked about; the
 * returned flag tells the UI (and the prompt) that the sample is partial.
 */
export function clampLines(
  lines: LogLine[],
  maxLines = MAX_LINES,
  maxChars = MAX_CHARS,
): Clamped {
  const out: LogLine[] = [];
  let chars = 0;
  let truncated = false;
  for (const line of lines) {
    if (out.length >= maxLines) {
      truncated = true;
      break;
    }
    const cost = line.text.length + 1;
    if (chars + cost > maxChars && out.length > 0) {
      truncated = true;
      break;
    }
    out.push(line);
    chars += cost;
  }
  if (out.length < lines.length) truncated = true;
  return { lines: out, truncated };
}

/** Total characters of the message contents (the panel's "约 N 字符"). */
export function estimateChars(messages: AiMessage[]): number {
  return messages.reduce((n, m) => n + m.content.length, 0);
}

function renderLines(lines: LogLine[]): string {
  return lines.map((l) => `L${l.line} | ${l.text}`).join("\n");
}

export interface ExplainInput {
  path: string;
  from: number;
  to: number;
  totalLines: number;
  lines: LogLine[];
  /** False for code/config/data files: use file wording, not log wording. */
  isLog?: boolean;
}

/** "解释选中日志行" — selection plus a few lines of context on each side. */
export function buildExplainMessages(input: ExplainInput): AiContext {
  const { lines, truncated } = clampLines(input.lines);
  const isLog = input.isLog !== false;
  const range = input.from === input.to ? `第 ${input.from} 行` : `第 ${input.from}–${input.to} 行`;
  const body = [
    `文件：${input.path}`,
    `选中范围：${range}（文件共 ${input.totalLines} 行）`,
    truncated ? `注意：上下文样例已截断，只包含 ${lines.length} 行。` : "",
    isLog ? "日志行（行号 | 内容）：" : "文件内容（行号 | 内容）：",
    renderLines(lines),
    "",
    isLog
      ? "请解释这段日志：发生了什么、涉及哪些错误码/超时/重试，最可能的原因是什么，下一步应该查什么。"
      : "请解释这段内容：它实现了什么、关键逻辑或结构是什么、有哪些潜在问题，下一步建议看哪里。",
  ]
    .filter((s) => s !== "")
    .join("\n");
  return {
    label: `${range} / 约 ${estimateChars([{ role: "user", content: body }])} 字符${
      truncated ? "（已截断）" : ""
    }`,
    messages: [
      { role: "system", content: isLog ? SYSTEM_PROMPT : FILE_SYSTEM_PROMPT },
      { role: "user", content: body },
    ],
  };
}

export interface ErrorScanInput {
  path: string;
  pattern: string;
  /** Sampled error-like lines (already deduplicated by the caller). */
  lines: LogLine[];
  /** Unique lines matched before truncation. */
  hitCount: number;
  totalLines: number;
  /** The caller stopped scanning at the sample cap, so `hitCount` is a floor,
   *  not the true unique match count. */
  truncated?: boolean;
}

/** "错误模式归纳" — sampled error/warn lines only, with explicit caps. */
export function buildErrorMessages(input: ErrorScanInput): AiContext {
  const { lines, truncated } = clampLines(input.lines);
  const partial = truncated || input.truncated === true || input.hitCount > lines.length;
  const body = [
    `文件：${input.path}`,
    `扫描范围：全文（${input.totalLines} 行），匹配式：${input.pattern}`,
    `命中的不重复行数：${input.hitCount}${input.truncated ? "（扫描在样例上限处停止，实际更多）" : ""}；本次提供 ${lines.length} 行样例${
      partial ? "（已截断，不是全量）" : ""
    }。`,
    "样例（行号 | 内容）：",
    renderLines(lines),
    "",
    "请归纳错误模式：",
    "1. 按错误类型/错误码/异常消息分组，说明各自出现次数（仅在样例可判断时）。",
    "2. 指出重复出现的模式、时间或线程上的聚集。",
    "3. 每组给 1–2 条代表行（带行号）。",
    "4. 给出按优先级排序的排查步骤。",
  ]
    .filter((s) => s !== "")
    .join("\n");
  return {
    label: `错误样例 ${lines.length} 行（命中 ${input.hitCount} 行${partial ? "，已截断" : ""}）`,
    messages: [
      { role: "system", content: SYSTEM_PROMPT },
      { role: "user", content: body },
    ],
  };
}

export interface QuerySummaryInput {
  path: string;
  conditions: string[];
  caseSensitive: boolean;
  hitCount: number;
  totalLines: number;
  lines: LogLine[];
}

/** "按查询条件总结" — the applied query's conditions + a bounded hit sample. */
export function buildQueryMessages(input: QuerySummaryInput): AiContext {
  const { lines, truncated } = clampLines(input.lines);
  const conds = input.conditions.length ? input.conditions.join("；") : "（仅时间/行号约束）";
  const body = [
    `文件：${input.path}`,
    `查询条件：${conds}${input.caseSensitive ? "（区分大小写）" : ""}`,
    `命中行数：${input.hitCount}（文件共 ${input.totalLines} 行）；本次提供 ${lines.length} 行样例${
      truncated || input.hitCount > lines.length ? "（已截断，不是全量）" : ""
    }。`,
    "命中样例（行号 | 内容）：",
    renderLines(lines),
    "",
    "请总结这次查询结果：命中概况、时间跨度（能从行内容判断时）、级别/来源分布、典型样例（带行号）、值得注意的异常聚集或趋势，以及建议的下一步查询。",
  ]
    .filter((s) => s !== "")
    .join("\n");
  return {
    label: `查询命中 ${input.hitCount} 行（取样 ${lines.length} 行${
      truncated ? "，已截断" : ""
    }）`,
    messages: [
      { role: "system", content: SYSTEM_PROMPT },
      { role: "user", content: body },
    ],
  };
}
