//! Inspector Git section (Plans 34/35): repo probe, init, and stage+commit via
//! built-in libgit2 (`git2`) — no PATH dependency, no shell. Read-only besides
//! init/commit; the commit covers the whole index (same as the git CLI).

use git2::{
    BranchType, Cred, CredentialType, ErrorClass, ErrorCode, FetchOptions, ObjectType,
    PushOptions, RemoteCallbacks, Repository, RepositoryState, ResetType, Signature, Status,
};
use git2::build::CheckoutBuilder;
use serde::Serialize;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex};

/// Repo-relative path for an absolute file. macOS `/var` is a symlink to
/// `/private/var`, and libgit2 reports the canonical workdir while callers may
/// pass the symlinked form; normalize both sides before stripping the prefix.
fn repo_relative_path(repo: &Repository, file: &Path) -> Option<PathBuf> {
    let norm = |p: PathBuf| p.canonicalize().unwrap_or(p);
    let workdir = norm(repo.workdir()?.to_path_buf());
    let target = norm(file.to_path_buf());
    Some(target.strip_prefix(&workdir).ok()?.to_path_buf())
}

#[derive(Debug, Serialize)]
pub struct GitInfo {
    pub repo: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub root: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_commit: Option<LastCommit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub identity: Option<GitIdentity>,
    /// Some(msg) when git itself is unusable (repository open failure beyond
    /// the normal "no repo here").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unavailable: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct GitIdentity {
    pub name: String,
    pub email: String,
}

#[derive(Debug, Serialize)]
pub struct LastCommit {
    pub hash: String,
    pub subject: String,
    /// Unix seconds (front end formats).
    pub time: i64,
}

/// Map libgit2 status flags to a zh label; None = clean (Plan 35 spec §1).
pub fn status_label(s: Status) -> Option<String> {
    if s.is_empty() {
        return None;
    }
    if s.contains(Status::IGNORED) {
        return Some("已忽略".to_string());
    }
    let index = if s.contains(Status::INDEX_NEW) {
        "新增"
    } else if s.contains(Status::INDEX_MODIFIED) {
        "修改"
    } else if s.contains(Status::INDEX_DELETED) {
        "删除"
    } else if s.contains(Status::INDEX_RENAMED) {
        "重命名"
    } else {
        ""
    };
    let wt = if s.contains(Status::WT_NEW) {
        Some("未暂存的未跟踪文件")
    } else if s.contains(Status::WT_MODIFIED) {
        Some("未暂存修改")
    } else if s.contains(Status::WT_DELETED) {
        Some("未暂存删除")
    } else if s.contains(Status::WT_RENAMED) {
        Some("未暂存重命名")
    } else {
        None
    };
    match (index.is_empty(), wt) {
        (false, None) => Some(format!("已暂存（{index}）")),
        (false, Some(w)) => Some(format!("已暂存（{index}），另有{w}")),
        (true, Some(_)) if s.contains(Status::WT_NEW) => Some("未跟踪".to_string()),
        (true, Some(w)) => Some(w.to_string()),
        (true, None) => Some(format!("状态 {s:?}")),
    }
}

fn probe_repo(repo: &Repository, file: &Path) -> GitInfo {
    let root = repo
        .workdir()
        .map(|p| p.to_string_lossy().to_string())
        .or_else(|| repo.path().to_str().map(String::from));
    let branch = match repo.head() {
        // Detached HEAD: `head()` still succeeds (name "HEAD"), so check
        // `is_branch` before trusting the shorthand.
        Ok(h) if h.is_branch() => h.shorthand().map(String::from),
        Ok(_) => Some("（分离 HEAD）".to_string()),
        Err(e) if e.code() == ErrorCode::UnbornBranch => repo
            .find_reference("HEAD")
            .ok()
            .and_then(|r| r.symbolic_target().map(String::from))
            .map(|t| t.trim_start_matches("refs/heads/").to_string()),
        // detached HEAD: symbolic target is a raw oid
        Err(_) => repo
            .find_reference("HEAD")
            .ok()
            .filter(|r| r.symbolic_target().is_none())
            .map(|_| "（分离 HEAD）".to_string()),
    };
    let file_status = repo_relative_path(repo, file)
        .and_then(|rel| repo.status_file(&rel).ok())
        .and_then(status_label);
    let last_commit = last_commit_for(repo, file);
    let identity = repo_config_identity(repo);
    GitInfo {
        repo: true,
        root,
        branch,
        file_status,
        last_commit,
        identity,
        unavailable: None,
    }
}

fn probe_err(unavailable: Option<String>) -> GitInfo {
    GitInfo {
        repo: false,
        root: None,
        branch: None,
        file_status: None,
        last_commit: None,
        identity: None,
        unavailable,
    }
}

/// Probe the repo containing `dir`, reporting `file`'s status (absolute path).
pub fn git_probe(dir: &Path, file: &Path) -> GitInfo {
    match Repository::discover(dir) {
        Ok(repo) => probe_repo(&repo, file),
        Err(e) if e.code() == ErrorCode::NotFound => probe_err(None),
        Err(e) => probe_err(Some(format!("git 不可用：{e}"))),
    }
}

pub fn git_init(dir: &Path) -> Result<(), git2::Error> {
    Repository::init(dir).map(|_| ())
}

/// Last commit that touched `file` (absolute path); walks from HEAD comparing
/// the path's oid against each parent tree. Empty repo → None.
fn last_commit_for(repo: &Repository, file: &Path) -> Option<LastCommit> {
    let rel = repo_relative_path(repo, file)?;
    let mut rev = repo.revwalk().ok()?;
    rev.push_head().ok()?;
    let mut previous: Option<git2::Oid> = None;
    for oid in rev.flatten() {
        let commit = repo.find_commit(oid).ok()?;
        let tree = commit.tree().ok()?;
        let entry = tree.get_path(&rel).ok().map(|e| e.id());
        if entry != previous {
            if let (Some(cur), Some(parent)) = (&entry, commit.parent(0).ok()) {
                let had = parent.tree().ok()?.get_path(&rel).ok().map(|e| e.id());
                if had == Some(*cur) {
                    previous = entry;
                    continue; // path unchanged in this commit
                }
            }
            return Some(LastCommit {
                hash: commit.as_object().short_id().ok()?.as_str()?.to_string(),
                subject: commit.summary()?.to_string(),
                time: commit.time().seconds(),
            });
        }
        previous = entry;
    }
    None
}

fn repo_config_identity(repo: &Repository) -> Option<GitIdentity> {
    let cfg = repo.config().ok()?;
    let name = cfg.get_string("user.name").ok()?;
    let email = cfg.get_string("user.email").ok()?;
    Some(GitIdentity { name, email })
}

/// Resolve the commit identity: explicit params win, then repo/global config.
fn resolve_identity(
    explicit: Option<(&str, &str)>,
    from_config: Option<(String, String)>,
) -> Result<(String, String), String> {
    if let Some((n, e)) = explicit {
        return Ok((n.to_string(), e.to_string()));
    }
    if let Some(id) = from_config {
        return Ok(id);
    }
    Err(
        "缺少提交者身份：请在提交框填写姓名与邮箱，或执行 git config --global user.name / user.email"
            .to_string(),
    )
}

/// Stage `file` (absolute) and commit the whole index. Returns the short hash.
pub fn git_stage_and_commit(
    file: &Path,
    message: &str,
    name: Option<&str>,
    email: Option<&str>,
) -> Result<String, String> {
    if message.trim().is_empty() {
        return Err("提交信息不能为空".to_string());
    }
    let repo = Repository::discover(file.parent().unwrap_or(file))
        .map_err(|e| format!("未找到 Git 仓库：{e}"))?;
    let rel = repo_relative_path(&repo, file)
        .ok_or_else(|| "文件不在仓库工作区内".to_string())?;
    let mut index = repo.index().map_err(|e| e.to_string())?;
    index.add_path(&rel).map_err(|e| e.to_string())?;
    index.write().map_err(|e| e.to_string())?;
    let tree_oid = index.write_tree().map_err(|e| e.to_string())?;
    let tree = repo.find_tree(tree_oid).map_err(|e| e.to_string())?;
    let explicit = match (name, email) {
        (Some(n), Some(e)) => Some((n, e)),
        // 姓名邮箱只填一半时整体走配置回退，避免拼出半真半假的作者
        _ => None,
    };
    let from_cfg = repo_config_identity(&repo).map(|i| (i.name, i.email));
    let (n, e) = resolve_identity(explicit, from_cfg)?;
    let sig = Signature::now(&n, &e).map_err(|er| er.to_string())?;
    let head_commit = repo.head().ok().and_then(|h| h.peel_to_commit().ok());
    let parents: Vec<&git2::Commit> = match &head_commit {
        Some(c) => vec![c],
        None => vec![], // empty repo: HEAD update creates the branch
    };
    let oid = repo
        .commit(Some("HEAD"), &sig, &sig, message, &tree, &parents)
        .map_err(|er| er.to_string())?;
    Ok(repo
        .find_commit(oid)
        .ok()
        .and_then(|c| c.as_object().short_id().ok())
        .and_then(|b| b.as_str().map(String::from))
        .unwrap_or_else(|| oid.to_string()))
}

/// Stage every tracked/untracked change (same as `git add -A`) and commit.
/// Identity: explicit params → repo/global config → error. Returns the short
/// hash. An unchanged tree is rejected instead of creating an empty commit.
pub fn git_stage_all_and_commit(
    root: &Path,
    message: &str,
    name: Option<&str>,
    email: Option<&str>,
) -> Result<String, String> {
    let message = message.trim_end();
    if message.trim().is_empty() {
        return Err("提交信息不能为空".to_string());
    }
    let repo = Repository::discover(root).map_err(|e| format!("未找到 Git 仓库：{e}"))?;
    let mut index = repo.index().map_err(|e| e.to_string())?;
    index
        .add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)
        .map_err(|e| e.to_string())?;
    index
        .update_all(["*"].iter(), None)
        .map_err(|e| e.to_string())?;
    index.write().map_err(|e| e.to_string())?;
    let tree_oid = index.write_tree().map_err(|e| e.to_string())?;
    let tree = repo.find_tree(tree_oid).map_err(|e| e.to_string())?;

    let head_commit = repo.head().ok().and_then(|h| h.peel_to_commit().ok());
    if let Some(hc) = &head_commit {
        if hc.tree().map(|t| t.id()).ok() == Some(tree_oid) {
            return Err("没有可提交的更改".to_string());
        }
    }
    let explicit = match (name, email) {
        (Some(n), Some(e)) if !n.trim().is_empty() && !e.trim().is_empty() => Some((n, e)),
        _ => None,
    };
    let from_cfg = repo_config_identity(&repo).map(|i| (i.name, i.email));
    let had_cfg = from_cfg.is_some();
    let (n, e) = resolve_identity(explicit, from_cfg)?;
    let sig = Signature::now(&n, &e).map_err(|er| er.to_string())?;
    let parents: Vec<&git2::Commit> = head_commit.iter().collect();
    let oid = repo
        .commit(Some("HEAD"), &sig, &sig, message, &tree, &parents)
        .map_err(|er| er.to_string())?;
    // An identity typed only for this commit is remembered in the local config.
    if !had_cfg && name.is_some() && email.is_some() {
        if let Ok(mut local) = git2::Config::open(&repo.path().join("config")) {
            let _ = local.set_str("user.name", &n);
            let _ = local.set_str("user.email", &e);
        }
    }
    Ok(repo
        .find_commit(oid)
        .ok()
        .and_then(|c| c.as_object().short_id().ok())
        .and_then(|b| b.as_str().map(String::from))
        .unwrap_or_else(|| oid.to_string()))
}

