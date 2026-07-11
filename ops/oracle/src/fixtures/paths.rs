use std::io::Read;
use std::path::{Component, Path, PathBuf};

use super::{FIXTURE_ROOT, FixtureError, FixturePathRequirement, RepoRoot};

const FIXTURE_FILE_LIMIT_BYTES: usize = crate::process::OUTPUT_LIMIT_BYTES;

pub(super) fn validate_fixture_root(root: &RepoRoot) -> Result<(), String> {
    crate::safe_file::open_directory(&root.path.join(FIXTURE_ROOT))
        .map(|_| ())
        .map_err(|source| format!("fixture root is not a source-owned directory tree: {source}"))
}

pub(super) fn validate_fixture_path(
    fixture_root: &Path,
    id: &str,
    field: &str,
    relative: &str,
    requirement: FixturePathRequirement,
) -> Result<(), String> {
    let relative_path = Path::new(relative);
    validate_relative_path_components(id, field, relative_path, relative)?;
    let canonical_root = fixture_root
        .canonicalize()
        .map_err(|source| format!("failed to resolve fixture root {fixture_root:?}: {source}"))?;
    validate_existing_fixture_components(&canonical_root, id, field, relative_path, requirement)?;
    let full_path = canonical_root.join(relative_path);
    if let Ok(canonical_path) = full_path.canonicalize()
        && !canonical_path.starts_with(&canonical_root)
    {
        return Err(format!("{id} {field} escapes fixture root: {relative}"));
    }
    if let Some(parent) = full_path.parent()
        && let Ok(canonical_parent) = parent.canonicalize()
        && !canonical_parent.starts_with(&canonical_root)
    {
        return Err(format!(
            "{id} {field} parent escapes fixture root: {relative}"
        ));
    }
    Ok(())
}

fn validate_relative_path_components(
    id: &str,
    field: &str,
    relative_path: &Path,
    relative: &str,
) -> Result<(), String> {
    let mut has_component = false;
    for component in relative_path.components() {
        has_component = true;
        if unsafe_component(component) {
            return Err(format!("{id} has unsafe {field} {relative}"));
        }
    }
    if has_component {
        Ok(())
    } else {
        Err(format!("{id} has empty {field}"))
    }
}

fn validate_existing_fixture_components(
    fixture_root: &Path,
    id: &str,
    field: &str,
    relative_path: &Path,
    requirement: FixturePathRequirement,
) -> Result<(), String> {
    let mut current = fixture_root.to_path_buf();
    let mut components = relative_path.components().peekable();
    while let Some(component) = components.next() {
        let Component::Normal(name) = component else {
            return Err(format!(
                "{id} has unsafe {field} {}",
                relative_path.display()
            ));
        };
        current.push(name);
        let is_final_component = components.peek().is_none();
        match std::fs::symlink_metadata(&current) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() {
                    return Err(format!(
                        "{id} {field} contains symlink: {}",
                        relative_path.display()
                    ));
                }
                if !is_final_component && !metadata.is_dir() {
                    return Err(format!(
                        "{id} {field} has non-directory parent: {}",
                        relative_path.display()
                    ));
                }
                if is_final_component && !metadata.is_file() {
                    return Err(format!(
                        "{id} {field} is not a file: {}",
                        relative_path.display()
                    ));
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                if requirement == FixturePathRequirement::MustExistFile {
                    return Err(format!(
                        "{id} {field} does not exist: {}",
                        relative_path.display()
                    ));
                }
                break;
            }
            Err(error) => {
                return Err(format!(
                    "{id} failed to inspect {field} {}: {error}",
                    relative_path.display()
                ));
            }
        }
    }
    Ok(())
}

pub(super) fn prepare_fixture_output_file(
    root: &RepoRoot,
    relative: &str,
) -> Result<(), FixtureError> {
    validate_fixture_root(root)
        .map_err(|violation| FixtureError::Validation(violation.into_boxed_str()))?;
    let fixture_root = root.path.join(FIXTURE_ROOT);
    validate_fixture_path(
        &fixture_root,
        "fixture",
        "path",
        relative,
        FixturePathRequirement::ExistingFileIfPresent,
    )
    .map_err(|violation| FixtureError::Validation(violation.into_boxed_str()))?;
    Ok(())
}

pub(super) fn fixture_file(root: &RepoRoot, relative: &str) -> Result<PathBuf, FixtureError> {
    validate_fixture_root(root)
        .map_err(|violation| FixtureError::Validation(violation.into_boxed_str()))?;
    let fixture_root = root.path.join(FIXTURE_ROOT);
    validate_fixture_path(
        &fixture_root,
        "fixture",
        "path",
        relative,
        FixturePathRequirement::ExistingFileIfPresent,
    )
    .map_err(|violation| FixtureError::Validation(violation.into_boxed_str()))?;
    Ok(fixture_root.join(relative))
}

pub(super) fn read_fixture_file(root: &RepoRoot, relative: &str) -> Result<Vec<u8>, FixtureError> {
    let path = fixture_file(root, relative)?;
    let mut file =
        crate::safe_file::open_regular_file(&path).map_err(|source| FixtureError::ReadFile {
            path: path.clone(),
            source: std::io::Error::other(source),
        })?;
    let mut bytes = Vec::new();
    Read::by_ref(&mut file)
        .take(u64::try_from(FIXTURE_FILE_LIMIT_BYTES).unwrap_or(u64::MAX) + 1)
        .read_to_end(&mut bytes)
        .map_err(|source| FixtureError::ReadFile {
            path: path.clone(),
            source,
        })?;
    if bytes.len() > FIXTURE_FILE_LIMIT_BYTES {
        return Err(FixtureError::FixtureFileTooLarge {
            path,
            limit: FIXTURE_FILE_LIMIT_BYTES,
        });
    }
    Ok(bytes)
}

pub(super) fn write_fixture_file(
    root: &RepoRoot,
    relative: &str,
    bytes: &[u8],
) -> Result<(), FixtureError> {
    if bytes.len() > FIXTURE_FILE_LIMIT_BYTES {
        return Err(FixtureError::FixtureFileTooLarge {
            path: root.path.join(FIXTURE_ROOT).join(relative),
            limit: FIXTURE_FILE_LIMIT_BYTES,
        });
    }
    prepare_fixture_output_file(root, relative)?;
    let path = root.path.join(FIXTURE_ROOT).join(relative);
    crate::safe_file::atomic_write_regular_file(&path, bytes).map_err(|source| {
        FixtureError::WriteOutput {
            path: path.clone(),
            source: std::io::Error::other(source),
        }
    })
}

pub(super) fn unsafe_component(component: Component<'_>) -> bool {
    matches!(
        component,
        Component::Prefix(_) | Component::RootDir | Component::ParentDir | Component::CurDir
    )
}
