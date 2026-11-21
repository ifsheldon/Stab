use super::*;

fn package(name: &str, prefix: &str) -> PackageSpec {
    PackageSpec {
        name: name.to_owned(),
        relative_path: PathBuf::from(prefix).join(name),
        rust_version: requires_minimum_rust(name).then(minimum_rust_version),
        version: Version::new(0, 2, 0),
        publish: if prefix == "crates" {
            None
        } else {
            Some(Vec::new())
        },
        binary_targets: if name == "stab-cli" {
            vec!["stab".to_owned()]
        } else {
            Vec::new()
        },
        targets: Vec::new(),
    }
}

fn graph(packages: Vec<PackageSpec>) -> WorkspaceGraph {
    WorkspaceGraph {
        packages,
        edges: Vec::new(),
        declared_path_dependencies: Vec::new(),
        resolved_dependencies: Vec::new(),
    }
}

#[test]
fn test_support_edges_are_development_only_and_downward() {
    let mut graph = graph(vec![
        package("stab-core", "crates"),
        PackageSpec {
            name: "stab-compat-corpus".to_owned(),
            relative_path: PathBuf::from("test-support/compat-corpus"),
            rust_version: None,
            version: Version::new(0, 2, 0),
            publish: Some(Vec::new()),
            binary_targets: Vec::new(),
            targets: Vec::new(),
        },
    ]);
    graph.edges = vec![WorkspaceEdge {
        from: "stab-core".to_owned(),
        to: "stab-compat-corpus".to_owned(),
        kind: DependencyKind::Development,
        optional: false,
    }];
    let report = validate_graph(&graph);
    assert!(report.violations.is_empty());

    let support_edge = graph
        .edges
        .first()
        .expect("fixture should contain its test-support edge")
        .clone();
    let normal_graph = WorkspaceGraph {
        edges: vec![WorkspaceEdge {
            kind: DependencyKind::Normal,
            ..support_edge
        }],
        ..graph.clone()
    };
    let report = validate_graph(&normal_graph);
    assert_eq!(
        report
            .violations
            .first()
            .expect("normal product-to-test-support edge should fail")
            .code,
        "product-test-support-runtime-edge"
    );

    let upward_graph = WorkspaceGraph {
        edges: vec![WorkspaceEdge {
            from: "stab-compat-corpus".to_owned(),
            to: "stab-core".to_owned(),
            kind: DependencyKind::Development,
            optional: false,
        }],
        ..graph
    };
    assert_eq!(
        validate_graph(&upward_graph)
            .violations
            .first()
            .expect("test support must not depend on product code")
            .code,
        "test-support-upward-dependency"
    );
}

#[test]
fn reference_decoder_has_only_its_earned_component_edges() {
    let packages = vec![
        package("stab-reference-decoder", "test-support"),
        package("stab-decoder", "crates"),
        package("stab-model", "crates"),
        package("stab-records", "crates"),
        package("stab-analysis", "crates"),
        package("stab-engine", "crates"),
        package("stab-core", "crates"),
        package("stab-bench", "ops"),
    ];
    let permitted = WorkspaceGraph {
        packages: packages.clone(),
        edges: vec![
            WorkspaceEdge {
                from: "stab-reference-decoder".to_owned(),
                to: "stab-decoder".to_owned(),
                kind: DependencyKind::Normal,
                optional: false,
            },
            WorkspaceEdge {
                from: "stab-reference-decoder".to_owned(),
                to: "stab-model".to_owned(),
                kind: DependencyKind::Normal,
                optional: false,
            },
            WorkspaceEdge {
                from: "stab-reference-decoder".to_owned(),
                to: "stab-records".to_owned(),
                kind: DependencyKind::Normal,
                optional: false,
            },
            WorkspaceEdge {
                from: "stab-reference-decoder".to_owned(),
                to: "stab-analysis".to_owned(),
                kind: DependencyKind::Development,
                optional: false,
            },
            WorkspaceEdge {
                from: "stab-reference-decoder".to_owned(),
                to: "stab-engine".to_owned(),
                kind: DependencyKind::Development,
                optional: false,
            },
        ],
        declared_path_dependencies: Vec::new(),
        resolved_dependencies: Vec::new(),
    };
    assert!(validate_graph(&permitted).violations.is_empty());

    for (target, kind) in [
        ("stab-core", DependencyKind::Normal),
        ("stab-engine", DependencyKind::Normal),
        ("stab-bench", DependencyKind::Development),
    ] {
        let forbidden = WorkspaceGraph {
            edges: vec![WorkspaceEdge {
                from: "stab-reference-decoder".to_owned(),
                to: target.to_owned(),
                kind,
                optional: false,
            }],
            ..permitted.clone()
        };
        assert_eq!(
            validate_graph(&forbidden)
                .violations
                .first()
                .expect("forbidden reference edge")
                .code,
            "test-support-upward-dependency",
            "{kind:?} reference edge to {target}"
        );
    }
}

