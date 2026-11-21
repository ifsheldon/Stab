use std::collections::BTreeSet;

use super::*;
use crate::TargetSpec;

fn namespaces() -> &'static str {
    "pub use stab_analysis as analysis;\npub use stab_decoder as decoder;\npub use stab_engine as execution;\n"
}

fn inventory(prefix: &str) -> String {
    format!(
        "{prefix}stab_analysis as analysis\nstab_decoder as decoder\nstab_engine as execution\n"
    )
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

fn source_package(name: &str, relative_path: &str) -> PackageSpec {
    PackageSpec {
        name: name.to_owned(),
        relative_path: PathBuf::from(relative_path),
        rust_version: None,
        version: cargo_metadata::semver::Version::new(0, 2, 0),
        publish: None,
        binary_targets: Vec::new(),
        targets: Vec::new(),
    }
}

#[test]
fn every_product_crate_rejects_unstable_feature_gates() {
    let source =
        "#![feature(portable_simd)]\nmod inner { #![feature(core_intrinsics, never_type)] }\n";
    for (name, relative_path) in [
        ("stab-cli", "crates/stab-cli"),
        ("stab-core", "crates/stab-core"),
        ("stab-kernels-simd", "crates/stab-kernels-simd"),
        ("stab-reference-decoder", "test-support/reference-decoder"),
    ] {
        let package = source_package(name, relative_path);
        let source_path = Path::new(relative_path).join("src/lib.rs");
        let violations = source_violations(&source_path, Some(&package), source);
        let [violation] = violations.as_slice() else {
            panic!("{name} should report one feature-gate violation: {violations:?}");
        };
        assert_eq!(violation.code, "unstable-feature-gate");
        assert!(violation.message.contains(name));
        assert!(violation.message.contains("portable_simd"));
        assert!(violation.message.contains("never_type"));

        let stable_source =
            "//! #![feature(portable_simd)] in documentation only.\npub fn run() {}\n";
        assert!(
            source_violations(&source_path, Some(&package), stable_source).is_empty(),
            "{name} should accept sources without feature gates"
        );
    }
}

#[test]
fn metadata_target_roots_drive_the_facade_and_source_scan() {
    let root = std::fs::canonicalize(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/facade-root-override-workspace"),
    )
    .expect("resolve facade-root-override fixture");
    let graph = crate::metadata::load_workspace_graph(&root).expect("load facade-root fixture");

    let report = scan_workspace_sources(&root, &graph.packages).expect("scan fixture sources");
    let codes = report
        .violations
        .iter()
        .map(|violation| violation.code)
        .collect::<BTreeSet<_>>();

    assert_eq!(
        codes,
        BTreeSet::from([
            "facade-root-path",
            "product-module-path-override",
            "target-source-outside-package",
            "unstable-feature-gate",
        ]),
        "{:?}",
        report.violations
    );
    assert!(report.violations.iter().any(|violation| {
        violation.code == "unstable-feature-gate"
            && violation.message.contains("shared/facade.rs")
            && violation.message.contains("stab-core")
    }));
    assert_eq!(report.rust_source_count, 3);
}

#[test]
fn facade_root_requires_one_conventional_library_target() {
    let root = Path::new("/workspace");
    let mut facade = source_package("stab-core", "crates/stab-core");
    let mut violations = Vec::new();
    assert_eq!(
        facade_crate_root(root, std::slice::from_ref(&facade), &mut violations),
        None
    );
    assert_eq!(
        violations.first().map(|violation| violation.code),
        Some("facade-root-missing")
    );

    facade.targets = vec![TargetSpec {
        name: "stab_core".to_owned(),
        library: true,
        src_path: root.join("crates/stab-core/src/lib.rs"),
    }];
    let mut violations = Vec::new();
    assert_eq!(
        facade_crate_root(root, std::slice::from_ref(&facade), &mut violations),
        Some(PathBuf::from("crates/stab-core/src/lib.rs"))
    );
    assert!(violations.is_empty(), "{violations:?}");
}

#[test]
fn product_module_path_overrides_are_limited_to_contained_test_trees() {
    let product = source_package("stab-model", "crates/stab-model");
    for (source_path, value, allowed) in [
        ("crates/stab-model/src/lib.rs", "elsewhere.rs", false),
        ("crates/stab-model/src/bin/tool.rs", "../shared.rs", false),
        ("crates/stab-model/tests/flow.rs", "support/flow.rs", true),
        (
            "crates/stab-model/tests/flow.rs",
            "../src/internal.rs",
            true,
        ),
        (
            "crates/stab-model/benches/bench.rs",
            "../../stab-engine/src/lib.rs",
            false,
        ),
        ("crates/stab-model/tests/flow.rs", "/etc/passwd.rs", false),
        (
            "crates/stab-model/tests/flow.rs",
            rust_source::NON_LITERAL_PATH,
            false,
        ),
    ] {
        let violations =
            module_path_override_violations(Path::new(source_path), &product, &[value.to_owned()]);
        assert_eq!(
            violations.is_empty(),
            allowed,
            "{source_path} -> {value}: {violations:?}"
        );
    }

    let support = source_package("stab-compat-corpus", "test-support/compat-corpus");
    let source = "#[path = \"elsewhere.rs\"] mod moved;\n";
    assert!(
        source_violations(
            Path::new("test-support/compat-corpus/src/lib.rs"),
            Some(&support),
            source
        )
        .is_empty()
    );
    assert_eq!(
        source_violations(
            Path::new("crates/stab-model/src/lib.rs"),
            Some(&product),
            source
        )
        .first()
        .map(|violation| violation.code),
        Some("product-module-path-override")
    );
}

#[test]
fn facade_contract_accepts_current_surface() {
    let violations = facade_surface_violations(
        FacadeSource::new(
            Path::new("lib.rs"),
            include_str!("../../../../crates/stab-core/src/lib.rs"),
        ),
        FacadeSource::new(
            Path::new("root-reexports.txt"),
            include_str!("../../facade-root-reexports.txt"),
        ),
    );

    assert!(violations.is_empty(), "{violations:?}");
}
