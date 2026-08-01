use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use syn::punctuated::Punctuated;
use syn::{Item, Meta, Token, UseTree, Visibility};

use crate::policy::is_stable_source_package;
use crate::{CheckError, MigrationAllowance, PackageSpec, Violation};

const SIMD_PACKAGE: &str = "stab-kernels-simd";
const FACADE_ROOT_REEXPORTS: &str = "ops/architecture/facade-root-reexports.txt";
const FACADE_ROOT_MODULES: [&str; 4] = ["advanced", "analysis", "execution", "experimental"];
const FACADE_EXPERIMENTAL_REEXPORTS: [&str; 13] = [
    "CircuitPass",
    "CircuitPassContext",
    "CircuitPassError",
    "CircuitPassInput",
    "CircuitPassLimits",
    "CircuitPassOutput",
    "CircuitPassProjectionError",
    "CircuitPassResources",
    "CircuitPassStage",
    "WithoutNoiseOptions",
    "WithoutNoisePass",
    "WithoutNoiseReport",
    "run_circuit_pass",
];
const FACADE_ADVANCED_MODULES: [&str; 6] = [
    "storage",
    "algebra",
    "records",
    "backend",
    "traversal",
    "compat",
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

    validate_facade_tiers(root, &mut violations)?;

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

fn validate_facade_tiers(root: &Path, violations: &mut Vec<Violation>) -> Result<(), CheckError> {
    let facade_root = Path::new("crates/stab-core/src/lib.rs");
    let facade_advanced = Path::new("crates/stab-core/src/advanced.rs");
    let facade_experimental = Path::new("crates/stab-core/src/experimental.rs");
    let root_reexports = Path::new(FACADE_ROOT_REEXPORTS);
    let root_source = read_source(root, facade_root)?;
    let advanced_source = read_source(root, facade_advanced)?;
    let experimental_source = read_source(root, facade_experimental)?;
    let root_reexport_inventory = read_source(root, root_reexports)?;
    violations.extend(facade_tier_violations(
        FacadeSource::new(facade_root, &root_source),
        FacadeSource::new(facade_advanced, &advanced_source),
        FacadeSource::new(facade_experimental, &experimental_source),
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

fn facade_tier_violations(
    root: FacadeSource<'_>,
    advanced: FacadeSource<'_>,
    experimental: FacadeSource<'_>,
    root_reexports: FacadeSource<'_>,
) -> Vec<Violation> {
    let mut violations = Vec::new();
    let root_surface =
        parse_facade_surface(root.path, root.source, FacadeTier::Root, &mut violations);
    let expected_root_modules = FACADE_ROOT_MODULES
        .into_iter()
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();
    for missing in expected_root_modules.difference(&root_surface.modules) {
        violations.push(Violation::new(
            "facade-tier-missing",
            format!(
                "{} must publicly declare module `{missing}`",
                root.path.display()
            ),
        ));
    }
    for unexpected in root_surface.modules.difference(&expected_root_modules) {
        violations.push(Violation::new(
            "facade-root-module-unassigned",
            format!(
                "{} publicly declares unassigned root module `{unexpected}`",
                root.path.display()
            ),
        ));
    }

    let expected_root_reexports =
        parse_root_reexport_inventory(root_reexports.path, root_reexports.source, &mut violations);
    for missing in expected_root_reexports.difference(&root_surface.reexports) {
        violations.push(Violation::new(
            "facade-root-reexport-missing",
            format!(
                "{} assigns `{missing}` to the facade root, but {} does not reexport it",
                root_reexports.path.display(),
                root.path.display()
            ),
        ));
    }
    for unexpected in root_surface.reexports.difference(&expected_root_reexports) {
        violations.push(Violation::new(
            "facade-root-reexport-unassigned",
            format!(
                "{} publicly reexports unassigned root item `{unexpected}`; assign it in {} or move it under `advanced`",
                root.path.display(),
                root_reexports.path.display()
            ),
        ));
    }

    let advanced_surface = parse_facade_surface(
        advanced.path,
        advanced.source,
        FacadeTier::Advanced,
        &mut violations,
    );
    let required_advanced_modules = FACADE_ADVANCED_MODULES
        .into_iter()
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();
    for required in required_advanced_modules.difference(&advanced_surface.modules) {
        violations.push(Violation::new(
            "facade-advanced-module-missing",
            format!(
                "{} must publicly declare module `{required}`",
                advanced.path.display()
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
                advanced.path.display()
            ),
        ));
    }
    for reexport in &advanced_surface.reexports {
        violations.push(Violation::new(
            "facade-advanced-reexport",
            format!(
                "{} publicly reexports top-level item `{reexport}`; advanced items must live under an assigned module",
                advanced.path.display()
            ),
        ));
    }

    let experimental_surface = parse_facade_surface(
        experimental.path,
        experimental.source,
        FacadeTier::Experimental,
        &mut violations,
    );
    for module in &experimental_surface.modules {
        violations.push(Violation::new(
            "facade-experimental-module",
            format!(
                "{} publicly declares unassigned experimental module `{module}`",
                experimental.path.display()
            ),
        ));
    }
    let expected_experimental_reexports = FACADE_EXPERIMENTAL_REEXPORTS
        .into_iter()
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();
    for missing in expected_experimental_reexports.difference(&experimental_surface.reexports) {
        violations.push(Violation::new(
            "facade-experimental-reexport-missing",
            format!(
                "{} must publicly reexport assigned experimental item `{missing}`",
                experimental.path.display()
            ),
        ));
    }
    for unexpected in experimental_surface
        .reexports
        .difference(&expected_experimental_reexports)
    {
        violations.push(Violation::new(
            "facade-experimental-reexport-unassigned",
            format!(
                "{} publicly reexports unassigned experimental item `{unexpected}`",
                experimental.path.display()
            ),
        ));
    }

    violations
}

#[derive(Clone, Copy)]
enum FacadeTier {
    Root,
    Advanced,
    Experimental,
}

impl FacadeTier {
    fn direct_item_code(self) -> &'static str {
        match self {
            Self::Root => "facade-root-direct-item",
            Self::Advanced => "facade-advanced-direct-item",
            Self::Experimental => "facade-experimental-direct-item",
        }
    }

    fn anonymous_reexport_code(self) -> &'static str {
        match self {
            Self::Root => "facade-root-anonymous-reexport",
            Self::Advanced => "facade-advanced-anonymous-reexport",
            Self::Experimental => "facade-experimental-anonymous-reexport",
        }
    }

    fn glob_reexport_code(self) -> &'static str {
        match self {
            Self::Root => "facade-root-glob-reexport",
            Self::Advanced => "facade-advanced-glob-reexport",
            Self::Experimental => "facade-experimental-glob-reexport",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Root => "facade root",
            Self::Advanced => "advanced facade",
            Self::Experimental => "experimental facade",
        }
    }
}

#[derive(Default)]
struct FacadeSurface {
    modules: BTreeSet<String>,
    reexports: BTreeSet<String>,
}

fn parse_facade_surface(
    path: &Path,
    source: &str,
    tier: FacadeTier,
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
            Item::Mod(item) => {
                let pathless = match module_has_path_override(&item.attrs) {
                    Ok(false) => true,
                    Ok(true) => {
                        violations.push(Violation::new(
                            "facade-module-path-override",
                            format!(
                                "{} declares module `{}` through a path override; facade tier modules must use canonical pathless declarations",
                                path.display(),
                                item.ident
                            ),
                        ));
                        false
                    }
                    Err(error) => {
                        violations.push(Violation::new(
                            "facade-module-attribute-parse",
                            format!(
                                "failed to validate attributes on module `{}` in {}: {error}",
                                item.ident,
                                path.display()
                            ),
                        ));
                        false
                    }
                };
                if is_public(&item.vis) && pathless {
                    if matches!(tier, FacadeTier::Root) && item.content.is_some() {
                        violations.push(Violation::new(
                            "facade-root-inline-module",
                            format!(
                                "{} defines public root module `{}` inline; root facade tiers must be canonical out-of-line modules",
                                path.display(),
                                item.ident
                            ),
                        ));
                    }
                    surface.modules.insert(item.ident.to_string());
                }
            }
            Item::Use(item) if is_public(&item.vis) => {
                collect_public_use_names(path, &item.tree, None, tier, &mut surface, violations);
            }
            Item::Const(item) if is_public(&item.vis) => {
                report_direct_item(path, tier, "constant", &item.ident, violations);
            }
            Item::Enum(item) if is_public(&item.vis) => {
                report_direct_item(path, tier, "enum", &item.ident, violations);
            }
            Item::ExternCrate(item) if is_public(&item.vis) => {
                report_direct_item(path, tier, "extern crate", &item.ident, violations);
            }
            Item::ForeignMod(_) => {
                violations.push(Violation::new(
                    tier.direct_item_code(),
                    format!(
                        "{} contains a foreign module directly in the {}; foreign declarations cannot bypass its tier policy",
                        path.display(),
                        tier.label()
                    ),
                ));
            }
            Item::Fn(item) if is_public(&item.vis) => {
                report_direct_item(path, tier, "function", &item.sig.ident, violations);
            }
            Item::Static(item) if is_public(&item.vis) => {
                report_direct_item(path, tier, "static", &item.ident, violations);
            }
            Item::Struct(item) if is_public(&item.vis) => {
                report_direct_item(path, tier, "struct", &item.ident, violations);
            }
            Item::Trait(item) if is_public(&item.vis) => {
                report_direct_item(path, tier, "trait", &item.ident, violations);
            }
            Item::TraitAlias(item) if is_public(&item.vis) => {
                report_direct_item(path, tier, "trait alias", &item.ident, violations);
            }
            Item::Type(item) if is_public(&item.vis) => {
                report_direct_item(path, tier, "type alias", &item.ident, violations);
            }
            Item::Union(item) if is_public(&item.vis) => {
                report_direct_item(path, tier, "union", &item.ident, violations);
            }
            Item::Macro(item) => {
                let macro_name = item.mac.path.segments.last().map_or_else(
                    || "<anonymous>".to_owned(),
                    |segment| segment.ident.to_string(),
                );
                violations.push(Violation::new(
                    tier.direct_item_code(),
                    format!(
                        "{} invokes or defines item macro `{}` directly in the {}; generated items cannot bypass its tier policy",
                        path.display(),
                        macro_name,
                        tier.label()
                    ),
                ));
            }
            _ => {}
        }
    }

    surface
}

