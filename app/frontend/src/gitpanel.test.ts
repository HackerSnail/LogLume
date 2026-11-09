// @vitest-environment jsdom
// Git inspector parity checks (NoteLume's head / changes / graph / diff /
// commit-detail blocks). IPC is mocked at the module boundary; the real git2
// paths are covered by `gitinfo.rs`'s `status_log_diff_and_commit_all` test.
import { beforeEach, describe, expect, it, vi } from "vitest";
import type {
  GitBranchInfo,
  GitCommit,
  GitFileChange,
  GitInfo,
  GitRemoteStatus,
} from "./ipc";

const probe = vi.fn<(path: string) => Promise<GitInfo>>();
const status = vi.fn<(root: string) => Promise<GitFileChange[]>>();
const log = vi.fn<(root: string, limit: number) => Promise<GitCommit[]>>();
const commitAll = vi.fn<(root: string, msg: string, name?: string, email?: string) => Promise<string>>();
const commitFiles = vi.fn<(root: string, id: string) => Promise<{ path: string; kind: string }[]>>();
const diffFile = vi.fn<(root: string, file: string, against: string) => Promise<string>>();
const init = vi.fn<(path: string) => Promise<void>>();
const ask = vi.fn<(msg: string, opts?: unknown) => Promise<boolean>>();
const mergeState = vi.fn<(root: string) => Promise<boolean>>();
const remoteStatus = vi.fn<(root: string) => Promise<GitRemoteStatus>>();
const branches = vi.fn<(root: string) => Promise<GitBranchInfo[]>>();
const checkoutBranch = vi.fn<(root: string, name: string, force?: boolean) => Promise<string>>();
const checkoutCommit = vi.fn<(root: string, id: string, force?: boolean) => Promise<string>>();
const revertCommit = vi.fn<(root: string, id: string) => Promise<string>>();
const stagePath = vi.fn<(root: string, path: string) => Promise<void>>();
const unstagePath = vi.fn<(root: string, path: string) => Promise<void>>();
const discardPath = vi.fn<(root: string, path: string, confirmed?: boolean) => Promise<void>>();
const resolveConflict = vi.fn<(root: string, path: string, side: string) => Promise<void>>();
const fetchRemote = vi.fn<(root: string) => Promise<string>>();
const pullRemote = vi.fn<(root: string) => Promise<{ status: string; message: string }>>();
const pushRemote = vi.fn<(root: string, setUpstream?: boolean) => Promise<string>>();
const mergeCommit = vi.fn<(root: string, msg: string) => Promise<string>>();
const mergeAbort = vi.fn<(root: string) => Promise<void>>();

class FakeIpcError extends Error {
  zh: string;
  payload: { code?: string };
  constructor(zh: string, code?: string) {
    super(zh);
    this.zh = zh;
    this.payload = { code };
  }
}

vi.mock("./ipc", () => ({
  gitProbe: (p: string) => probe(p),
  gitStatus: (r: string) => status(r),
  gitLog: (r: string, l: number) => log(r, l),
  gitCommitAll: (r: string, m: string, n?: string, e?: string) => commitAll(r, m, n, e),
  gitCommitFiles: (r: string, i: string) => commitFiles(r, i),
  gitDiffFile: (r: string, f: string, a: string) => diffFile(r, f, a),
  gitInit: (p: string) => init(p),
  gitMergeState: (r: string) => mergeState(r),
  gitRemoteStatus: (r: string) => remoteStatus(r),
  gitBranches: (r: string) => branches(r),
  gitCheckoutBranch: (r: string, n: string, f?: boolean) => checkoutBranch(r, n, f),
  gitCheckoutCommit: (r: string, i: string, f?: boolean) => checkoutCommit(r, i, f),
  gitRevertCommit: (r: string, i: string) => revertCommit(r, i),
  gitStagePath: (r: string, p: string) => stagePath(r, p),
  gitUnstagePath: (r: string, p: string) => unstagePath(r, p),
  gitDiscardPath: (r: string, p: string, c?: boolean) => discardPath(r, p, c),
  gitResolveConflict: (r: string, p: string, s: string) => resolveConflict(r, p, s),
  gitFetch: (r: string) => fetchRemote(r),
  gitPull: (r: string) => pullRemote(r),
  gitPush: (r: string, u?: boolean) => pushRemote(r, u),
  gitMergeCommit: (r: string, m: string) => mergeCommit(r, m),
  gitMergeAbort: (r: string) => mergeAbort(r),
  IpcError: FakeIpcError,
}));
vi.mock("@tauri-apps/plugin-dialog", () => ({ ask: (m: string, o?: unknown) => ask(m, o) }));