#[test]
fn facade_cannot_depend_directly_on_low_level_bits() {
    let graph = WorkspaceGraph {
        packages: vec![
            package("stab-core", "crates"),
            package("stab-bits", "crates"),
        ],
        edges: vec![WorkspaceEdge {
            from: "stab-core".to_owned(),
            to: "stab-bits".to_owned(),
            kind: DependencyKind::Normal,
            optional: false,
        }],
        declared_path_dependencies: Vec::new(),
        resolved_dependencies: Vec::new(),
    };

    let violations = validate_graph(&graph).violations;
    assert_eq!(violations.len(), 1, "{violations:?}");
    assert_eq!(
        violations.first().expect("one forbidden facade edge").code,
        "forbidden-product-edge"
    );
}

#[test]
fn reference_noise_pass_has_only_its_earned_component_edges() {
    let packages = vec![
        package("stab-reference-noise-pass", "test-support"),
        package("stab-analysis", "crates"),
        package("stab-model", "crates"),
        package("stab-core", "crates"),
        package("stab-engine", "crates"),
        package("stab-bench", "ops"),
    ];
    let permitted = WorkspaceGraph {
        packages: packages.clone(),
        edges: vec![
            WorkspaceEdge {
                from: "stab-reference-noise-pass".to_owned(),
                to: "stab-analysis".to_owned(),
                kind: DependencyKind::Normal,
                optional: false,
            },
            WorkspaceEdge {
                from: "stab-reference-noise-pass".to_owned(),
                to: "stab-model".to_owned(),
                kind: DependencyKind::Normal,
                optional: false,
            },
        ],
        declared_path_dependencies: Vec::new(),
        resolved_dependencies: Vec::new(),
    };
    assert!(validate_graph(&permitted).violations.is_empty());

    for (target, kind, optional) in [
        ("stab-core", DependencyKind::Normal, false),
        ("stab-engine", DependencyKind::Development, false),
        ("stab-bench", DependencyKind::Development, false),
        ("stab-model", DependencyKind::Development, false),
        ("stab-analysis", DependencyKind::Normal, true),
    ] {
        let forbidden = WorkspaceGraph {
            edges: vec![WorkspaceEdge {
                from: "stab-reference-noise-pass".to_owned(),
                to: target.to_owned(),
                kind,
                optional,
            }],
            ..permitted.clone()
        };
        assert_eq!(
            validate_graph(&forbidden)
                .violations
                .first()
                .expect("forbidden reference edge")
                .code,
            "test-support-upward-dependency",
            "{kind:?} reference edge to {target} optional={optional}"
        );
    }
}

#[test]
fn new_product_packages_require_an_explicit_role() {
    let graph = graph(vec![package("stab-plugin", "crates")]);
    let report = validate_graph(&graph);
    assert_eq!(
        report
            .violations
            .first()
            .expect("unknown product package should fail")
            .code,
        "unknown-product-package"
    );
}

