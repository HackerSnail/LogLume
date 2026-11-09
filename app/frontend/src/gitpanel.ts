//! Inspector Git section, rebuilt on NoteLume's git panel (`git-panel.js`).
//!
//! Layout parity: head (branch · change count · refresh) / commit box /
//! collapsible "变更" list / collapsible "图形" commit graph (lane SVG, HEAD
//! branch pill) / diff-or-commit-detail box. Row metrics (24px rows, 16px
//! status badge, 11px section titles and diff) follow `style.css:4522-5080`.
//!
//! NoteLume affordances that need new IPC are intentionally omitted instead of
//! faked: per-file stage/unstage/discard, checkout/revert, branch list + switch,
//! remote fetch/pull/push/merge, ahead/behind and conflict resolution. The
//! reads below (probe / status / log / commit files / diff) and commit-all are
//! real git2 calls added in `app/src-tauri/src/gitinfo.rs`.

import { ask } from "@tauri-apps/plugin-dialog";
import {
  gitCommitAll,
  gitCommitFiles,
  gitDiffFile,
  gitInit,
  gitLog,
  gitProbe,
  gitStatus,
  IpcError,
  type GitCommit,
  type GitCommitFile,
  type GitFileChange,
  type GitInfo,
} from "./ipc";

export interface GitPanelHooks {
  toast: (msg: string) => void;
  error: (err: unknown) => void;
}

const KIND_BADGE: Record<string, string> = {
  modified: "M",
  new: "N",
  deleted: "D",
  renamed: "R",
  untracked: "?",
  conflicted: "C",
  ignored: "·",
};

const KIND_LABEL: Record<string, string> = {
  modified: "修改",
  new: "新增",
  deleted: "删除",
  renamed: "重命名",
  untracked: "未跟踪",
  conflicted: "冲突",
  ignored: "忽略",
};

const GW = 14; // one lane's horizontal width (NoteLume's graph)
const FOLD_KEY = "lumelog.git-fold.";

/** Fold state (true = collapsed) survives refresh and tab switches, like
 *  NoteLume's `nl-git-fold.<key>` localStorage entries. */
const fold = { changes: readFold("changes"), graph: readFold("graph") };
/** Commit drafts keyed by repo root; cleared only on a successful commit. */
const drafts = new Map<string, string>();
let token = 0;

function readFold(key: string): boolean {
  try {
    return localStorage.getItem(FOLD_KEY + key) === "1";
  } catch {
    return false;
  }
}

function writeFold(key: string, collapsed: boolean): void {
  try {
    localStorage.setItem(FOLD_KEY + key, collapsed ? "1" : "0");
  } catch {
    // localStorage can be unavailable; folds then just reset next launch.
  }
}

function el<K extends keyof HTMLElementTagNameMap>(tag: K, cls = ""): HTMLElementTagNameMap[K] {
  const node = document.createElement(tag);
  if (cls) node.className = cls;
  return node;
}

function icon(id: string): SVGSVGElement {
  const NS = "http://www.w3.org/2000/svg";
  const svg = document.createElementNS(NS, "svg");
  svg.setAttribute("class", "ico");
  svg.setAttribute("aria-hidden", "true");
  const use = document.createElementNS(NS, "use");
  use.setAttribute("href", "#" + id);
  svg.append(use);
  return svg;
}

function empty(text: string): HTMLElement {
  const n = el("div", "git-empty");
  n.textContent = text;
  return n;
}

function emptyInline(text: string): HTMLElement {
  const n = el("div", "git-empty-inline");
  n.textContent = text;
  return n;
}

function errText(err: unknown): string {
  if (err instanceof IpcError) return err.zh;
  if (err instanceof Error) return err.message;
  return String(err);
}

/** Unix seconds → NoteLume's relative time (minutes → hours → days → date). */
function fmtTime(secs: number): string {
  const then = secs * 1000;
  const delta = Date.now() - then;
  const min = Math.floor(delta / 60_000);
  if (min < 1) return "刚刚";
  if (min < 60) return `${min} 分钟前`;
  const hr = Math.floor(min / 60);
  if (hr < 24) return `${hr} 小时前`;
  const day = Math.floor(hr / 24);
  if (day < 30) return `${day} 天前`;
  const d = new Date(then);
  const pad = (n: number): string => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
}

interface LaneRow {
  c: GitCommit;
  lane: number;
  lanesIn: (string | null)[];
  lanesOut: (string | null)[];
}

/** Lane assignment copied from NoteLume: the first parent inherits the lane,
 *  extra merge parents open a side lane, a missing parent ends the lane. */