const { mountGitPanel } = await import("./gitpanel");

const emptyInfo: GitInfo = {
  repo: false,
};

function repoInfo(overrides: Partial<GitInfo> = {}): GitInfo {
  return {
    repo: true,
    root: "/repo/",
    branch: "main",
    identity: { name: "Tester", email: "tester@example.com" },
    ...overrides,
  };
}

function change(path: string, kind: string, staged = false): GitFileChange {
  return { path, kind, staged };
}

function commit(id: string, summary: string, parents: string[] = []): GitCommit {
  return {
    id,
    id_short: id.slice(0, 7),
    summary,
    author_name: "Tester",
    author_email: "tester@example.com",
    time: Math.floor(Date.now() / 1000) - 3600,
    parents,
  };
}

function mount(path = "/repo/a.log"): { box: HTMLElement; toast: ReturnType<typeof vi.fn>; error: ReturnType<typeof vi.fn> } {
  const box = document.createElement("div");
  box.id = "insp-git";
  document.body.append(box);
  const toast = vi.fn();
  const error = vi.fn();
  mountGitPanel(box, path, { toast, error });
  return { box, toast, error };
}

/** Two macrotasks: probe resolves, then status+log (Promise.all) resolves. */
async function settle(): Promise<void> {
  await new Promise((r) => setTimeout(r, 0));
  await new Promise((r) => setTimeout(r, 0));
}

beforeEach(() => {
  vi.clearAllMocks();
  localStorage.clear();
  document.body.textContent = "";
  ask.mockResolvedValue(true);
  mergeState.mockResolvedValue(false);
  remoteStatus.mockResolvedValue({
    has_remote: true,
    remote: "origin",
    branch: "main",
    detached: false,
    has_upstream: true,
    upstream: "origin/main",
    ahead: 0,
    behind: 0,
  });
  branches.mockResolvedValue([]);
  checkoutBranch.mockResolvedValue("main");
  stagePath.mockResolvedValue(undefined);
  unstagePath.mockResolvedValue(undefined);
  discardPath.mockResolvedValue(undefined);
  resolveConflict.mockResolvedValue(undefined);
  fetchRemote.mockResolvedValue("已从 origin 获取最新引用");
  pullRemote.mockResolvedValue({ status: "up_to_date", message: "已是最新（origin/main）" });
  pushRemote.mockResolvedValue("已推送 main → origin/main");
  mergeCommit.mockResolvedValue("abc1234");
  mergeAbort.mockResolvedValue(undefined);
});

