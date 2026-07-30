//! GitHub Flavored Markdown validation for repository-owned links and heading anchors.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsStr;
use std::fs;
use std::path::{Component, Path, PathBuf};

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use thiserror::Error;
use unicode_general_category::{GeneralCategory, get_general_category};

const PREFIX: &str = "stab-architecture";
const EXCLUDED_DIRECTORIES: &[&str] = &[
    ".git",
    ".next",
    ".venv",
    "build",
    "coverage",
    "dist",
    "generated",
    "node_modules",
    "target",
    "vendor",
];
const EXTERNAL_SCHEMES: &[&str] = &[
    "data", "ftp", "git", "http", "https", "irc", "ircs", "mailto", "news", "ssh", "tel",
];

/// A deterministic local Markdown link failure.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct DocsViolation {
    pub path: PathBuf,
    pub line: usize,
    pub column: usize,
    pub code: &'static str,
    pub destination: String,
    pub message: String,
}

/// Summary and failures from a repository Markdown check.
#[derive(Debug)]
pub struct DocsCheckReport {
    pub markdown_file_count: usize,
    pub local_link_count: usize,
    pub external_link_count: usize,
    pub violations: Vec<DocsViolation>,
}

impl DocsCheckReport {
    pub fn passed(&self) -> bool {
        self.violations.is_empty()
    }

    pub fn print(&self) {
        for violation in &self.violations {
            eprintln!(
                "[{PREFIX}] documentation violation [{}] {}:{}:{}: {} (destination: {:?})",
                violation.code,
                violation.path.display(),
                violation.line,
                violation.column,
                violation.message,
                violation.destination
            );
        }
        println!(
            "[{PREFIX}] checked {} Markdown files and {} local links; skipped {} external links",
            self.markdown_file_count, self.local_link_count, self.external_link_count
        );
    }
}

/// Errors that prevent repository Markdown validation from starting.
#[derive(Debug, Error)]
pub enum DocsCheckError {
    #[error("failed to resolve repository root {path}: {source}")]
    ResolveRoot {
        path: PathBuf,
        source: std::io::Error,
    },
}

#[derive(Debug)]
struct MarkdownDocument {
    relative_path: PathBuf,
    line_starts: Vec<usize>,
    anchors: BTreeSet<String>,
    links: Vec<MarkdownLink>,
}

#[derive(Debug)]
struct MarkdownLink {
    offset: usize,
    destination: String,
}

#[derive(Debug)]
struct Heading {
    text: String,
}

#[derive(Debug)]
enum Destination {
    External,
    Local {
        path: String,
        fragment: Option<String>,
    },
    Invalid {
        code: &'static str,
        message: String,
    },
}

/// Recursively validates repository-owned Markdown links and heading anchors.
pub fn check_markdown_docs(root: &Path) -> Result<DocsCheckReport, DocsCheckError> {
    let root = fs::canonicalize(root).map_err(|source| DocsCheckError::ResolveRoot {
        path: root.to_path_buf(),
        source,
    })?;
    let mut markdown_paths = Vec::new();
    let mut walk_violations = Vec::new();
    collect_markdown_paths(&root, &root, &mut markdown_paths, &mut walk_violations);
    markdown_paths.sort();
    markdown_paths.dedup();

    let mut documents = BTreeMap::new();
    let mut violations = walk_violations;
    for relative_path in markdown_paths {
        match read_document(&root, &relative_path) {
            Ok(document) => {
                documents.insert(relative_path, document);
            }
            Err(violation) => violations.push(violation),
        }
    }

    let mut local_link_count = 0;
    let mut external_link_count = 0;
    for document in documents.values() {
        for link in &document.links {
            let (line, column) = document.line_column(link.offset);
            match classify_destination(&link.destination) {
                Destination::External => external_link_count += 1,
                Destination::Invalid { code, message } => {
                    violations.push(DocsViolation {
                        path: document.relative_path.clone(),
                        line,
                        column,
                        code,
                        destination: link.destination.clone(),
                        message,
                    });
                }
                Destination::Local { path, fragment } => {
                    local_link_count += 1;
                    if let Err((code, message)) =
                        validate_local_link(&root, &documents, document, &path, fragment.as_deref())
                    {
                        violations.push(DocsViolation {
                            path: document.relative_path.clone(),
                            line,
                            column,
                            code,
                            destination: link.destination.clone(),
                            message,
                        });
                    }
                }
            }
        }
    }

    violations.sort();
    violations.dedup();
    Ok(DocsCheckReport {
        markdown_file_count: documents.len(),
        local_link_count,
        external_link_count,
        violations,
    })
}