function laneRows(commits: GitCommit[]): LaneRow[] {
  const rows: LaneRow[] = [];
  const lanes: (string | null)[] = [];
  for (const c of commits) {
    let idx = lanes.indexOf(c.id);
    if (idx < 0) {
      idx = lanes.indexOf(null);
      if (idx < 0) {
        lanes.push(null);
        idx = lanes.length - 1;
      }
    }
    const lanesIn = lanes.slice();
    lanes[idx] = c.parents.length ? c.parents[0] : null;
    for (const p of c.parents.slice(1)) {
      let pi = lanes.indexOf(p);
      if (pi < 0) {
        pi = lanes.indexOf(null);
        if (pi < 0) {
          lanes.push(p);
          pi = lanes.length - 1;
        } else {
          lanes[pi] = p;
        }
      }
    }
    rows.push({ c, lane: idx, lanesIn, lanesOut: lanes.slice() });
  }
  return rows;
}

/** Mount (or refresh) the Git block for `path` into `box`. */
export function mountGitPanel(box: HTMLElement, path: string, hooks: GitPanelHooks): void {
  const my = ++token;
  let busy = false;
  /** Bumped whenever the diff box is (re)opened/closed, so a late async reply
   *  cannot overwrite a newer selection. */
  let diffSeq = 0;
  let diffBox: HTMLElement | null = null;
  let diffTitle: HTMLElement | null = null;
  let diffContent: HTMLElement | null = null;
  /** Re-evaluates the current composer's disabled state after `busy` clears. */
  let syncCommit: (() => void) | null = null;

  box.dataset.path = path;
  box.className = "git-panel";
  box.textContent = "";
  box.append(empty("读取中…"));

  const stale = (): boolean => my !== token || box.dataset.path !== path || !box.isConnected;

  void load();

  async function load(): Promise<void> {
    let info: GitInfo;
    try {
      info = await gitProbe(path);
    } catch (err) {
      if (stale()) return;
      placeholder(`读取 Git 状态失败：${errText(err)}`);
      return;
    }
    if (stale()) return;
    if (info.unavailable) {
      placeholder(info.unavailable);
      return;
    }
    box.textContent = "";
    if (!info.repo) {
      buildHead(info, 0);
      box.append(empty("此文件夹不是 Git 仓库。点上方「初始化仓库」即可纳入版本管理。"));
      return;
    }

    const root = info.root ?? path;
    let changes: GitFileChange[];
    let commits: GitCommit[];
    try {
      [changes, commits] = await Promise.all([gitStatus(root), gitLog(root, 30)]);
    } catch (err) {
      if (stale()) return;
      placeholder(`读取 Git 状态失败：${errText(err)}`);
      return;
    }
    if (stale()) return;

    buildHead(info, changes.length);
    renderBody(root, info, changes, commits);
  }

  function placeholder(text: string): void {
    box.textContent = "";
    box.append(empty(text));
  }

  async function refreshPanel(): Promise<void> {
    if (busy) return;
    busy = true;
    try {
      await load();
    } finally {
      busy = false;
      syncCommit?.();
    }
  }

  function buildHead(info: GitInfo, changeCount: number): void {
    const head = el("div", "git-head");
    if (!info.repo) {
      const tip = el("span", "git-branch git-muted");
      tip.textContent = "未检测到 Git 仓库";
      const actions = el("span", "git-actions");
      const init = el("button", "git-btn");
      init.textContent = "初始化仓库";
      init.title = "在当前库根执行 git init";
      init.addEventListener("click", () => void doInit());
      actions.append(init);
      head.append(tip, actions);
      box.append(head);
      return;
    }

    const left = el("span", "git-branch");
    left.title = info.root ?? path;
    const name = el("span", "git-branch-name");
    name.textContent = info.branch || "（无提交）";
    left.append(icon("i-branch"), name);

    const count = el("span", "git-count");
    count.textContent = changeCount > 0 ? `${changeCount} 项改动` : "工作树干净";

    const actions = el("span", "git-actions");
    const refresh = el("button", "git-btn");
    refresh.append(icon("i-refresh"));
    refresh.title = "刷新状态";
    refresh.setAttribute("aria-label", "刷新状态");
    refresh.addEventListener("click", () => void refreshPanel());
    actions.append(refresh);

    head.append(left, count, actions);
    box.append(head);
  }

  async function doInit(): Promise<void> {
    if (busy) return;
    const dir = path.replace(/[\\/][^\\/]*$/, "") || path;
    const go = await ask(`将在 ${dir} 执行 git init，是否继续？`, {
      title: "初始化仓库",
      kind: "info",
      okLabel: "初始化",
    });
    if (!go) return;
    busy = true;
    try {
      await gitInit(path);
      hooks.toast("已初始化仓库");
      await load();
    } catch (err) {
      hooks.error(err);
    } finally {
      busy = false;
      syncCommit?.();
    }
  }

  function renderBody(
    root: string,
    info: GitInfo,
    changes: GitFileChange[],
    commits: GitCommit[],
  ): void {
    const body = el("div", "git-body");
    body.append(buildCommitBox(root, info, changes));
    body.append(buildChanges(root, changes));
    body.append(buildGraph(root, commits, info.branch ?? ""));
    diffBox = buildDiffBox();
    body.append(diffBox);
    box.append(body);
  }

  // ---- commit box ---------------------------------------------------------
  function buildCommitBox(root: string, info: GitInfo, changes: GitFileChange[]): HTMLElement {
    const sec = el("div", "git-sec git-commit-box");
    const row = el("div", "git-commit-row");
    const msg = el("textarea", "git-commit-input");
    msg.rows = 1;
    msg.placeholder = `提交信息（${changes.length} 项变更，Ctrl+Enter 提交）`;
    msg.value = drafts.get(root) ?? "";

    const submit = el("button", "git-btn git-btn-primary git-commit-btn");
    const label = el("span");
    label.textContent = "提交";
    submit.append(icon("i-check"), label);
    submit.title = "提交（Ctrl+Enter）";

    let nameEl: HTMLInputElement | null = null;
    let emailEl: HTMLInputElement | null = null;

    const sync = (): void => {
      submit.disabled = busy || msg.value.trim() === "" || changes.length === 0;
      msg.style.height = "auto";
      msg.style.height = `${Math.min(msg.scrollHeight, 160)}px`;
    };
    syncCommit = sync;

    msg.addEventListener("input", () => {
      if (msg.value) drafts.set(root, msg.value);
      else drafts.delete(root);
      sync();
    });

    const commit = async (): Promise<void> => {
      if (busy || submit.disabled) return;
      const message = msg.value.trim();
      if (!message) return;
      const name = nameEl?.value.trim() ?? "";
      const email = emailEl?.value.trim() ?? "";
      if (!info.identity && (!name || !email)) {
        hooks.error("首次提交需要作者姓名和邮箱");
        return;
      }
      busy = true;
      sync();
      try {
        const hash = await gitCommitAll(root, message, name || undefined, email || undefined);
        drafts.delete(root);
        hooks.toast(`已提交 ${hash}`);
        await load();
      } catch (err) {
        hooks.error(err);
      } finally {
        busy = false;
        sync();
        syncCommit?.();
      }
    };

    msg.addEventListener("keydown", (e) => {
      if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) {
        e.preventDefault();
        void commit();
      }
    });
    submit.addEventListener("click", () => void commit());
    row.append(msg, submit);
    sec.append(row);

    // NoteLume asks for user.name/user.email in dialogs on first commit.
    // LogLume has no text-prompt dialog, so the identity lives inline (only
    // while the repo has no configured identity) instead of being dropped.
    if (!info.identity) {
      const idRow = el("div", "git-ident-row");
      nameEl = el("input", "git-ident");
      nameEl.placeholder = "user.name";
      nameEl.setAttribute("aria-label", "作者姓名");
      emailEl = el("input", "git-ident");
      emailEl.placeholder = "user@email.com";
      emailEl.setAttribute("aria-label", "作者邮箱");
      idRow.append(nameEl, emailEl);
      const hint = el("div", "git-ident-note");
      hint.textContent = "首次提交需要作者信息（会记入本仓库配置）";
      sec.append(idRow, hint);
    }

    sync();
    // Initial autosize must wait for layout: scrollHeight is 0 before mount.
    const raf =
      window.requestAnimationFrame?.bind(window) ??
      ((fn: () => void) => window.setTimeout(fn, 0));
    raf(() => sync());
    return sec;
  }

  // ---- collapsible sections ----------------------------------------------
  function section(
    key: "changes" | "graph",
    title: string,
    badgeText: string,
    rows: HTMLElement,
  ): HTMLElement {
    const sec = el("div", "git-sec git-fold");
    const bar = el("button", "git-fold-bar");
    const collapsed = fold[key];
    bar.setAttribute("aria-expanded", String(!collapsed));
    const chev = el("span", "git-fold-chev");
    chev.append(icon("i-chevron"));
    const label = el("span", "git-fold-title");
    label.textContent = title;
    bar.append(chev, label);
    if (badgeText !== "") {
      const badge = el("span", "git-fold-badge");
      badge.textContent = badgeText;
      bar.append(badge);
    }
    const toggle = (): void => {
      fold[key] = !fold[key];
      writeFold(key, fold[key]);
      bar.setAttribute("aria-expanded", String(!fold[key]));
      sec.classList.toggle("collapsed", fold[key]);
      rows.hidden = fold[key];
    };
    bar.addEventListener("click", toggle);
    sec.classList.toggle("collapsed", collapsed);
    rows.hidden = collapsed;
    sec.append(bar, rows);
    return sec;
  }

  // ---- changes ------------------------------------------------------------
  function buildChanges(root: string, changes: GitFileChange[]): HTMLElement {
    const list = el("div", "git-list");
    if (changes.length === 0) {
      list.append(emptyInline("没有改动"));
    } else {
      for (const c of changes) {
        const row = el("div", "git-item");
        if (c.kind === "conflicted") row.classList.add("git-conflict");
        row.title = c.path + (c.staged ? "（已暂存）" : "");
        const badge = el("span", `git-badge git-badge-${c.kind}`);
        badge.textContent = KIND_BADGE[c.kind] ?? "•";
        const pathEl = el("span", "git-path");
        pathEl.textContent = c.path;
        row.append(badge, pathEl);
        const label = KIND_LABEL[c.kind] ?? c.kind;
        row.addEventListener("click", () =>
          void showDiff(root, c.path, "HEAD", `${label}：${c.path}`),
        );
        list.append(row);
      }
      if (changes.length >= 1000) list.append(emptyInline("改动过多，仅显示前 1000 项"));
    }
    return section("changes", "变更", changes.length > 0 ? String(changes.length) : "", list);
  }

  // ---- commit graph -------------------------------------------------------
  function buildGraph(root: string, commits: GitCommit[], branch: string): HTMLElement {
    const list = el("div", "git-list git-graph");
    if (commits.length === 0) {
      list.append(emptyInline("尚无提交"));
    } else {
      const headId = commits[0].id;
      const laneX = (i: number): number => i * GW + GW / 2;
      let prevOut: (string | null)[] | null = null;
      for (const { c, lane, lanesIn, lanesOut } of laneRows(commits)) {
        const nLane = Math.max(lanesIn.length, lanesOut.length, lane + 1);
        const cx = laneX(lane);
        const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
        svg.setAttribute("class", "git-gsvg");
        svg.setAttribute("width", String(nLane * GW));
        svg.setAttribute("height", "24");

        const line = (x1: number, y1: number, x2: number, y2: number, main: boolean): void => {
          const l = document.createElementNS("http://www.w3.org/2000/svg", "line");
          l.setAttribute("x1", String(x1));
          l.setAttribute("y1", String(y1));
          l.setAttribute("x2", String(x2));
          l.setAttribute("y2", String(y2));
          l.setAttribute("class", main ? "git-lane-main" : "git-lane");
          svg.append(l);
        };

        // Upper half (0→12): previous row's live lanes connect to this row.
        if (prevOut) {
          for (let i = 0; i < prevOut.length; i++) {
            const oid = prevOut[i];
            if (!oid) continue;
            let xb: number | null = null;
            if (oid === c.id) xb = cx;
            else {
              const j = lanesIn.indexOf(oid);
              if (j >= 0) xb = laneX(j);
            }
            if (xb == null) continue;
            line(laneX(i), 0, xb, 12, i === 0);
          }
        }
        // Lower half (12→24): lanes not consumed here keep going down.
        for (let i = 0; i < lanesIn.length; i++) {
          const oid = lanesIn[i];
          if (!oid || oid === c.id) continue;
          line(laneX(i), 12, laneX(i), 24, i === 0);
        }
        // Lower half: node → parent lanes (first parent inherits, others branch).
        for (let pi = 0; pi < c.parents.length; pi++) {
          const j = lanesOut.indexOf(c.parents[pi]);
          if (j < 0) continue;
          line(cx, 12, laneX(j), 24, pi === 0 && lane === 0);
        }
        // Node; HEAD gets the outlined ring.
        for (const [r, cls] of c.id === headId
          ? [
              [5.5, "git-node-head"],
              [2.5, "git-node-core"],
            ]
          : [[3.5, "git-node"]]) {
          const dot = document.createElementNS("http://www.w3.org/2000/svg", "circle");
          dot.setAttribute("cx", String(cx));
          dot.setAttribute("cy", "12");
          dot.setAttribute("r", String(r));
          dot.setAttribute("class", String(cls));
          svg.append(dot);
        }

        const row = el("div", "git-item git-commit");
        row.title = `${c.summary}\n${c.author_name} · ${fmtTime(c.time)}`;
        const summary = el("span", "git-summary");
        summary.textContent = c.summary || "（无提交信息）";
        const author = el("span", "git-meta");
        author.textContent = c.author_name;
        row.append(svg, summary, author);
        if (c.id === headId && branch) {
          const pill = el("span", "git-branch-pill");
          pill.textContent = branch;
          pill.title = branch;
          row.append(pill);
        }
        row.addEventListener("click", () => void showCommit(root, c));
        list.append(row);
        prevOut = lanesOut;
      }
    }
    return section("graph", "图形", commits.length > 0 ? String(commits.length) : "", list);
  }

  // ---- diff / commit detail ----------------------------------------------
  function buildDiffBox(): HTMLElement {
    const wrap = el("div", "git-diff");
    wrap.hidden = true;
    const head = el("div", "git-diff-head");
    diffTitle = el("span", "git-diff-title");
    const close = el("button", "git-diff-close");
    close.textContent = "×";
    close.title = "关闭";
    close.setAttribute("aria-label", "关闭");
    close.addEventListener("click", closeDiff);
    head.append(diffTitle, close);
    diffContent = el("div", "git-diff-content");
    wrap.append(head, diffContent);
    return wrap;
  }

  function closeDiff(): void {
    diffSeq++;
    if (diffBox) diffBox.hidden = true;
    if (diffContent) diffContent.textContent = "";
  }

  function openDiff(title: string, node: HTMLElement): number {
    const seq = ++diffSeq;
    if (!diffBox || !diffTitle || !diffContent) return seq;
    diffBox.hidden = false;
    diffTitle.textContent = title;
    diffTitle.title = title;
    diffContent.textContent = "";
    diffContent.append(node);
    return seq;
  }

  /** `+` add / `-` delete / `@@` hunk / file header / context, one div per line. */
  function paintDiff(text: string): HTMLElement {
    if (!text.trim()) return emptyInline("无差异");
    const pre = el("pre", "git-diff-pre");
    for (const ln of text.split("\n")) {
      let cls = "git-diff-ctx";
      if (ln.startsWith("+++") || ln.startsWith("---")) cls = "git-diff-file";
      else if (ln.startsWith("@@")) cls = "git-diff-hunk";
      else if (ln.startsWith("+")) cls = "git-diff-add";
      else if (ln.startsWith("-")) cls = "git-diff-del";
      const d = el("div", cls);
      d.textContent = ln;
      pre.append(d);
    }
    return pre;
  }

  async function showDiff(
    root: string,
    file: string,
    against: string,
    title: string,
  ): Promise<void> {
    try {
      const diff = await gitDiffFile(root, file, against);
      if (stale()) return;
      openDiff(title, paintDiff(diff));
    } catch (err) {
      hooks.error(err);
    }
  }

  function metaRow(key: string, value: string, extra = ""): HTMLElement {
    const row = el("div", "git-detail-row");
    const k = el("span", "git-detail-k");
    k.textContent = key;
    const v = el("span", extra ? `git-detail-v ${extra}` : "git-detail-v");
    v.textContent = value;
    row.append(k, v);
    return row;
  }

  async function showCommit(root: string, c: GitCommit): Promise<void> {
    const detail = el("div", "git-commit-detail");
    detail.append(
      metaRow("提交", c.id, "mono"),
      metaRow("摘要", c.summary || "（无摘要）"),
      metaRow("作者", `${c.author_name} <${c.author_email}>`),
      metaRow("时间", fmtTime(c.time)),
    );
    const seq = openDiff(`提交 ${c.id_short}`, detail);
    let files: GitCommitFile[];
    try {
      files = await gitCommitFiles(root, c.id);
    } catch {
      return; // NoteLume silently omits the file list when it can't be read.
    }
    if (stale() || seq !== diffSeq || !diffBox?.isConnected || diffBox.hidden) return;
    if (!files || files.length === 0) return;

    const values = el("span", "git-detail-v git-commit-files");
    for (const f of files) {
      const row = el("div", "git-item git-commit-file");
      row.title = f.path;
      const badge = el("span", `git-badge git-badge-${f.kind.toLowerCase()}`);
      badge.textContent = f.kind;
      const pathEl = el("span", "git-path");
      pathEl.textContent = f.path;
      row.append(badge, pathEl);
      values.append(row);
    }
    const row = el("div", "git-detail-row");
    const k = el("span", "git-detail-k");
    k.textContent = "变更文件";
    row.append(k, values);
    detail.append(row);
  }
}
