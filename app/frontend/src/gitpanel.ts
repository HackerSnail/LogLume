//! Inspector Git section, rebuilt on NoteLume's git panel (`git-panel.js`).
//!
//! Layout parity: head (branch · change count · refresh) / merge bar or commit
//! box / collapsible "变更" list with per-file tools / collapsible "图形" commit
//! graph (lane SVG, HEAD pill, checkout/revert tools) / "同步" remote block /
//! diff-or-commit-detail box. Row metrics (24px rows, 16px status badge, 11px
//! section titles and diff) follow `style.css:4522-5080`.
//!
//! Every affordance here is a real git2 call from `app/src-tauri/src/gitinfo.rs`
//! — no fake buttons. Still absent on purpose: new-branch creation and remote
//! add/edit (they need extra forms/dialogs, and NoteLume's dialogs have no
//! LogLume equivalent yet).

import { ask } from "@tauri-apps/plugin-dialog";
import {
  gitBranches,
  gitCheckoutBranch,
  gitCheckoutCommit,
  gitCommitAll,
  gitCommitFiles,
  gitDiffFile,
  gitDiscardPath,
  gitFetch,
  gitInit,
  gitLog,
  gitMergeAbort,
  gitMergeCommit,
  gitMergeState,
  gitProbe,
  gitPull,
  gitPush,
  gitRemoteStatus,
  gitResolveConflict,
  gitRevertCommit,
  gitStagePath,
  gitStatus,
  gitUnstagePath,
  IpcError,
  type GitBranchInfo,
  type GitCommit,
  type GitCommitFile,
  type GitFileChange,
  type GitInfo,
  type GitRemoteStatus,
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
  /** Open branch popup + its document-level click-away listener. */
  let menu: HTMLElement | null = null;
  let menuCleanup: (() => void) | null = null;
  /** Last remote snapshot, reused by the sync buttons' disabled state. */
  let remote: GitRemoteStatus | null = null;

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
    closeMenu();
    box.textContent = "";
    if (!info.repo) {
      buildHead(info, 0);
      box.append(empty("此文件夹不是 Git 仓库。点上方「初始化仓库」即可纳入版本管理。"));
      return;
    }

    const root = info.root ?? path;
    let changes: GitFileChange[];
    let commits: GitCommit[];
    let merging: boolean;
    let rs: GitRemoteStatus;
    try {
      [changes, commits, merging, rs] = await Promise.all([
        gitStatus(root),
        gitLog(root, 30),
        gitMergeState(root),
        gitRemoteStatus(root),
      ]);
    } catch (err) {
      if (stale()) return;
      placeholder(`读取 Git 状态失败：${errText(err)}`);
      return;
    }
    if (stale()) return;

    remote = rs;
    buildHead(info, changes.length);
    renderBody(root, info, changes, commits, merging);
  }

  function placeholder(text: string): void {
    closeMenu();
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

    const left = el("button", "git-branch git-branch-btn");
    left.title = `${info.root ?? path}（点击切换分支）`;
    const name = el("span", "git-branch-name");
    name.textContent = info.branch || "（无提交）";
    left.append(icon("i-branch"), name, icon("i-chevron"));
    left.addEventListener("click", () => {
      const root = info.root ?? path;
      if (menu) closeMenu();
      else void openBranchMenu(head, root, left);
    });

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

  // ---- branch menu -----------------------------------------------------
  function closeMenu(): void {
    menuCleanup?.();
    menuCleanup = null;
    menu?.remove();
    menu = null;
  }

  async function openBranchMenu(
    head: HTMLElement,
    root: string,
    anchor: HTMLElement,
  ): Promise<void> {
    const pop = el("div", "git-branch-menu");
    pop.setAttribute("role", "menu");
    pop.append(emptyInline("读取分支…"));
    head.append(pop);
    menu = pop;
    const away = (e: MouseEvent): void => {
      const t = e.target as Node;
      if (!pop.contains(t) && !anchor.contains(t)) closeMenu();
    };
    const key = (e: KeyboardEvent): void => {
      if (e.key === "Escape") closeMenu();
    };
    // Defer: the click that opened the menu must not immediately close it.
    const raf = window.setTimeout(() => {
      document.addEventListener("mousedown", away);
      document.addEventListener("keydown", key);
    }, 0);
    menuCleanup = () => {
      window.clearTimeout(raf);
      document.removeEventListener("mousedown", away);
      document.removeEventListener("keydown", key);
    };

    let branches: GitBranchInfo[];
    try {
      branches = await gitBranches(root);
    } catch (err) {
      if (menu !== pop) return;
      pop.textContent = "";
      pop.append(emptyInline(`读取分支失败：${errText(err)}`));
      return;
    }
    if (menu !== pop || !pop.isConnected) return;
    pop.textContent = "";
    const local = branches.filter((b) => !b.remote);
    const remoteBranches = branches.filter((b) => b.remote);
    if (local.length === 0) pop.append(emptyInline("没有本地分支"));
    for (const b of local) pop.append(branchRow(root, b, pop));
    if (remoteBranches.length > 0) {
      const sep = el("div", "git-branch-menu-title");
      sep.textContent = "远端";
      pop.append(sep);
      for (const b of remoteBranches) pop.append(branchRow(root, b, pop));
    }
  }

  function branchRow(root: string, b: GitBranchInfo, pop: HTMLElement): HTMLElement {
    const row = el("button", "git-branch-row");
    row.setAttribute("role", "menuitem");
    if (b.current) row.classList.add("git-branch-row-current");
    const mark = el("span", "git-branch-check");
    if (b.current) mark.append(icon("i-check"));
    const name = el("span", "git-branch-row-name");
    name.textContent = b.name;
    name.title = b.remote ? `远端分支 ${b.name}` : `本地分支 ${b.name}`;
    row.append(mark, name);
    if (b.upstream) {
      const up = el("span", "git-branch-row-up");
      up.textContent = b.upstream;
      up.title = `上游 ${b.upstream}`;
      row.append(up);
    }
    row.addEventListener("click", () => {
      pop.remove();
      if (menu === pop) {
        menu = null;
        menuCleanup?.();
        menuCleanup = null;
      }
      if (!b.current) void switchBranch(root, b.name);
    });
    return row;
  }

  async function switchBranch(root: string, name: string): Promise<void> {
    if (busy) return;
    busy = true;
    try {
      const now = await gitCheckoutBranch(root, name, false);
      hooks.toast(`已切换到分支 ${now}`);
      await load();
    } catch (err) {
      // Safe checkout refused because local edits would be overwritten.
      // Force only after a second, explicit confirmation.
      if (err instanceof IpcError && err.payload.code === "git_dirty") {
        const go = await ask(
          "切换分支被阻止：工作区有未提交改动会与目标分支冲突。\n\n强制切换会丢弃这些未提交改动，且无法撤销。确定继续？",
          {
            title: "切换分支",
            kind: "warning",
            okLabel: "强制切换",
            cancelLabel: "取消",
          },
        );
        if (go) {
          try {
            const now = await gitCheckoutBranch(root, name, true);
            hooks.toast(`已强制切换到分支 ${now}（未提交改动已丢弃）`);
            await load();
          } catch (forced) {
            hooks.error(forced);
          }
        }
      } else {
        hooks.error(err);
      }
    } finally {
      busy = false;
      syncCommit?.();
    }
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
    merging: boolean,
  ): void {
    const conflicts = changes.filter((c) => c.kind === "conflicted").length;
    const body = el("div", "git-body");
    // NoteLume swaps the commit form for a merge bar while MERGE_HEAD exists,
    // so a resolved merge is committed with its second parent intact.
    body.append(
      merging ? buildMergeBar(root, conflicts) : buildCommitBox(root, info, changes),
    );
    body.append(buildChanges(root, changes));
    body.append(buildGraph(root, commits, info.branch ?? ""));
    body.append(buildSync(root, changes));
    diffBox = buildDiffBox();
    body.append(diffBox);
    box.append(body);
  }

  /** One busy-wrapped git action: toast the backend message, then reload. */
  async function runAction(task: () => Promise<unknown>, fallback: string): Promise<void> {
    if (busy) return;
    busy = true;
    try {
      const result = await task();
      hooks.toast(typeof result === "string" ? result : fallback);
      await load();
    } catch (err) {
      hooks.error(err);
    } finally {
      busy = false;
      syncCommit?.();
    }
  }

  function errCode(err: unknown): string | undefined {
    if (!(err instanceof IpcError)) return undefined;
    const payload = err.payload as { code?: string } | undefined;
    return payload?.code;
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

  // ---- merge bar ----------------------------------------------------------
  function buildMergeBar(root: string, conflicts: number): HTMLElement {
    const sec = el("div", "git-sec git-merge-bar");
    const head = el("div", "git-merge-head");
    const title = el("span", "git-merge-title");
    title.textContent =
      conflicts > 0
        ? `合并进行中 · ${conflicts} 处冲突待解决`
        : "合并进行中 · 冲突已全部解决";
    const hint = el("span", "git-merge-hint");
    hint.textContent =
      conflicts > 0 ? "在「变更」中逐项选择用本地 / 用远端" : "可以提交合并";
    head.append(title, hint);

    const row = el("div", "git-commit-row");
    const msg = el("textarea", "git-commit-input");
    msg.rows = 1;
    msg.value = `Merge ${remote?.upstream ?? remote?.remote ?? "remote"}`;
    const submit = el("button", "git-btn git-btn-primary");
    submit.append(icon("i-check"));
    const submitLabel = el("span");
    submitLabel.textContent = "提交合并";
    submit.append(submitLabel);
    submit.title = "提交合并（Ctrl+Enter）";
    const abort = el("button", "git-btn");
    abort.append(icon("i-close"));
    const abortLabel = el("span");
    abortLabel.textContent = "中止合并";
    abort.append(abortLabel);
    abort.title = "中止合并并回到合并前的 HEAD";

    const sync = (): void => {
      submit.disabled = busy || conflicts > 0 || msg.value.trim() === "";
      msg.style.height = "auto";
      msg.style.height = `${Math.min(msg.scrollHeight, 120)}px`;
    };
    syncCommit = sync;
    msg.addEventListener("input", sync);

    const commit = async (): Promise<void> => {
      if (submit.disabled) return;
      const message = msg.value.trim();
      if (!message) return;
      await runAction(async () => `已提交合并 ${await gitMergeCommit(root, message)}`, "已提交合并");
    };
    msg.addEventListener("keydown", (e) => {
      if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) {
        e.preventDefault();
        void commit();
      }
    });
    submit.addEventListener("click", () => void commit());
    abort.addEventListener("click", async () => {
      if (busy) return;
      const go = await ask(
        "中止合并会丢弃本次合并引入的改动，回到合并前的 HEAD。确定中止？",
        { title: "中止合并", kind: "warning", okLabel: "中止合并", cancelLabel: "取消" },
      );
      if (go) void runAction(() => gitMergeAbort(root), "已中止合并");
    });
    row.append(msg, submit, abort);
    sec.append(head, row);
    sync();
    return sec;
  }

  // ---- sync (remote / ahead-behind / fetch-pull-push) ---------------------
  function buildSync(root: string, changes: GitFileChange[]): HTMLElement {
    const sec = el("div", "git-sec git-sync");
    const title = el("div", "git-sec-title");
    title.textContent = "同步";
    const line = el("div", "git-sync-line");
    const target = el("span", "git-sync-target");
    const state = el("span", "git-sync-state");
    const actions = el("span", "git-sync-actions");
    const rs = remote;

    const hasRemote = rs?.has_remote === true;
    if (!rs || !hasRemote) {
      target.textContent = "未配置远端";
      state.textContent = "添加 origin 后可获取 / 拉取 / 推送";
      state.classList.add("git-sync-dim");
    } else {
      target.title = rs.url ?? "";
      const branchLabel = rs.detached ? "分离 HEAD" : rs.branch ?? "（无提交）";
      target.textContent = `${rs.remote ?? "origin"} · ${branchLabel}`;
      if (rs.detached) {
        state.textContent = "分离 HEAD";
        state.classList.add("git-sync-dim");
      } else if (!rs.has_upstream) {
        state.textContent = "无上游";
        state.classList.add("git-sync-dim");
      } else if (rs.ahead === 0 && rs.behind === 0) {
        state.textContent = "已同步";
      } else {
        if (rs.ahead > 0) {
          const a = el("span", "git-sync-ahead");
          a.append(icon("i-up"));
          a.append(`${rs.ahead}`);
          a.title = `本地领先 ${rs.ahead} 个提交`;
          state.append(a);
        }
        if (rs.behind > 0) {
          const b = el("span", "git-sync-behind");
          b.append(icon("i-down"));
          b.append(`${rs.behind}`);
          b.title = `本地落后 ${rs.behind} 个提交`;
          state.append(b);
        }
      }
    }

    const tool = (
      iconId: string,
      label: string,
      disabled: boolean,
      title: string,
      action: () => void,
    ): HTMLButtonElement => {
      const btn = el("button", "git-btn git-sync-btn");
      btn.append(icon(iconId));
      const text = el("span");
      text.textContent = label;
      btn.append(text);
      btn.disabled = disabled;
      btn.title = title;
      if (!disabled) btn.addEventListener("click", action);
      return btn;
    };

    const detached = rs?.detached === true;
    actions.append(
      tool(
        "i-globe",
        "获取",
        !hasRemote,
        hasRemote ? "从远端获取最新引用" : "需要先配置远端",
        () => void runAction(() => gitFetch(root), "已获取远端"),
      ),
      tool(
        "i-down",
        "拉取",
        !hasRemote || detached,
        detached
          ? "分离 HEAD 下无法拉取"
          : hasRemote
            ? "获取并快进或合并远端提交"
            : "需要先配置远端",
        () => void doPull(root, changes),
      ),
      tool(
        "i-up",
        "推送",
        !hasRemote || detached,
        detached
          ? "分离 HEAD 下无法推送"
          : hasRemote
            ? "推送当前分支到远端"
            : "需要先配置远端",
        () => void doPush(root),
      ),
    );
    line.append(target, state, actions);
    sec.append(title, line);
    return sec;
  }

  async function doPull(root: string, changes: GitFileChange[]): Promise<void> {
    if (busy) return;
    if (changes.length > 0) {
      const go = await ask(
        `工作区有 ${changes.length} 项未提交改动，拉取可能被阻止或产生冲突。仍然拉取？`,
        { title: "拉取", kind: "warning", okLabel: "拉取", cancelLabel: "取消" },
      );
      if (!go) return;
    }
    busy = true;
    try {
      const result = await gitPull(root);
      hooks.toast(result.message);
      await load();
    } catch (err) {
      hooks.error(err);
    } finally {
      busy = false;
      syncCommit?.();
    }
  }

  async function doPush(root: string): Promise<void> {
    if (busy) return;
    busy = true;
    try {
      const message = await gitPush(root, false);
      hooks.toast(message);
      await load();
    } catch (err) {
      if (errCode(err) === "no_upstream") {
        const r = remote;
        const branch = r?.branch ?? "当前分支";
        const remoteName = r?.remote ?? "origin";
        const go = await ask(
          `当前分支 ${branch} 没有上游。向 ${remoteName}/${branch} 首次推送并建立上游？`,
          { title: "首次推送", kind: "info", okLabel: "推送", cancelLabel: "取消" },
        );
        if (go) {
          try {
            const message = await gitPush(root, true);
            hooks.toast(message);
            await load();
          } catch (inner) {
            hooks.error(inner);
          }
        }
      } else {
        hooks.error(err);
      }
    } finally {
      busy = false;
      syncCommit?.();
    }
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
        const label = KIND_LABEL[c.kind] ?? c.kind;
        row.append(badge, pathEl, buildChangeTools(root, c));
        row.addEventListener("click", () =>
          void showDiff(root, c.path, "HEAD", `${label}：${c.path}`),
        );
        list.append(row);
      }
      if (changes.length >= 1000) list.append(emptyInline("改动过多，仅显示前 1000 项"));
    }
    return section("changes", "变更", changes.length > 0 ? String(changes.length) : "", list);
  }

  /** Per-file tools: stage/unstage/discard, or conflict side-picking. */
  function buildChangeTools(root: string, c: GitFileChange): HTMLElement {
    const tools = el("span", "git-row-tools");
    const add = (iconId: string, title: string, action: () => void): void => {
      const btn = el("button", "git-row-tool");
      btn.append(icon(iconId));
      btn.title = title;
      btn.setAttribute("aria-label", title);
      btn.addEventListener("click", (e) => {
        e.stopPropagation();
        action();
      });
      tools.append(btn);
    };
    if (c.kind === "conflicted") {
      add("i-check", "用本地版本解决（ours）", () =>
        void resolveConflict(root, c.path, "ours", "本地"),
      );
      add("i-target", "用远端版本解决（theirs）", () =>
        void resolveConflict(root, c.path, "theirs", "远端"),
      );
      return tools;
    }
    if (c.staged) {
      add("i-minus", "取消暂存", () =>
        void runAction(() => gitUnstagePath(root, c.path), `已取消暂存：${c.path}`),
      );
    } else {
      add("i-plus", "暂存此文件", () =>
        void runAction(() => gitStagePath(root, c.path), `已暂存：${c.path}`),
      );
    }
    add("i-undo", "丢弃此文件的改动", () => void discardChange(root, c));
    return tools;
  }

  async function discardChange(root: string, c: GitFileChange): Promise<void> {
    if (busy) return;
    const deletes = c.kind === "untracked" || c.kind === "new";
    const effect = deletes
      ? "文件将从磁盘删除（含暂存的新文件），无法撤销。"
      : "改动将还原为 HEAD 版本，无法撤销。";
    const go = await ask(`丢弃 ${c.path} 的改动？\n\n${effect}`, {
      title: "丢弃改动",
      kind: "warning",
      okLabel: "丢弃",
      cancelLabel: "取消",
    });
    if (!go) return;
    // The backend also requires confirmed=true; this is the second gate.
    await runAction(
      () => gitDiscardPath(root, c.path, true),
      `已丢弃改动：${c.path}`,
    );
  }

  async function resolveConflict(
    root: string,
    path: string,
    side: "ours" | "theirs",
    label: string,
  ): Promise<void> {
    await runAction(
      () => gitResolveConflict(root, path, side),
      `已用${label}版本解决：${path}`,
    );
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
        const tools = el("span", "git-row-tools git-commit-tools");
        const tool = (iconId: string, title: string, action: () => void): void => {
          const btn = el("button", "git-row-tool");
          btn.append(icon(iconId));
          btn.title = title;
          btn.setAttribute("aria-label", title);
          btn.addEventListener("click", (e) => {
            e.stopPropagation();
            action();
          });
          tools.append(btn);
        };
        tool("i-target", "检出此提交（分离 HEAD）", () =>
          void checkoutCommit(root, c),
        );
        tool("i-undo", "还原此提交（生成新提交）", () => void revertCommit(root, c));
        row.append(svg, summary, author, tools);
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

  async function checkoutCommit(root: string, c: GitCommit): Promise<void> {
    if (busy) return;
    const go = await ask(
      `检出提交 ${c.id_short}？\n\n将进入分离 HEAD 状态：当前不在任何分支上，新的提交不会推进已有分支；随时可以从分支菜单切回。工作区有未提交改动时会被阻止。`,
      { title: "检出提交", kind: "warning", okLabel: "检出", cancelLabel: "取消" },
    );
    if (!go) return;
    busy = true;
    try {
      const message = await gitCheckoutCommit(root, c.id, false);
      hooks.toast(message);
      await load();
    } catch (err) {
      if (errCode(err) === "git_dirty") {
        const force = await ask(
          "工作区有未提交改动与目标提交冲突。强制检出会丢弃这些改动，且无法撤销。确定继续？",
          { title: "强制检出", kind: "warning", okLabel: "强制检出", cancelLabel: "取消" },
        );
        if (force) {
          try {
            const message = await gitCheckoutCommit(root, c.id, true);
            hooks.toast(message);
            await load();
          } catch (inner) {
            hooks.error(inner);
          }
        }
      } else {
        hooks.error(err);
      }
    } finally {
      busy = false;
      syncCommit?.();
    }
  }

  async function revertCommit(root: string, c: GitCommit): Promise<void> {
    if (busy) return;
    const go = await ask(
      `还原提交 ${c.id_short}「${c.summary}」？\n\n会创建一个新的反向提交；工作区有未提交改动时会被阻止。`,
      { title: "还原提交", kind: "warning", okLabel: "还原", cancelLabel: "取消" },
    );
    if (!go) return;
    await runAction(() => gitRevertCommit(root, c.id), `已还原 ${c.id_short}`);
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
