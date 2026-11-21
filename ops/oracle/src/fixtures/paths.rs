use std::io::Read;
use std::path::{Component, Path, PathBuf};

use super::{FIXTURE_ROOT, FixtureError, FixturePathRequirement, RepoRoot};

const FIXTURE_FILE_LIMIT_BYTES: usize = crate::process::OUTPUT_LIMIT_BYTES;

pub(super) fn validate_fixture_root(root: &RepoRoot) -> Result<(), String> {
    crate::safe_file::open_directory(&root.path.join(FIXTURE_ROOT))
        .map(|_| ())
        .map_err(|source| format!("fixture root is not a source-owned directory tree: {source}"))
}

/// Validates `relative` below `fixture_root` by walking descriptors opened relative to the
/// fixture root, never following a symlink, so no component can resolve outside it.
pub(super) fn validate_fixture_path(
    fixture_root: &Path,
    id: &str,
    field: &str,
    relative: &str,
    requirement: FixturePathRequirement,
) -> Result<(), String> {
    let relative_path = Path::new(relative);
    validate_relative_path_components(id, field, relative_path, relative)?;
    validate_existing_fixture_components(fixture_root, id, field, relative_path, requirement)
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
    use rustix::fs::{AtFlags, FileType};

    let shown = relative_path.display();
    let mut directory = std::os::fd::OwnedFd::from(
        crate::safe_file::open_directory(fixture_root)
            .map_err(|source| format!("failed to open fixture root {fixture_root:?}: {source}"))?,
    );
    let mut components = relative_path.components().peekable();
    while let Some(component) = components.next() {
        let Component::Normal(name) = component else {
            return Err(format!("{id} has unsafe {field} {shown}"));
        };
        let is_final_component = components.peek().is_none();
        let file_type = match rustix::fs::statat(&directory, name, AtFlags::SYMLINK_NOFOLLOW) {
            Ok(stat) => FileType::from_raw_mode(stat.st_mode),
            Err(rustix::io::Errno::NOENT) => {
                if requirement == FixturePathRequirement::MustExistFile {
                    return Err(format!("{id} {field} does not exist: {shown}"));
                }
                return Ok(());
            }
            Err(error) => {
                return Err(format!("{id} failed to inspect {field} {shown}: {error}"));
            }
        };
        if file_type == FileType::Symlink {
            return Err(format!("{id} {field} contains symlink: {shown}"));
        }
        if is_final_component {
            if file_type != FileType::RegularFile {
                return Err(format!("{id} {field} is not a file: {shown}"));
            }
        } else if file_type == FileType::Directory {
            directory = crate::safe_file::open_child_directory(&directory, name)
                .map_err(|error| format!("{id} failed to open {field} parent {shown}: {error}"))?;
        } else {
            return Err(format!("{id} {field} has non-directory parent: {shown}"));
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
