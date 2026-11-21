//! Stab instruction-document policy checks over the staged tree.
//!
//! Paths, file kinds, and symlink targets all come from the index the commit will record, so an untracked file, a file removed with `git rm --cached`, or an ignored path that was force-added is judged by what is staged rather than by what is on disk.
//! Submodule pointers carry no tree of their own here, so instruction documents inside submodules are never scanned.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use thiserror::Error;

use crate::staged_tree::{EntryKind, ResolveError, StagedTree, StagedTreeError};

const PREFIX: &str = "stab-instruction-docs";
const AGENTS_FILE: &str = "AGENTS.md";
const CLAUDE_FILE: &str = "CLAUDE.md";
const README_FILE: &str = "README.md";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DocCheckReport {
    readme_files_seen: usize,
    effective_agent_sources: Vec<EffectiveAgentSource>,
    claude_files_seen: usize,
    valid_claude_links: Vec<ClaudeLink>,
    violations: Vec<DocViolation>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct EffectiveAgentSource {
    path: PathBuf,
    canonical_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ClaudeLink {
    path: PathBuf,
    target: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AgentEntry {
    path: PathBuf,
    canonical_path: PathBuf,
    symlink_target: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DocViolation {
    MissingColocatedAgents { path: PathBuf },
    ReadmeBrokenSymlink { path: PathBuf, error: String },
    AgentsSymlinkTargetIsNotReadme { path: PathBuf, target: PathBuf },
    AgentsBrokenSymlink { path: PathBuf, error: String },
    MissingClaudeSymlink { path: PathBuf },
    ClaudeIsNotSymlink { path: PathBuf },
    ClaudeBrokenSymlink { path: PathBuf, error: String },
    ClaudeTargetIsNotAgentSource { path: PathBuf, target: PathBuf },
}

#[derive(Debug, Error)]
pub(crate) enum DocCheckError {
    #[error(transparent)]
    Tree(#[from] StagedTreeError),

    #[error("scan cancelled")]
    Cancelled,
}

/// Resolves `path` through staged symlinks, turning a broken chain into its message.
fn resolve(tree: &StagedTree, path: &Path) -> Result<Result<PathBuf, String>, DocCheckError> {
    match tree.resolve(path) {
        Ok(target) => Ok(Ok(target)),
        Err(ResolveError::Broken(reason)) => Ok(Err(reason.to_string())),
        Err(ResolveError::Tree(error)) => Err(error.into()),
    }
}

pub(crate) fn scan_instruction_docs(
    tree: &StagedTree,
    cancel: Option<&AtomicBool>,
) -> Result<DocCheckReport, DocCheckError> {
    let mut readme_directories = BTreeSet::<PathBuf>::new();
    let mut readmes = Vec::<PathBuf>::new();
    let mut readme_canonicals = BTreeSet::<PathBuf>::new();
    let mut agents_by_directory = BTreeMap::<PathBuf, AgentEntry>::new();
    let mut claude_targets = BTreeMap::<PathBuf, BTreeSet<PathBuf>>::new();
    let mut claude_files_seen = 0usize;
    let mut violations = Vec::new();

    for (path, entry) in tree.entries() {
        ensure_not_cancelled(cancel)?;
        if entry.kind == EntryKind::Gitlink {
            continue;
        }
        let Some(file_name) = path.file_name() else {
            continue;
        };
        if !is_instruction_doc_name(file_name) {
            continue;
        }
        let relative = path.to_path_buf();
        let directory = relative
            .parent()
            .map_or_else(PathBuf::new, Path::to_path_buf);
        let is_symlink = entry.kind == EntryKind::Symlink;

        if file_name == OsStr::new(README_FILE) {
            readme_directories.insert(directory);
            readmes.push(relative.clone());
            match resolve(tree, path)? {
                Ok(canonical) => {
                    readme_canonicals.insert(canonical);
                }
                Err(error) => violations.push(DocViolation::ReadmeBrokenSymlink {
                    path: relative,
                    error,
                }),
            }
            continue;
        }

        if file_name == OsStr::new(AGENTS_FILE) {
            let canonical = match resolve(tree, path)? {
                Ok(canonical) => canonical,
                Err(error) => {
                    violations.push(DocViolation::AgentsBrokenSymlink {
                        path: relative,
                        error,
                    });
                    continue;
                }
            };
            let symlink_target = is_symlink.then(|| canonical.clone());
            agents_by_directory.insert(
                directory,
                AgentEntry {
                    path: relative,
                    canonical_path: canonical,
                    symlink_target,
                },
            );
            continue;
        }

        claude_files_seen = claude_files_seen.saturating_add(1);
        if !is_symlink {
            violations.push(DocViolation::ClaudeIsNotSymlink { path: relative });
            continue;
        }
        match resolve(tree, path)? {
            Ok(target) => {
                claude_targets.entry(target).or_default().insert(relative);
            }
            Err(error) => violations.push(DocViolation::ClaudeBrokenSymlink {
                path: relative,
                error,
            }),
        }
    }

    for readme in &readmes {
        let directory = readme.parent().map_or_else(PathBuf::new, Path::to_path_buf);
        if !agents_by_directory.contains_key(&directory) {
            violations.push(DocViolation::MissingColocatedAgents {
                path: readme.clone(),
            });
        }
    }

    let mut source_by_canonical = BTreeMap::<PathBuf, PathBuf>::new();
    for agent in agents_by_directory.values() {
        if let Some(target) = &agent.symlink_target
            && !readme_canonicals.contains(&agent.canonical_path)
        {
            violations.push(DocViolation::AgentsSymlinkTargetIsNotReadme {
                path: agent.path.clone(),
                target: target.clone(),
            });
            continue;
        }
        source_by_canonical
            .entry(agent.canonical_path.clone())
            .or_insert_with(|| agent.path.clone());
    }

    for (target, paths) in &claude_targets {
        if !source_by_canonical.contains_key(target) {
            for path in paths {
                violations.push(DocViolation::ClaudeTargetIsNotAgentSource {
                    path: path.clone(),
                    target: target.clone(),
                });
            }
        }
    }

    for (canonical_path, display_path) in &source_by_canonical {
        if !claude_targets.contains_key(canonical_path) {
            violations.push(DocViolation::MissingClaudeSymlink {
                path: display_path.clone(),
            });
        }
    }

    let mut effective_agent_sources = source_by_canonical
        .into_iter()
        .map(|(canonical_path, path)| EffectiveAgentSource {
            path,
            canonical_path,
        })
        .collect::<Vec<_>>();
    effective_agent_sources.sort_by(|left, right| left.path.cmp(&right.path));

    let mut valid_claude_links = Vec::new();
    for source in &effective_agent_sources {
        if let Some(paths) = claude_targets.get(&source.canonical_path) {
            for path in paths {
                valid_claude_links.push(ClaudeLink {
                    path: path.clone(),
                    target: source.path.clone(),
                });
            }
        }
    }
    valid_claude_links.sort_by(|left, right| {
        left.target
            .cmp(&right.target)
            .then_with(|| left.path.cmp(&right.path))
    });
    sort_violations(&mut violations);

    Ok(DocCheckReport {
        readme_files_seen: readme_directories.len(),
        effective_agent_sources,
        claude_files_seen,
        valid_claude_links,
        violations,
    })
}

pub(crate) fn report_passed(report: &DocCheckReport) -> bool {
    report.violations.is_empty()
}

pub(crate) fn render_report(report: &DocCheckReport) -> Vec<String> {
    if report.violations.is_empty() {
        return vec![format!(
            "[{PREFIX}] PASS scanned {} README.md files, {} effective AGENTS.md sources, and {} CLAUDE.md files; {} valid CLAUDE.md symlink(s)",
            report.readme_files_seen,
            report.effective_agent_sources.len(),
            report.claude_files_seen,
            report.valid_claude_links.len()
        )];
    }

    let mut lines = vec![format!(
        "[{PREFIX}] FAIL {} instruction-doc violation(s)",
        report.violations.len()
    )];
    for violation in &report.violations {
        lines.push(format!("[{PREFIX}] FAIL {}", violation.message()));
    }
    lines
}

fn ensure_not_cancelled(cancel: Option<&AtomicBool>) -> Result<(), DocCheckError> {
    if cancel.is_some_and(|cancel| cancel.load(Ordering::Relaxed)) {
        Err(DocCheckError::Cancelled)
    } else {
        Ok(())
    }
}

impl DocViolation {
    fn message(&self) -> String {
        match self {
            Self::MissingColocatedAgents { path } => {
                format!("{} has no colocated AGENTS.md", display_path(path))
            }
            Self::ReadmeBrokenSymlink { path, error } => {
                format!("{} is a broken symlink: {error}", display_path(path))
            }
            Self::AgentsSymlinkTargetIsNotReadme { path, target } => format!(
                "{} is a symlink to {}; AGENTS.md symlinks may only target scanned README.md files",
                display_path(path),
                display_path(target)
            ),
            Self::AgentsBrokenSymlink { path, error } => {
                format!("{} is a broken symlink: {error}", display_path(path))
            }
            Self::MissingClaudeSymlink { path } => format!(
                "{} has no CLAUDE.md symlink pointing to its effective instruction source",
                display_path(path)
            ),
            Self::ClaudeIsNotSymlink { path } => format!("{} is not a symlink", display_path(path)),
            Self::ClaudeBrokenSymlink { path, error } => {
                format!("{} is a broken symlink: {error}", display_path(path))
            }
            Self::ClaudeTargetIsNotAgentSource { path, target } => format!(
                "{} points to {}, which is not an effective AGENTS.md instruction source",
                display_path(path),
                display_path(target)
            ),
        }
    }

    fn sort_key(&self) -> (&Path, u8, Option<&Path>) {
        match self {
            Self::MissingColocatedAgents { path } => (path.as_path(), 0, None),
            Self::ReadmeBrokenSymlink { path, .. } => (path.as_path(), 0, None),
            Self::AgentsSymlinkTargetIsNotReadme { path, target } => {
                (path.as_path(), 1, Some(target.as_path()))
            }
            Self::AgentsBrokenSymlink { path, .. } => (path.as_path(), 2, None),
            Self::MissingClaudeSymlink { path } => (path.as_path(), 3, None),
            Self::ClaudeIsNotSymlink { path } => (path.as_path(), 4, None),
            Self::ClaudeBrokenSymlink { path, .. } => (path.as_path(), 5, None),
            Self::ClaudeTargetIsNotAgentSource { path, target } => {
                (path.as_path(), 6, Some(target.as_path()))
            }
        }
    }
}

fn sort_violations(violations: &mut [DocViolation]) {
    violations.sort_by(|left, right| left.sort_key().cmp(&right.sort_key()));
}

fn is_instruction_doc_name(file_name: &OsStr) -> bool {
    [AGENTS_FILE, CLAUDE_FILE, README_FILE]
        .into_iter()
        .any(|expected| file_name == OsStr::new(expected))
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::expect_used,
        clippy::unwrap_used,
        reason = "tests use direct assertions for concise diagnostics"
    )]

    use super::*;

    fn scan(staged: &[(&str, EntryKind, &[u8])]) -> DocCheckReport {
        let tree = StagedTree::from_memory(PathBuf::from("/repo"), &[], staged);
        scan_instruction_docs(&tree, None).expect("scan staged tree")
    }

    #[test]
    fn scan_accepts_claude_link_through_agents_symlink_to_readme() {
        let report = scan(&[
            ("docs/README.md", EntryKind::File, b"docs"),
            ("docs/AGENTS.md", EntryKind::Symlink, b"README.md"),
            ("docs/CLAUDE.md", EntryKind::Symlink, b"README.md"),
            ("docs/x/README.md", EntryKind::File, b"x"),
            ("docs/x/AGENTS.md", EntryKind::Symlink, b"README.md"),
            ("docs/x/CLAUDE.md", EntryKind::Symlink, b"AGENTS.md"),
        ]);

        assert!(report.violations.is_empty(), "{:?}", report.violations);
        assert_eq!(report.effective_agent_sources.len(), 2);
        assert_eq!(report.valid_claude_links.len(), 2);
    }

    #[test]
    fn scan_reports_each_policy_violation_from_staged_entries() {
        let report = scan(&[
            ("README.md", EntryKind::File, b"root"),
            ("AGENTS.md", EntryKind::File, b"agents"),
            ("CLAUDE.md", EntryKind::File, b"not a link"),
            ("a/README.md", EntryKind::File, b"a"),
            ("notes.md", EntryKind::File, b"notes"),
            ("b/AGENTS.md", EntryKind::Symlink, b"../notes.md"),
            ("b/CLAUDE.md", EntryKind::Symlink, b"AGENTS.md"),
            ("c/AGENTS.md", EntryKind::Symlink, b"missing.md"),
            ("d/CLAUDE.md", EntryKind::Symlink, b"../README.md"),
            ("vendor/stim", EntryKind::Gitlink, b"commit"),
        ]);

        assert_eq!(
            report.violations,
            vec![
                DocViolation::MissingClaudeSymlink {
                    path: PathBuf::from("AGENTS.md"),
                },
                DocViolation::ClaudeIsNotSymlink {
                    path: PathBuf::from("CLAUDE.md"),
                },
                DocViolation::MissingColocatedAgents {
                    path: PathBuf::from("a/README.md"),
                },
                DocViolation::AgentsSymlinkTargetIsNotReadme {
                    path: PathBuf::from("b/AGENTS.md"),
                    target: PathBuf::from("notes.md"),
                },
                DocViolation::ClaudeTargetIsNotAgentSource {
                    path: PathBuf::from("b/CLAUDE.md"),
                    target: PathBuf::from("notes.md"),
                },
                DocViolation::AgentsBrokenSymlink {
                    path: PathBuf::from("c/AGENTS.md"),
                    error: "target c/missing.md is not a staged file".to_string(),
                },
                DocViolation::ClaudeTargetIsNotAgentSource {
                    path: PathBuf::from("d/CLAUDE.md"),
                    target: PathBuf::from("README.md"),
                },
            ]
        );
    }
}