fn module_has_path_override(attributes: &[syn::Attribute]) -> syn::Result<bool> {
    for attribute in attributes {
        if meta_sets_module_path(&attribute.meta)? {
            return Ok(true);
        }
    }
    Ok(false)
}

fn meta_sets_module_path(meta: &Meta) -> syn::Result<bool> {
    if matches!(meta, Meta::NameValue(value) if value.path.is_ident("path")) {
        return Ok(true);
    }
    let Meta::List(list) = meta else {
        return Ok(false);
    };
    if !list.path.is_ident("cfg_attr") {
        return Ok(false);
    }
    let nested = list.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)?;
    for meta in nested {
        if meta_sets_module_path(&meta)? {
            return Ok(true);
        }
    }
    Ok(false)
}

fn is_public(visibility: &Visibility) -> bool {
    matches!(visibility, Visibility::Public(_))
}

fn report_direct_item(
    path: &Path,
    tier: FacadeTier,
    kind: &str,
    name: &syn::Ident,
    violations: &mut Vec<Violation>,
) {
    violations.push(Violation::new(
        tier.direct_item_code(),
        format!(
            "{} defines public {kind} `{name}` directly in the {}; exported items must follow its tier policy",
            path.display(),
            tier.label()
        ),
    ));
}

fn collect_public_use_names(
    path: &Path,
    tree: &UseTree,
    parent: Option<&syn::Ident>,
    tier: FacadeTier,
    surface: &mut FacadeSurface,
    violations: &mut Vec<Violation>,
) {
    match tree {
        UseTree::Path(path_tree) => collect_public_use_names(
            path,
            &path_tree.tree,
            Some(&path_tree.ident),
            tier,
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
                    tier.anonymous_reexport_code(),
                    format!(
                        "{} contains anonymous public reexport `{}` as `_` in the {}",
                        path.display(),
                        rename.ident,
                        tier.label()
                    ),
                ));
            } else {
                surface.reexports.insert(rename.rename.to_string());
            }
        }
        UseTree::Glob(_) => {
            violations.push(Violation::new(
                tier.glob_reexport_code(),
                format!(
                    "{} contains a public glob reexport in the {}; facade tiers require explicit item names",
                    path.display(), tier.label()
                ),
            ));
        }
        UseTree::Group(group) => {
            for item in &group.items {
                collect_public_use_names(path, item, parent, tier, surface, violations);
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
    let surface = parse_facade_surface(
        Path::new("fixture.rs"),
        source,
        FacadeTier::Root,
        &mut violations,
    );
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

#[cfg(test)]
mod tests {
    use super::*;

    fn assigned_experimental_surface() -> FacadeSource<'static> {
        FacadeSource::new(
            Path::new("experimental.rs"),
            include_str!("../../../crates/stab-core/src/experimental.rs"),
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
    fn facade_tier_contract_reports_missing_and_owner_shaped_surfaces() {
        let violations = facade_tier_violations(
            FacadeSource::new(
                Path::new("lib.rs"),
                "pub mod advanced;\npub mod analysis;\npub mod bits;\npub mod execution;\npub mod experimental;\n",
            ),
            FacadeSource::new(
                Path::new("advanced.rs"),
                "pub mod algebra {}\npub mod backend {}\npub mod compat {}\npub mod records {}\npub mod storage {}\npub mod traversal {}\n",
            ),
            assigned_experimental_surface(),
            FacadeSource::new(Path::new("root-reexports.txt"), ""),
        );

        let [violation] = violations.as_slice() else {
            panic!("expected one unassigned module violation: {violations:?}");
        };
        assert_eq!(violation.code, "facade-root-module-unassigned");
        assert!(violation.message.contains("`bits`"));
    }

    #[test]
    fn facade_tiers_reject_path_overrides_and_item_macros() {
        let violations = facade_tier_violations(
            FacadeSource::new(
                Path::new("lib.rs"),
                "#[path = \"alternate.rs\"] pub mod advanced;\npub mod analysis;\npub mod execution;\npub mod experimental;\ninclude!(\"exports.rs\");\n",
            ),
            FacadeSource::new(
                Path::new("advanced.rs"),
                "pub mod algebra {}\npub mod backend {}\npub mod compat {}\npub mod records {}\npub mod storage {}\npub mod traversal {}\n",
            ),
            assigned_experimental_surface(),
            FacadeSource::new(Path::new("root-reexports.txt"), ""),
        );
        let codes = violations
            .iter()
            .map(|violation| violation.code)
            .collect::<BTreeSet<_>>();

        assert!(codes.contains("facade-module-path-override"));
        assert!(codes.contains("facade-root-direct-item"));
        assert!(codes.contains("facade-tier-missing"));
    }

    #[test]
    fn facade_tiers_reject_inline_root_modules_and_private_path_overrides() {
        let violations = facade_tier_violations(
            FacadeSource::new(
                Path::new("lib.rs"),
                "pub mod advanced {}\npub mod analysis;\npub mod execution;\npub mod experimental { pub struct UnreviewedExtension; }\n#[path = \"../../outside.rs\"] mod hidden;\n#[cfg_attr(all(), cfg_attr(any(), path = \"../../alternate.rs\"))] mod nested_hidden;\nunsafe extern \"C\" { pub fn unreviewed_foreign_export(); }\n",
            ),
            FacadeSource::new(
                Path::new("advanced.rs"),
                "pub mod algebra {}\npub mod backend {}\npub mod compat {}\npub mod records {}\npub mod storage {}\npub mod traversal {}\n",
            ),
            assigned_experimental_surface(),
            FacadeSource::new(Path::new("root-reexports.txt"), ""),
        );
        let codes = violations
            .iter()
            .map(|violation| violation.code)
            .collect::<Vec<_>>();

        assert_eq!(
            codes
                .iter()
                .filter(|code| **code == "facade-root-inline-module")
                .count(),
            2
        );
        assert_eq!(
            codes
                .iter()
                .filter(|code| **code == "facade-module-path-override")
                .count(),
            2
        );
        assert!(codes.contains(&"facade-root-direct-item"));
    }

    #[test]
    fn facade_tier_parser_ignores_comments_strings_and_restricted_modules() {
        let modules = public_module_names(
            "pub mod advanced;\n// pub mod bits;\nconst TEXT: &str = \"pub mod stabilizers;\";\npub(crate) mod result_formats;\npub mod experimental;\n",
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
            FacadeSource::new(
                Path::new("lib.rs"),
                "pub mod advanced;\npub mod analysis;\npub mod execution;\npub mod experimental;\npub use crate::circuit::Circuit;\npub use stab_bits::{\n    BitVec,\n    BitMatrix as Matrix,\n};\n",
            ),
            FacadeSource::new(
                Path::new("advanced.rs"),
                "pub mod algebra {}\npub mod backend {}\npub mod compat {}\npub mod records {}\npub mod storage {}\npub mod traversal {}\n",
            ),
            assigned_experimental_surface(),
            FacadeSource::new(Path::new("root-reexports.txt"), "Circuit\n"),
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
            FacadeSource::new(
                Path::new("lib.rs"),
                "pub mod advanced;\npub mod analysis;\npub mod execution;\npub mod experimental;\npub use crate::circuit::*;\n",
            ),
            FacadeSource::new(
                Path::new("advanced.rs"),
                "pub mod algebra {}\npub mod backend {}\npub mod compat {}\npub mod records {}\npub mod storage {}\npub mod traversal {}\n",
            ),
            assigned_experimental_surface(),
            FacadeSource::new(Path::new("root-reexports.txt"), "Circuit\n"),
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
            FacadeSource::new(
                Path::new("lib.rs"),
                "pub mod advanced;\npub mod analysis;\npub mod execution;\npub mod experimental;\npub use crate::circuit::Circuit;\n",
            ),
            FacadeSource::new(
                Path::new("advanced.rs"),
                "pub mod algebra {}\npub mod backend {}\npub mod compat {}\npub mod records {}\npub mod storage {}\npub mod traversal {}\n",
            ),
            assigned_experimental_surface(),
            FacadeSource::new(
                Path::new("root-reexports.txt"),
                "Circuit\nCircuit\nnot-valid\n",
            ),
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

    #[test]
    fn facade_tier_contract_accepts_current_advanced_and_experimental_shape() {
        let violations = facade_tier_violations(
            FacadeSource::new(
                Path::new("lib.rs"),
                "pub mod advanced;\npub mod analysis;\npub mod execution;\npub mod experimental;\n",
            ),
            FacadeSource::new(
                Path::new("advanced.rs"),
                include_str!("../../../crates/stab-core/src/advanced.rs"),
            ),
            assigned_experimental_surface(),
            FacadeSource::new(Path::new("root-reexports.txt"), ""),
        );

        assert!(violations.is_empty(), "{violations:?}");
    }

    #[test]
    fn facade_tier_contract_rejects_unassigned_advanced_and_experimental_exports() {
        let violations = facade_tier_violations(
            FacadeSource::new(
                Path::new("lib.rs"),
                "pub mod advanced;\npub mod analysis;\npub mod execution;\npub mod experimental;\n",
            ),
            FacadeSource::new(
                Path::new("advanced.rs"),
                "pub mod algebra {}\npub mod backend {}\npub mod compat {}\npub mod records {}\npub mod storage {}\npub mod traversal {}\npub use crate::Circuit;\npub struct Escape;\n",
            ),
            FacadeSource::new(
                Path::new("experimental.rs"),
                "pub mod decoder {}\npub use crate::Circuit;\npub fn escape() {}\n",
            ),
            FacadeSource::new(Path::new("root-reexports.txt"), ""),
        );
        let codes = violations
            .iter()
            .map(|violation| violation.code)
            .collect::<BTreeSet<_>>();

        assert_eq!(
            codes,
            BTreeSet::from([
                "facade-advanced-direct-item",
                "facade-advanced-reexport",
                "facade-experimental-direct-item",
                "facade-experimental-module",
                "facade-experimental-reexport-missing",
                "facade-experimental-reexport-unassigned",
            ])
        );
    }
}
