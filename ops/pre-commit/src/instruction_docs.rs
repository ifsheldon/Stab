//! Stab instruction-document policy checks.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use ignore::WalkBuilder;
use thiserror::Error;

const PREFIX: &str = "stab-instruction-docs";
const AGENTS_FILE: &str = "AGENTS.md";
const CLAUDE_FILE: &str = "CLAUDE.md";
const README_FILE: &str = "README.md";

const EXCLUDED_COMPONENTS: &[&str] = &[
    ".git",
    ".next",
    ".pytest_cache",
    ".turbo",
    "build",
    "coverage",
    "dist",
    "node_modules",
    "target",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DocCheckConfig {
    root: PathBuf,
}

impl DocCheckConfig {
    pub(crate) fn new(root: PathBuf) -> Self {
        Self { root }
    }
}

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
struct ReadmeEntry {
    path: PathBuf,
    canonical_path: PathBuf,
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
    AgentsSymlinkTargetIsNotReadme { path: PathBuf, target: PathBuf },
    AgentsBrokenSymlink { path: PathBuf, error: String },
    MissingClaudeSymlink { path: PathBuf },
    ClaudeIsNotSymlink { path: PathBuf },
    ClaudeBrokenSymlink { path: PathBuf, error: String },
    ClaudeTargetIsNotAgentSource { path: PathBuf, target: PathBuf },
}

#[derive(Debug, Error)]
pub(crate) enum DocCheckError {
    #[error("failed to resolve scan root {path}: {source}")]
    ResolveRoot {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("failed to read {path}: {source}")]
    ReadGitmodules {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("failed to walk source tree: {0}")]
    Walk(#[from] ignore::Error),

    #[error("failed to inspect {path}: {source}")]
    Metadata {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("failed to canonicalize {path}: {source}")]
    Canonicalize {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("scan cancelled")]
    Cancelled,
}

pub(crate) fn scan_instruction_docs_with_cancel(
    config: &DocCheckConfig,
    cancel: Option<&AtomicBool>,
) -> Result<DocCheckReport, DocCheckError> {
    let root =
        std::fs::canonicalize(&config.root).map_err(|source| DocCheckError::ResolveRoot {
            path: config.root.clone(),
            source,
        })?;
    let submodule_paths = submodule_paths(&root)?;
    let mut readmes_by_directory = BTreeMap::<PathBuf, ReadmeEntry>::new();
    let mut readme_canonicals = BTreeSet::<PathBuf>::new();
    let mut agents_by_directory = BTreeMap::<PathBuf, AgentEntry>::new();
    let mut claude_targets = BTreeMap::<PathBuf, BTreeSet<PathBuf>>::new();
    let mut seen_claude_links = BTreeSet::<PathBuf>::new();
    let mut claude_files_seen = 0usize;
    let mut violations = Vec::new();

    let filter_root = root.clone();
    let filter_submodule_paths = submodule_paths.clone();
    for entry in WalkBuilder::new(&root)
        .parents(true)
        .hidden(false)
        .follow_links(false)
        .filter_entry(move |entry| {
            let relative = relative_path(&filter_root, entry.path());
            !is_submodule_path(&relative, &filter_submodule_paths)
                && !is_git_internal_path(&relative)
                && !has_excluded_component(&relative)
        })
        .build()
    {
        ensure_not_cancelled(cancel)?;
        let entry = entry?;
        let path = entry.path();
        let Some(file_name) = path.file_name() else {
            continue;
        };
        if !is_instruction_doc_name(file_name) {
            continue;
        }

        let relative = relative_path(&root, path);
        let directory = relative
            .parent()
            .map_or_else(PathBuf::new, Path::to_path_buf);
        let metadata =
            std::fs::symlink_metadata(path).map_err(|source| DocCheckError::Metadata {
                path: relative.clone(),
                source,
            })?;
        let file_type = metadata.file_type();

        if file_name == OsStr::new(README_FILE) {
            let canonical =
                std::fs::canonicalize(path).map_err(|source| DocCheckError::Canonicalize {
                    path: relative.clone(),
                    source,
                })?;
            readme_canonicals.insert(canonical.clone());
            readmes_by_directory
                .entry(directory)
                .or_insert(ReadmeEntry {
                    path: relative,
                    canonical_path: canonical,
                });
            continue;
        }

        if file_name == OsStr::new(AGENTS_FILE) {
            let canonical = match std::fs::canonicalize(path) {
                Ok(canonical) => canonical,
                Err(error) if file_type.is_symlink() => {
                    violations.push(DocViolation::AgentsBrokenSymlink {
                        path: relative,
                        error: error.to_string(),
                    });
                    continue;
                }
                Err(source) => {
                    return Err(DocCheckError::Canonicalize {
                        path: relative,
                        source,
                    });
                }
            };
            let symlink_target = if file_type.is_symlink() {
                Some(display_target(&root, &canonical))
            } else {
                None
            };
            agents_by_directory.entry(directory).or_insert(AgentEntry {
                path: relative,
                canonical_path: canonical,
                symlink_target,
            });
            continue;
        }

        claude_files_seen = claude_files_seen.saturating_add(1);
        if !file_type.is_symlink() {
            violations.push(DocViolation::ClaudeIsNotSymlink { path: relative });
            continue;
        }

        let canonical_link =
            canonical_link_path(path).map_err(|source| DocCheckError::Canonicalize {
                path: relative.clone(),
                source,
            })?;
        if !seen_claude_links.insert(canonical_link) {
            continue;
        }
        match std::fs::canonicalize(path) {
            Ok(target) => {
                claude_targets.entry(target).or_default().insert(relative);
            }
            Err(error) => violations.push(DocViolation::ClaudeBrokenSymlink {
                path: relative,
                error: error.to_string(),
            }),
        }
    }

    for (directory, readme) in &readmes_by_directory {
        if !agents_by_directory.contains_key(directory) {
            violations.push(DocViolation::MissingColocatedAgents {
                path: readme.path.clone(),
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

    for target in claude_targets.keys() {
        if !source_by_canonical.contains_key(target) {
            let display_target = display_target(&root, target);
            if let Some(paths) = claude_targets.get(target) {
                for path in paths {
                    violations.push(DocViolation::ClaudeTargetIsNotAgentSource {
                        path: path.clone(),
                        target: display_target.clone(),
                    });
                }
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
        readme_files_seen: readmes_by_directory.len(),
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

fn submodule_paths(root: &Path) -> Result<Vec<PathBuf>, DocCheckError> {
    let path = root.join(".gitmodules");
    let raw = match std::fs::read_to_string(&path) {
        Ok(raw) => raw,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(source) => {
            return Err(DocCheckError::ReadGitmodules {
                path: relative_path(root, &path),
                source,
            });
        }
    };
    let mut paths = Vec::new();
    for line in raw.lines() {
        let trimmed = line.trim();
        let Some((key, value)) = trimmed.split_once('=') else {
            continue;
        };
        if key.trim() != "path" {
            continue;
        }
        let value = value.trim().trim_matches('"');
        if value.is_empty() {
            continue;
        }
        let submodule_path = PathBuf::from(value);
        if submodule_path.is_absolute() {
            continue;
        }
        paths.push(submodule_path);
    }
    paths.sort();
    paths.dedup();
    Ok(paths)
}

fn is_submodule_path(path: &Path, submodule_paths: &[PathBuf]) -> bool {
    submodule_paths
        .iter()
        .any(|submodule_path| path == submodule_path || path.starts_with(submodule_path))
}

fn is_git_internal_path(path: &Path) -> bool {
    path.components()
        .next()
        .is_some_and(|component| component.as_os_str() == OsStr::new(".git"))
}

fn has_excluded_component(path: &Path) -> bool {
    path.components().any(|component| {
        let name = component.as_os_str();
        EXCLUDED_COMPONENTS
            .iter()
            .any(|excluded| name == OsStr::new(*excluded))
    })
}

fn is_instruction_doc_name(file_name: &OsStr) -> bool {
    [AGENTS_FILE, CLAUDE_FILE, README_FILE]
        .into_iter()
        .any(|expected| file_name == OsStr::new(expected))
}

fn canonical_link_path(path: &Path) -> Result<PathBuf, std::io::Error> {
    let Some(parent) = path.parent() else {
        return std::fs::canonicalize(path);
    };
    let Some(file_name) = path.file_name() else {
        return std::fs::canonicalize(path);
    };
    Ok(std::fs::canonicalize(parent)?.join(file_name))
}

fn display_target(root: &Path, target: &Path) -> PathBuf {
    target
        .strip_prefix(root)
        .map_or_else(|_| target.to_path_buf(), Path::to_path_buf)
}

fn relative_path(root: &Path, path: &Path) -> PathBuf {
    path.strip_prefix(root)
        .map_or_else(|_| path.to_path_buf(), Path::to_path_buf)
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

    #[cfg(unix)]
    use std::os::unix::fs::symlink;

    #[test]
    fn scan_reports_agents_without_claude_symlink() {
        let temp = tempfile::tempdir().expect("create temp repo");
        write_file(&temp.path().join("README.md"), "root readme");
        write_file(&temp.path().join("AGENTS.md"), "root agents");

        let report = scan_instruction_docs_with_cancel(
            &DocCheckConfig::new(temp.path().to_path_buf()),
            None,
        )
        .expect("scan temp repo");

        assert_eq!(
            report.violations,
            vec![DocViolation::MissingClaudeSymlink {
                path: PathBuf::from("AGENTS.md"),
            }]
        );
        assert_eq!(report.readme_files_seen, 1);
        assert_eq!(report.effective_agent_sources.len(), 1);
        assert_eq!(report.claude_files_seen, 0);
    }

    #[test]
    fn scan_reports_readme_without_colocated_agents() {
        let temp = tempfile::tempdir().expect("create temp repo");
        std::fs::create_dir_all(temp.path().join("docs")).expect("create docs");
        write_file(&temp.path().join("docs/README.md"), "docs");

        let report = scan_instruction_docs_with_cancel(
            &DocCheckConfig::new(temp.path().to_path_buf()),
            None,
        )
        .expect("scan temp repo");

        assert_eq!(
            report.violations,
            vec![DocViolation::MissingColocatedAgents {
                path: PathBuf::from("docs/README.md"),
            }]
        );
    }

    #[test]
    fn scan_ignores_submodule_readmes() {
        let temp = tempfile::tempdir().expect("create temp repo");
        write_file(
            &temp.path().join(".gitmodules"),
            "[submodule \"vendor/stim\"]\n\tpath = vendor/stim\n\turl = https://example.invalid/stim\n",
        );
        std::fs::create_dir_all(temp.path().join("vendor/stim")).expect("create vendor");
        write_file(&temp.path().join("vendor/stim/README.md"), "upstream");

        let report = scan_instruction_docs_with_cancel(
            &DocCheckConfig::new(temp.path().to_path_buf()),
            None,
        )
        .expect("scan temp repo");

        assert!(report.violations.is_empty());
        assert_eq!(report.readme_files_seen, 0);
    }

    #[cfg(unix)]
    #[test]
    fn scan_accepts_claude_symlink_to_agents() {
        let temp = tempfile::tempdir().expect("create temp repo");
        write_file(&temp.path().join("README.md"), "root readme");
        write_file(&temp.path().join("AGENTS.md"), "root agents");
        symlink("AGENTS.md", temp.path().join("CLAUDE.md")).expect("link claude");

        let report = scan_instruction_docs_with_cancel(
            &DocCheckConfig::new(temp.path().to_path_buf()),
            None,
        )
        .expect("scan temp repo");

        assert!(report.violations.is_empty());
        assert_eq!(report.valid_claude_links.len(), 1);
    }

    #[test]
    fn scan_rejects_regular_claude_file() {
        let temp = tempfile::tempdir().expect("create temp repo");
        write_file(&temp.path().join("README.md"), "root readme");
        write_file(&temp.path().join("AGENTS.md"), "root agents");
        write_file(&temp.path().join("CLAUDE.md"), "not a symlink");

        let report = scan_instruction_docs_with_cancel(
            &DocCheckConfig::new(temp.path().to_path_buf()),
            None,
        )
        .expect("scan temp repo");

        assert!(
            report
                .violations
                .contains(&DocViolation::ClaudeIsNotSymlink {
                    path: PathBuf::from("CLAUDE.md"),
                })
        );
    }

    fn write_file(path: &Path, content: &str) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("create parent");
        }
        std::fs::write(path, content).expect("write file");
    }
}
