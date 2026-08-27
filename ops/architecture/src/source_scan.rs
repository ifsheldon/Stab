use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use syn::{Item, UseTree, Visibility};

use crate::policy::is_stable_source_package;
use crate::{CheckError, MigrationAllowance, PackageSpec, Violation};

const SIMD_PACKAGE: &str = "stab-kernels-simd";
const FACADE_ROOT_REEXPORTS: &str = "ops/architecture/facade-root-reexports.txt";
const FACADE_COMPONENT_CRATES: &[&str] = &[
    "stab_algebra",
    "stab_analysis",
    "stab_decoder",
    "stab_engine",
    "stab_model",
    "stab_records",
];

mod rust_source;

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

    validate_facade_surface(root, &mut violations)?;

    for source_path in &rust_sources {
        let source = std::fs::read_to_string(root.join(source_path)).map_err(|source| {
            CheckError::ReadSource {
                path: source_path.clone(),
                source,
            }
        })?;
        let facts = match rust_source::inspect(&source) {
            Ok(facts) => facts,
            Err(error) => {
                violations.push(Violation::new(
                    "architecture-source-parse",
                    format!(
                        "failed to parse {} while checking source-boundary contracts: {error}",
                        source_path.display()
                    ),
                ));
                continue;
            }
        };
        let package = package_for_source(source_path, packages);
        if package.is_some_and(|package| is_stable_source_package(&package.name))
            && !facts.feature_gates.is_empty()
        {
            violations.push(Violation::new(
                "stable-component-feature-gate",
                format!(
                    "Stable component source {} enables unstable Rust features {:?}",
                    source_path.display(),
                    facts.feature_gates
                ),
            ));
        }
        if package.is_some_and(|package| package.name == "stab-core") && facts.has_macro_export {
            violations.push(Violation::new(
                "facade-exported-macro",
                format!(
                    "{} exports a macro from stab-core; facade exports must remain inventory-owned Rust items",
                    source_path.display()
                ),
            ));
        }
        if facts.contains_portable_simd {
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
    }

    Ok(SourceReport {
        rust_source_count: rust_sources.len(),
        violations,
        migration_allowances: Vec::new(),
    })
}

fn validate_facade_surface(root: &Path, violations: &mut Vec<Violation>) -> Result<(), CheckError> {
    let facade_root = Path::new("crates/stab-core/src/lib.rs");
    let root_reexports = Path::new(FACADE_ROOT_REEXPORTS);
    let root_source = read_source(root, facade_root)?;
    let root_reexport_inventory = read_source(root, root_reexports)?;
    violations.extend(facade_surface_violations(
        FacadeSource::new(facade_root, &root_source),
        FacadeSource::new(root_reexports, &root_reexport_inventory),
    ));
    Ok(())
}

#[derive(Clone, Copy)]
struct FacadeSource<'a> {
    path: &'a Path,
    source: &'a str,
}

impl<'a> FacadeSource<'a> {
    fn new(path: &'a Path, source: &'a str) -> Self {
        Self { path, source }
    }
}

fn facade_surface_violations(
    root: FacadeSource<'_>,
    root_reexports: FacadeSource<'_>,
) -> Vec<Violation> {
    let mut violations = Vec::new();
    let root_surface = parse_facade_surface(root.path, root.source, &mut violations);
    let expected_root_reexports =
        parse_root_reexport_inventory(root_reexports.path, root_reexports.source, &mut violations);
    for (exported, expected_source) in &expected_root_reexports {
        match root_surface.reexports.get(exported) {
            Some(actual_source) if actual_source == expected_source => {}
            Some(actual_source) => violations.push(Violation::new(
                "facade-root-reexport-wrong-owner",
                format!(
                    "{} requires root item `{exported}` to reexport `{expected_source}`, but {} reexports `{actual_source}`",
                    root_reexports.path.display(),
                    root.path.display()
                ),
            )),
            None => violations.push(Violation::new(
                "facade-root-reexport-missing",
                format!(
                    "{} assigns `{exported}` from `{expected_source}` to the facade root, but {} does not reexport it",
                    root_reexports.path.display(),
                    root.path.display()
                ),
            )),
        }
    }
    for (unexpected, source) in &root_surface.reexports {
        if !expected_root_reexports.contains_key(unexpected) {
            violations.push(Violation::new(
                "facade-root-reexport-unassigned",
                format!(
                    "{} publicly reexports unassigned root item `{unexpected}` from `{source}`; assign its canonical owner in {} or use a component crate directly",
                    root.path.display(),
                    root_reexports.path.display()
                ),
            ));
        }
    }

    violations
}

