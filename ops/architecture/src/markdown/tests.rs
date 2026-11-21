use std::fs;
use std::path::Path;

use tempfile::TempDir;

use super::{DocsCheckError, DocsCheckReport, check_markdown_docs, github_heading_slug};

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
fn reports_undefined_reference_labels_but_not_bracketed_text() {
    let root = repository(&[(
        "README.md",
        "# Home\n\n[full][missing]\n[collapsed][]\n[defined][home]\n[plain bracketed note]\n- [ ] task\n\n[home]: #home\n",
    )]);

    let report = report(root.path());

    assert_eq!(
        codes(&report),
        vec![
            "markdown-undefined-reference",
            "markdown-undefined-reference"
        ]
    );
    assert_eq!(
        report
            .violations
            .iter()
            .map(|violation| (violation.line, violation.destination.as_str()))
            .collect::<Vec<_>>(),
        [(3, "missing"), (4, "collapsed")]
    );
}

#[test]
fn email_autolinks_are_not_local_paths() {
    let root = repository(&[("README.md", "# Home\n\n<maintainer@example.com>\n")]);

    let report = report(root.path());

    assert!(report.passed(), "{:?}", report.violations);
    assert_eq!(report.local_link_count, 0);
    assert_eq!(report.external_link_count, 1);
}

#[test]
fn checks_local_targets_of_raw_html_links_and_images() {
    let root = repository(&[
        (
            "README.md",
            "# Home\n\n<a href=\"docs/guide.md#part\">guide</a>\n\n<p><img src=\"missing.svg\" alt=\"x\"></p>\n\nText <a href=\"https://example.com\">site</a> and <a name=\"anchor\"></a>.\n",
        ),
        ("docs/guide.md", "# Part\n"),
    ]);

    let report = report(root.path());

    assert_eq!(codes(&report), vec!["markdown-missing-target"]);
    assert_eq!(
        report.violations.first().map(|violation| violation.line),
        Some(5)
    );
    assert_eq!(report.local_link_count, 2);
    assert_eq!(report.external_link_count, 1);
}

fn git(root: &Path, args: &[&str]) {
    let status = std::process::Command::new("git")
        .args(args)
        .current_dir(root)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .status()
        .expect("git must be on PATH to run the tracked-inventory Markdown tests");
    assert!(status.success(), "git {args:?} failed with {status}");
}

#[test]
fn git_checkouts_check_only_tracked_files_and_tracked_targets() {
    let root = repository(&[
        (
            "README.md",
            "# Home\n\n[tracked](.config/notes.md#notes)\n[untracked](scratch.md)\n[directory](.config)\n",
        ),
        (".config/notes.md", "# Notes\n\n[bad](absent.md)\n"),
        ("scratch.md", "# Scratch\n\n[bad](also-absent.md)\n"),
        ("target/output.md", "[bad](missing.md)\n"),
    ]);
    git(root.path(), &["init", "-q"]);
    git(root.path(), &["add", "README.md", ".config/notes.md"]);

    let report = report(root.path());

    assert_eq!(report.markdown_file_count, 2, "{:?}", report.violations);
    assert_eq!(
        report
            .violations
            .iter()
            .map(|violation| (
                violation.path.to_string_lossy().into_owned(),
                violation.code
            ))
            .collect::<Vec<_>>(),
        [
            (".config/notes.md".to_owned(), "markdown-missing-target"),
            ("README.md".to_owned(), "markdown-untracked-target"),
        ]
    );
}

#[test]
fn an_unopenable_git_entry_is_an_error_not_a_filesystem_fallback() {
    let root = repository(&[("README.md", "# Home\n"), (".git", "gitdir: missing\n")]);

    let error = check_markdown_docs(root.path()).expect_err("broken .git must fail");

    assert!(
        matches!(error, DocsCheckError::OpenRepository { .. }),
        "{error}"
    );
}

#[test]
fn filesystem_fallback_excludes_generated_build_and_vendor_trees() {
    let root = repository(&[
        ("README.md", "# Home\n"),
        ("target/broken.md", "[bad](missing.md)\n"),
        ("build/broken.md", "[bad](missing.md)\n"),
        ("generated/broken.md", "[bad](missing.md)\n"),
        ("vendor/broken.md", "[bad](missing.md)\n"),
    ]);

    let report = report(root.path());

    assert!(report.passed(), "{:?}", report.violations);
    assert_eq!(report.markdown_file_count, 1);
}