// ---- Read-only panel data (NoteLume parity) --------------------------------
// NoteLume's Git panel needs three reads beyond the probe: the change list,
// the commit log, and a per-file unified diff. `git2` supplies all three
// without a PATH dependency, so the inspector can render real data.

#[derive(Debug, Serialize)]
pub struct GitFileStatus {
    pub path: String,
    /// "new" | "modified" | "deleted" | "renamed" | "untracked" | "conflicted"
    pub kind: String,
    pub staged: bool,
}

#[derive(Debug, Serialize)]
pub struct GitCommitInfo {
    pub id: String,
    pub id_short: String,
    pub summary: String,
    pub author_name: String,
    pub author_email: String,
    /// Unix seconds (front end formats).
    pub time: i64,
    pub parents: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct GitCommitFile {
    pub path: String,
    /// "A" | "M" | "D" | "R" | "C" | "T"
    pub kind: String,
}

/// Classify a status flag the way NoteLume does: index state first (staged),
/// then worktree state (unstaged), conflicts before the rest.
fn classify_status(s: Status) -> (&'static str, bool) {
    if s.contains(Status::INDEX_NEW) {
        ("new", true)
    } else if s.contains(Status::INDEX_MODIFIED) {
        ("modified", true)
    } else if s.contains(Status::INDEX_DELETED) {
        ("deleted", true)
    } else if s.contains(Status::INDEX_RENAMED) {
        ("renamed", true)
    } else if s.contains(Status::CONFLICTED) {
        ("conflicted", false)
    } else if s.contains(Status::WT_NEW) {
        ("untracked", false)
    } else if s.contains(Status::WT_MODIFIED) {
        ("modified", false)
    } else if s.contains(Status::WT_DELETED) {
        ("deleted", false)
    } else if s.contains(Status::WT_RENAMED) {
        ("renamed", false)
    } else {
        ("modified", false)
    }
}

/// Whole-repo change list (untracked included, ignored excluded), capped at 1000.
pub fn git_status(root: &Path) -> Result<Vec<GitFileStatus>, String> {
    let repo = Repository::discover(root).map_err(|e| format!("未找到 Git 仓库：{e}"))?;
    let mut opts = git2::StatusOptions::new();
    opts.recurse_untracked_dirs(true)
        .include_untracked(true)
        .include_ignored(false);
    let statuses = repo.statuses(Some(&mut opts)).map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for entry in statuses.iter() {
        let path = match entry.path() {
            Some(p) => p.to_string(),
            None => continue,
        };
        let (kind, staged) = classify_status(entry.status());
        out.push(GitFileStatus {
            path,
            kind: kind.to_string(),
            staged,
        });
        if out.len() >= 1000 {
            break;
        }
    }
    Ok(out)
}

/// Walk HEAD backwards (time order), cap `limit`. Empty repo → empty list.
pub fn git_log(root: &Path, limit: usize) -> Result<Vec<GitCommitInfo>, String> {
    let repo = Repository::discover(root).map_err(|e| format!("未找到 Git 仓库：{e}"))?;
    if repo.head().is_err() {
        return Ok(Vec::new());
    }
    let mut revwalk = repo.revwalk().map_err(|e| e.to_string())?;
    revwalk.push_head().map_err(|e| e.to_string())?;
    revwalk
        .set_sorting(git2::Sort::TIME)
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for oid in revwalk.flatten().take(limit) {
        let commit = repo.find_commit(oid).map_err(|e| e.to_string())?;
        let id = commit.id().to_string();
        let author = commit.author();
        out.push(GitCommitInfo {
            id: id.clone(),
            id_short: id.chars().take(7).collect(),
            summary: commit.summary().unwrap_or("").to_string(),
            author_name: author.name().unwrap_or("").to_string(),
            author_email: author.email().unwrap_or("").to_string(),
            time: commit.time().seconds(),
            parents: commit.parent_ids().map(|o| o.to_string()).collect(),
        });
    }
    Ok(out)
}

/// One file's unified diff. `against="HEAD"` compares the worktree (untracked
/// included) with HEAD; a commit id compares that commit with its first parent.
pub fn git_diff_file(root: &Path, rel: &str, against: &str) -> Result<String, String> {
    let repo = Repository::discover(root).map_err(|e| format!("未找到 Git 仓库：{e}"))?;
    let workdir = repo
        .workdir()
        .ok_or_else(|| "仓库没有工作区".to_string())?
        .to_path_buf();

    // New/untracked file: git2's tree→workdir diff omits untracked entries,
    // so synthesize "vs empty tree" from the bytes on disk.
    if (against.eq_ignore_ascii_case("head") || against.is_empty())
        && !head_has_path(&repo, rel)
    {
        let abs = workdir.join(rel);
        let bytes = std::fs::read(&abs).map_err(|e| format!("读取失败：{e}"))?;
        return Ok(synthesize_added_patch(rel, &bytes));
    }

    let mut opts = git2::DiffOptions::new();
    opts.pathspec(rel)
        .include_untracked(true)
        .recurse_untracked_dirs(true);
    let diff = if against.eq_ignore_ascii_case("head") || against.is_empty() {
        let head_tree = repo
            .head()
            .ok()
            .and_then(|h| h.peel_to_tree().ok());
        repo.diff_tree_to_workdir_with_index(head_tree.as_ref(), Some(&mut opts))
            .map_err(|e| e.to_string())?
    } else {
        let oid = git2::Oid::from_str(against).map_err(|e| e.to_string())?;
        let commit = repo.find_commit(oid).map_err(|e| e.to_string())?;
        let tree = commit.tree().map_err(|e| e.to_string())?;
        let parent_tree = commit.parents().next().and_then(|p| p.tree().ok());
        repo.diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), Some(&mut opts))
            .map_err(|e| e.to_string())?
    };
    Ok(render_patch(&diff))
}

fn head_has_path(repo: &Repository, path: &str) -> bool {
    match repo.head().ok().and_then(|h| h.peel_to_tree().ok()) {
        Some(tree) => tree.get_path(Path::new(path)).is_ok(),
        None => false,
    }
}

fn render_patch(diff: &git2::Diff) -> String {
    let mut buf = String::new();
    let _ = diff.print(git2::DiffFormat::Patch, |_delta, _hunk, line| {
        use git2::DiffLineType::*;
        let content = std::str::from_utf8(line.content()).unwrap_or("");
        match line.origin_value() {
            Context | Addition | Deletion | ContextEOFNL | AddEOFNL | DeleteEOFNL => {
                buf.push(line.origin());
                buf.push_str(content);
            }
            _ => buf.push_str(content),
        }
        true
    });
    buf
}

fn synthesize_added_patch(path: &str, bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes);
    let mut out = String::new();
    out.push_str(&format!("diff --git a/{p} b/{p}\n", p = path));
    out.push_str("--- /dev/null\n");
    out.push_str(&format!("+++ b/{p}\n", p = path));
    let lines: Vec<&str> = text.split('\n').collect();
    let n = if lines.last() == Some(&"") {
        lines.len() - 1
    } else {
        lines.len()
    };
    out.push_str(&format!("@@ -0,0 +1,{n} @@\n"));
    for l in lines.iter().take(n) {
        out.push('+');
        out.push_str(l);
        out.push('\n');
    }
    out
}

/// Files touched by a commit (vs its first parent), for the detail preview.
pub fn git_commit_files(root: &Path, id: &str) -> Result<Vec<GitCommitFile>, String> {
    let repo = Repository::discover(root).map_err(|e| format!("未找到 Git 仓库：{e}"))?;
    let oid = git2::Oid::from_str(id).map_err(|e| e.to_string())?;
    let commit = repo.find_commit(oid).map_err(|e| e.to_string())?;
    let tree = commit.tree().map_err(|e| e.to_string())?;
    let parent_tree = commit.parents().next().and_then(|p| p.tree().ok());
    let diff = repo
        .diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), None)
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    diff.foreach(
        &mut |delta, _| {
            let path = delta
                .new_file()
                .path()
                .or_else(|| delta.old_file().path())
                .map(|p| p.to_string_lossy().to_string());
            if let Some(path) = path {
                let kind = match delta.status() {
                    git2::Delta::Added => "A",
                    git2::Delta::Deleted => "D",
                    git2::Delta::Renamed => "R",
                    git2::Delta::Copied => "C",
                    git2::Delta::Typechange => "T",
                    _ => "M",
                };
                out.push(GitCommitFile {
                    path,
                    kind: kind.to_string(),
                });
            }
            true
        },
        None,
        None,
        None,
    )
    .map_err(|e| e.to_string())?;
    Ok(out)
}

// ---- Write operations (NoteLume parity) ------------------------------------
// Every entry point below stays inside libgit2; no shell, no PATH. Errors use
// `[code]message` prefixes so the IPC layer can hand the front end a stable
// code (git_dirty / no_upstream / auth / network / git_conflict) instead of a
// single opaque "git failed". Codes are stripped by `commands::git_err`.

fn open_repo(root: &Path) -> Result<Repository, String> {
    Repository::discover(root).map_err(|e| format!("未找到 Git 仓库：{e}"))
}