describe("Git panel parity", () => {
  it("renders the non-repo head, hint and initialises through the same button", async () => {
    probe.mockResolvedValue(emptyInfo);
    const { box, toast } = mount("/plain/a.log");
    await settle();

    expect(box.className).toBe("git-panel");
    expect(box.querySelector(".git-branch")?.textContent).toBe("未检测到 Git 仓库");
    expect(box.querySelector(".git-empty")?.textContent).toContain("不是 Git 仓库");

    const initBtn = box.querySelector<HTMLButtonElement>(".git-btn");
    expect(initBtn?.textContent).toBe("初始化仓库");
    initBtn?.click();
    await settle();

    expect(ask).toHaveBeenCalledOnce();
    expect(init).toHaveBeenCalledWith("/plain/a.log");
    expect(toast).toHaveBeenCalledWith("已初始化仓库");
  });

  it("shows change count, folding lists, HEAD pill and coloured diff", async () => {
    probe.mockResolvedValue(repoInfo());
    status.mockResolvedValue([change("a.log", "modified"), change("b.log", "untracked", true), change("c.log", "conflicted")]);
    log.mockResolvedValue([commit("abcdef0123456", "first"), commit("0123456789abc", "second", ["abcdef0123456"])]);
    diffFile.mockResolvedValue("--- a/a.log\n+++ b/a.log\n@@ -1 +1 @@\n-old\n+new");

    const { box } = mount();
    await settle();

    expect(box.querySelector(".git-branch-name")?.textContent).toBe("main");
    expect(box.querySelector(".git-count")?.textContent).toBe("3 项改动");
    expect(box.querySelectorAll(".git-fold-bar")).toHaveLength(2);
    expect(box.querySelectorAll(".git-fold-badge")).toHaveLength(2);
    expect(box.querySelectorAll(".git-badge-modified")).toHaveLength(1);
    expect(box.querySelectorAll(".git-badge-untracked")).toHaveLength(1);
    expect(box.querySelectorAll(".git-conflict")).toHaveLength(1);
    expect(box.querySelector(".git-branch-pill")?.textContent).toBe("main");
    expect(box.querySelectorAll(".git-graph .git-item")).toHaveLength(2);
    expect(box.querySelectorAll(".git-graph svg line").length).toBeGreaterThan(0);

    // Folding hides the rows and persists like NoteLume's localStorage fold.
    const bar = box.querySelector<HTMLButtonElement>(".git-fold-bar");
    bar?.click();
    expect((box.querySelectorAll(".git-list")[0] as HTMLElement).hidden).toBe(true);
    expect(localStorage.getItem("lumelog.git-fold.changes")).toBe("1");
    bar?.click();

    // Change row opens the shared diff box with the kind-labelled header.
    (box.querySelectorAll(".git-list")[0].querySelector(".git-item") as HTMLElement).click();
    await settle();
    const diff = box.querySelector(".git-diff") as HTMLElement;
    expect(diff.hidden).toBe(false);
    expect(diff.querySelector(".git-diff-title")?.textContent).toBe("修改：a.log");
    expect(diff.querySelectorAll(".git-diff-add")).toHaveLength(1);
    expect(diff.querySelectorAll(".git-diff-del")).toHaveLength(1);
    expect(diff.querySelectorAll(".git-diff-hunk")).toHaveLength(1);

    // Commit click reuses the box for the detail + file list.
    commitFiles.mockResolvedValue([{ path: "a.log", kind: "M" }]);
    (box.querySelectorAll(".git-graph .git-item")[0] as HTMLElement).click();
    await settle();
    expect(diff.querySelector(".git-diff-title")?.textContent).toBe("提交 abcdef0");
    const keys = [...diff.querySelectorAll(".git-detail-k")].map((n) => n.textContent);
    expect(keys).toEqual(["提交", "摘要", "作者", "时间", "变更文件"]);
    expect(diff.querySelectorAll(".git-commit-file")).toHaveLength(1);
  });

  it("renders empty repo/lists and commits the typed message with Ctrl+Enter", async () => {
    probe.mockResolvedValue(repoInfo());
    status.mockResolvedValue([]);
    log.mockResolvedValue([]);
    commitAll.mockResolvedValue("deadbee");

    const { box, toast } = mount();
    await settle();

    expect(box.querySelector(".git-count")?.textContent).toBe("工作树干净");
    expect(box.querySelector(".git-empty-inline")?.textContent).toBe("没有改动");
    expect([...box.querySelectorAll(".git-empty-inline")].some((n) => n.textContent === "尚无提交")).toBe(true);
    expect(box.querySelector<HTMLButtonElement>(".git-commit-btn")?.disabled).toBe(true);

    // A draft survives a manual refresh (same repo root key), and the submit
    // button enables once the refreshed status reports a change.
    let ta = box.querySelector<HTMLTextAreaElement>(".git-commit-input")!;
    ta.value = "feat: one";
    ta.dispatchEvent(new Event("input", { bubbles: true }));
    expect(box.querySelector<HTMLButtonElement>(".git-commit-btn")?.disabled).toBe(true);

    status.mockResolvedValue([change("a.log", "modified")]);
    (box.querySelector(".git-btn") as HTMLButtonElement).click();
    await settle();
    ta = box.querySelector<HTMLTextAreaElement>(".git-commit-input")!;
    expect(ta.placeholder).toBe("提交信息（1 项变更，Ctrl+Enter 提交）");
    expect(ta.value).toBe("feat: one");
    expect(box.querySelector<HTMLButtonElement>(".git-commit-btn")?.disabled).toBe(false);
    ta.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", ctrlKey: true }));
    await settle();

    expect(commitAll).toHaveBeenCalledWith("/repo/", "feat: one", undefined, undefined);
    expect(toast).toHaveBeenCalledWith("已提交 deadbee");
    expect(box.querySelector<HTMLTextAreaElement>(".git-commit-input")?.value).toBe("");
    expect(box.querySelector(".git-count")?.textContent).toBe("1 项改动");
  });

  it("stages, discards after confirmation, shows ahead/behind and switches branch", async () => {
    probe.mockResolvedValue(repoInfo());
    status.mockResolvedValue([change("a.log", "modified"), change("b.log", "untracked")]);
    log.mockResolvedValue([]);
    remoteStatus.mockResolvedValue({
      has_remote: true,
      remote: "origin",
      branch: "main",
      detached: false,
      has_upstream: true,
      upstream: "origin/main",
      ahead: 1,
      behind: 2,
    });
    branches.mockResolvedValue([
      { name: "main", current: true, remote: false, upstream: "origin/main" },
      { name: "feature", current: false, remote: false },
    ]);
    checkoutBranch.mockResolvedValue("feature");

    const { box, toast } = mount();
    await settle();

    expect(box.querySelector(".git-sync-ahead")?.textContent).toBe("1");
    expect(box.querySelector(".git-sync-behind")?.textContent).toBe("2");
    expect(box.querySelectorAll(".git-sync-btn")).toHaveLength(3);

    const changes = () => box.querySelectorAll(".git-list")[0];
    const rowFor = (path: string): HTMLElement =>
      [...changes().querySelectorAll<HTMLElement>(".git-item")].find(
        (r) => r.querySelector(".git-path")?.textContent === path,
      )!;

    // Stage tool → git_stage_path, then the panel reloads.
    rowFor("a.log")
      .querySelector<HTMLButtonElement>('.git-row-tool[title="暂存此文件"]')!
      .click();
    await settle();
    expect(stagePath).toHaveBeenCalledWith("/repo/", "a.log");
    expect(toast).toHaveBeenCalledWith("已暂存：a.log");

    // Discard asks first, then passes confirmed=true (backend second gate).
    rowFor("b.log")
      .querySelector<HTMLButtonElement>('.git-row-tool[title="丢弃此文件的改动"]')!
      .click();
    await settle();
    expect(ask).toHaveBeenCalled();
    expect(discardPath).toHaveBeenCalledWith("/repo/", "b.log", true);
    expect(toast).toHaveBeenCalledWith("已丢弃改动：b.log");

    // Branch menu lists local branches and switches on click.
    (box.querySelector(".git-branch-btn") as HTMLButtonElement).click();
    await settle();
    expect(branches).toHaveBeenCalledWith("/repo/");
    const feature = [...box.querySelectorAll<HTMLElement>(".git-branch-row")].find(
      (r) => r.textContent?.includes("feature"),
    )!;
    feature.click();
    await settle();
    expect(checkoutBranch).toHaveBeenCalledWith("/repo/", "feature", false);
    expect(toast).toHaveBeenCalledWith("已切换到分支 feature");
  });

  it("switches the commit form for a merge in progress", async () => {
    probe.mockResolvedValue(repoInfo());
    status.mockResolvedValue([change("a.log", "conflicted")]);
    log.mockResolvedValue([commit("abcdef0123456", "first")]);
    mergeState.mockResolvedValue(true);
    mergeCommit.mockResolvedValue("merge01");

    const { box, toast } = mount();
    await settle();

    expect(box.querySelector(".git-merge-bar")).not.toBeNull();
    expect(box.querySelector(".git-merge-title")?.textContent).toBe(
      "合并进行中 · 1 处冲突待解决",
    );
    expect(box.querySelector<HTMLButtonElement>(".git-merge-bar .git-btn-primary")?.disabled).toBe(
      true,
    );
    // Conflicts offer exactly the local/remote side pickers.
    expect(box.querySelectorAll('.git-row-tool[title="用本地版本解决（ours）"]')).toHaveLength(1);
    expect(box.querySelectorAll('.git-row-tool[title="用远端版本解决（theirs）"]')).toHaveLength(1);

    // Refresh with no conflicts → the merge can be committed with the message.
    status.mockResolvedValue([]);
    (box.querySelector(".git-btn") as HTMLButtonElement).click();
    await settle();
    const submit = box.querySelector<HTMLButtonElement>(".git-merge-bar .git-btn-primary");
    expect(submit?.disabled).toBe(false);
    submit?.click();
    await settle();
    expect(mergeCommit).toHaveBeenCalledWith("/repo/", "Merge origin/main");
    expect(toast).toHaveBeenCalledWith("已提交合并 merge01");
  });

  it("falls back to the error placeholder when git cannot be read", async () => {
    probe.mockRejectedValue(new FakeIpcError("仓库损坏"));
    const { box } = mount();
    await settle();
    expect(box.querySelector(".git-empty")?.textContent).toBe("读取 Git 状态失败：仓库损坏");
  });
});