impl MarkdownDocument {
    fn line_column(&self, offset: usize) -> (usize, usize) {
        let line_index = self.line_starts.partition_point(|start| *start <= offset) - 1;
        let line_start = self.line_starts.get(line_index).copied().unwrap_or(0);
        (line_index + 1, offset.saturating_sub(line_start) + 1)
    }
}

fn collect_markdown_paths(
    root: &Path,
    directory: &Path,
    paths: &mut Vec<PathBuf>,
    violations: &mut Vec<DocsViolation>,
) {
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(source) => {
            violations.push(filesystem_violation(
                root,
                directory,
                "markdown-read-directory",
                format!("failed to read directory: {source}"),
            ));
            return;
        }
    };
    let mut entries = match entries.collect::<Result<Vec<_>, _>>() {
        Ok(entries) => entries,
        Err(source) => {
            violations.push(filesystem_violation(
                root,
                directory,
                "markdown-read-directory-entry",
                format!("failed to read a directory entry: {source}"),
            ));
            return;
        }
    };
    entries.sort_by_key(std::fs::DirEntry::file_name);

    for entry in entries {
        let path = entry.path();
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(source) => {
                violations.push(filesystem_violation(
                    root,
                    &path,
                    "markdown-inspect-path",
                    format!("failed to inspect path: {source}"),
                ));
                continue;
            }
        };
        if file_type.is_dir() {
            if !is_excluded_directory(&entry.file_name()) {
                collect_markdown_paths(root, &path, paths, violations);
            }
        } else if (file_type.is_file() || file_type.is_symlink())
            && is_markdown_path(&path)
            && let Ok(relative) = path.strip_prefix(root)
        {
            paths.push(relative.to_path_buf());
        }
    }
}

fn is_excluded_directory(name: &OsStr) -> bool {
    name.to_str()
        .is_some_and(|name| EXCLUDED_DIRECTORIES.contains(&name))
}

fn is_markdown_path(path: &Path) -> bool {
    path.extension()
        .and_then(OsStr::to_str)
        .is_some_and(|extension| {
            extension.eq_ignore_ascii_case("md") || extension.eq_ignore_ascii_case("markdown")
        })
}

fn read_document(root: &Path, relative_path: &Path) -> Result<MarkdownDocument, DocsViolation> {
    let path = root.join(relative_path);
    let canonical_path = fs::canonicalize(&path).map_err(|source| {
        filesystem_violation(
            root,
            &path,
            "markdown-read-file",
            format!("failed to resolve Markdown file: {source}"),
        )
    })?;
    if !canonical_path.starts_with(root) {
        return Err(filesystem_violation(
            root,
            &path,
            "markdown-source-outside-repository",
            format!(
                "Markdown source resolves outside repository root to {}",
                canonical_path.display()
            ),
        ));
    }
    let source = fs::read_to_string(&path).map_err(|source| {
        filesystem_violation(
            root,
            &path,
            "markdown-read-file",
            format!("failed to read Markdown as UTF-8: {source}"),
        )
    })?;
    let line_starts = std::iter::once(0)
        .chain(
            source
                .match_indices('\n')
                .map(|(offset, _)| offset.saturating_add(1)),
        )
        .collect();
    let (anchors, links) = parse_document(&source);
    Ok(MarkdownDocument {
        relative_path: relative_path.to_path_buf(),
        line_starts,
        anchors,
        links,
    })
}