/// Reject absolute paths and `..` so an IPC path can never escape the repo.
fn safe_rel(rel: &str) -> Result<&Path, String> {
    let p = Path::new(rel);
    if p.is_absolute()
        || p.components().any(|c| {
            matches!(
                c,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(format!("[git]非法的仓库相对路径：{rel}"));
    }
    Ok(p)
}

/// Substantive uncommitted changes (untracked files are not counted: they are
/// never overwritten by a safe checkout).
fn dirty_count(repo: &Repository) -> Result<usize, String> {
    let mut opts = git2::StatusOptions::new();
    opts.include_untracked(false).include_ignored(false);
    let statuses = repo.statuses(Some(&mut opts)).map_err(|e| e.to_string())?;
    let mask = Status::INDEX_NEW
        | Status::INDEX_MODIFIED
        | Status::INDEX_DELETED
        | Status::INDEX_RENAMED
        | Status::INDEX_TYPECHANGE
        | Status::WT_MODIFIED
        | Status::WT_DELETED
        | Status::WT_RENAMED
        | Status::WT_TYPECHANGE
        | Status::CONFLICTED;
    Ok(statuses
        .iter()
        .filter(|e| e.status().intersects(mask))
        .count())
}

fn head_branch_name(repo: &Repository) -> Option<String> {
    repo.head()
        .ok()
        .filter(|h| h.is_branch())
        .and_then(|h| h.shorthand().map(String::from))
}

fn workdir(repo: &Repository) -> Result<std::path::PathBuf, String> {
    repo.workdir()
        .map(|p| p.to_path_buf())
        .ok_or_else(|| "[git]仓库没有工作区（裸仓库不支持此操作）".to_string())
}

/// Stage one repo-relative path. Deleted files are removed from the index
/// instead (libgit2's `add_path` cannot add a missing file).
pub fn git_stage_path(root: &Path, rel: &str) -> Result<(), String> {
    let repo = open_repo(root)?;
    let path = safe_rel(rel)?;
    let mut index = repo.index().map_err(|e| format!("[git]读取索引失败：{e}"))?;
    match index.add_path(path) {
        Ok(()) => {}
        Err(e) if e.code() == ErrorCode::NotFound => {
            index
                .remove_path(path)
                .map_err(|e| format!("[git]暂存删除失败：{e}"))?;
        }
        Err(e) => return Err(format!("[git]暂存失败：{e}")),
    }
    index.write().map_err(|e| format!("[git]写索引失败：{e}"))
}

/// Unstage one path: reset its index entry to HEAD (unborn HEAD → drop it).
pub fn git_unstage_path(root: &Path, rel: &str) -> Result<(), String> {
    let repo = open_repo(root)?;
    let path = safe_rel(rel)?;
    // Bind the peeled object before matching: the temporary must drop before
    // `repo`, otherwise the borrow outlives the repository.
    let head_obj = repo.head().ok().and_then(|h| h.peel(ObjectType::Commit).ok());
    match head_obj.as_ref() {
        Some(obj) => repo
            .reset_default(Some(obj), [path])
            .map_err(|e| format!("[git]取消暂存失败：{e}")),
        None => {
            let mut index = repo.index().map_err(|e| format!("[git]读取索引失败：{e}"))?;
            let _ = index.remove_path(path); // not in the index: nothing to undo
            index.write().map_err(|e| format!("[git]写索引失败：{e}"))
        }
    }
}

/// Discard one worktree change: untracked/staged-new → delete the file (and
/// its index entry); tracked → write the HEAD blob back and re-add it.
/// `confirmed` is a hard guard: an unconfirmed call is rejected, so a UI
/// wiring bug can never throw away user edits silently.
pub fn git_discard_path(root: &Path, rel: &str, confirmed: bool) -> Result<(), String> {
    if !confirmed {
        return Err("[git_unconfirmed]丢弃文件改动需要显式确认；已取消".to_string());
    }
    let repo = open_repo(root)?;
    let path = safe_rel(rel)?;
    let status = repo
        .status_file(path)
        .map_err(|e| format!("[git]读取文件状态失败：{e}"))?;
    if status.is_empty() {
        return Err("[git]该文件没有可丢弃的改动".to_string());
    }
    let full = workdir(&repo)?.join(path);

    if status.contains(Status::WT_NEW) || status.contains(Status::INDEX_NEW) {
        if full.exists() {
            std::fs::remove_file(&full).map_err(|e| format!("[git]删除文件失败：{e}"))?;
        }
        if status.contains(Status::INDEX_NEW) {
            let mut index = repo.index().map_err(|e| format!("[git]读取索引失败：{e}"))?;
            index
                .remove_path(path)
                .map_err(|e| format!("[git]移除索引条目失败：{e}"))?;
            index.write().map_err(|e| format!("[git]写索引失败：{e}"))?;
        }
        return Ok(());
    }

    let head_commit = repo
        .head()
        .ok()
        .and_then(|h| h.peel_to_commit().ok())
        .ok_or_else(|| "[git]仓库还没有提交，无法恢复文件".to_string())?;
    let entry_id = head_commit
        .tree()
        .ok()
        .and_then(|t| t.get_path(path).ok().map(|e| e.id()))
        .ok_or_else(|| {
            "[git]文件不在 HEAD 中（已暂存的新文件请先取消暂存再删除）".to_string()
        })?;
    let blob = repo
        .find_blob(entry_id)
        .map_err(|e| format!("[git]读取 HEAD 版本失败：{e}"))?;
    if let Some(dir) = full.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("[git]创建目录失败：{e}"))?;
    }
    std::fs::write(&full, blob.content()).map_err(|e| format!("[git]恢复文件失败：{e}"))?;
    let mut index = repo.index().map_err(|e| format!("[git]读取索引失败：{e}"))?;
    index
        .add_path(path)
        .map_err(|e| format!("[git]重置索引条目失败：{e}"))?;
    index.write().map_err(|e| format!("[git]写索引失败：{e}"))
}

#[derive(Debug, Serialize)]
pub struct GitBranchInfo {
    pub name: String,
    pub current: bool,
    pub remote: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub upstream: Option<String>,
}

/// Local branches first, remote branches after (NoteLume's order).
pub fn git_branches(root: &Path) -> Result<Vec<GitBranchInfo>, String> {
    let repo = open_repo(root)?;
    let current = head_branch_name(&repo);
    let mut out = Vec::new();
    for (kind, remote) in [(BranchType::Local, false), (BranchType::Remote, true)] {
        for item in repo
            .branches(Some(kind))
            .map_err(|e| format!("[git]读取分支失败：{e}"))?
        {
            let (branch, _) = item.map_err(|e| format!("[git]读取分支失败：{e}"))?;
            let name = match branch.name().ok().flatten() {
                Some(n) if !n.is_empty() => n.to_string(),
                _ => continue,
            };
            if name.ends_with("/HEAD") {
                continue; // symbolic origin/HEAD
            }
            let upstream = if remote {
                None
            } else {
                branch
                    .upstream()
                    .ok()
                    .and_then(|u| u.name().ok().flatten().map(str::to_string))
            };
            out.push(GitBranchInfo {
                current: !remote && current.as_deref() == Some(name.as_str()),
                name,
                remote,
                upstream,
            });
        }
    }
    Ok(out)
}

/// Check out a local branch, or a remote-tracking name (`origin/foo` → local
/// `foo`, creating it with its upstream when missing). `force=false` keeps
/// libgit2's safe mode: any local edit that would be overwritten aborts the
/// checkout (nothing is discarded). `force=true` is only reached after an
/// explicit UI confirmation because it *will* discard those edits.
pub fn git_checkout_branch(root: &Path, name: &str, force: bool) -> Result<String, String> {
    let repo = open_repo(root)?;
    let (target_ref, target_oid) = match repo.find_branch(name, BranchType::Local) {
        Ok(branch) => {
            let commit = branch
                .get()
                .peel_to_commit()
                .map_err(|e| format!("[git]读取分支失败：{e}"))?;
            (format!("refs/heads/{name}"), commit.id())
        }
        Err(_) => {
            let remote_branch = repo
                .find_branch(name, BranchType::Remote)
                .map_err(|_| format!("[git]分支不存在：{name}"))?;
            let commit = remote_branch
                .get()
                .peel_to_commit()
                .map_err(|e| format!("[git]读取远端分支失败：{e}"))?;
            let local = name.split_once('/').map(|(_, b)| b).unwrap_or(name);
            match repo.find_branch(local, BranchType::Local) {
                Ok(local_branch) => {
                    let tip = local_branch
                        .get()
                        .peel_to_commit()
                        .map_err(|e| format!("[git]读取分支失败：{e}"))?;
                    (format!("refs/heads/{local}"), tip.id())
                }
                Err(_) => {
                    let mut created = repo
                        .branch(local, &commit, false)
                        .map_err(|e| format!("[git]创建本地分支失败：{e}"))?;
                    created
                        .set_upstream(Some(name))
                        .map_err(|e| format!("[git]设置上游失败：{e}"))?;
                    (format!("refs/heads/{local}"), commit.id())
                }
            }
        }
    };

    let obj = repo
        .find_object(target_oid, Some(ObjectType::Commit))
        .map_err(|e| format!("[git]读取目标提交失败：{e}"))?;
    let mut opts = CheckoutBuilder::new();
    opts.update_index(true);
    if force {
        opts.force();
    } else {
        opts.safe();
    }
    if let Err(e) = repo.checkout_tree(&obj, Some(&mut opts)) {
        let n = dirty_count(&repo).unwrap_or(0);
        return Err(format!(
            "[git_dirty]检出被阻止：工作区有 {n} 项未提交改动，或它们与目标分支冲突；未覆盖任何本地改动。请先提交/暂存，或明确选择强制切换（会丢弃这些改动）。原始错误：{e}"
        ));
    }
    repo.set_head(&target_ref)
        .map_err(|e| format!("[git]切换 HEAD 失败：{e}"))?;
    Ok(target_ref.trim_start_matches("refs/heads/").to_string())
}

/// Create a branch at `start_point` (commit-ish, default HEAD) and optionally
/// switch to it. The name is validated against git's own ref rules *here* too,
/// so a bad name can never reach the ref store. When the switch would overwrite
/// local edits the new branch is deleted again and `[git_dirty]` is returned —
/// the worktree is never touched.
pub fn git_branch_create(
    root: &Path,
    name: &str,
    start_point: Option<&str>,
    checkout: bool,
) -> Result<String, String> {
    let repo = open_repo(root)?;
    let name = name.trim();
    if name.is_empty() {
        return Err("[git_invalid_name]分支名不能为空".to_string());
    }
    // `Reference::is_valid_name` covers most of git's rules; these are the
    // shorthand cases it accepts but `git check-ref-format --branch` does not.
    let shorthand_bad = name == "@"
        || name.eq_ignore_ascii_case("HEAD")
        || name.starts_with('-')
        || name.starts_with('.')
        || name.ends_with('.')
        || name.ends_with(".lock")
        || name.contains("@{")
        || name.ends_with('/')
        || name.starts_with('/')
        || name.contains("//")
        || name.contains("..")
        || name.chars().any(|c| c.is_whitespace() || c.is_control());
    if shorthand_bad || !git2::Reference::is_valid_name(&format!("refs/heads/{name}")) {
        return Err(format!("[git_invalid_name]分支名不合法：{name}"));
    }
    if repo.find_branch(name, BranchType::Local).is_ok() {
        return Err(format!("[git_exists]分支已存在：{name}"));
    }
    let start = start_point
        .map(str::trim)
        .filter(|s| !s.is_empty() && !s.eq_ignore_ascii_case("HEAD"));
    let commit = match start {
        Some(rev) => repo
            .revparse_single(rev)
            .and_then(|o| o.peel_to_commit())
            .map_err(|e| format!("[git]找不到起点 {rev}：{e}"))?,
        None => repo
            .head()
            .and_then(|h| h.peel_to_commit())
            .map_err(|_| "[git]仓库还没有任何提交，无法创建分支".to_string())?,
    };
    let oid = commit.id();
    let short = oid.to_string().chars().take(7).collect::<String>();
    let mut branch = repo
        .branch(name, &commit, false)
        .map_err(|e| format!("[git]创建分支失败：{e}"))?;
    if !checkout {
        return Ok(format!("已创建分支 {name}（起点 {short}）"));
    }
    let obj = repo
        .find_object(oid, Some(ObjectType::Commit))
        .map_err(|e| format!("[git]读取起点提交失败：{e}"))?;
    let mut opts = CheckoutBuilder::new();
    opts.update_index(true).safe();
    if let Err(e) = repo.checkout_tree(&obj, Some(&mut opts)) {
        let _ = branch.delete();
        let n = dirty_count(&repo).unwrap_or(0);
        return Err(format!(
            "[git_dirty]新建分支已取消：工作区有 {n} 项未提交改动会与目标冲突，未覆盖任何本地改动。原始错误：{e}"
        ));
    }
    repo.set_head(&format!("refs/heads/{name}"))
        .map_err(|e| format!("[git]切换 HEAD 失败：{e}"))?;
    Ok(format!("已创建并切换到分支 {name}（起点 {short}）"))
}

