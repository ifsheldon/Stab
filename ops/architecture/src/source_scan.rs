use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use syn::{Item, UseTree, Visibility};

use crate::{CheckError, MigrationAllowance, PackageSpec, Violation};

const SIMD_PACKAGE: &str = "stab-kernels-simd";
const FACADE_ROOT_REEXPORTS: &str = "ops/architecture/facade-root-reexports.txt";
const FACADE_ROOT_MODULES: [&str; 4] = ["advanced", "analysis", "execution", "experimental"];

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

    validate_facade_tiers(root, &mut violations)?;

    for source_path in &rust_sources {
        let source = std::fs::read_to_string(root.join(source_path)).map_err(|source| {
            CheckError::ReadSource {
                path: source_path.clone(),
                source,
            }
        })?;
        if !contains_portable_simd_site(&source) {
            continue;
        }
        let package = package_for_source(source_path, packages);
        match classify_simd_site(source_path, package) {
            SimdSite::Kernel => {}
            SimdSite::Forbidden => {
                violations.push(Violation::new(
                    "portable-simd-outside-kernel",
                    format!(
                        "portable-SIMD source in {} must move to {}",
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

fn validate_facade_tiers(root: &Path, violations: &mut Vec<Violation>) -> Result<(), CheckError> {
    let facade_root = Path::new("crates/stab-core/src/lib.rs");
    let facade_advanced = Path::new("crates/stab-core/src/advanced.rs");
    let root_reexports = Path::new(FACADE_ROOT_REEXPORTS);
    let root_source = read_source(root, facade_root)?;
    let advanced_source = read_source(root, facade_advanced)?;
    let root_reexport_inventory = read_source(root, root_reexports)?;
    violations.extend(facade_tier_violations(
        facade_root,
        &root_source,
        facade_advanced,
        &advanced_source,
        root_reexports,
        &root_reexport_inventory,
    ));
    Ok(())
}

fn facade_tier_violations(
    facade_root: &Path,
    root_source: &str,
    facade_advanced: &Path,
    advanced_source: &str,
    root_reexports: &Path,
    root_reexport_inventory: &str,
) -> Vec<Violation> {
    let mut violations = Vec::new();
    let root_surface = parse_facade_surface(facade_root, root_source, &mut violations);
    let expected_root_modules = FACADE_ROOT_MODULES
        .into_iter()
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();
    for missing in expected_root_modules.difference(&root_surface.modules) {
        violations.push(Violation::new(
            "facade-tier-missing",
            format!(
                "{} must publicly declare module `{missing}`",
                facade_root.display()
            ),
        ));
    }
    for unexpected in root_surface.modules.difference(&expected_root_modules) {
        violations.push(Violation::new(
            "facade-root-module-unassigned",
            format!(
                "{} publicly declares unassigned root module `{unexpected}`",
                facade_root.display()
            ),
        ));
    }

    let expected_root_reexports =
        parse_root_reexport_inventory(root_reexports, root_reexport_inventory, &mut violations);
    for missing in expected_root_reexports.difference(&root_surface.reexports) {
        violations.push(Violation::new(
            "facade-root-reexport-missing",
            format!(
                "{} assigns `{missing}` to the facade root, but {} does not reexport it",
                root_reexports.display(),
                facade_root.display()
            ),
        ));
    }
    for unexpected in root_surface.reexports.difference(&expected_root_reexports) {
        violations.push(Violation::new(
            "facade-root-reexport-unassigned",
            format!(
                "{} publicly reexports unassigned root item `{unexpected}`; assign it in {} or move it under `advanced`",
                facade_root.display(),
                root_reexports.display()
            ),
        ));
    }

    let advanced_surface = parse_facade_surface(facade_advanced, advanced_source, &mut violations);
    let required_advanced_modules = [
        "storage",
        "algebra",
        "records",
        "backend",
        "traversal",
        "compat",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect::<BTreeSet<_>>();
    for required in required_advanced_modules.difference(&advanced_surface.modules) {
        violations.push(Violation::new(
            "facade-advanced-module-missing",
            format!(
                "{} must publicly declare module `{required}`",
                facade_advanced.display()
            ),
        ));
    }
    for unexpected in advanced_surface
        .modules
        .difference(&required_advanced_modules)
    {
        violations.push(Violation::new(
            "facade-advanced-module-unassigned",
            format!(
                "{} publicly declares unassigned advanced module `{unexpected}`",
                facade_advanced.display()
            ),
        ));
    }

    violations
}

#[derive(Default)]
struct FacadeSurface {
    modules: BTreeSet<String>,
    reexports: BTreeSet<String>,
}

fn parse_facade_surface(
    path: &Path,
    source: &str,
    violations: &mut Vec<Violation>,
) -> FacadeSurface {
    let syntax = match syn::parse_file(source) {
        Ok(syntax) => syntax,
        Err(error) => {
            violations.push(Violation::new(
                "facade-source-parse",
                format!(
                    "failed to parse {} while checking facade tiers: {error}",
                    path.display()
                ),
            ));
            return FacadeSurface::default();
        }
    };

    let mut surface = FacadeSurface::default();
    for item in syntax.items {
        match item {
            Item::Mod(item) if is_public(&item.vis) => {
                surface.modules.insert(item.ident.to_string());
            }
            Item::Use(item) if is_public(&item.vis) => {
                collect_public_use_names(path, &item.tree, None, &mut surface, violations);
            }
            Item::Const(item) if is_public(&item.vis) => {
                report_direct_root_item(path, "constant", &item.ident, violations);
            }
            Item::Enum(item) if is_public(&item.vis) => {
                report_direct_root_item(path, "enum", &item.ident, violations);
            }
            Item::ExternCrate(item) if is_public(&item.vis) => {
                report_direct_root_item(path, "extern crate", &item.ident, violations);
            }
            Item::Fn(item) if is_public(&item.vis) => {
                report_direct_root_item(path, "function", &item.sig.ident, violations);
            }
            Item::Static(item) if is_public(&item.vis) => {
                report_direct_root_item(path, "static", &item.ident, violations);
            }
            Item::Struct(item) if is_public(&item.vis) => {
                report_direct_root_item(path, "struct", &item.ident, violations);
            }
            Item::Trait(item) if is_public(&item.vis) => {
                report_direct_root_item(path, "trait", &item.ident, violations);
            }
            Item::TraitAlias(item) if is_public(&item.vis) => {
                report_direct_root_item(path, "trait alias", &item.ident, violations);
            }
            Item::Type(item) if is_public(&item.vis) => {
                report_direct_root_item(path, "type alias", &item.ident, violations);
            }
            Item::Union(item) if is_public(&item.vis) => {
                report_direct_root_item(path, "union", &item.ident, violations);
            }
            Item::Macro(item) if has_macro_export(&item.attrs) => {
                violations.push(Violation::new(
                    "facade-root-direct-item",
                    format!(
                        "{} defines a root-exported macro; facade root items must be explicit reexports",
                        path.display()
                    ),
                ));
            }
            _ => {}
        }
    }

    surface
}

fn is_public(visibility: &Visibility) -> bool {
    matches!(visibility, Visibility::Public(_))
}

fn has_macro_export(attributes: &[syn::Attribute]) -> bool {
    attributes
        .iter()
        .any(|attribute| attribute.path().is_ident("macro_export"))
}

fn report_direct_root_item(
    path: &Path,
    kind: &str,
    name: &syn::Ident,
    violations: &mut Vec<Violation>,
) {
    violations.push(Violation::new(
        "facade-root-direct-item",
        format!(
            "{} defines public {kind} `{name}`; facade root items must be explicit reexports",
            path.display()
        ),
    ));
}

fn collect_public_use_names(
    path: &Path,
    tree: &UseTree,
    parent: Option<&syn::Ident>,
    surface: &mut FacadeSurface,
    violations: &mut Vec<Violation>,
) {
    match tree {
        UseTree::Path(path_tree) => collect_public_use_names(
            path,
            &path_tree.tree,
            Some(&path_tree.ident),
            surface,
            violations,
        ),
        UseTree::Name(name) => {
            let exported = if name.ident == "self" {
                parent.unwrap_or(&name.ident)
            } else {
                &name.ident
            };
            surface.reexports.insert(exported.to_string());
        }
        UseTree::Rename(rename) => {
            if rename.rename == "_" {
                violations.push(Violation::new(
                    "facade-root-anonymous-reexport",
                    format!(
                        "{} contains anonymous public reexport `{}` as `_`",
                        path.display(),
                        rename.ident
                    ),
                ));
            } else {
                surface.reexports.insert(rename.rename.to_string());
            }
        }
        UseTree::Glob(_) => {
            violations.push(Violation::new(
                "facade-root-glob-reexport",
                format!(
                    "{} contains a public glob reexport; facade tiers require explicit item names",
                    path.display()
                ),
            ));
        }
        UseTree::Group(group) => {
            for item in &group.items {
                collect_public_use_names(path, item, parent, surface, violations);
            }
        }
    }
}

fn parse_root_reexport_inventory(
    path: &Path,
    source: &str,
    violations: &mut Vec<Violation>,
) -> BTreeSet<String> {
    let mut entries = BTreeSet::new();
    for (line_index, line) in source.lines().enumerate() {
        let entry = line.trim();
        if entry.is_empty() || entry.starts_with('#') {
            continue;
        }
        if syn::parse_str::<syn::Ident>(entry).is_err() {
            violations.push(Violation::new(
                "facade-root-inventory-invalid",
                format!(
                    "{}:{} contains invalid Rust identifier `{entry}`",
                    path.display(),
                    line_index + 1
                ),
            ));
            continue;
        }
        if !entries.insert(entry.to_owned()) {
            violations.push(Violation::new(
                "facade-root-inventory-duplicate",
                format!(
                    "{}:{} repeats root item `{entry}`",
                    path.display(),
                    line_index + 1
                ),
            ));
        }
    }
    entries
}

#[cfg(test)]
fn public_module_names(source: &str) -> BTreeSet<String> {
    let mut violations = Vec::new();
    let surface = parse_facade_surface(Path::new("fixture.rs"), source, &mut violations);
    assert!(violations.is_empty(), "{violations:?}");
    surface.modules
}

fn read_source(root: &Path, relative_path: &Path) -> Result<String, CheckError> {
    std::fs::read_to_string(root.join(relative_path)).map_err(|source| CheckError::ReadSource {
        path: relative_path.to_path_buf(),
        source,
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

fn contains_portable_simd_site(source: &str) -> bool {
    let compact = source
        .chars()
        .filter(|character| !character.is_ascii_whitespace())
        .collect::<String>();
    contains_rooted_simd(&compact, "std")
        || contains_rooted_simd(&compact, "core")
        || compact.contains(&["#![feature(", "portable", "_simd", ")]"].concat())
}

fn contains_rooted_simd(compact: &str, root: &str) -> bool {
    if compact.contains(&format!("{root}::simd")) {
        return true;
    }

    let grouped_prefix = format!("{root}::{{");
    let mut remainder = compact;
    while let Some(prefix_index) = remainder.find(&grouped_prefix) {
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

    fn direct_import(root: &str) -> String {
        ["use ", root, "::", "simd", "::Simd;"].concat()
    }

    fn grouped_import(root: &str) -> String {
        ["use ", root, "::{mem, ", "simd", "::{Simd}};"].concat()
    }

    #[test]
    fn finds_direct_grouped_and_feature_gated_portable_simd() {
        assert!(contains_portable_simd_site(&direct_import("std")));
        assert!(contains_portable_simd_site(&grouped_import("std")));
        assert!(contains_portable_simd_site(&direct_import("core")));
        assert!(contains_portable_simd_site(&grouped_import("core")));
        assert!(contains_portable_simd_site(
            &["#![feature(", "portable", "_simd", ")]"].concat()
        ));
    }

    #[test]
    fn does_not_confuse_simd_like_identifiers_with_std_simd() {
        let unrelated = ["use crate::simd; fn f() { let std_simd = 1; }"].concat();
        assert!(!contains_portable_simd_site(&unrelated));
    }

    #[test]
    fn kernel_package_owns_direct_simd() {
        let package = PackageSpec {
            name: SIMD_PACKAGE.to_owned(),
            relative_path: PathBuf::from("crates/stab-kernels-simd"),
            default_features: Vec::new(),
            version: cargo_metadata::semver::Version::new(0, 2, 0),
            publish: None,
            binary_targets: Vec::new(),
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
            default_features: Vec::new(),
            version: cargo_metadata::semver::Version::new(0, 2, 0),
            publish: None,
            binary_targets: Vec::new(),
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

    #[test]
    fn facade_tier_contract_reports_missing_and_owner_shaped_surfaces() {
        let violations = facade_tier_violations(
            Path::new("lib.rs"),
            "pub mod advanced;\npub mod analysis;\npub mod bits;\npub mod execution;\npub mod experimental;\n",
            Path::new("advanced.rs"),
            "pub mod algebra {}\npub mod backend {}\npub mod compat {}\npub mod records {}\npub mod storage {}\npub mod traversal {}\n",
            Path::new("root-reexports.txt"),
            "",
        );

        let [violation] = violations.as_slice() else {
            panic!("expected one unassigned module violation: {violations:?}");
        };
        assert_eq!(violation.code, "facade-root-module-unassigned");
        assert!(violation.message.contains("`bits`"));
    }

    #[test]
    fn facade_tier_parser_ignores_comments_strings_and_restricted_modules() {
        let modules = public_module_names(
            "pub mod advanced;\n// pub mod bits;\nconst TEXT: &str = \"pub mod stabilizers;\";\npub(crate) mod result_formats;\npub mod experimental {}\n",
        );
        assert_eq!(
            modules,
            ["advanced", "experimental"]
                .into_iter()
                .map(str::to_owned)
                .collect()
        );
    }

    #[test]
    fn facade_tier_contract_parses_grouped_multiline_and_aliased_reexports() {
        let violations = facade_tier_violations(
            Path::new("lib.rs"),
            "pub mod advanced;\npub mod analysis;\npub mod execution;\npub mod experimental;\npub use crate::circuit::Circuit;\npub use stab_bits::{\n    BitVec,\n    BitMatrix as Matrix,\n};\n",
            Path::new("advanced.rs"),
            "pub mod algebra {}\npub mod backend {}\npub mod compat {}\npub mod records {}\npub mod storage {}\npub mod traversal {}\n",
            Path::new("root-reexports.txt"),
            "Circuit\n",
        );
        let unexpected = violations
            .iter()
            .filter(|violation| violation.code == "facade-root-reexport-unassigned")
            .map(|violation| violation.message.as_str())
            .collect::<Vec<_>>();

        assert_eq!(unexpected.len(), 2);
        assert!(
            unexpected
                .iter()
                .any(|message| message.contains("`BitVec`"))
        );
        assert!(
            unexpected
                .iter()
                .any(|message| message.contains("`Matrix`"))
        );
    }

    #[test]
    fn facade_tier_contract_rejects_globs_and_missing_assignments() {
        let violations = facade_tier_violations(
            Path::new("lib.rs"),
            "pub mod advanced;\npub mod analysis;\npub mod execution;\npub mod experimental;\npub use crate::circuit::*;\n",
            Path::new("advanced.rs"),
            "pub mod algebra {}\npub mod backend {}\npub mod compat {}\npub mod records {}\npub mod storage {}\npub mod traversal {}\n",
            Path::new("root-reexports.txt"),
            "Circuit\n",
        );
        let codes = violations
            .iter()
            .map(|violation| violation.code)
            .collect::<BTreeSet<_>>();

        assert_eq!(
            codes,
            BTreeSet::from(["facade-root-glob-reexport", "facade-root-reexport-missing"])
        );
    }

    #[test]
    fn facade_tier_inventory_rejects_invalid_and_duplicate_entries() {
        let violations = facade_tier_violations(
            Path::new("lib.rs"),
            "pub mod advanced;\npub mod analysis;\npub mod execution;\npub mod experimental;\npub use crate::circuit::Circuit;\n",
            Path::new("advanced.rs"),
            "pub mod algebra {}\npub mod backend {}\npub mod compat {}\npub mod records {}\npub mod storage {}\npub mod traversal {}\n",
            Path::new("root-reexports.txt"),
            "Circuit\nCircuit\nnot-valid\n",
        );
        let codes = violations
            .iter()
            .map(|violation| violation.code)
            .collect::<BTreeSet<_>>();

        assert_eq!(
            codes,
            BTreeSet::from([
                "facade-root-inventory-duplicate",
                "facade-root-inventory-invalid"
            ])
        );
    }
}