fn parse_document(source: &str) -> (BTreeSet<String>, Vec<MarkdownLink>) {
    let parser = Parser::new_ext(source, gfm_options()).into_offset_iter();
    let mut links = Vec::new();
    let mut anchors = BTreeSet::new();
    let mut used_anchors = BTreeSet::new();
    let mut heading = None;

    for (event, range) in parser {
        match event {
            Event::Start(Tag::Heading { .. }) => {
                heading = Some(Heading {
                    text: String::new(),
                });
            }
            Event::End(TagEnd::Heading(_)) => {
                if let Some(heading) = heading.take() {
                    let base = github_heading_slug(&heading.text);
                    anchors.insert(unique_anchor(base, &mut used_anchors));
                }
            }
            Event::Start(Tag::Link { dest_url, .. } | Tag::Image { dest_url, .. }) => {
                links.push(MarkdownLink {
                    offset: range.start,
                    destination: dest_url.into_string(),
                });
            }
            Event::Text(text) | Event::Code(text) if heading.is_some() => {
                if let Some(heading) = heading.as_mut() {
                    heading.text.push_str(&text);
                }
            }
            Event::SoftBreak | Event::HardBreak if heading.is_some() => {
                if let Some(heading) = heading.as_mut() {
                    heading.text.push(' ');
                }
            }
            _ => {}
        }
    }
    (anchors, links)
}

fn gfm_options() -> Options {
    Options::ENABLE_GFM
        | Options::ENABLE_TABLES
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
}

/// Mirrors GitHub's section-link category policy.
///
/// GitHub lowercases heading text, replaces only U+0020 with `-`, and strips
/// selected Unicode general categories. Alphabetic symbols are retained. This
/// intentionally does not normalize or transliterate Unicode text.
fn github_heading_slug(text: &str) -> String {
    let mut slug = String::with_capacity(text.len());
    for character in text.chars().flat_map(char::to_lowercase) {
        if character == ' ' {
            slug.push('-');
        } else if !github_strips_from_heading_slug(character) {
            slug.push(character);
        }
    }
    slug
}

fn github_strips_from_heading_slug(character: char) -> bool {
    use GeneralCategory::{
        ClosePunctuation, Control, CurrencySymbol, DashPunctuation, FinalPunctuation, Format,
        InitialPunctuation, LineSeparator, MathSymbol, ModifierSymbol, OpenPunctuation,
        OtherNumber, OtherPunctuation, OtherSymbol, ParagraphSeparator, PrivateUse, SpaceSeparator,
        Surrogate, Unassigned,
    };

    match get_general_category(character) {
        OtherNumber | ClosePunctuation | FinalPunctuation | InitialPunctuation
        | OpenPunctuation | OtherPunctuation | Control | PrivateUse | Format | Unassigned
        | Surrogate | LineSeparator | ParagraphSeparator | SpaceSeparator => true,
        DashPunctuation => character != '-',
        CurrencySymbol | MathSymbol | ModifierSymbol | OtherSymbol => !character.is_alphabetic(),
        _ => false,
    }
}

fn unique_anchor(base: String, used: &mut BTreeSet<String>) -> String {
    if used.insert(base.clone()) {
        return base;
    }
    for suffix in 1usize.. {
        let candidate = format!("{base}-{suffix}");
        if used.insert(candidate.clone()) {
            return candidate;
        }
    }
    unreachable!("usize suffix iteration cannot terminate");
}