/// Detached-HEAD checkout of an arbitrary commit (view an old state). The
/// returned message states the detached state explicitly.
pub fn git_checkout_commit(root: &Path, id: &str, force: bool) -> Result<String, String> {
    let repo = open_repo(root)?;
    let oid = git2::Oid::from_str(id).map_err(|_| "[git]无效的提交哈希".to_string())?;
    let commit = repo
        .find_commit(oid)
        .map_err(|e| format!("[git]提交不存在：{e}"))?;
    let mut opts = CheckoutBuilder::new();
    opts.update_index(true);
    if force {
        opts.force();
    } else {
        opts.safe();
    }
    if let Err(e) = repo.checkout_tree(commit.as_object(), Some(&mut opts)) {
        let n = dirty_count(&repo).unwrap_or(0);
        return Err(format!(
            "[git_dirty]检出被阻止：工作区有 {n} 项未提交改动，或它们与该提交冲突；未覆盖任何本地改动。请先提交/暂存，或明确选择强制检出。原始错误：{e}"
        ));
    }
    repo.set_head_detached(oid)
        .map_err(|e| format!("[git]切换分离 HEAD 失败：{e}"))?;
    let short = commit
        .as_object()
        .short_id()
        .ok()
        .and_then(|b| b.as_str().map(String::from))
        .unwrap_or_else(|| oid.to_string());
    Ok(format!(
        "已检出提交 {short}（分离 HEAD：当前不在任何分支上，新提交不会推进已有分支；可用分支菜单切回分支继续工作）"
    ))
}

#[derive(Debug, Serialize)]
pub struct GitRemoteStatus {
    pub has_remote: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    pub detached: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub upstream: Option<String>,
    pub has_upstream: bool,
    pub ahead: usize,
    pub behind: usize,
}

fn pick_remote(repo: &Repository) -> Option<(String, String)> {
    let names = repo.remotes().ok()?;
    let mut chosen = names
        .iter()
        .flatten()
        .find(|n| *n == "origin")
        .map(String::from);
    if chosen.is_none() {
        chosen = names.iter().flatten().next().map(String::from);
    }
    let name = chosen?;
    let url = repo
        .find_remote(&name)
        .ok()
        .and_then(|r| r.url().map(String::from))
        .unwrap_or_default();
    Some((name, url))
}

fn upstream_short(repo: &Repository, branch: &str) -> Option<String> {
    // libgit2 wants the full local ref name here; older shorthands silently
    // resolve to "no upstream", which would hide ahead/behind.
    if let Ok(buf) = repo.branch_upstream_name(&format!("refs/heads/{branch}")) {
        if let Some(name) = buf.as_str() {
            return Some(
                name.strip_prefix("refs/remotes/")
                    .unwrap_or(name)
                    .to_string(),
            );
        }
    }
    // Config fallback (also keeps branch names containing dots working).
    let config = repo.config().ok()?;
    let remote = config
        .get_string(&format!("branch.{branch}.remote"))
        .ok()?;
    let merge = config
        .get_string(&format!("branch.{branch}.merge"))
        .ok()?;
    let merge = merge.strip_prefix("refs/heads/").unwrap_or(&merge);
    if remote == "." {
        Some(merge.to_string())
    } else {
        Some(format!("{remote}/{merge}"))
    }
}

fn ahead_behind(repo: &Repository, local: git2::Oid, up_ref: &str) -> Option<(usize, usize)> {
    let r = repo.find_reference(up_ref).ok()?;
    let up_oid = r.peel_to_commit().ok()?.id();
    repo.graph_ahead_behind(local, up_oid).ok()
}

/// Ahead/behind + remote metadata for the sync block. Reads only local state
/// (call `git_fetch` first to refresh `refs/remotes/*`).
pub fn git_remote_status(root: &Path) -> Result<GitRemoteStatus, String> {
    let repo = open_repo(root)?;
    let picked = pick_remote(&repo);
    let remote = picked.as_ref().map(|(n, _)| n.clone());
    let url = picked.as_ref().map(|(_, u)| u.clone()).filter(|u| !u.is_empty());
    let head = repo.head().ok();
    let detached = head.as_ref().map(|h| !h.is_branch()).unwrap_or(false);
    let branch = head_branch_name(&repo);
    let local_oid = head.as_ref().and_then(|h| h.target());

    let mut upstream: Option<String> = None;
    let mut ahead = 0;
    let mut behind = 0;
    if let (Some(b), Some(local)) = (&branch, &local_oid) {
        if let Some(up) = upstream_short(&repo, b) {
            if let Some((a, be)) = ahead_behind(&repo, *local, &format!("refs/remotes/{up}")) {
                ahead = a;
                behind = be;
            }
            upstream = Some(up);
        } else if let Some(rn) = &remote {
            // Not configured, but a same-named remote branch exists: show the
            // counts for orientation; `has_upstream` stays false so push still
            // asks before creating the upstream.
            if let Some((a, be)) = ahead_behind(&repo, *local, &format!("refs/remotes/{rn}/{b}")) {
                ahead = a;
                behind = be;
            }
        }
    }
    Ok(GitRemoteStatus {
        has_remote: remote.is_some(),
        remote,
        url,
        branch,
        detached,
        has_upstream: upstream.is_some(),
        upstream,
        ahead,
        behind,
    })
}

#[derive(Debug, Serialize)]
pub struct GitRemoteInfo {
    pub name: String,
    pub url: String,
}

/// Remote names: no whitespace/control chars, no ref punctuation, no `/`
/// (git rejects all of these too), and not starting with `-` or `.`.
fn validate_remote_name(name: &str) -> Result<(), String> {
    let bad = name.is_empty()
        || name.starts_with('-')
        || name.starts_with('.')
        || name.contains('/')
        || name.contains("..")
        || name.chars().any(|c| {
            c.is_whitespace()
                || c.is_control()
                || matches!(c, '~' | '^' | ':' | '?' | '*' | '[' | '\\' | '@' | '{' | '}')
        });
    if bad {
        return Err(format!(
            "[git_invalid_name]远端名不合法：{name}（可用字母、数字、. _ -，且不以 - 或 . 开头）"
        ));
    }
    Ok(())
}

/// Accept only the shapes git itself accepts: http(s)/ssh/git/file URLs,
/// scp-like `user@host:path`, or a local absolute path. Junk never reaches the
/// config file.
fn validate_remote_url(url: &str) -> Result<(), String> {
    let u = url.trim();
    let lower = u.to_ascii_lowercase();
    let scheme_ok = ["https://", "http://", "ssh://", "git://", "file://"]
        .iter()
        .any(|p| lower.len() > p.len() && lower.starts_with(p));
    let scp_ok = match (u.find('@'), u.find(':')) {
        (Some(at), Some(colon)) if at > 0 && colon > at + 1 => !u.chars().any(|c| c.is_whitespace()),
        _ => false,
    };
    let path_ok = !u.chars().any(|c| c.is_whitespace())
        && u.chars().any(|c| c.is_alphanumeric())
        && (u.starts_with('/')
            || u.starts_with('\\')
            || {
                let b = u.as_bytes();
                b.len() > 2
                    && b[0].is_ascii_alphabetic()
                    && b[1] == b':'
                    && (b[2] == b'\\' || b[2] == b'/')
            });
    if !(scheme_ok || scp_ok || path_ok) {
        return Err(format!(
            "[git_invalid_url]远端地址不合法：{url}（支持 https://、ssh://、git@host:path 或本地路径）"
        ));
    }
    Ok(())
}

pub fn git_remotes(root: &Path) -> Result<Vec<GitRemoteInfo>, String> {
    let repo = open_repo(root)?;
    let names = repo
        .remotes()
        .map_err(|e| format!("[git]读取远端失败：{e}"))?;
    let mut out = Vec::new();
    for name in names.iter().flatten() {
        let url = repo
            .find_remote(name)
            .ok()
            .and_then(|r| r.url().map(str::to_string))
            .unwrap_or_default();
        out.push(GitRemoteInfo {
            name: name.to_string(),
            url,
        });
    }
    Ok(out)
}

/// Config-only write (no network). Name and URL are validated first so the
/// dialog's inline rules and the backend agree.
pub fn git_remote_add(root: &Path, name: &str, url: &str) -> Result<(), String> {
    let repo = open_repo(root)?;
    let name = name.trim();
    let url = url.trim();
    validate_remote_name(name)?;
    validate_remote_url(url)?;
    if repo.find_remote(name).is_ok() {
        return Err(format!("[git_exists]远端已存在：{name}"));
    }
    repo.remote(name, url)
        .map(|_| ())
        .map_err(|e| format!("[git]添加远端失败：{e}"))
}

pub fn git_remote_set_url(root: &Path, name: &str, url: &str) -> Result<(), String> {
    let repo = open_repo(root)?;
    let name = name.trim();
    let url = url.trim();
    validate_remote_url(url)?;
    if repo.find_remote(name).is_err() {
        return Err(format!("[git]远端不存在：{name}"));
    }
    repo.remote_set_url(name, url)
        .map_err(|e| format!("[git]更新远端地址失败：{e}"))
}

/// Remove one remote from this repository only: its config section, its
/// `refs/remotes/<name>/*` tracking refs and any `branch.*.remote` /
/// `branch.*.merge` entries that pointed at it go away, exactly like
/// `git remote remove`. Nothing is sent over the network, so the server-side
/// repository and branches are untouched.
pub fn git_remote_remove(root: &Path, name: &str) -> Result<Vec<GitRemoteInfo>, String> {
    let repo = open_repo(root)?;
    let name = name.trim();
    if name.is_empty() {
        return Err("[git_invalid_name]远端名不能为空".to_string());
    }
    if repo.find_remote(name).is_err() {
        return Err(format!("[git]远端不存在：{name}"));
    }
    repo.remote_delete(name)
        .map_err(|e| format!("[git]移除远端失败：{e}"))?;
    git_remotes(root)
}