#[test]
fn every_product_package_requires_the_exact_minimum_rust_version() {
    for contract in PRODUCT_PACKAGE_CONTRACTS {
        let valid = graph(vec![package(contract.name, "crates")]);
        assert!(
            validate_graph(&valid).violations.is_empty(),
            "{} should accept the exact minimum Rust version",
            contract.name
        );

        for actual in [
            None,
            Some(Version::new(1, 97, 1)),
            Some(Version::new(1, 99, 0)),
            Some(Version::new(1, 100, 1)),
            Some(Version::new(1, 101, 0)),
        ] {
            let mut invalid = valid.clone();
            invalid
                .packages
                .first_mut()
                .expect("fixture should contain one package")
                .rust_version = actual.clone();
            let report = validate_graph(&invalid);
            let [violation] = report.violations.as_slice() else {
                panic!(
                    "{} with rust-version {actual:?} should fail only the minimum: {:?}",
                    contract.name, report.violations
                );
            };
            assert_eq!(violation.code, "minimum-rust-version");
            assert!(violation.message.contains(contract.name));
            assert!(violation.message.contains("1.100.0"));
        }
    }
}

#[test]
fn stable_reference_proofs_require_exact_msrv() {
    for proof_package in TEST_SUPPORT_PACKAGE_CONTRACTS
        .iter()
        .map(|contract| contract.name)
    {
        let valid = graph(vec![package(proof_package, "test-support")]);
        assert!(validate_graph(&valid).violations.is_empty());

        for actual in [None, Some(Version::new(1, 97, 1))] {
            let mut invalid = valid.clone();
            invalid
                .packages
                .first_mut()
                .expect("reference package")
                .rust_version = actual;
            assert!(validate_graph(&invalid).violations.iter().any(|violation| {
                violation.code == "minimum-rust-version"
                    && violation.message.contains(proof_package)
            }));
        }
    }
}

fn simd_library_dependency(
    from_package: &str,
    dependency_name: &str,
    to_package: &str,
    kind: DependencyKind,
) -> ResolvedDependencyIdentity {
    ResolvedDependencyIdentity {
        from_package: from_package.to_owned(),
        from_package_id: format!("path+file:///workspace/crates/{from_package}#0.2.0"),
        dependency_name: dependency_name.to_owned(),
        to_package: to_package.to_owned(),
        to_package_id: format!(
            "registry+https://github.com/rust-lang/crates.io-index#{to_package}@1.2.0"
        ),
        to_workspace_package: None,
        kind,
    }
}

#[test]
fn only_the_kernel_crate_may_depend_on_the_simd_library() {
    let packages = vec![
        package("stab-kernels-simd", "crates"),
        package("stab-bits", "crates"),
        package("stab-architecture", "ops"),
    ];
    let mut valid = graph(packages.clone());
    valid.resolved_dependencies = SIMD_LIBRARY_PACKAGES
        .iter()
        .map(|library| {
            simd_library_dependency(
                "stab-kernels-simd",
                library,
                library,
                DependencyKind::Normal,
            )
        })
        .collect();
    assert!(validate_graph(&valid).violations.is_empty());

    for (from_package, dependency_name, to_package, kind) in [
        (
            "stab-bits",
            "fearless_simd",
            "fearless_simd",
            DependencyKind::Normal,
        ),
        (
            "stab-bits",
            "simd",
            "fearless_simd",
            DependencyKind::Development,
        ),
        (
            "stab-bits",
            "fearless_simd_macros",
            "fearless_simd_macros",
            DependencyKind::Build,
        ),
        (
            "stab-architecture",
            "fearless_simd",
            "fearless_simd",
            DependencyKind::Development,
        ),
    ] {
        let mut invalid = graph(packages.clone());
        invalid.resolved_dependencies.push(simd_library_dependency(
            from_package,
            dependency_name,
            to_package,
            kind,
        ));
        let report = validate_graph(&invalid);
        let [violation] = report.violations.as_slice() else {
            panic!(
                "expected one SIMD library violation: {:?}",
                report.violations
            );
        };
        assert_eq!(violation.code, "simd-library-outside-kernel");
        assert!(violation.message.contains(from_package));
        assert!(violation.message.contains(to_package));
    }
}

