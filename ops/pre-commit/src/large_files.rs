//! Staged source-file line-count checks.

use std::path::{Component, Path, PathBuf};

use thiserror::Error;

const PREFIX: &str = "stab-large-files";

pub(crate) const DEFAULT_LINE_THRESHOLD: usize = 1_200;
pub(crate) const DEFAULT_WATCH_THRESHOLD: usize = 900;

const CODE_EXTENSIONS: &[&str] = &[
    "bash", "cjs", "css", "html", "js", "jsx", "mjs", "py", "rs", "sass", "scss", "sh", "sql",
    "toml", "ts", "tsx", "vue", "yaml", "yml", "zsh",
];

const CODE_BASENAMES: &[&str] = &["Dockerfile", "Justfile", "Makefile", "Rakefile", "justfile"];

const EXCLUDED_COMPONENTS: &[&str] = &[
    ".git",
    ".next",
    ".turbo",
    "build",
    "coverage",
    "dist",
    "generated",
    "node_modules",
    "target",
    "vendor",
];

const EXCLUDED_FILENAMES: &[&str] = &[
    "Cargo.lock",
    "bun.lock",
    "package-lock.json",
    "pnpm-lock.yaml",
    "yarn.lock",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SourceBlob {
    path: PathBuf,
    content: Vec<u8>,
}

impl SourceBlob {
    pub(crate) fn new(path: PathBuf, content: Vec<u8>) -> Self {
        Self { path, content }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BlobScanConfig {
    files: Vec<SourceBlob>,
    threshold: usize,
    watch_threshold: Option<usize>,
}

impl BlobScanConfig {
    pub(crate) fn new(
        files: Vec<SourceBlob>,
        threshold: usize,
        watch_threshold: Option<usize>,
    ) -> Result<Self, LargeFileScanError> {
        if threshold == 0 {
            return Err(LargeFileScanError::InvalidConfig(
                "threshold must be greater than zero".to_string(),
            ));
        }
        if let Some(watch_threshold) = watch_threshold
            && watch_threshold >= threshold
        {
            return Err(LargeFileScanError::InvalidConfig(format!(
                "watch threshold {watch_threshold} must be lower than threshold {threshold}"
            )));
        }
        Ok(Self {
            files,
            threshold,
            watch_threshold,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ScanReport {
    scanned_files: usize,
    threshold: usize,
    watch_threshold: Option<usize>,
    oversized: Vec<FileLineCount>,
    watch: Vec<FileLineCount>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FileLineCount {
    path: PathBuf,
    lines: usize,
}

#[derive(Debug, Error)]
pub(crate) enum LargeFileScanError {
    #[error("invalid scan config: {0}")]
    InvalidConfig(String),

    #[error("line count overflowed while reading {0}")]
    LineCountOverflow(PathBuf),
}

pub(crate) fn scan_large_file_blobs(
    config: &BlobScanConfig,
) -> Result<ScanReport, LargeFileScanError> {
    let mut oversized = Vec::new();
    let mut watch = Vec::new();
    let mut scanned_files = 0usize;

    for file in &config.files {
        if !is_source_file_path(&file.path) {
            continue;
        }
        scanned_files = scanned_files
            .checked_add(1)
            .ok_or_else(|| LargeFileScanError::LineCountOverflow(file.path.clone()))?;
        let lines = count_lines_in_bytes(&file.content)
            .ok_or_else(|| LargeFileScanError::LineCountOverflow(file.path.clone()))?;
        let count = FileLineCount {
            path: file.path.clone(),
            lines,
        };
        if lines >= config.threshold {
            oversized.push(count);
        } else if config
            .watch_threshold
            .is_some_and(|watch_threshold| lines >= watch_threshold)
        {
            watch.push(count);
        }
    }

    sort_counts(&mut oversized);
    sort_counts(&mut watch);
    Ok(ScanReport {
        scanned_files,
        threshold: config.threshold,
        watch_threshold: config.watch_threshold,
        oversized,
        watch,
    })
}

pub(crate) fn is_source_file_path(path: &Path) -> bool {
    !should_skip(path) && is_code_file(path)
}

pub(crate) fn report_passed(report: &ScanReport) -> bool {
    report.oversized.is_empty()
}

pub(crate) fn render_report(report: &ScanReport) -> Vec<String> {
    let mut lines = Vec::new();
    if report.oversized.is_empty() {
        lines.push(format!(
            "[{PREFIX}] PASS scanned {} code files; no files at or above {} lines",
            report.scanned_files, report.threshold
        ));
    } else {
        lines.push(format!(
            "[{PREFIX}] FAIL {} code files are at or above {} lines",
            report.oversized.len(),
            report.threshold
        ));
        for file in &report.oversized {
            lines.push(format!(
                "[{PREFIX}] FAIL {:>5} {}",
                file.lines,
                display_path(&file.path)
            ));
        }
    }

    if let Some(watch_threshold) = report.watch_threshold
        && !report.watch.is_empty()
    {
        lines.push(format!(
            "[{PREFIX}] WARN {} code files are between {} and {} lines",
            report.watch.len(),
            watch_threshold,
            report.threshold
        ));
        for file in &report.watch {
            lines.push(format!(
                "[{PREFIX}] WARN {:>5} {}",
                file.lines,
                display_path(&file.path)
            ));
        }
    }

    lines
}

fn sort_counts(files: &mut [FileLineCount]) {
    files.sort_by(|left, right| {
        right
            .lines
            .cmp(&left.lines)
            .then_with(|| left.path.cmp(&right.path))
    });
}

fn count_lines_in_bytes(bytes: &[u8]) -> Option<usize> {
    if bytes.is_empty() {
        return Some(0);
    }
    let newline_count = bytes.iter().filter(|byte| **byte == b'\n').count();
    if bytes.ends_with(b"\n") {
        Some(newline_count)
    } else {
        newline_count.checked_add(1)
    }
}

fn should_skip(path: &Path) -> bool {
    has_excluded_component(path) || has_excluded_filename(path)
}

fn has_excluded_component(path: &Path) -> bool {
    path.components().any(|component| {
        let Component::Normal(name) = component else {
            return false;
        };
        EXCLUDED_COMPONENTS.iter().any(|excluded| name == *excluded)
    })
}

fn has_excluded_filename(path: &Path) -> bool {
    path.file_name()
        .is_some_and(|name| EXCLUDED_FILENAMES.iter().any(|excluded| name == *excluded))
}

fn is_code_file(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| CODE_BASENAMES.contains(&name))
        || path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| CODE_EXTENSIONS.contains(&extension))
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

    #[test]
    fn staged_blob_scan_reports_oversized_files() {
        let content = "x\n".repeat(4).into_bytes();
        let config = BlobScanConfig::new(
            vec![SourceBlob::new(PathBuf::from("src/lib.rs"), content)],
            4,
            Some(2),
        )
        .expect("valid config");

        let report = scan_large_file_blobs(&config).expect("scan blobs");

        assert!(!report_passed(&report));
        assert_eq!(report.oversized.len(), 1);
    }

    #[test]
    fn staged_blob_scan_skips_lockfiles_and_vendor() {
        let config = BlobScanConfig::new(
            vec![
                SourceBlob::new(PathBuf::from("Cargo.lock"), b"x\n".to_vec()),
                SourceBlob::new(PathBuf::from("vendor/stim/src/stim.cc"), b"x\n".to_vec()),
            ],
            1,
            None,
        )
        .expect("valid config");

        let report = scan_large_file_blobs(&config).expect("scan blobs");

        assert_eq!(report.scanned_files, 0);
        assert!(report_passed(&report));
    }
}
