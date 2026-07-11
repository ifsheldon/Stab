use std::collections::BTreeSet;
use std::io::Read;
use std::path::{Component, Path};

use super::evidence::open_regular_file;
use super::{BlockerCase, CaseStatus, StimSourcePath, UpstreamProvenance, validate_display_text};
use crate::RepoRoot;

const MAX_UPSTREAM_SOURCE_BYTES: u64 = 16 << 20;

pub(super) fn validate_upstream_source(
    root: &RepoRoot,
    case: &BlockerCase,
    tracked_stim_paths: &BTreeSet<StimSourcePath>,
    violations: &mut Vec<String>,
) {
    let relative = &case.upstream.path.0;
    let relative_text = relative.to_string_lossy();
    validate_display_text("upstream path", &relative_text, violations);
    if !is_safe_relative_path(relative) {
        violations.push(format!(
            "case {:?} has unsafe upstream path {:?}",
            case.id, relative
        ));
        return;
    }
    if !tracked_stim_paths.contains(&case.upstream.path) {
        violations.push(format!(
            "case {:?} upstream source is not tracked by pinned Stim: {:?}",
            case.id, relative
        ));
        return;
    }
    let path = root.stim_source().join(relative);
    match std::fs::symlink_metadata(&path) {
        Ok(metadata) if metadata.file_type().is_file() && !metadata.file_type().is_symlink() => {
            let source_name = relative.to_string_lossy();
            match case.upstream.kind {
                UpstreamProvenance::TestFamily => {
                    if !source_name.contains(".test.") && !source_name.contains("_test.py") {
                        violations.push(format!(
                            "case {:?} marks non-test source {:?} as a test family",
                            case.id, relative
                        ));
                    }
                    if case.status != CaseStatus::Planned {
                        violations.push(format!(
                            "case {:?} uses a test-family aggregation after claiming implementation",
                            case.id
                        ));
                    }
                    if case.upstream.anchors.is_empty() || case.upstream.anchors.len() > 16 {
                        violations.push(format!(
                            "case {:?} test family must name 1..=16 exact upstream anchors",
                            case.id
                        ));
                    }
                    let mut anchors = BTreeSet::new();
                    for anchor in &case.upstream.anchors {
                        validate_display_text("upstream family anchor", anchor, violations);
                        if !anchors.insert(anchor) {
                            violations.push(format!(
                                "case {:?} repeats upstream family anchor {:?}",
                                case.id, anchor
                            ));
                        }
                        validate_named_gtest_anchor(case, &path, anchor, violations);
                    }
                }
                UpstreamProvenance::GtestCase => {
                    validate_no_family_anchors(case, violations);
                    validate_gtest_anchor(case, &path, violations);
                }
                UpstreamProvenance::PytestCase => {
                    validate_no_family_anchors(case, violations);
                    validate_pytest_anchor(case, &path, violations);
                }
                UpstreamProvenance::SourceSymbol => {
                    validate_no_family_anchors(case, violations);
                    validate_source_symbol_anchor(case, &path, violations);
                }
            }
        }
        Ok(_) => violations.push(format!(
            "case {:?} upstream source is not a regular non-symlink file at {:?}",
            case.id, path
        )),
        Err(error) => violations.push(format!(
            "case {:?} cannot inspect upstream source {:?}: {error}",
            case.id, path
        )),
    }
}

fn validate_no_family_anchors(case: &BlockerCase, violations: &mut Vec<String>) {
    if !case.upstream.anchors.is_empty() {
        violations.push(format!(
            "case {:?} has family anchors but is not a test-family record",
            case.id
        ));
    }
}

fn read_upstream_anchor_source(
    case: &BlockerCase,
    path: &Path,
    violations: &mut Vec<String>,
) -> Option<String> {
    let file = match open_regular_file(path) {
        Ok(file) => file,
        Err(error) => {
            violations.push(format!(
                "case {:?} cannot read upstream anchor file {:?}: {error}",
                case.id, path
            ));
            return None;
        }
    };
    let mut content = String::new();
    if let Err(error) = file
        .take(MAX_UPSTREAM_SOURCE_BYTES + 1)
        .read_to_string(&mut content)
    {
        violations.push(format!(
            "case {:?} cannot read upstream anchor file {:?}: {error}",
            case.id, path
        ));
        return None;
    }
    if u64::try_from(content.len()).unwrap_or(u64::MAX) > MAX_UPSTREAM_SOURCE_BYTES {
        violations.push(format!(
            "case {:?} upstream anchor file {:?} exceeds the {}-byte limit",
            case.id, path, MAX_UPSTREAM_SOURCE_BYTES
        ));
        return None;
    }
    Some(content)
}