#[test]
fn external_copies_of_operational_and_test_support_packages_are_rejected() {
    for (shadowed, prefix) in [
        ("stab-architecture", "ops"),
        ("stab-compat-corpus", "test-support"),
        ("stab-reference-decoder", "test-support"),
    ] {
        let packages = vec![
            package(shadowed, prefix),
            package("fixture-consumer", "ops"),
        ];
        let dependency =
            |to_package_id: &str, to_workspace_package: Option<&str>| ResolvedDependencyIdentity {
                from_package: "fixture-consumer".to_owned(),
                from_package_id: "path+file:///workspace/ops/fixture-consumer#0.2.0".to_owned(),
                dependency_name: "shadow".to_owned(),
                to_package: shadowed.to_owned(),
                to_package_id: to_package_id.to_owned(),
                to_workspace_package: to_workspace_package.map(str::to_owned),
                kind: DependencyKind::Development,
            };
        for to_package_id in [
            format!("path+file:///tmp/external/{shadowed}#0.2.0"),
            format!("git+https://example.invalid/{shadowed}#01234567"),
            format!("registry+https://github.com/rust-lang/crates.io-index#{shadowed}@0.2.0"),
        ] {
            let mut invalid = graph(packages.clone());
            invalid
                .resolved_dependencies
                .push(dependency(&to_package_id, None));
            let report = validate_graph(&invalid);
            let [violation] = report.violations.as_slice() else {
                panic!(
                    "{shadowed} copy {to_package_id} should fail once: {:?}",
                    report.violations
                );
            };
            assert_eq!(violation.code, "external-workspace-package-dependency");
            assert!(violation.message.contains(&to_package_id));
        }

        let mut valid = graph(packages);
        valid.resolved_dependencies.push(dependency(
            &format!("path+file:///workspace/{prefix}/{shadowed}#0.2.0"),
            Some(shadowed),
        ));
        assert!(validate_graph(&valid).violations.is_empty(), "{shadowed}");
    }
}

#[test]
fn external_product_dependencies_are_rejected_for_every_source_kind() {
    let packages = vec![
        package("stab-core", "crates"),
        package("fixture-consumer", "ops"),
    ];
    for to_package_id in [
        "path+file:///tmp/external/stab-core#9.9.9",
        "git+https://example.invalid/stab-core#01234567",
        "registry+https://example.invalid/index#stab-core@9.9.9",
    ] {
        let mut invalid = graph(packages.clone());
        invalid
            .resolved_dependencies
            .push(ResolvedDependencyIdentity {
                from_package: "fixture-consumer".to_owned(),
                from_package_id: "path+file:///workspace/ops/fixture-consumer#0.2.0".to_owned(),
                dependency_name: "stab_core_copy".to_owned(),
                to_package: "stab-core".to_owned(),
                to_package_id: to_package_id.to_owned(),
                to_workspace_package: None,
                kind: DependencyKind::Normal,
            });
        let report = validate_graph(&invalid);
        let violation = report
            .violations
            .iter()
            .find(|violation| violation.code == "external-product-dependency")
            .expect("external product package copy should fail");
        assert!(violation.message.contains("fixture-consumer"));
        assert!(violation.message.contains("stab-core"));
        assert!(violation.message.contains(to_package_id));
    }

    let mut valid = graph(packages);
    valid
        .resolved_dependencies
        .push(ResolvedDependencyIdentity {
            from_package: "fixture-consumer".to_owned(),
            from_package_id: "path+file:///workspace/ops/fixture-consumer#0.2.0".to_owned(),
            dependency_name: "stab_core".to_owned(),
            to_package: "stab-core".to_owned(),
            to_package_id: "path+file:///workspace/crates/stab-core#0.2.0".to_owned(),
            to_workspace_package: Some("stab-core".to_owned()),
            kind: DependencyKind::Normal,
        });
    assert!(validate_graph(&valid).violations.is_empty());
}