/// Credentials come from the system git setup only: the configured
/// `credential.helper` (HTTPS) or the running SSH agent. There is no token in
/// this codebase; when neither is available the callback fails loudly.
fn remote_callbacks<'a>() -> RemoteCallbacks<'a> {
    let mut cb = RemoteCallbacks::new();
    cb.credentials(|url, username, allowed| {
        if allowed.contains(CredentialType::USERNAME) {
            return Cred::username(username.unwrap_or("git"));
        }
        if allowed.contains(CredentialType::SSH_KEY) {
            let user = username.unwrap_or("git");
            if let Ok(cred) = Cred::ssh_key_from_agent(user) {
                return Ok(cred);
            }
        }
        if allowed.intersects(CredentialType::USER_PASS_PLAINTEXT | CredentialType::DEFAULT) {
            if let Ok(config) = git2::Config::open_default() {
                if let Ok(cred) = Cred::credential_helper(&config, url, username) {
                    return Ok(cred);
                }
            }
        }
        Err(git2::Error::from_str(
            "无可用 Git 凭据：请配置系统 git 的 credential.helper 或 SSH key（本程序不使用任何硬编码令牌）",
        ))
    });
    cb
}

/// libgit2 localises transport errors; a Chinese Windows build reports
/// "无法与服务器建立连接" with `ErrorClass::None`, so check the class as well as
/// the English keywords before falling back to the generic git code.
fn is_network_failure(msg_lower: &str, class: ErrorClass) -> bool {
    class == ErrorClass::Net
        || msg_lower.contains("could not resolve host")
        || msg_lower.contains("failed to connect")
        || msg_lower.contains("timed out")
        || msg_lower.contains("connection refused")
        || msg_lower.contains("network")
        || msg_lower.contains("无法与服务器建立连接")
        || msg_lower.contains("无法连接")
        || msg_lower.contains("连接失败")
}

fn map_remote_err(e: &git2::Error) -> String {
    let msg = e.message().to_string();
    let lower = msg.to_lowercase();
    match e.code() {
        ErrorCode::Auth => format!("[auth]认证失败：{msg}"),
        ErrorCode::Certificate => format!("[auth]HTTPS 证书校验失败：{msg}"),
        _ if is_network_failure(&lower, e.class()) => format!("[network]无法连接远端：{msg}"),
        _ if lower.contains("authentication") || lower.contains("credentials") => {
            format!("[auth]认证失败：{msg}")
        }
        _ => format!("[git]远端操作失败：{msg}"),
    }
}

/// Fetch the default remote (origin, else the first one).
pub fn git_fetch(root: &Path) -> Result<String, String> {
    let repo = open_repo(root)?;
    let (name, _) =
        pick_remote(&repo).ok_or_else(|| "[git_no_remote]未配置远端：请先添加 origin".to_string())?;
    let mut opts = FetchOptions::new();
    opts.remote_callbacks(remote_callbacks());
    let mut remote = repo
        .find_remote(&name)
        .map_err(|e| format!("[git]远端 {name} 不可用：{e}"))?;
    remote
        .fetch(&[] as &[&str], Some(&mut opts), None)
        .map_err(|e| map_remote_err(&e))?;
    Ok(format!("已从 {name} 获取最新引用"))
}

#[derive(Debug, Serialize)]
pub struct GitPullResult {
    /// "up_to_date" | "fast_forward" | "merged" | "conflicts"
    pub status: String,
    pub message: String,
    pub conflicts: Vec<String>,
    pub ahead: usize,
    pub behind: usize,
}

fn clear_merge_state(repo: &Repository) {
    for f in ["MERGE_HEAD", "MERGE_MSG", "MERGE_MODE"] {
        let _ = std::fs::remove_file(repo.path().join(f));
    }
}

/// Pull = fetch + fast-forward, or a real merge when the branches diverged.
/// Conflicts are left in the index (plus MERGE_HEAD) so the UI can resolve
/// them; a clean merge is committed immediately with the configured identity.
pub fn git_pull(root: &Path) -> Result<GitPullResult, String> {
    let repo = open_repo(root)?;
    let (remote_name, _) =
        pick_remote(&repo).ok_or_else(|| "[git_no_remote]未配置远端：请先添加 origin".to_string())?;
    let branch = head_branch_name(&repo)
        .ok_or_else(|| "[git_detached]分离 HEAD 下无法拉取；请先切回分支".to_string())?;
    let up_short =
        upstream_short(&repo, &branch).unwrap_or_else(|| format!("{remote_name}/{branch}"));

    let mut fetch_opts = FetchOptions::new();
    fetch_opts.remote_callbacks(remote_callbacks());
    let mut remote = repo
        .find_remote(&remote_name)
        .map_err(|e| format!("[git]远端 {remote_name} 不可用：{e}"))?;
    remote
        .fetch(&[] as &[&str], Some(&mut fetch_opts), None)
        .map_err(|e| map_remote_err(&e))?;

    let up_ref_name = format!("refs/remotes/{up_short}");
    let up_ref = repo.find_reference(&up_ref_name).map_err(|_| {
        format!("[no_upstream]远端 {remote_name} 上不存在分支 {branch}；可先推送建立上游")
    })?;
    let up_oid = up_ref
        .peel_to_commit()
        .map_err(|e| format!("[git]读取远端提交失败：{e}"))?
        .id();
    let local_oid = repo
        .head()
        .ok()
        .and_then(|h| h.target())
        .ok_or_else(|| "[git]当前分支还没有提交".to_string())?;

    if local_oid == up_oid {
        return Ok(GitPullResult {
            status: "up_to_date".to_string(),
            message: format!("已是最新（{up_short}）"),
            conflicts: Vec::new(),
            ahead: 0,
            behind: 0,
        });
    }
    let (ahead, behind) = repo
        .graph_ahead_behind(local_oid, up_oid)
        .map_err(|e| format!("[git]计算领先/落后失败：{e}"))?;
    let annotated = repo
        .reference_to_annotated_commit(&up_ref)
        .map_err(|e| format!("[git]读取远端提交失败：{e}"))?;
    let (analysis, _) = repo
        .merge_analysis(&[&annotated])
        .map_err(|e| format!("[git]合并分析失败：{e}"))?;

    if analysis.is_fast_forward() {
        let up_obj = repo
            .find_object(up_oid, Some(ObjectType::Commit))
            .map_err(|e| format!("[git]读取远端提交失败：{e}"))?;
        let mut opts = CheckoutBuilder::new();
        opts.update_index(true).allow_conflicts(false).safe();
        if let Err(e) = repo.checkout_tree(&up_obj, Some(&mut opts)) {
            return Err(format!(
                "[git_dirty]快进被阻止：工作区改动会与目标冲突，未覆盖任何本地改动；请先提交/暂存。原始错误：{e}"
            ));
        }
        repo.reset(&up_obj, ResetType::Soft, None)
            .map_err(|e| format!("[git]更新分支失败：{e}"))?;
        return Ok(GitPullResult {
            status: "fast_forward".to_string(),
            message: format!("已快进到 {up_short}（领先 {ahead} / 落后 {behind}）"),
            conflicts: Vec::new(),
            ahead,
            behind,
        });
    }
    if !analysis.is_normal() {
        return Err(format!(
            "[git]无法合并：本地与 {up_short} 的分叉无法自动处理"
        ));
    }

    // Identity is checked before touching the worktree: a missing identity
    // must not leave a half-done merge behind.
    let (name, email) = resolve_identity(
        None,
        repo_config_identity(&repo).map(|i| (i.name, i.email)),
    )
    .map_err(|e| format!("[git_identity]{e}"))?;
    let sig = Signature::now(&name, &email).map_err(|e| format!("[git]身份无效：{e}"))?;

    repo.merge(&[&annotated], None, None)
        .map_err(|e| format!("[git]合并失败：{e}"))?;
    let mut index = repo.index().map_err(|e| format!("[git]读取索引失败：{e}"))?;
    let conflicts = collect_conflicts(&index);
    if !conflicts.is_empty() {
        std::fs::write(repo.path().join("MERGE_HEAD"), format!("{up_oid}\n"))
            .map_err(|e| format!("[git]写入合并状态失败：{e}"))?;
        return Ok(GitPullResult {
            status: "conflicts".to_string(),
            message: format!(
                "拉取合并产生 {} 处冲突，请在「变更」列表中逐项解决",
                conflicts.len()
            ),
            conflicts,
            ahead,
            behind,
        });
    }

    let tree_oid = index
        .write_tree()
        .map_err(|e| format!("[git]写入合并树失败：{e}"))?;
    index.write().map_err(|e| format!("[git]写索引失败：{e}"))?;
    let tree = repo
        .find_tree(tree_oid)
        .map_err(|e| format!("[git]读取合并树失败：{e}"))?;
    let local_commit = repo
        .find_commit(local_oid)
        .map_err(|e| format!("[git]读取本地提交失败：{e}"))?;
    let up_commit = repo
        .find_commit(up_oid)
        .map_err(|e| format!("[git]读取远端提交失败：{e}"))?;
    repo.commit(
        Some("HEAD"),
        &sig,
        &sig,
        &format!("Merge remote-tracking branch '{up_short}'"),
        &tree,
        &[&local_commit, &up_commit],
    )
    .map_err(|e| format!("[git]提交合并失败：{e}"))?;
    clear_merge_state(&repo);
    Ok(GitPullResult {
        status: "merged".to_string(),
        message: format!("已合并 {up_short}（领先 {ahead} / 落后 {behind}）"),
        conflicts: Vec::new(),
        ahead,
        behind,
    })
}

fn collect_conflicts(index: &git2::Index) -> Vec<String> {
    let mut out: Vec<String> = index
        .conflicts()
        .map(|iter| {
            iter.flatten()
                .filter_map(|c| {
                    c.our
                        .as_ref()
                        .or(c.their.as_ref())
                        .or(c.ancestor.as_ref())
                        .map(|e| String::from_utf8_lossy(&e.path).to_string())
                })
                .collect()
        })
        .unwrap_or_default();
    out.sort();
    out.dedup();
    out
}