fn classify_destination(destination: &str) -> Destination {
    if destination.starts_with("//") {
        return Destination::External;
    }
    if let Some(scheme) = uri_scheme(destination) {
        if EXTERNAL_SCHEMES
            .iter()
            .any(|external| scheme.eq_ignore_ascii_case(external))
        {
            return Destination::External;
        }
        return Destination::Invalid {
            code: "markdown-unsupported-scheme",
            message: format!("unsupported or unsafe link scheme {scheme:?}"),
        };
    }

    let (before_fragment, fragment) = destination
        .split_once('#')
        .map_or((destination, None), |(path, fragment)| {
            (path, Some(fragment))
        });
    let path = before_fragment
        .split_once('?')
        .map_or(before_fragment, |(path, _)| path);
    let path = match percent_decode(path) {
        Ok(path) => path,
        Err(message) => {
            return Destination::Invalid {
                code: "markdown-invalid-escape",
                message,
            };
        }
    };
    let fragment = match fragment.map(percent_decode).transpose() {
        Ok(fragment) => fragment,
        Err(message) => {
            return Destination::Invalid {
                code: "markdown-invalid-escape",
                message,
            };
        }
    };
    Destination::Local { path, fragment }
}

fn uri_scheme(destination: &str) -> Option<&str> {
    let separator = destination.find(':')?;
    let (candidate, _) = destination.split_at(separator);
    if candidate.is_empty()
        || candidate
            .chars()
            .any(|character| matches!(character, '/' | '?' | '#'))
    {
        return None;
    }
    let mut characters = candidate.chars();
    let first = characters.next()?;
    (first.is_ascii_alphabetic()
        && characters.all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '+' | '-' | '.')
        }))
    .then_some(candidate)
}

fn percent_decode(value: &str) -> Result<String, String> {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while let Some(byte) = bytes.get(index).copied() {
        if byte == b'%' {
            let Some(high) = bytes.get(index + 1).copied().and_then(hex_value) else {
                return Err(format!("invalid percent escape at byte {index}"));
            };
            let Some(low) = bytes.get(index + 2).copied().and_then(hex_value) else {
                return Err(format!("invalid percent escape at byte {index}"));
            };
            decoded.push((high << 4) | low);
            index += 3;
        } else {
            decoded.push(byte);
            index += 1;
        }
    }
    String::from_utf8(decoded).map_err(|_| "percent-decoded destination is not UTF-8".to_owned())
}

fn hex_value(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

fn validate_local_link(
    root: &Path,
    documents: &BTreeMap<PathBuf, MarkdownDocument>,
    source: &MarkdownDocument,
    destination_path: &str,
    fragment: Option<&str>,
) -> Result<(), (&'static str, String)> {
    if destination_path.contains('\\') {
        return Err((
            "markdown-unsafe-path",
            "local link paths must use forward slashes".to_owned(),
        ));
    }
    let target_relative = normalize_target_path(&source.relative_path, destination_path)?;
    let target_path = root.join(&target_relative);
    let metadata = fs::metadata(&target_path).map_err(|source| {
        (
            "markdown-missing-target",
            format!(
                "local target {} does not exist or is not accessible: {source}",
                target_relative.display()
            ),
        )
    })?;
    let canonical_target = fs::canonicalize(&target_path).map_err(|source| {
        (
            "markdown-missing-target",
            format!(
                "failed to resolve local target {}: {source}",
                target_relative.display()
            ),
        )
    })?;
    if !canonical_target.starts_with(root) {
        return Err((
            "markdown-target-outside-repository",
            format!(
                "local target {} resolves outside the repository",
                target_relative.display()
            ),
        ));
    }

    let Some(fragment) = fragment.filter(|fragment| !fragment.is_empty()) else {
        return Ok(());
    };
    let markdown_target = if metadata.is_dir() {
        target_relative.join("README.md")
    } else {
        target_relative
    };
    if !is_markdown_path(&markdown_target) {
        return Ok(());
    }
    let target_document = documents.get(&markdown_target).ok_or_else(|| {
        (
            "markdown-unchecked-anchor-target",
            format!(
                "anchor target {} is Markdown but is excluded or unreadable",
                markdown_target.display()
            ),
        )
    })?;
    if target_document.anchors.contains(fragment) {
        Ok(())
    } else {
        Err((
            "markdown-missing-anchor",
            format!(
                "heading anchor #{fragment} does not exist in {}",
                markdown_target.display()
            ),
        ))
    }
}

fn normalize_target_path(
    source_path: &Path,
    destination_path: &str,
) -> Result<PathBuf, (&'static str, String)> {
    let path = Path::new(destination_path);
    if path.is_absolute() {
        return Err((
            "markdown-unsafe-path",
            "absolute local paths are not repository-owned links".to_owned(),
        ));
    }

    let mut components = source_path
        .parent()
        .unwrap_or_else(|| Path::new(""))
        .components()
        .filter_map(|component| match component {
            Component::Normal(component) => Some(component.to_os_string()),
            _ => None,
        })
        .collect::<Vec<_>>();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(component) => components.push(component.to_os_string()),
            Component::ParentDir => {
                if components.pop().is_none() {
                    return Err((
                        "markdown-path-traversal",
                        "local link traverses outside the repository".to_owned(),
                    ));
                }
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err((
                    "markdown-unsafe-path",
                    "absolute local paths are not repository-owned links".to_owned(),
                ));
            }
        }
    }
    if destination_path.is_empty() {
        return Ok(source_path.to_path_buf());
    }
    Ok(components.into_iter().collect())
}

