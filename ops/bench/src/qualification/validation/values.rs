use super::Issues;
use crate::qualification::model::FixtureLocator;

const MAX_TEXT_BYTES: usize = 4_096;

pub(super) fn validate_identifier(label: &str, value: &str, issues: &mut Issues) {
    if value.is_empty()
        || value.len() > 256
        || !value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':' | b'@')
        })
    {
        issues.push(format!("{label} has invalid identifier {value:?}"));
    }
}

pub(super) fn validate_digest(label: &str, value: &str, issues: &mut Issues) {
    if !is_digest(value) {
        issues.push(format!("{label} has invalid SHA-256 digest {value:?}"));
    }
}

pub(super) fn is_digest(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

pub(super) fn validate_text(label: &str, value: &str, issues: &mut Issues) {
    if value.trim().is_empty() || value.len() > MAX_TEXT_BYTES || value.contains('\0') {
        issues.push(format!("{label} has invalid bounded text"));
    }
}

pub(super) fn validate_relative_path(label: &str, value: &str, issues: &mut Issues) {
    let path = std::path::Path::new(value);
    if path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                std::path::Component::Prefix(_)
                    | std::path::Component::RootDir
                    | std::path::Component::ParentDir
                    | std::path::Component::CurDir
            )
        })
    {
        issues.push(format!("{label} has unsafe path {value:?}"));
    }
}

pub(super) fn validate_fixture_locator(fixture: &FixtureLocator, issues: &mut Issues) {
    match fixture {
        FixtureLocator::RepositoryFile { path, sha256 } => {
            validate_text("qualification fixture path", path, issues);
            validate_relative_path("qualification fixture path", path, issues);
            validate_digest("qualification fixture corpus", sha256, issues);
        }
        FixtureLocator::Generated { id } => {
            validate_text("qualification generated fixture", id, issues);
            validate_identifier("qualification generated fixture", id, issues);
        }
        FixtureLocator::Inline { id } => {
            validate_text("qualification inline fixture", id, issues);
            validate_identifier("qualification inline fixture", id, issues);
        }
    }
}

pub(super) fn filter_matches_any(filter: &str, symbols: &[String]) -> bool {
    filter.split(',').all(|part| {
        symbols
            .iter()
            .any(|symbol| filter_matches_symbol(part, symbol))
    })
}

pub(super) fn filter_selects_symbol(filter: &str, symbol: &str) -> bool {
    filter
        .split(',')
        .any(|part| filter_matches_symbol(part, symbol))
}

fn filter_matches_symbol(filter: &str, symbol: &str) -> bool {
    let filter = filter.trim();
    filter
        .strip_suffix('*')
        .map_or_else(|| symbol == filter, |prefix| symbol.starts_with(prefix))
}