/// Push the current branch to its upstream (same-name branch on the remote).
/// `set_upstream=true` is the confirmed "first push": it pushes to the same
/// name and records `branch.<name>.remote/.merge`. No force push exists.
pub fn git_push(root: &Path, set_upstream: bool) -> Result<String, String> {
    let repo = open_repo(root)?;
    let branch = head_branch_name(&repo).ok_or_else(|| {
        "[git_detached]分离 HEAD 或空仓库无法推送；请先切回分支并提交".to_string()
    })?;
    let (remote_name, remote_branch) = if set_upstream {
        (
            pick_remote(&repo)
                .map(|(n, _)| n)
                .ok_or_else(|| "[git_no_remote]未配置远端：请先添加 origin".to_string())?,
            branch.clone(),
        )
    } else {
        let up = upstream_short(&repo, &branch).ok_or_else(|| {
            "[no_upstream]当前分支未配置上游；可确认后首次推送并建立上游".to_string()
        })?;
        match up.split_once('/') {
            Some((r, b)) => (r.to_string(), b.to_string()),
            None => return Err(format!("[no_upstream]无法解析上游名称：{up}")),
        }
    };
    let refspec = format!("refs/heads/{branch}:refs/heads/{remote_branch}");

    let rejection: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let mut callbacks = remote_callbacks();
    let reject = rejection.clone();
    callbacks.push_update_reference(move |refname, reason| {
        if let Some(reason) = reason {
            if let Ok(mut slot) = reject.lock() {
                *slot = Some(format!("{refname}: {reason}"));
            }
            return Err(git2::Error::from_str("push rejected"));
        }
        Ok(())
    });
    let mut opts = PushOptions::new();
    opts.remote_callbacks(callbacks);
    let mut remote = repo
        .find_remote(&remote_name)
        .map_err(|e| format!("[git]远端 {remote_name} 不可用：{e}"))?;
    let refspecs = [refspec.as_str()];
    if let Err(e) = remote.push(&refspecs, Some(&mut opts)) {
        if let Some(reason) = rejection.lock().ok().and_then(|mut r| r.take()) {
            let lower = reason.to_lowercase();
            let code = if lower.contains("fast-forward")
                || lower.contains("fetch first")
                || lower.contains("non-fast-forward")
                || lower.contains("rejected")
                || lower.contains("behind")
            {
                "non_fast_forward"
            } else if lower.contains("permission")
                || lower.contains("denied")
                || lower.contains("read-only")
                || lower.contains("protected")
                || lower.contains("auth")
            {
                "auth"
            } else {
                "git"
            };
            return Err(format!("[{code}]推送被远端拒绝：{reason}"));
        }
        return Err(map_remote_err(&e));
    }
    if set_upstream {
        let mut config = repo
            .config()
            .map_err(|e| format!("[git]读取配置失败：{e}"))?;
        config
            .set_str(&format!("branch.{branch}.remote"), &remote_name)
            .map_err(|e| format!("[git]写入上游配置失败：{e}"))?;
        config
            .set_str(
                &format!("branch.{branch}.merge"),
                &format!("refs/heads/{branch}"),
            )
            .map_err(|e| format!("[git]写入上游配置失败：{e}"))?;
    }
    Ok(format!(
        "已推送 {branch} → {remote_name}/{remote_branch}"
    ))
}

/// Revert a commit as a new commit (`git revert --no-edit`). Refuses when the
/// worktree is dirty; a conflicting revert is rolled back completely.
pub fn git_revert_commit(root: &Path, id: &str) -> Result<String, String> {
    let repo = open_repo(root)?;
    if repo.state() != RepositoryState::Clean {
        return Err("[git_dirty]仓库正处于合并/还原等中间状态，请先完成或中止".to_string());
    }
    if dirty_count(&repo)? > 0 {
        return Err("[git_dirty]工作区有未提交改动；还原提交会修改文件，请先提交或暂存".to_string());
    }
    let oid = git2::Oid::from_str(id).map_err(|_| "[git]无效的提交哈希".to_string())?;
    let commit = repo
        .find_commit(oid)
        .map_err(|e| format!("[git]提交不存在：{e}"))?;
    let (name, email) = resolve_identity(
        None,
        repo_config_identity(&repo).map(|i| (i.name, i.email)),
    )
    .map_err(|e| format!("[git_identity]{e}"))?;
    let sig = Signature::now(&name, &email).map_err(|e| format!("[git]身份无效：{e}"))?;

    repo.revert(&commit, None)
        .map_err(|e| format!("[git]还原失败（合并提交需要指定主线）：{e}"))?;
    let mut index = repo
        .index()
        .map_err(|e| format!("[git]读取索引失败：{e}"))?;
    if index.has_conflicts() {
        if let Some(head_obj) = repo.head().ok().and_then(|h| h.peel(ObjectType::Commit).ok()) {
            let _ = repo.reset(&head_obj, ResetType::Hard, None);
        }
        return Err("[git_conflict]还原产生冲突，已回滚；未改动你的工作区".to_string());
    }
    let tree_oid = index
        .write_tree()
        .map_err(|e| format!("[git]写入还原树失败：{e}"))?;
    index.write().map_err(|e| format!("[git]写索引失败：{e}"))?;
    let tree = repo
        .find_tree(tree_oid)
        .map_err(|e| format!("[git]读取还原树失败：{e}"))?;
    let parent = repo
        .head()
        .map_err(|e| format!("[git]读取 HEAD 失败：{e}"))?
        .peel_to_commit()
        .map_err(|e| format!("[git]读取 HEAD 失败：{e}"))?;
    let message = format!(
        "Revert \"{}\"\n\nThis reverts commit {}.",
        commit.summary().unwrap_or(""),
        commit.id()
    );
    let new_oid = repo
        .commit(Some("HEAD"), &sig, &sig, &message, &tree, &[&parent])
        .map_err(|e| format!("[git]提交还原失败：{e}"))?;
    Ok(repo
        .find_commit(new_oid)
        .ok()
        .and_then(|c| c.as_object().short_id().ok())
        .and_then(|b| b.as_str().map(String::from))
        .unwrap_or_else(|| new_oid.to_string()))
}

/// True while a merge is in progress (MERGE_HEAD present).
pub fn git_merge_state(root: &Path) -> Result<bool, String> {
    let repo = open_repo(root)?;
    Ok(repo.state() == RepositoryState::Merge)
}

/// Finish a conflicted merge: index → tree, commit with HEAD + MERGE_HEAD as
/// parents. Refuses while any conflict stage remains.
pub fn git_merge_commit(root: &Path, message: &str) -> Result<String, String> {
    let repo = open_repo(root)?;
    if repo.state() != RepositoryState::Merge {
        return Err("[git]当前不在合并状态，没有可提交的合并".to_string());
    }
    let message = message.trim();
    if message.is_empty() {
        return Err("[git]提交信息不能为空".to_string());
    }
    let mut index = repo
        .index()
        .map_err(|e| format!("[git]读取索引失败：{e}"))?;
    if index.has_conflicts() {
        return Err(format!(
            "[git_conflict]仍有 {} 处冲突未解决，不能提交合并",
            collect_conflicts(&index).len()
        ));
    }
    let tree_oid = index
        .write_tree()
        .map_err(|e| format!("[git]写入合并树失败：{e}"))?;
    index.write().map_err(|e| format!("[git]写索引失败：{e}"))?;
    let tree = repo
        .find_tree(tree_oid)
        .map_err(|e| format!("[git]读取合并树失败：{e}"))?;
    let head_commit = repo
        .head()
        .map_err(|e| format!("[git]读取 HEAD 失败：{e}"))?
        .peel_to_commit()
        .map_err(|e| format!("[git]读取 HEAD 失败：{e}"))?;
    let merge_head = std::fs::read_to_string(repo.path().join("MERGE_HEAD"))
        .map_err(|_| "[git]缺少 MERGE_HEAD，合并状态已丢失".to_string())?;
    let merge_oid = git2::Oid::from_str(merge_head.trim())
        .map_err(|e| format!("[git]MERGE_HEAD 无效：{e}"))?;
    let merge_commit = repo
        .find_commit(merge_oid)
        .map_err(|e| format!("[git]读取被合并提交失败：{e}"))?;
    let (name, email) = resolve_identity(
        None,
        repo_config_identity(&repo).map(|i| (i.name, i.email)),
    )
    .map_err(|e| format!("[git_identity]{e}"))?;
    let sig = Signature::now(&name, &email).map_err(|e| format!("[git]身份无效：{e}"))?;
    let oid = repo
        .commit(
            Some("HEAD"),
            &sig,
            &sig,
            message,
            &tree,
            &[&head_commit, &merge_commit],
        )
        .map_err(|e| format!("[git]提交合并失败：{e}"))?;
    clear_merge_state(&repo);
    Ok(repo
        .find_commit(oid)
        .ok()
        .and_then(|c| c.as_object().short_id().ok())
        .and_then(|b| b.as_str().map(String::from))
        .unwrap_or_else(|| oid.to_string()))
}

/// Abort a merge: hard-reset to HEAD and drop the merge state files.
pub fn git_merge_abort(root: &Path) -> Result<(), String> {
    let repo = open_repo(root)?;
    if repo.state() != RepositoryState::Merge {
        return Err("[git]当前不在合并状态".to_string());
    }
    let head_obj = repo
        .head()
        .map_err(|e| format!("[git]读取 HEAD 失败：{e}"))?
        .peel(ObjectType::Commit)
        .map_err(|e| format!("[git]读取 HEAD 失败：{e}"))?;
    repo.reset(&head_obj, ResetType::Hard, None)
        .map_err(|e| format!("[git]中止合并失败：{e}"))?;
    clear_merge_state(&repo);
    Ok(())
}

#[derive(Debug, Serialize)]
pub struct GitConflictInfo {
    pub path: String,
    pub ours: bool,
    pub theirs: bool,
    pub base: bool,
}

/// Unmerged index entries (three stages) — the row set the UI offers
/// "用本地 / 用远端" for.
pub fn git_conflicts(root: &Path) -> Result<Vec<GitConflictInfo>, String> {
    let repo = open_repo(root)?;
    let index = repo.index().map_err(|e| format!("[git]读取索引失败：{e}"))?;
    let mut out = Vec::new();
    for conflict in index
        .conflicts()
        .map_err(|e| format!("[git]读取冲突失败：{e}"))?
    {
        let conflict = conflict.map_err(|e| format!("[git]读取冲突失败：{e}"))?;
        let path = conflict
            .our
            .as_ref()
            .or(conflict.their.as_ref())
            .or(conflict.ancestor.as_ref())
            .map(|e| String::from_utf8_lossy(&e.path).to_string())
            .unwrap_or_default();
        if path.is_empty() {
            continue;
        }
        out.push(GitConflictInfo {
            path,
            ours: conflict.our.is_some(),
            theirs: conflict.their.is_some(),
            base: conflict.ancestor.is_some(),
        });
    }
    Ok(out)
}