#[derive(Default)]
struct FacadeSurface {
    reexports: BTreeMap<String, String>,
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
                    "failed to parse {} while checking the facade surface: {error}",
                    path.display()
                ),
            ));
            return FacadeSurface::default();
        }
    };

    for attribute in &syntax.attrs {
        if !attribute.path().is_ident("doc") {
            violations.push(Violation::new(
                "facade-root-crate-attribute",
                format!(
                    "{} attaches a non-documentation crate attribute to the facade root; its export surface must remain unconditional and source-auditable",
                    path.display()
                ),
            ));
        }
    }

    let mut surface = FacadeSurface::default();
    for item in syntax.items {
        match item {
            Item::Use(item) if is_public(&item.vis) => {
                if !item.attrs.is_empty() {
                    violations.push(Violation::new(
                        "facade-root-conditional-reexport",
                        format!(
                            "{} attaches attributes to a public reexport; the facade root must expose one unconditional source-auditable surface",
                            path.display()
                        ),
                    ));
                }
                collect_public_reexports(
                    path,
                    &item.tree,
                    &mut Vec::new(),
                    &mut surface,
                    violations,
                );
            }
            item => report_local_facade_item(path, &item, violations),
        }
    }

    surface
}

fn is_public(visibility: &Visibility) -> bool {
    matches!(visibility, Visibility::Public(_))
}

fn report_local_facade_item(path: &Path, item: &Item, violations: &mut Vec<Violation>) {
    let kind = match item {
        Item::Const(_) => "constant",
        Item::Enum(_) => "enum",
        Item::ExternCrate(_) => "extern crate",
        Item::Fn(_) => "function",
        Item::ForeignMod(_) => "foreign module",
        Item::Impl(_) => "implementation",
        Item::Macro(_) => "item macro",
        Item::Mod(_) => "module",
        Item::Static(_) => "static",
        Item::Struct(_) => "struct",
        Item::Trait(_) => "trait",
        Item::TraitAlias(_) => "trait alias",
        Item::Type(_) => "type alias",
        Item::Union(_) => "union",
        Item::Use(_) => "private use",
        _ => "item",
    };
    violations.push(Violation::new(
        "facade-root-direct-item",
        format!(
            "{} contains a local {kind}; stab-core/src/lib.rs may contain only documentation and unconditional direct public component reexports",
            path.display()
        ),
    ));
}

fn collect_public_reexports(
    path: &Path,
    tree: &UseTree,
    prefix: &mut Vec<String>,
    surface: &mut FacadeSurface,
    violations: &mut Vec<Violation>,
) {
    match tree {
        UseTree::Path(path_tree) => {
            prefix.push(path_tree.ident.to_string());
            collect_public_reexports(path, &path_tree.tree, prefix, surface, violations);
            prefix.pop();
        }
        UseTree::Name(name) => {
            let (exported, source) = if name.ident == "self" {
                let Some(exported) = prefix.last().cloned() else {
                    violations.push(Violation::new(
                        "facade-root-reexport-invalid",
                        format!(
                            "{} contains a root `self` reexport without a source path",
                            path.display()
                        ),
                    ));
                    return;
                };
                (exported, prefix.join("::"))
            } else {
                let exported = name.ident.to_string();
                prefix.push(exported.clone());
                let source = prefix.join("::");
                prefix.pop();
                (exported, source)
            };
            insert_facade_reexport(path, exported, source, surface, violations);
        }
        UseTree::Rename(rename) => {
            if rename.rename == "_" {
                violations.push(Violation::new(
                    "facade-root-anonymous-reexport",
                    format!(
                        "{} contains anonymous public reexport `{}` as `_` in the facade root",
                        path.display(),
                        rename.ident
                    ),
                ));
            } else {
                prefix.push(rename.ident.to_string());
                let source = prefix.join("::");
                prefix.pop();
                insert_facade_reexport(
                    path,
                    rename.rename.to_string(),
                    source,
                    surface,
                    violations,
                );
            }
        }
        UseTree::Glob(_) => {
            violations.push(Violation::new(
                "facade-root-glob-reexport",
                format!(
                    "{} contains a public glob reexport; the facade root requires explicit inventory-owned names",
                    path.display()
                ),
            ));
        }
        UseTree::Group(group) => {
            for item in &group.items {
                collect_public_reexports(path, item, prefix, surface, violations);
            }
        }
    }
}

fn insert_facade_reexport(
    path: &Path,
    exported: String,
    source: String,
    surface: &mut FacadeSurface,
    violations: &mut Vec<Violation>,
) {
    let owner = source.split("::").next().unwrap_or_default();
    if !FACADE_COMPONENT_CRATES.contains(&owner) {
        violations.push(Violation::new(
            "facade-root-reexport-local-source",
            format!(
                "{} reexports root item `{exported}` through `{source}`; facade sources must begin with an approved component crate",
                path.display()
            ),
        ));
    }
    if let Some(previous) = surface.reexports.insert(exported.clone(), source.clone()) {
        violations.push(Violation::new(
            "facade-root-reexport-duplicate",
            format!(
                "{} reexports root item `{exported}` from both `{previous}` and `{source}`",
                path.display()
            ),
        ));
    }
}

