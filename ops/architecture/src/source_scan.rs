use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use syn::{Item, UseTree, Visibility};

use crate::policy::{product_contract, requires_minimum_rust};
use crate::{CheckError, PackageSpec, Violation};

const FACADE_ROOT_REEXPORTS: &str = "ops/architecture/facade-root-reexports.txt";
const FACADE_PACKAGE: &str = "stab-core";
const FACADE_PACKAGE_DIRECTORY: &str = "crates/stab-core";
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

    // Cargo target roots may be declared outside the package directory with
    // `[lib] path` or `[[bin]] path`; attribute and scan them explicitly.
    let mut target_sources = BTreeMap::<PathBuf, &PackageSpec>::new();
    for package in packages {
        let package_root = root.join(&package.relative_path);
        for target in &package.targets {
            if !target.src_path.starts_with(&package_root) {
                violations.push(Violation::new(
                    "target-source-outside-package",
                    format!(
                        "{} target {} has crate root {} outside its package directory {}; target roots must live inside the package so the whole module tree is scanned",
                        package.name,
                        target.name,
                        target.src_path.display(),
                        package.relative_path.display()
                    ),
                ));
            }
            let display = target
                .src_path
                .strip_prefix(root)
                .map_or_else(|_| target.src_path.clone(), Path::to_path_buf);
            if rust_sources.binary_search(&display).is_err() {
                target_sources.insert(display, package);
            }
        }
    }

    validate_facade_surface(root, packages, &mut violations)?;

    for source_path in rust_sources.iter().chain(target_sources.keys()) {
        let source = std::fs::read_to_string(root.join(source_path)).map_err(|source| {
            CheckError::ReadSource {
                path: source_path.clone(),
                source,
            }
        })?;
        let package = target_sources
            .get(source_path)
            .copied()
            .or_else(|| package_for_source(source_path, packages));
        violations.extend(source_violations(source_path, package, &source));
    }

    Ok(SourceReport {
        rust_source_count: rust_sources.len() + target_sources.len(),
        violations,
    })
}

fn source_violations(
    source_path: &Path,
    package: Option<&PackageSpec>,
    source: &str,
) -> Vec<Violation> {
    let mut violations = Vec::new();
    let facts = match rust_source::inspect(source) {
        Ok(facts) => facts,
        Err(error) => {
            violations.push(Violation::new(
                "architecture-source-parse",
                format!(
                    "failed to parse {} while checking source-boundary contracts: {error}",
                    source_path.display()
                ),
            ));
            return violations;
        }
    };
    if package.is_some_and(|package| requires_minimum_rust(&package.name))
        && !facts.feature_gates.is_empty()
    {
        violations.push(Violation::new(
            "unstable-feature-gate",
            format!(
                "{} source {} enables unstable Rust features {:?}; product crates must build on the pinned Stable toolchain",
                package.map_or("unknown", |package| package.name.as_str()),
                source_path.display(),
                facts.feature_gates
            ),
        ));
    }
    if let Some(package) = package.filter(|package| product_contract(&package.name).is_some()) {
        violations.extend(module_path_override_violations(
            source_path,
            package,
            &facts.module_path_overrides,
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
    violations
}

/// Product library and binary module trees must follow the filesystem layout
/// the scan walks. Integration tests and benches may share support files with
/// `#[path]`, but only with literal relative paths that stay in the package.
fn module_path_override_violations(
    source_path: &Path,
    package: &PackageSpec,
    overrides: &[String],
) -> Vec<Violation> {
    let in_shared_test_tree = ["tests", "benches"]
        .iter()
        .any(|directory| source_path.starts_with(package.relative_path.join(directory)));
    let mut violations = Vec::new();
    for value in overrides {
        let escapes = value == rust_source::NON_LITERAL_PATH
            || !lexically_stays_within(
                source_path.parent().unwrap_or_else(|| Path::new("")),
                Path::new(value),
                &package.relative_path,
            );
        if !in_shared_test_tree || escapes {
            violations.push(Violation::new(
                "product-module-path-override",
                format!(
                    "{} source {} overrides a module path with #[path = {value:?}]; product library and binary modules must follow the scanned file layout, and test or bench overrides must stay inside {}",
                    package.name,
                    source_path.display(),
                    package.relative_path.display()
                ),
            ));
        }
    }
    violations
}

fn lexically_stays_within(base: &Path, relative: &Path, boundary: &Path) -> bool {
    let mut components = base
        .components()
        .map(|component| component.as_os_str().to_os_string())
        .collect::<Vec<_>>();
    for component in relative.components() {
        match component {
            std::path::Component::Normal(name) => components.push(name.to_os_string()),
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                if components.pop().is_none() {
                    return false;
                }
            }
            std::path::Component::RootDir | std::path::Component::Prefix(_) => return false,
        }
    }
    components.iter().collect::<PathBuf>().starts_with(boundary)
}

fn validate_facade_surface(
    root: &Path,
    packages: &[PackageSpec],
    violations: &mut Vec<Violation>,
) -> Result<(), CheckError> {
    let Some(facade_root) = facade_crate_root(root, packages, violations) else {
        return Ok(());
    };
    let root_reexports = Path::new(FACADE_ROOT_REEXPORTS);
    let root_source = read_source(root, &facade_root)?;
    let root_reexport_inventory = read_source(root, root_reexports)?;
    violations.extend(facade_surface_violations(
        FacadeSource::new(&facade_root, &root_source),
        FacadeSource::new(root_reexports, &root_reexport_inventory),
    ));
    Ok(())
}

/// Resolves the facade crate root from Cargo metadata rather than assuming it,
/// so a `[lib] path` override cannot move the audited surface elsewhere.
fn facade_crate_root(
    root: &Path,
    packages: &[PackageSpec],
    violations: &mut Vec<Violation>,
) -> Option<PathBuf> {
    let expected = Path::new(FACADE_PACKAGE_DIRECTORY).join("src/lib.rs");
    let Some(facade) = packages
        .iter()
        .find(|package| package.name == FACADE_PACKAGE)
    else {
        violations.push(Violation::new(
            "facade-root-missing",
            format!("workspace has no {FACADE_PACKAGE} facade package"),
        ));
        return None;
    };
    let libraries = facade
        .targets
        .iter()
        .filter(|target| target.library)
        .collect::<Vec<_>>();
    let [library] = libraries.as_slice() else {
        violations.push(Violation::new(
            "facade-root-missing",
            format!(
                "{FACADE_PACKAGE} must have exactly one library target, found {}",
                libraries.len()
            ),
        ));
        return None;
    };
    if library.src_path != root.join(&expected) {
        violations.push(Violation::new(
            "facade-root-path",
            format!(
                "{FACADE_PACKAGE} library crate root is {}, expected {}; the audited facade surface must be the conventional crate root",
                library.src_path.display(),
                expected.display()
            ),
        ));
        return None;
    }
    Some(expected)
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
    path.trim_prefix(root).to_path_buf()
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
mod tests;
