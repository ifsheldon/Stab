use std::path::{Component, Path, PathBuf};

use super::{FIXTURE_ROOT, FixtureError, FixturePathRequirement, RepoRoot};

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
    let fixture_root = root.path.join(FIXTURE_ROOT);
    validate_fixture_path(
        &fixture_root,
        "fixture",
        "path",
        relative,
        FixturePathRequirement::ExistingFileIfPresent,
    )
    .map_err(|violation| FixtureError::Validation(violation.into_boxed_str()))?;
    let path = fixture_root.join(relative);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| FixtureError::CreateOutputDir {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    validate_fixture_path(
        &fixture_root,
        "fixture",
        "path",
        relative,
        FixturePathRequirement::ExistingFileIfPresent,
    )
    .map_err(|violation| FixtureError::Validation(violation.into_boxed_str()))
}

pub(super) fn fixture_file(root: &RepoRoot, relative: &str) -> Result<PathBuf, FixtureError> {
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

pub(super) fn unsafe_component(component: Component<'_>) -> bool {
    matches!(
        component,
        Component::Prefix(_) | Component::RootDir | Component::ParentDir | Component::CurDir
    )
}