fn parse_root_reexport_inventory(
    path: &Path,
    source: &str,
    violations: &mut Vec<Violation>,
) -> BTreeMap<String, String> {
    let mut entries = BTreeMap::new();
    let mut previous_exported: Option<String> = None;
    for (line_index, line) in source.lines().enumerate() {
        let entry = line.trim();
        if entry.is_empty() || entry.starts_with('#') {
            continue;
        }
        let use_item = match syn::parse_str::<syn::ItemUse>(&format!("pub use {entry};")) {
            Ok(use_item) => use_item,
            Err(_) => {
                violations.push(Violation::new(
                    "facade-root-inventory-invalid",
                    format!(
                        "{}:{} contains invalid Rust reexport `{entry}`",
                        path.display(),
                        line_index + 1
                    ),
                ));
                continue;
            }
        };
        let mut parsed = FacadeSurface::default();
        collect_public_reexports(
            path,
            &use_item.tree,
            &mut Vec::new(),
            &mut parsed,
            violations,
        );
        if parsed.reexports.len() != 1 {
            violations.push(Violation::new(
                "facade-root-inventory-invalid",
                format!(
                    "{}:{} must contain exactly one named Rust reexport",
                    path.display(),
                    line_index + 1
                ),
            ));
            continue;
        }
        let Some((exported, owner)) = parsed.reexports.into_iter().next() else {
            continue;
        };
        if previous_exported
            .as_ref()
            .is_some_and(|previous| previous >= &exported)
        {
            violations.push(Violation::new(
                "facade-root-inventory-order",
                format!(
                    "{}:{} root item `{exported}` is not in ascending ASCII order",
                    path.display(),
                    line_index + 1
                ),
            ));
        }
        previous_exported = Some(exported.clone());
        if let Some(previous_owner) = entries.insert(exported.clone(), owner.clone()) {
            violations.push(Violation::new(
                "facade-root-inventory-duplicate",
                format!(
                    "{}:{} repeats root item `{exported}` from `{previous_owner}` and `{owner}`",
                    path.display(),
                    line_index + 1
                ),
            ));
        }
    }
    entries
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

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    fn namespaces() -> &'static str {
        "pub use stab_analysis as analysis;\npub use stab_decoder as decoder;\npub use stab_engine as execution;\n"
    }

    fn inventory(prefix: &str) -> String {
        format!(
            "{prefix}stab_analysis as analysis\nstab_decoder as decoder\nstab_engine as execution\n"
        )
    }

    #[test]
    fn kernel_package_owns_direct_simd() {
        let package = PackageSpec {
            name: SIMD_PACKAGE.to_owned(),
            relative_path: PathBuf::from("crates/stab-kernels-simd"),
            default_features: Vec::new(),
            rust_version: None,
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
            rust_version: None,
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
    fn facade_contract_rejects_modules_regardless_of_visibility() {
        let source = format!(
            "{}mod private_bits;\npub mod public_bits {{}}\n",
            namespaces()
        );
        let inventory = inventory("");
        let violations = facade_surface_violations(
            FacadeSource::new(Path::new("lib.rs"), &source),
            FacadeSource::new(Path::new("root-reexports.txt"), &inventory),
        );

        assert_eq!(violations.len(), 2, "{violations:?}");
        assert!(
            violations
                .iter()
                .all(|violation| violation.code == "facade-root-direct-item")
        );
    }

    #[test]
    fn facade_contract_rejects_private_uses_direct_items_and_macros() {
        let source = format!(
            "{}use stab_model as local_model;\nstruct Escape;\ninclude!(\"exports.rs\");\n",
            namespaces()
        );
        let inventory = inventory("");
        let violations = facade_surface_violations(
            FacadeSource::new(Path::new("lib.rs"), &source),
            FacadeSource::new(Path::new("root-reexports.txt"), &inventory),
        );
        let codes = violations
            .iter()
            .map(|violation| violation.code)
            .collect::<Vec<_>>();

        assert_eq!(codes, vec!["facade-root-direct-item"; 3]);
    }

    #[test]
    fn facade_contract_accepts_grouped_and_aliased_component_reexports() {
        let source = format!(
            "{}pub use stab_model::{{Circuit, DetectorErrorModel as Dem}};\n",
            namespaces()
        );
        let inventory = inventory("stab_model::Circuit\nstab_model::DetectorErrorModel as Dem\n");
        let violations = facade_surface_violations(
            FacadeSource::new(Path::new("lib.rs"), &source),
            FacadeSource::new(Path::new("root-reexports.txt"), &inventory),
        );

        assert!(violations.is_empty(), "{violations:?}");
    }

    #[test]
    fn facade_contract_rejects_private_alias_indirection() {
        let source = format!(
            "{}use stab_model as local_model;\npub use local_model::Circuit;\n",
            namespaces()
        );
        let inventory = inventory("stab_model::Circuit\n");
        let violations = facade_surface_violations(
            FacadeSource::new(Path::new("lib.rs"), &source),
            FacadeSource::new(Path::new("root-reexports.txt"), &inventory),
        );
        let codes = violations
            .iter()
            .map(|violation| violation.code)
            .collect::<BTreeSet<_>>();

        assert!(codes.contains("facade-root-direct-item"));
        assert!(codes.contains("facade-root-reexport-local-source"));
        assert!(codes.contains("facade-root-reexport-wrong-owner"));
    }

    #[test]
    fn facade_contract_rejects_conditional_reexports() {
        let source = format!(
            "{}#[cfg(feature = \"conditional\")] pub use stab_model::Circuit;\n",
            namespaces()
        );
        let inventory = inventory("stab_model::Circuit\n");
        let violations = facade_surface_violations(
            FacadeSource::new(Path::new("lib.rs"), &source),
            FacadeSource::new(Path::new("root-reexports.txt"), &inventory),
        );

        let [violation] = violations.as_slice() else {
            panic!("expected one conditional reexport violation: {violations:?}");
        };
        assert_eq!(violation.code, "facade-root-conditional-reexport");
    }

    #[test]
    fn facade_contract_accepts_crate_docs_and_rejects_crate_conditions() {
        let source = format!(
            "#![cfg(feature = \"conditional\")]\n#![cfg_attr(doc, allow(dead_code))]\n//! Facade documentation.\n{}",
            namespaces()
        );
        let inventory = inventory("");
        let violations = facade_surface_violations(
            FacadeSource::new(Path::new("lib.rs"), &source),
            FacadeSource::new(Path::new("root-reexports.txt"), &inventory),
        );

        assert_eq!(violations.len(), 2, "{violations:?}");
        assert!(
            violations
                .iter()
                .all(|violation| violation.code == "facade-root-crate-attribute")
        );
    }

    #[test]
    fn facade_contract_rejects_globs_missing_items_and_bad_inventory() {
        let source = format!("{}pub use stab_model::*;\n", namespaces());
        let inventory = inventory("stab_model::Circuit\nstab_analysis::Circuit\nnot-valid\n");
        let violations = facade_surface_violations(
            FacadeSource::new(Path::new("lib.rs"), &source),
            FacadeSource::new(Path::new("root-reexports.txt"), &inventory),
        );
        let codes = violations
            .iter()
            .map(|violation| violation.code)
            .collect::<BTreeSet<_>>();

        assert_eq!(
            codes,
            BTreeSet::from([
                "facade-root-inventory-duplicate",
                "facade-root-inventory-invalid",
                "facade-root-inventory-order",
                "facade-root-glob-reexport",
                "facade-root-reexport-missing",
            ])
        );
    }

    #[test]
    fn facade_contract_rejects_wrong_item_and_namespace_owners() {
        let source = "pub use stab_model as analysis;\npub use stab_decoder as decoder;\npub use stab_engine as execution;\npub use stab_analysis::Circuit;\n";
        let inventory = inventory("stab_model::Circuit\n");
        let violations = facade_surface_violations(
            FacadeSource::new(Path::new("lib.rs"), source),
            FacadeSource::new(Path::new("root-reexports.txt"), &inventory),
        );
        let wrong_owners = violations
            .iter()
            .filter(|violation| violation.code == "facade-root-reexport-wrong-owner")
            .map(|violation| violation.message.as_str())
            .collect::<Vec<_>>();

        assert_eq!(wrong_owners.len(), 2, "{violations:?}");
        assert!(
            wrong_owners
                .iter()
                .any(|message| message.contains("`analysis`"))
        );
        assert!(
            wrong_owners
                .iter()
                .any(|message| message.contains("`Circuit`"))
        );
    }

    #[test]
    fn facade_contract_accepts_current_surface() {
        let violations = facade_surface_violations(
            FacadeSource::new(
                Path::new("lib.rs"),
                include_str!("../../../crates/stab-core/src/lib.rs"),
            ),
            FacadeSource::new(
                Path::new("root-reexports.txt"),
                include_str!("../facade-root-reexports.txt"),
            ),
        );

        assert!(violations.is_empty(), "{violations:?}");
    }
}
