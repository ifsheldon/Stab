use std::path::{Path, PathBuf};

use crate::{CheckError, MigrationAllowance, PackageSpec, Violation};

const SIMD_PACKAGE: &str = "stab-kernels-simd";

pub(super) struct SourceReport {
    pub rust_source_count: usize,
    pub violations: Vec<Violation>,
    pub migration_allowances: Vec<MigrationAllowance>,
}

pub(super) fn scan_workspace_sources(
    root: &Path,
    packages: &[PackageSpec],
) -> Result<SourceReport, CheckError> {
    let mut rust_sources = Vec::new();
    let mut violations = Vec::new();
    for package in packages {
        collect_rust_sources(
            root,
            &root.join(&package.relative_path),
            &mut rust_sources,
            &mut violations,
        )?;
    }
    rust_sources.sort();
    rust_sources.dedup();

    for source_path in &rust_sources {
        let source = std::fs::read_to_string(root.join(source_path)).map_err(|source| {
            CheckError::ReadSource {
                path: source_path.clone(),
                source,
            }
        })?;
        if !contains_direct_std_simd(&source) {
            continue;
        }
        let package = package_for_source(source_path, packages);
        match classify_simd_site(source_path, package) {
            SimdSite::Kernel => {}
            SimdSite::Forbidden => {
                violations.push(Violation::new(
                    "direct-std-simd-outside-kernel",
                    format!(
                        "direct portable-SIMD use in {} must move to {}",
                        source_path.display(),
                        SIMD_PACKAGE
                    ),
                ));
            }
        }
    }

    Ok(SourceReport {
        rust_source_count: rust_sources.len(),
        violations,
        migration_allowances: Vec::new(),
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SimdSite {
    Kernel,
    Forbidden,
}

fn classify_simd_site(_source_path: &Path, package: Option<&PackageSpec>) -> SimdSite {
    if package.is_some_and(|package| package.name == SIMD_PACKAGE) {
        return SimdSite::Kernel;
    }
    SimdSite::Forbidden
}

fn collect_rust_sources(
    root: &Path,
    directory: &Path,
    output: &mut Vec<PathBuf>,
    violations: &mut Vec<Violation>,
) -> Result<(), CheckError> {
    let mut entries = std::fs::read_dir(directory)
        .map_err(|source| CheckError::InspectSource {
            path: directory.to_path_buf(),
            source,
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|source| CheckError::InspectSource {
            path: directory.to_path_buf(),
            source,
        })?;
    entries.sort_by_key(std::fs::DirEntry::file_name);

    for entry in entries {
        let path = entry.path();
        let metadata =
            std::fs::symlink_metadata(&path).map_err(|source| CheckError::InspectSource {
                path: path.clone(),
                source,
            })?;
        if metadata.file_type().is_symlink() {
            violations.push(Violation::new(
                "workspace-source-symlink",
                format!(
                    "workspace package source tree contains symlink {}",
                    relative_to_root(root, &path).display()
                ),
            ));
            continue;
        }
        if metadata.is_dir() {
            collect_rust_sources(root, &path, output, violations)?;
        } else if metadata.is_file() && path.extension().is_some_and(|extension| extension == "rs")
        {
            output.push(relative_to_root(root, &path));
        }
    }
    Ok(())
}

fn relative_to_root(root: &Path, path: &Path) -> PathBuf {
    path.strip_prefix(root)
        .map_or_else(|_| path.to_path_buf(), Path::to_path_buf)
}

fn package_for_source<'a>(
    source_path: &Path,
    packages: &'a [PackageSpec],
) -> Option<&'a PackageSpec> {
    packages
        .iter()
        .filter(|package| source_path.starts_with(&package.relative_path))
        .max_by_key(|package| package.relative_path.components().count())
}

fn contains_direct_std_simd(source: &str) -> bool {
    let compact = source
        .chars()
        .filter(|character| !character.is_ascii_whitespace())
        .collect::<String>();
    if compact.contains(concat!("std", "::", "simd")) {
        return true;
    }

    let grouped_prefix = concat!("std", "::{");
    let mut remainder = compact.as_str();
    while let Some(prefix_index) = remainder.find(grouped_prefix) {
        let Some(group) = remainder.get(prefix_index + grouped_prefix.len()..) else {
            return false;
        };
        let group_end = group.find(';').unwrap_or(group.len());
        let Some(group_contents) = group.get(..group_end) else {
            return false;
        };
        if group_contents
            .split(|character: char| !character.is_ascii_alphanumeric() && character != '_')
            .any(|identifier| identifier == "simd")
        {
            return true;
        }
        let Some(next_remainder) = group.get(group_end..) else {
            return false;
        };
        remainder = next_remainder;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn direct_import() -> String {
        ["use ", "std", "::", "simd", "::Simd;"].concat()
    }

    fn grouped_import() -> String {
        ["use ", "std", "::{mem, ", "simd", "::{Simd}};"].concat()
    }

    #[test]
    fn finds_direct_and_grouped_std_simd_paths() {
        assert!(contains_direct_std_simd(&direct_import()));
        assert!(contains_direct_std_simd(&grouped_import()));
    }

    #[test]
    fn does_not_confuse_simd_like_identifiers_with_std_simd() {
        let unrelated = ["use crate::simd; fn f() { let std_simd = 1; }"].concat();
        assert!(!contains_direct_std_simd(&unrelated));
    }

    #[test]
    fn kernel_package_owns_direct_simd() {
        let package = PackageSpec {
            name: SIMD_PACKAGE.to_owned(),
            relative_path: PathBuf::from("crates/stab-kernels-simd"),
        };
        let packages = [package.clone()];
        let source = Path::new("crates/stab-kernels-simd/src/lib.rs");
        assert_eq!(
            package_for_source(source, &packages).map(|package| package.name.as_str()),
            Some(SIMD_PACKAGE)
        );
        assert_eq!(classify_simd_site(source, Some(&package)), SimdSite::Kernel);
    }

    #[test]
    fn former_legacy_simd_paths_are_forbidden() {
        let core = PackageSpec {
            name: "stab-core".to_owned(),
            relative_path: PathBuf::from("crates/stab-core"),
        };
        assert_eq!(
            classify_simd_site(Path::new("crates/stab-core/src/bits/simd.rs"), Some(&core)),
            SimdSite::Forbidden
        );
        assert_eq!(
            classify_simd_site(
                Path::new("crates/stab-core/src/bits/new_kernel.rs"),
                Some(&core)
            ),
            SimdSite::Forbidden
        );
    }
}