#[test]
fn publishable_product_packages_require_release_version() {
    let mut core = package("stab-core", "crates");
    core.version = Version::new(0, 2, 1);
    let report = validate_graph(&graph(vec![core.clone()]));
    assert_eq!(report.violations.len(), 1);
    let violation = report
        .violations
        .first()
        .expect("wrong release version should fail");
    assert_eq!(violation.code, "publishable-product-version");
    assert!(violation.message.contains("stab-core"));
    assert!(violation.message.contains("0.2.1"));
    assert!(violation.message.contains("0.2.0"));

    core.publish = Some(Vec::new());
    assert!(
        validate_graph(&graph(vec![core])).violations.is_empty(),
        "unpublished product packages are outside the publication-version contract"
    );
}

#[test]
fn publishable_product_path_dependencies_require_exact_release_version() {
    let packages = vec![
        package("stab-records", "crates"),
        package("stab-bits", "crates"),
    ];
    for kind in [
        DependencyKind::Normal,
        DependencyKind::Development,
        DependencyKind::Build,
    ] {
        let mut exact = graph(packages.clone());
        exact.declared_path_dependencies = vec![DeclaredPathDependency {
            from: "stab-records".to_owned(),
            to: "stab-bits".to_owned(),
            kind,
            version_req: VersionReq::parse("=0.2.0")
                .expect("exact release requirement should parse"),
        }];
        assert!(
            validate_graph(&exact).violations.is_empty(),
            "{kind:?} exact path requirement should pass"
        );

        for requirement in ["0.2.0", "=0.2", ">=0.2.0", "=0.2.0-alpha.1"] {
            let mut inexact = exact.clone();
            inexact
                .declared_path_dependencies
                .first_mut()
                .expect("fixture should contain its path dependency")
                .version_req =
                VersionReq::parse(requirement).expect("fixture requirement should parse");
            let report = validate_graph(&inexact);
            let violation = report
                .violations
                .iter()
                .find(|violation| violation.code == "publishable-product-path-version")
                .expect("inexact product path requirement should fail");
            assert!(violation.message.contains("stab-records"));
            assert!(violation.message.contains("stab-bits"));
            assert!(violation.message.contains("=0.2.0"));
        }
    }
}

#[test]
fn operational_and_test_support_packages_must_be_unpublished() {
    for (name, prefix, expected_code) in [
        (
            "stab-architecture",
            "ops",
            "operational-package-publishable",
        ),
        (
            "stab-compat-corpus",
            "test-support",
            "test-support-package-publishable",
        ),
    ] {
        let mut support = package(name, prefix);
        support.publish = None;
        let report = validate_graph(&graph(vec![support]));
        let violation = report
            .violations
            .first()
            .expect("publishable support package should fail");
        assert_eq!(violation.code, expected_code);
        assert!(violation.message.contains(name));
        assert!(violation.message.contains("publish = false"));
    }
}

#[test]
fn cli_exposes_exactly_one_stab_binary() {
    let valid = package("stab-cli", "crates");
    assert!(
        validate_graph(&graph(vec![valid.clone()]))
            .violations
            .is_empty()
    );

    for targets in [
        Vec::new(),
        vec!["stab-cli".to_owned()],
        vec!["stab".to_owned(), "stab-helper".to_owned()],
    ] {
        let mut cli = valid.clone();
        cli.binary_targets = targets;
        let report = validate_graph(&graph(vec![cli]));
        let violation = report
            .violations
            .iter()
            .find(|violation| violation.code == "product-binary-targets")
            .expect("invalid CLI binary targets should fail");
        assert!(violation.message.contains("[\"stab\"]"));
    }
}