/// Resolve one conflict by taking a side: `ours` (用本地) or `theirs`
/// (用远端). Writes that blob to the worktree and re-adds the path, which
/// collapses the three index stages into stage 0.
pub fn git_resolve_conflict(root: &Path, rel: &str, side: &str) -> Result<(), String> {
    if side != "ours" && side != "theirs" {
        return Err("[git]冲突处理方式只能是 ours 或 theirs".to_string());
    }
    let repo = open_repo(root)?;
    let path = safe_rel(rel)?;
    let mut index = repo.index().map_err(|e| format!("[git]读取索引失败：{e}"))?;
    let mut chosen: Option<git2::Oid> = None;
    for conflict in index
        .conflicts()
        .map_err(|e| format!("[git]读取冲突失败：{e}"))?
    {
        let conflict = conflict.map_err(|e| format!("[git]读取冲突失败：{e}"))?;
        let current = conflict
            .our
            .as_ref()
            .or(conflict.their.as_ref())
            .or(conflict.ancestor.as_ref())
            .map(|e| String::from_utf8_lossy(&e.path).to_string())
            .unwrap_or_default();
        if current != rel {
            continue;
        }
        let entry = if side == "ours" {
            conflict.our.as_ref()
        } else {
            conflict.their.as_ref()
        };
        chosen = entry.map(|e| e.id);
        break;
    }
    let oid = chosen.ok_or_else(|| {
        format!("[git_conflict]{rel} 不是冲突文件，或所选一侧不存在")
    })?;
    let blob = repo
        .find_blob(oid)
        .map_err(|e| format!("[git]读取所选版本失败：{e}"))?;
    let full = workdir(&repo)?.join(path);
    if let Some(dir) = full.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("[git]创建目录失败：{e}"))?;
    }
    std::fs::write(&full, blob.content()).map_err(|e| format!("[git]写入冲突文件失败：{e}"))?;
    index
        .add_path(path)
        .map_err(|e| format!("[git]标记冲突已解决失败：{e}"))?;
    index.write().map_err(|e| format!("[git]写索引失败：{e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn remote_error_classifies_network_messages() {
        assert!(is_network_failure(
            "failed to send request: 无法与服务器建立连接",
            ErrorClass::None
        ));
        assert!(is_network_failure("failed to connect to 127.0.0.1", ErrorClass::Invalid));
        assert!(is_network_failure("could not resolve host", ErrorClass::Invalid));
        assert!(is_network_failure("opaque transport failure", ErrorClass::Net));
        assert!(!is_network_failure("authentication failed", ErrorClass::Invalid));
        assert!(!is_network_failure("远端操作失败", ErrorClass::Invalid));
    }

    #[test]
    fn status_label_maps_flag_combinations() {
        assert_eq!(status_label(Status::empty()), None);
        assert_eq!(status_label(Status::IGNORED), Some("已忽略".into()));
        assert_eq!(status_label(Status::WT_NEW), Some("未跟踪".into()));
        assert_eq!(
            status_label(Status::INDEX_NEW),
            Some("已暂存（新增）".into())
        );
        assert_eq!(
            status_label(Status::INDEX_MODIFIED),
            Some("已暂存（修改）".into())
        );
        assert_eq!(
            status_label(Status::WT_MODIFIED),
            Some("未暂存修改".into())
        );
        let both = Status::INDEX_MODIFIED | Status::WT_MODIFIED;
        assert_eq!(
            status_label(both),
            Some("已暂存（修改），另有未暂存修改".into())
        );
    }

    #[test]
    fn init_probe_commit_roundtrip() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        let file = dir.join("a.log");
        fs::write(&file, "hello\n").unwrap();

        // no repo → repo=false, no "unavailable" noise
        let info = git_probe(dir, &file);
        assert!(!info.repo && info.unavailable.is_none());

        git_init(dir).unwrap();
        let info = git_probe(dir, &file);
        assert!(info.repo);
        assert!(!info.branch.as_deref().unwrap_or("").is_empty());
        assert_eq!(info.file_status.as_deref(), Some("未跟踪"));
        assert!(info.last_commit.is_none());

        // stage + commit with an explicit identity (config-independent)
        let hash = git_stage_and_commit(&file, "首提交", Some("测试者"), Some("t@t")).unwrap();
        assert!(!hash.is_empty());
        let info = git_probe(dir, &file);
        assert_eq!(info.file_status, None);
        let lc = info.last_commit.unwrap();
        assert_eq!(lc.subject, "首提交");
        assert_eq!(lc.hash, hash);
    }

    #[cfg(unix)]
    #[test]
    fn probe_handles_symlinked_workdir_path() {
        use std::os::unix::fs::symlink;
        let tmp = tempfile::tempdir().unwrap();
        let real = tmp.path().join("repo");
        fs::create_dir(&real).unwrap();
        let link = tmp.path().join("link");
        symlink(&real, &link).unwrap();
        let file = link.join("a.log");
        fs::write(&file, "hello\n").unwrap();
        git_init(&link).unwrap();
        // libgit2 reports `repo` (canonical) as the workdir while the caller
        // passes paths through `link` — status must still be found.
        let info = git_probe(&link, &file);
        assert_eq!(info.file_status.as_deref(), Some("未跟踪"));
    }

    #[test]
    fn identity_resolution_prefers_explicit_then_config_then_error() {
        assert!(resolve_identity(Some(("a", "b")), None).is_ok());
        assert_eq!(
            resolve_identity(None, Some(("c".into(), "d".into()))).unwrap(),
            ("c".to_string(), "d".to_string())
        );
        assert!(resolve_identity(None, None).is_err());
    }

    #[test]
    fn status_log_diff_and_commit_all() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        fs::write(dir.join("a.log"), "one\n").unwrap();
        git_init(dir).unwrap();

        // Untracked file shows in the whole-repo list, empty repo has no log.
        let st = git_status(dir).unwrap();
        assert_eq!(st.len(), 1);
        assert_eq!(st[0].kind, "untracked");
        assert!(!st[0].staged);
        assert!(git_log(dir, 10).unwrap().is_empty());

        let hash = git_stage_all_and_commit(dir, "首提交", Some("测试者"), Some("t@t")).unwrap();
        assert_eq!(git_status(dir).unwrap().len(), 0);
        let log = git_log(dir, 10).unwrap();
        assert_eq!(log.len(), 1);
        assert_eq!(log[0].summary, "首提交");
        assert_eq!(log[0].id_short, hash);
        assert_eq!(
            git_commit_files(dir, &log[0].id).unwrap()[0].kind,
            "A"
        );

        // Modify → status kind + real worktree diff.
        fs::write(dir.join("a.log"), "one\ntwo\n").unwrap();
        let st = git_status(dir).unwrap();
        assert_eq!(st[0].kind, "modified");
        assert!(!st[0].staged);
        let diff = git_diff_file(dir, "a.log", "HEAD").unwrap();
        assert!(diff.contains("+two"), "diff was: {diff}");

        // Commit-all again; a no-change commit is rejected.
        git_stage_all_and_commit(dir, "第二提交", Some("测试者"), Some("t@t")).unwrap();
        assert!(git_stage_all_and_commit(dir, "空提交", Some("测试者"), Some("t@t")).is_err());
    }

    fn set_identity(repo: &Repository, name: &str) {
        let mut config = repo.config().unwrap();
        config.set_str("user.name", name).unwrap();
        config.set_str("user.email", "t@t").unwrap();
    }

    fn head_id(repo: &Repository) -> String {
        repo.head().unwrap().target().unwrap().to_string()
    }

    /// libgit2 checkout applies the platform's EOL filter (CRLF on Windows);
    /// compare content with one normalized line ending.
    fn read_text(path: &Path) -> String {
        fs::read_to_string(path).unwrap().replace("\r\n", "\n")
    }

    #[test]
    fn stage_unstage_discard_roundtrip() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        fs::write(dir.join("a.log"), "one\n").unwrap();
        git_init(dir).unwrap();
        git_stage_all_and_commit(dir, "首提交", Some("测试者"), Some("t@t")).unwrap();

        // Modify → stage → unstage.
        fs::write(dir.join("a.log"), "one\ntwo\n").unwrap();
        git_stage_path(dir, "a.log").unwrap();
        let st = git_status(dir).unwrap();
        assert_eq!(st[0].kind, "modified");
        assert!(st[0].staged);
        git_unstage_path(dir, "a.log").unwrap();
        let st = git_status(dir).unwrap();
        assert!(!st[0].staged);

        // Discard without confirmation is rejected; confirmed restores HEAD.
        assert!(git_discard_path(dir, "a.log", false).is_err());
        git_discard_path(dir, "a.log", true).unwrap();
        assert_eq!(read_text(&dir.join("a.log")), "one\n");
        assert!(git_status(dir).unwrap().is_empty());

        // Staged-new file: discard removes the file and its index entry.
        fs::write(dir.join("b.log"), "new\n").unwrap();
        git_stage_path(dir, "b.log").unwrap();
        assert_eq!(git_status(dir).unwrap().len(), 1);
        git_discard_path(dir, "b.log", true).unwrap();
        assert!(!dir.join("b.log").exists());
        assert!(git_status(dir).unwrap().is_empty());

        // Path traversal is refused.
        assert!(git_stage_path(dir, "../escape.log").is_err());
    }

    #[test]
    fn branches_checkout_safe_and_force() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        fs::write(dir.join("a.log"), "one\n").unwrap();
        git_init(dir).unwrap();
        git_stage_all_and_commit(dir, "首提交", Some("测试者"), Some("t@t")).unwrap();
        let repo = Repository::open(dir).unwrap();
        let main = head_branch_name(&repo).unwrap();
        let head = repo.head().unwrap().peel_to_commit().unwrap();
        repo.branch("feature", &head, false).unwrap();

        let branches = git_branches(dir).unwrap();
        assert_eq!(branches.len(), 2);
        assert!(branches
            .iter()
            .any(|b| b.name == main && b.current && !b.remote));
        assert!(branches
            .iter()
            .any(|b| b.name == "feature" && !b.current));

        assert_eq!(git_checkout_branch(dir, "feature", false).unwrap(), "feature");
        // Diverge feature so the switch back really touches a.log.
        fs::write(dir.join("a.log"), "two\n").unwrap();
        git_stage_all_and_commit(dir, "feature 提交", Some("测试者"), Some("t@t")).unwrap();

        // Conflicting local edit: safe checkout refuses and leaves it intact.
        fs::write(dir.join("a.log"), "dirty\n").unwrap();
        let err = git_checkout_branch(dir, &main, false).unwrap_err();
        assert!(err.starts_with("[git_dirty]"), "err was: {err}");
        assert_eq!(read_text(&dir.join("a.log")), "dirty\n");

        // Force checkout (UI-confirmed) replaces the edit with HEAD content.
        git_checkout_branch(dir, &main, true).unwrap();
        assert_eq!(read_text(&dir.join("a.log")), "one\n");
        assert!(git_status(dir).unwrap().is_empty());
    }

    #[test]
    fn branch_create_validates_names_and_applies_safe_checkout() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        fs::write(dir.join("a.log"), "one\n").unwrap();
        git_init(dir).unwrap();
        let repo = Repository::open(dir).unwrap();
        set_identity(&repo, "测试者");
        git_stage_all_and_commit(dir, "第一", None, None).unwrap();
        let main = head_branch_name(&repo).unwrap();
        let first = head_id(&repo);
        fs::write(dir.join("a.log"), "one\ntwo\n").unwrap();
        git_stage_all_and_commit(dir, "第二", None, None).unwrap();
        let second = head_id(&repo);

        // Ref rules are enforced before touching the ref store.
        for bad in ["", "has space", "a..b", "a//b", "a.lock", "a~b", "a^b", "a:b", "a?b", "a*b", "a[b", "a\\b", "@", "a.", "/a", "a/"] {
            let err = git_branch_create(dir, bad, None, false).unwrap_err();
            assert!(err.starts_with("[git_invalid_name]"), "name {bad:?}: {err}");
        }
        assert!(git_branch_create(dir, &main, None, false)
            .unwrap_err()
            .starts_with("[git_exists]"));

        // Create without switching: ref exists, HEAD unchanged.
        let msg = git_branch_create(dir, "topic", None, false).unwrap();
        assert!(msg.contains("已创建分支 topic"), "msg was {msg}");
        assert!(repo.find_branch("topic", BranchType::Local).is_ok());
        assert_eq!(head_id(&repo), second);
        assert_eq!(head_branch_name(&repo).as_deref(), Some(main.as_str()));

        // Create + switch at an older commit: HEAD and worktree move to it.
        let msg = git_branch_create(dir, "old", Some(&first), true).unwrap();
        assert!(msg.contains("已创建并切换到分支 old"), "msg was {msg}");
        assert_eq!(head_branch_name(&repo).as_deref(), Some("old"));
        assert_eq!(head_id(&repo), first);
        assert_eq!(read_text(&dir.join("a.log")), "one\n");

        // Unknown start point is reported, not silently ignored.
        assert!(git_branch_create(dir, "nope", Some("deadbeef"), false)
            .unwrap_err()
            .starts_with("[git]"));

        // A switch that would overwrite local edits deletes the new branch and
        // leaves the edit intact.
        fs::write(dir.join("a.log"), "dirty\n").unwrap();
        let err = git_branch_create(dir, "blocked", Some(&second), true).unwrap_err();
        assert!(err.starts_with("[git_dirty]"), "err was {err}");
        assert_eq!(read_text(&dir.join("a.log")), "dirty\n");
        assert!(repo.find_branch("blocked", BranchType::Local).is_err());
        assert_eq!(head_branch_name(&repo).as_deref(), Some("old"));
    }

    #[test]
    fn remote_add_edit_and_validation() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        fs::write(dir.join("a.log"), "one\n").unwrap();
        git_init(dir).unwrap();
        assert!(git_remotes(dir).unwrap().is_empty());

        for bad_name in ["", "-bad", ".bad", "a/b", "a b", "a..b", "a:b"] {
            assert!(
                git_remote_add(dir, bad_name, "https://example.com/r.git")
                    .unwrap_err()
                    .starts_with("[git_invalid_name]"),
                "name {bad_name:?}"
            );
        }
        for bad_url in ["", "example.com/repo", "ftp:/nope", "https://", "git@host", "has space"] {
            assert!(
                git_remote_add(dir, "origin", bad_url)
                    .unwrap_err()
                    .starts_with("[git_invalid_url]"),
                "url {bad_url:?}"
            );
        }

        git_remote_add(dir, "origin", "https://example.com/a.git").unwrap();
        let remotes = git_remotes(dir).unwrap();
        assert_eq!(remotes.len(), 1);
        assert_eq!(remotes[0].name, "origin");
        assert_eq!(remotes[0].url, "https://example.com/a.git");
        assert!(git_remote_add(dir, "origin", "https://example.com/b.git")
            .unwrap_err()
            .starts_with("[git_exists]"));

        git_remote_set_url(dir, "origin", "git@github.com:me/b.git").unwrap();
        assert_eq!(
            git_remotes(dir).unwrap()[0].url,
            "git@github.com:me/b.git"
        );
        assert!(git_remote_set_url(dir, "missing", "https://example.com/c.git")
            .unwrap_err()
            .starts_with("[git]"));
        assert!(git_remote_status(dir).unwrap().has_remote);
    }

    #[test]
    fn remote_remove_reports_remaining_and_rejects_unknown() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        fs::write(dir.join("a.log"), "one\n").unwrap();
        git_init(dir).unwrap();
        git_remote_add(dir, "origin", "https://example.com/a.git").unwrap();
        git_remote_add(dir, "backup", "git@github.com:me/b.git").unwrap();

        let left = git_remote_remove(dir, "origin").unwrap();
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].name, "backup");

        // Unknown (and already removed) names fail loudly, not silently.
        assert!(git_remote_remove(dir, "origin")
            .unwrap_err()
            .starts_with("[git]远端不存在"));
        assert!(git_remote_remove(dir, "  ")
            .unwrap_err()
            .starts_with("[git_invalid_name]"));

        // Removing the last remote leaves the repo with no remotes at all.
        let left = git_remote_remove(dir, "backup").unwrap();
        assert!(left.is_empty());
        assert!(git_remotes(dir).unwrap().is_empty());
        let status = git_remote_status(dir).unwrap();
        assert!(!status.has_remote);
        assert!(status.remote.is_none());
        assert!(status.url.is_none());
    }

    #[test]
    fn checkout_commit_detached_and_revert() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        let file = dir.join("a.log");
        fs::write(&file, "one\n").unwrap();
        git_init(dir).unwrap();
        let repo = Repository::open(dir).unwrap();
        set_identity(&repo, "测试者");

        git_stage_all_and_commit(dir, "第一", Some("测试者"), Some("t@t")).unwrap();
        // Unborn HEAD has no branch name; read it after the first commit.
        let branch = head_branch_name(&repo).unwrap();
        let first = head_id(&repo);
        fs::write(&file, "one\ntwo\n").unwrap();
        git_stage_all_and_commit(dir, "第二", Some("测试者"), Some("t@t")).unwrap();
        let second = head_id(&repo);

        // Detached checkout + explicit warning in the message.
        let msg = git_checkout_commit(dir, &first, false).unwrap();
        assert!(msg.contains("分离 HEAD"), "msg was: {msg}");
        assert_eq!(
            git_probe(dir, &file).branch.as_deref(),
            Some("（分离 HEAD）")
        );

        // Back to the branch (force is fine: worktree matches the commit).
        git_checkout_branch(dir, &branch, true).unwrap();
        assert_eq!(head_id(&repo), second);

        // Revert the second commit → worktree equals the first commit.
        let hash = git_revert_commit(dir, &second).unwrap();
        assert!(!hash.is_empty());
        assert_eq!(read_text(&file), "one\n");
        assert!(git_status(dir).unwrap().is_empty());
        let log = git_log(dir, 10).unwrap();
        assert_eq!(log.len(), 3);
        assert!(log[0].summary.starts_with("Revert \"第二\""));

        // A revert never runs on a dirty worktree.
        fs::write(&file, "dirty\n").unwrap();
        let err = git_revert_commit(dir, &first).unwrap_err();
        assert!(err.starts_with("[git_dirty]"), "err was: {err}");
    }

    #[test]
    fn fetch_push_and_remote_status() {
        let tmp = tempfile::tempdir().unwrap();
        let bare = tmp.path().join("origin.git");
        Repository::init_bare(&bare).unwrap();
        let url = bare.to_str().unwrap().to_string();

        let work = tmp.path().join("work");
        fs::create_dir(&work).unwrap();
        fs::write(work.join("a.log"), "one\n").unwrap();
        git_init(&work).unwrap();
        let repo = Repository::open(&work).unwrap();
        set_identity(&repo, "测试者");
        {
            let mut config = repo.config().unwrap();
            config.set_str("remote.origin.url", &url).unwrap();
            config
                .set_str(
                    "remote.origin.fetch",
                    "+refs/heads/*:refs/remotes/origin/*",
                )
                .unwrap();
        }
        git_stage_all_and_commit(&work, "首提交", Some("测试者"), Some("t@t")).unwrap();
        let branch = head_branch_name(&repo).unwrap();

        // First push (confirmed) records the upstream.
        git_push(&work, true).unwrap();
        git_fetch(&work).unwrap();
        let rs = git_remote_status(&work).unwrap();
        assert!(rs.has_remote && rs.has_upstream, "rs = {rs:?}");
        assert_eq!(rs.upstream.as_deref(), Some(format!("origin/{branch}").as_str()));
        assert_eq!((rs.ahead, rs.behind), (0, 0));

        // New local commit → ahead 1 (against the fetched tracking ref).
        fs::write(work.join("b.log"), "b\n").unwrap();
        git_stage_all_and_commit(&work, "本地提交", Some("测试者"), Some("t@t")).unwrap();
        let rs = git_remote_status(&work).unwrap();
        assert_eq!(rs.ahead, 1);
        assert_eq!(rs.behind, 0);

        // A second clone pushes a different file → local is also behind.
        let work2 = tmp.path().join("work2");
        let repo2 = Repository::clone(&url, &work2).unwrap();
        set_identity(&repo2, "协作者");
        fs::write(work2.join("c.log"), "c\n").unwrap();
        git_stage_all_and_commit(&work2, "远端提交", None, None).unwrap();
        git_push(&work2, false).unwrap();

        git_fetch(&work).unwrap();
        let rs = git_remote_status(&work).unwrap();
        assert_eq!((rs.ahead, rs.behind), (1, 1));

        // Diverged but non-conflicting (different files) → auto merge commit.
        let pulled = git_pull(&work).unwrap();
        assert_eq!(pulled.status, "merged");
        assert!(work.join("c.log").exists());
        let log = git_log(&work, 10).unwrap();
        assert_eq!(log[0].parents.len(), 2);
        assert!(!git_merge_state(&work).unwrap());
    }

    #[test]
    fn conflicts_can_take_a_side_or_abort() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        let file = dir.join("a.log");
        fs::write(&file, "base\n").unwrap();
        git_init(dir).unwrap();
        let repo = Repository::open(dir).unwrap();
        set_identity(&repo, "测试者");
        git_stage_all_and_commit(dir, "base", None, None).unwrap();
        let main = head_branch_name(&repo).unwrap();
        let base = repo.head().unwrap().peel_to_commit().unwrap();
        repo.branch("side", &base, false).unwrap();

        // main: "main\n"; side: "side\n" — a real content conflict.
        fs::write(&file, "main\n").unwrap();
        git_stage_all_and_commit(dir, "main", None, None).unwrap();
        git_checkout_branch(dir, "side", true).unwrap();
        fs::write(&file, "side\n").unwrap();
        git_stage_all_and_commit(dir, "side", None, None).unwrap();
        git_checkout_branch(dir, &main, true).unwrap();

        let merge = |repo: &Repository| {
            let side = repo.find_reference("refs/heads/side").unwrap();
            let annotated = repo.reference_to_annotated_commit(&side).unwrap();
            repo.merge(&[&annotated], None, None).unwrap();
            std::fs::write(
                repo.path().join("MERGE_HEAD"),
                format!("{}\n", side.target().unwrap()),
            )
            .unwrap();
        };

        // First merge → read conflicts, then abort back to a clean HEAD.
        merge(&repo);
        let conflicts = git_conflicts(dir).unwrap();
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].path, "a.log");
        assert!(conflicts[0].ours && conflicts[0].theirs);
        assert!(git_merge_state(dir).unwrap());
        git_merge_abort(dir).unwrap();
        assert!(!git_merge_state(dir).unwrap());
        assert_eq!(read_text(&file), "main\n");
        assert!(git_conflicts(dir).unwrap().is_empty());

        // Second merge → commit refuses while unresolved, then resolves via
        // "theirs" (用远端) and commits with two parents.
        merge(&repo);
        let err = git_merge_commit(dir, "merge side").unwrap_err();
        assert!(err.starts_with("[git_conflict]"), "err was: {err}");
        git_resolve_conflict(dir, "a.log", "theirs").unwrap();
        assert_eq!(read_text(&file), "side\n");
        assert!(git_conflicts(dir).unwrap().is_empty());
        let hash = git_merge_commit(dir, "merge side").unwrap();
        assert!(!hash.is_empty());
        assert!(!git_merge_state(dir).unwrap());
        assert_eq!(git_log(dir, 10).unwrap()[0].parents.len(), 2);
        assert!(git_status(dir).unwrap().is_empty());
    }
}