fn validate_gtest_anchor(case: &BlockerCase, path: &Path, violations: &mut Vec<String>) {
    validate_named_gtest_anchor(case, path, &case.upstream.test, violations);
}

fn validate_named_gtest_anchor(
    case: &BlockerCase,
    path: &Path,
    anchor: &str,
    violations: &mut Vec<String>,
) {
    let Some((suite, name)) = anchor.split_once('.') else {
        violations.push(format!(
            "case {:?} gtest anchor {:?} must be Suite.Name",
            case.id, anchor
        ));
        return;
    };
    let Some(content) = read_upstream_anchor_source(case, path, violations) else {
        return;
    };
    let test = format!("TEST({suite}, {name})");
    let word_size_test = format!("TEST_EACH_WORD_SIZE_W({suite}, {name},");
    let anchor_start = content
        .find(&test)
        .or_else(|| content.find(&word_size_test));
    let Some(anchor_start) = anchor_start else {
        violations.push(format!(
            "case {:?} gtest anchor {:?} is absent from {:?}",
            case.id, anchor, path
        ));
        return;
    };
    if case.gate_families.is_empty() {
        return;
    }

    let anchor_body = gtest_anchor_body(&content, anchor_start);
    let uppercase_anchor_body = anchor_body.to_ascii_uppercase();
    for gate in stab_core::Gate::all() {
        let gate_name = gate.canonical_name();
        if gate_name.len() > 1
            && contains_identifier(&case.upstream.subcase, gate_name)
            && !uppercase_anchor_body.contains(gate_name)
        {
            violations.push(format!(
                "case {:?} upstream subcase names gate {gate_name}, but gtest anchor {:?} does not contain it",
                case.id, anchor
            ));
        }
    }
}

fn gtest_anchor_body(content: &str, anchor_start: usize) -> &str {
    let remainder = content.get(anchor_start..).unwrap_or(content);
    let next_test = ["\nTEST(", "\nTEST_EACH_WORD_SIZE_W("]
        .into_iter()
        .filter_map(|marker| remainder.get(1..)?.find(marker).map(|index| index + 1))
        .min()
        .unwrap_or(remainder.len());
    remainder.get(..next_test).unwrap_or(remainder)
}

fn contains_identifier(text: &str, identifier: &str) -> bool {
    text.match_indices(identifier).any(|(start, matched)| {
        let end = start + matched.len();
        let valid_start = start == 0
            || text
                .as_bytes()
                .get(start - 1)
                .is_some_and(|byte| !byte.is_ascii_alphanumeric() && *byte != b'_');
        let valid_end = end == text.len()
            || text
                .as_bytes()
                .get(end)
                .is_some_and(|byte| !byte.is_ascii_alphanumeric() && *byte != b'_');
        valid_start && valid_end
    })
}

fn validate_pytest_anchor(case: &BlockerCase, path: &Path, violations: &mut Vec<String>) {
    let Some(content) = read_upstream_anchor_source(case, path, violations) else {
        return;
    };
    let function = format!("def {}(", case.upstream.test);
    if !case.upstream.test.starts_with("test_") || !content.contains(&function) {
        violations.push(format!(
            "case {:?} pytest anchor {:?} is absent from {:?}",
            case.id, case.upstream.test, path
        ));
    }
}

fn validate_source_symbol_anchor(case: &BlockerCase, path: &Path, violations: &mut Vec<String>) {
    let Some(content) = read_upstream_anchor_source(case, path, violations) else {
        return;
    };
    if !content.contains(&case.upstream.test) {
        violations.push(format!(
            "case {:?} source symbol {:?} is absent from {:?}",
            case.id, case.upstream.test, path
        ));
    }
}

fn is_safe_relative_path(path: &Path) -> bool {
    !path.as_os_str().is_empty()
        && !path.is_absolute()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}
