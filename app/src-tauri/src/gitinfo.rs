//! Inspector Git section (Plans 34/35): repo probe, init, and stage+commit via
//! built-in libgit2 (`git2`) — no PATH dependency, no shell. Read-only besides
//! init/commit; the commit covers the whole index (same as the git CLI).

use git2::{ErrorCode, Repository, Signature, Status};
use serde::Serialize;
use std::path::Path;

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
        Ok(h) => h.shorthand().map(String::from),
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
    let file_status = repo
        .workdir()
        .and_then(|wd| file.strip_prefix(wd).ok())
        .and_then(|rel| repo.status_file(rel).ok())
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
    let rel = repo.workdir().and_then(|wd| file.strip_prefix(wd).ok())?;
    let mut rev = repo.revwalk().ok()?;
    rev.push_head().ok()?;
    let mut previous: Option<git2::Oid> = None;
    for oid in rev.flatten() {
        let commit = repo.find_commit(oid).ok()?;
        let tree = commit.tree().ok()?;
        let entry = tree.get_path(rel).ok().map(|e| e.id());
        if entry != previous {
            if let (Some(cur), Some(parent)) = (&entry, commit.parent(0).ok()) {
                let had = parent.tree().ok()?.get_path(rel).ok().map(|e| e.id());
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
    let rel = repo
        .workdir()
        .and_then(|wd| file.strip_prefix(wd).ok())
        .ok_or_else(|| "文件不在仓库工作区内".to_string())?;
    let mut index = repo.index().map_err(|e| e.to_string())?;
    index.add_path(rel).map_err(|e| e.to_string())?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

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
}