fn filesystem_violation(
    root: &Path,
    path: &Path,
    code: &'static str,
    message: String,
) -> DocsViolation {
    DocsViolation {
        path: path.strip_prefix(root).unwrap_or(path).to_path_buf(),
        line: 1,
        column: 1,
        code,
        destination: String::new(),
        message,
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use tempfile::TempDir;

    use super::{DocsCheckReport, check_markdown_docs, github_heading_slug};

    fn repository(files: &[(&str, &str)]) -> TempDir {
        let root = tempfile::tempdir().expect("fixture repository should be created");
        for (relative, contents) in files {
            let path = root.path().join(relative);
            fs::create_dir_all(path.parent().expect("fixture path should have a parent"))
                .expect("fixture parent should be created");
            fs::write(path, contents).expect("fixture Markdown should be written");
        }
        root
    }

    fn report(root: &Path) -> DocsCheckReport {
        check_markdown_docs(root).expect("fixture repository should be checkable")
    }

    fn codes(report: &DocsCheckReport) -> Vec<&'static str> {
        report
            .violations
            .iter()
            .map(|violation| violation.code)
            .collect()
    }

    #[test]
    fn accepts_local_files_same_and_cross_file_anchors_and_external_links() {
        let root = repository(&[
            (
                "README.md",
                "# Home\n\n[local](docs/guide.md#details)\n[same](#home)\n[site](https://example.com)\n",
            ),
            (
                "docs/guide.md",
                "# Guide\n\n## Details\n\n[up](../README.md)\n",
            ),
        ]);

        let report = report(root.path());

        assert!(report.passed(), "{:?}", report.violations);
        assert_eq!(report.markdown_file_count, 2);
        assert_eq!(report.local_link_count, 3);
        assert_eq!(report.external_link_count, 1);
    }

    #[test]
    fn reports_missing_paths_and_broken_anchors_together_in_source_order() {
        let root = repository(&[
            (
                "README.md",
                "# Home\n\n[missing](missing.md)\n[anchor](guide.md#absent)\n",
            ),
            ("guide.md", "# Present\n"),
        ]);

        let first_report = report(root.path());
        let repeated_report = report(root.path());

        assert_eq!(
            codes(&first_report),
            vec!["markdown-missing-target", "markdown-missing-anchor"]
        );
        assert_eq!(first_report.violations, repeated_report.violations);
        assert_eq!(
            first_report
                .violations
                .first()
                .expect("missing-target violation should exist")
                .line,
            3
        );
        assert_eq!(
            first_report
                .violations
                .get(1)
                .expect("missing-anchor violation should exist")
                .line,
            4
        );
    }

    #[test]
    fn disambiguates_duplicate_heading_anchors() {
        let root = repository(&[(
            "README.md",
            "# Echo\n\n# Echo\n\n# Echo 1\n\n# Echo-1\n\n# Echo\n\n[first](#echo)\n[second](#echo-1)\n[derived](#echo-1-1)\n[collision](#echo-1-2)\n[last](#echo-2)\n[missing](#echo-3)\n",
        )]);

        let report = report(root.path());

        assert_eq!(codes(&report), vec!["markdown-missing-anchor"]);
        assert_eq!(
            report
                .violations
                .first()
                .expect("missing-anchor violation should exist")
                .destination,
            "#echo-3"
        );
    }

    #[test]
    fn treats_heading_attribute_syntax_as_gfm_text() {
        let root = repository(&[(
            "README.md",
            "# Section {#custom}\n\n[rendered heading](#section-custom)\n[not a custom id](#custom)\n",
        )]);

        let report = report(root.path());

        assert_eq!(codes(&report), vec!["markdown-missing-anchor"]);
        assert_eq!(
            report
                .violations
                .first()
                .expect("missing-anchor violation should exist")
                .destination,
            "#custom"
        );
    }

    #[test]
    fn matches_github_unicode_heading_slug_rules() {
        assert_eq!(github_heading_slug("What’s new?"), "whats-new");
        assert_eq!(github_heading_slug("I ♥ Unicode"), "i--unicode");
        assert_eq!(github_heading_slug("Greek Θ"), "greek-θ");
        assert_eq!(github_heading_slug("alpha\tbeta\u{200e}"), "alphabeta");
        assert_eq!(github_heading_slug("alpha\u{a0}beta"), "alphabeta");
    }

    #[test]
    fn ignores_link_syntax_inside_code_fences_and_inline_code() {
        let root = repository(&[(
            "README.md",
            "# Code\n\n```markdown\n[not a link](missing.md)\n```\n\n`[also not](absent.md)`\n",
        )]);

        let report = report(root.path());

        assert!(report.passed(), "{:?}", report.violations);
        assert_eq!(report.local_link_count, 0);
    }

    #[test]
    fn rejects_lexical_and_symlink_traversal_outside_the_repository() {
        let parent = tempfile::tempdir().expect("fixture parent should be created");
        let root = parent.path().join("repo");
        fs::create_dir(&root).expect("fixture repository should be created");
        fs::write(parent.path().join("outside.md"), "# Outside\n")
            .expect("outside fixture should be written");
        fs::write(
            root.join("README.md"),
            "# Home\n\n[lexical](../../outside.md)\n[symlink](escape.md)\n",
        )
        .expect("fixture Markdown should be written");
        #[cfg(unix)]
        std::os::unix::fs::symlink(parent.path().join("outside.md"), root.join("escape.md"))
            .expect("fixture symlink should be created");

        let report = report(&root);

        #[cfg(unix)]
        assert_eq!(
            codes(&report),
            vec![
                "markdown-path-traversal",
                "markdown-target-outside-repository",
                "markdown-source-outside-repository"
            ]
        );
        #[cfg(not(unix))]
        assert_eq!(codes(&report), vec!["markdown-path-traversal"]);
    }

    #[test]
    fn excludes_generated_build_vendor_and_git_trees() {
        let root = repository(&[
            ("README.md", "# Home\n"),
            ("target/broken.md", "[bad](missing.md)\n"),
            ("build/broken.md", "[bad](missing.md)\n"),
            ("generated/broken.md", "[bad](missing.md)\n"),
            ("vendor/broken.md", "[bad](missing.md)\n"),
            (".git/broken.md", "[bad](missing.md)\n"),
        ]);

        let report = report(root.path());

        assert!(report.passed(), "{:?}", report.violations);
        assert_eq!(report.markdown_file_count, 1);
    }
}
