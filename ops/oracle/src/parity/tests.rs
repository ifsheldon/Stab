use super::*;

mod catalog_semantics;
mod pinned_stim;
mod support;

fn test_owner() -> TestOwner {
    TestOwner {
        package: "stab-oracle".to_string(),
        target: CargoTestTarget::Lib,
        name: "parity_owner".to_string(),
    }
}

fn format_routes() -> Vec<FormatRoute> {
    EXPECTED_FORMAT_ROUTES
        .into_iter()
        .map(|id| {
            let (command, role, record_types) =
                expected_format_route_shape(id).expect("known format route");
            FormatRoute {
                id: id.to_string(),
                command: command.to_string(),
                role,
                record_types: record_types.to_vec(),
                accepted_formats: EXPECTED_FORMATS
                    .into_iter()
                    .filter(|format| {
                        let (rejected, divergent) = expected_format_route_exceptions(id);
                        !rejected.contains(format) && !divergent.contains(format)
                    })
                    .map(str::to_string)
                    .collect(),
                rejected_formats: expected_format_route_exceptions(id)
                    .0
                    .iter()
                    .map(ToString::to_string)
                    .collect(),
                stim_bug_divergences: expected_format_route_exceptions(id)
                    .1
                    .iter()
                    .map(ToString::to_string)
                    .collect(),
                dets_observable_order: (id == "detect-output")
                    .then_some(DetsObservableOrder::PrependByDefault),
                stim_refs: vec!["doc/usage_command_line.md".to_string()],
            }
        })
        .collect()
}

fn command_surfaces() -> Vec<CommandSurface> {
    EXPECTED_COMMAND_SURFACES
        .into_iter()
        .map(|command| CommandSurface {
            command: command.to_string(),
            options: commands::expected_command_options(command)
                .expect("known command surface")
                .iter()
                .map(ToString::to_string)
                .collect(),
            stim_refs: vec!["doc/usage_command_line.md".to_string()],
        })
        .collect()
}

fn route_coverage(routes: &[FormatRoute]) -> Vec<String> {
    routes
        .iter()
        .flat_map(|route| {
            route
                .accepted_formats
                .iter()
                .chain(&route.rejected_formats)
                .chain(&route.stim_bug_divergences)
                .map(|format| format!("route:{}/{format}", route.id))
        })
        .collect()
}

fn option_coverage(surfaces: &[CommandSurface]) -> Vec<String> {
    surfaces
        .iter()
        .flat_map(|surface| {
            surface
                .options
                .iter()
                .map(|option| format!("option:{}/{}", surface.command, option))
        })
        .collect()
}

fn missing_family(id: &str, coverage: Vec<String>) -> Family {
    Family {
        id: id.to_string(),
        area: Area::CircuitModel,
        contract: "A meaningful behavior contract.".to_string(),
        stim_refs: vec!["doc/source.md".to_string()],
        coverage,
        disposition: Disposition::Missing {
            owner: "stab-model".to_string(),
            milestone: Milestone::P1,
        },
    }
}

fn done_family(id: &str) -> Family {
    Family {
        id: id.to_string(),
        area: Area::CircuitModel,
        contract: "A meaningful implemented behavior contract.".to_string(),
        stim_refs: vec!["doc/source.md".to_string()],
        coverage: Vec::new(),
        disposition: Disposition::Done {
            owner: "stab-model".to_string(),
            evidence: Evidence::Verified {
                test: test_owner(),
                stim_reproduction: None,
            },
        },
    }
}

fn implemented_without_owner(id: &str, milestone: Milestone) -> Family {
    Family {
        id: id.to_string(),
        area: Area::CircuitModel,
        contract: "An implemented behavior awaiting one lean owner.".to_string(),
        stim_refs: vec!["doc/source.md".to_string()],
        coverage: Vec::new(),
        disposition: Disposition::Done {
            owner: "stab-model".to_string(),
            evidence: Evidence::NeedsOwner { milestone },
        },
    }
}

fn divergence_family(
    id: &str,
    divergence_kind: DivergenceKind,
    stim_reproduction: Option<TestOwner>,
) -> Family {
    Family {
        id: id.to_string(),
        area: Area::ResultFormats,
        contract: "An intentional, independently verified behavior divergence.".to_string(),
        stim_refs: vec!["doc/source.md".to_string()],
        coverage: Vec::new(),
        disposition: Disposition::Divergence {
            owner: "stab-records".to_string(),
            divergence_kind,
            rationale: "Stab deliberately preserves the documented behavior.".to_string(),
            evidence: Evidence::Verified {
                test: test_owner(),
                stim_reproduction,
            },
        },
    }
}

fn divergence_without_owner(
    id: &str,
    divergence_kind: DivergenceKind,
    milestone: Milestone,
) -> Family {
    Family {
        id: id.to_string(),
        area: Area::ResultFormats,
        contract: "An implemented behavior divergence awaiting one lean owner.".to_string(),
        stim_refs: vec!["doc/source.md".to_string()],
        coverage: Vec::new(),
        disposition: Disposition::Divergence {
            owner: "stab-records".to_string(),
            divergence_kind,
            rationale: "Stab deliberately preserves the documented behavior.".to_string(),
            evidence: Evidence::NeedsOwner { milestone },
        },
    }
}

fn test_root() -> (tempfile::TempDir, RepoRoot) {
    let directory = tempfile::tempdir().expect("temporary repository");
    let root = directory
        .path()
        .canonicalize()
        .expect("canonical temporary root");
    std::fs::create_dir_all(root.join("vendor/stim/doc")).expect("Stim doc directory");
    std::fs::write(root.join("vendor/stim/doc/source.md"), "source\n").expect("source reference");
    std::fs::write(
        root.join("vendor/stim/doc/usage_command_line.md"),
        "usage\n",
    )
    .expect("CLI reference");
    (directory, RepoRoot { path: root })
}

fn valid_ledger() -> Ledger {
    let routes = format_routes();
    let command_surfaces = command_surfaces();
    let mut coverage = route_coverage(&routes);
    coverage.extend(option_coverage(&command_surfaces));
    Ledger {
        schema_version: LEDGER_SCHEMA_VERSION,
        required_fixture_ids: vec![
            FixtureId::try_from("required".to_string()).expect("fixture id"),
        ],
        stim: StimIdentity {
            version: STIM_TAG.to_string(),
            commit: STIM_COMMIT.to_string(),
        },
        families: vec![missing_family("all.missing", coverage)],
        command_surfaces,
        format_routes: routes,
    }
}

fn route_only_expected() -> ExpectedCoverage {
    ExpectedCoverage {
        members: BTreeSet::new(),
        canonical_gates: BTreeSet::new(),
        aliases: BTreeSet::new(),
    }
}

fn validation_message(error: ParityError) -> String {
    error.to_string()
}

#[test]
fn valid_atomic_ledger_passes() {
    let (_directory, root) = test_root();
    validate(&root, &valid_ledger(), &route_only_expected()).expect("valid ledger");
}

#[test]
fn tagged_dispositions_reject_fields_from_other_states() {
    let source = r#"
id = "format.01"
area = "result-formats"
contract = "Dense text records."
stim_refs = ["doc/result_formats.md"]
status = "done"
owner = "stab-records"
milestone = "P1"

[evidence]
status = "verified"
[evidence.test]
package = "stab-records"
kind = "lib"
name = "dense_text_records"
"#;
    let error = toml::from_str::<Family>(source).expect_err("foreign status field");
    assert!(error.to_string().contains("milestone"));
}

#[test]
fn all_four_dispositions_parse() {
    let cases = [
        (
            "done",
            r#"
id = "case.done"
area = "sampling"
contract = "Done behavior."
stim_refs = ["doc/gates.md"]
status = "done"
owner = "stab-engine"
[evidence]
status = "verified"
[evidence.test]
package = "stab-engine"
kind = "lib"
name = "owner"
"#,
        ),
        (
            "missing",
            r#"
id = "case.missing"
area = "sampling"
contract = "Missing behavior."
stim_refs = ["doc/gates.md"]
status = "missing"
owner = "stab-engine"
milestone = "P4"
"#,
        ),
        (
            "deferred",
            r#"
id = "case.deferred"
area = "cli"
contract = "Deferred behavior."
stim_refs = ["doc/usage_command_line.md"]
status = "deferred"
rationale = "This product is explicitly outside the selected scope."
"#,
        ),
        (
            "divergence",
            r#"
id = "case.divergence"
area = "resource-safety"
contract = "Bounded hostile input."
stim_refs = ["doc/result_formats.md"]
status = "divergence"
owner = "stab-records"
divergence_kind = "resource-limit"
rationale = "Stab rejects the input before unbounded allocation."
[evidence]
status = "verified"
[evidence.test]
package = "stab-records"
kind = "lib"
name = "bounded_input"
"#,
        ),
    ];
    for (expected, source) in cases {
        let family = toml::from_str::<Family>(source).expect(expected);
        assert_eq!(family.status().as_str(), expected);
    }
}

#[test]
fn implementation_and_evidence_status_are_independent() {
    let family = implemented_without_owner("case.needs-owner", Milestone::P1);
    assert_eq!(family.status(), Status::Done);
    assert_eq!(family.evidence_status(), EvidenceStatus::NeedsOwner);
    assert!(family.test().is_none());

    let ledger = Ledger {
        families: vec![family],
        ..valid_ledger()
    };
    assert!(collect_owner_tests(&ledger).is_empty());
}

#[test]
fn implemented_families_cannot_postpone_canonical_ownership_past_p1() {
    let (_directory, root) = test_root();
    let mut ledger = valid_ledger();
    ledger.families.push(implemented_without_owner(
        "case.postponed-owner",
        Milestone::P3,
    ));
    ledger
        .families
        .sort_by(|left, right| left.id.cmp(&right.id));
    let message = validation_message(
        validate(&root, &ledger, &route_only_expected()).expect_err("postponed owner"),
    );
    assert!(message.contains("must receive its lean canonical owner in P1"));
}

#[test]
fn malformed_dispositions_fail_closed() {
    let cases = [
        (
            "unknown status",
            r#"
id = "case.unknown"
area = "sampling"
contract = "Unknown state."
stim_refs = ["doc/gates.md"]
status = "partial"
owner = "stab-engine"
"#,
        ),
        (
            "missing owner",
            r#"
id = "case.done"
area = "sampling"
contract = "Done behavior."
stim_refs = ["doc/gates.md"]
status = "done"
[evidence]
status = "verified"
[evidence.test]
package = "stab-engine"
kind = "lib"
name = "owner"
"#,
        ),
        (
            "missing rationale",
            r#"
id = "case.deferred"
area = "cli"
contract = "Deferred behavior."
stim_refs = ["doc/usage_command_line.md"]
status = "deferred"
"#,
        ),
    ];
    for (label, source) in cases {
        toml::from_str::<Family>(source).expect_err(label);
    }
}

#[test]
fn stale_identity_and_unknown_owner_are_rejected() {
    let (_directory, root) = test_root();
    let mut ledger = valid_ledger();
    ledger.stim.commit = "stale".to_string();
    ledger.families.first_mut().expect("family").disposition = Disposition::Missing {
        owner: "stab-future-backend".to_string(),
        milestone: Milestone::P1,
    };
    let message = validation_message(
        validate(&root, &ledger, &route_only_expected()).expect_err("identity and owner"),
    );
    assert!(message.contains("stim.commit is stale"));
    assert!(message.contains("unknown product owner stab-future-backend"));
}

#[test]
fn facade_cannot_own_a_semantic_parity_family() {
    let (_directory, root) = test_root();
    let mut ledger = valid_ledger();
    ledger.families.first_mut().expect("family").disposition = Disposition::Missing {
        owner: "stab-core".to_string(),
        milestone: Milestone::P1,
    };

    let message = validation_message(
        validate(&root, &ledger, &route_only_expected()).expect_err("facade semantic owner"),
    );
    assert!(message.contains("unknown product owner stab-core"));
}

#[test]
fn canonical_semantic_tests_run_in_the_product_owner_package() {
    let (_directory, root) = test_root();
    let mut ledger = valid_ledger();
    ledger.families.push(done_family("zz.owner-package"));
    ledger
        .families
        .sort_by(|left, right| left.id.cmp(&right.id));
    let family = ledger
        .families
        .iter_mut()
        .find(|family| family.id == "zz.owner-package")
        .expect("added family");
    match &mut family.disposition {
        Disposition::Done {
            evidence: Evidence::Verified { test, .. },
            ..
        } => test.package = "stab-core".to_string(),
        disposition => assert!(
            matches!(
                disposition,
                Disposition::Done {
                    evidence: Evidence::Verified { .. },
                    ..
                }
            ),
            "valid ledger starts with verified done evidence"
        ),
    }

    let message = validation_message(
        validate(&root, &ledger, &route_only_expected()).expect_err("mismatched test owner"),
    );
    assert!(message.contains("is owned by stab-model but its canonical test runs in stab-core"));
}

#[test]
fn pinned_oracle_evidence_may_live_outside_the_product_owner_package() {
    let (_directory, root) = test_root();
    validate(&root, &valid_ledger(), &route_only_expected())
        .expect("independent oracle evidence remains valid");
}

#[test]
fn duplicate_ids_and_coverage_are_rejected() {
    let (_directory, root) = test_root();
    let mut ledger = valid_ledger();
    ledger.families.push(missing_family(
        "all.missing",
        vec!["route:sample-output/01".to_string()],
    ));
    let message = validation_message(
        validate(&root, &ledger, &route_only_expected()).expect_err("duplicates"),
    );
    assert!(message.contains("family id all.missing is duplicated"));
    assert!(message.contains("route:sample-output/01 is owned by both"));
}

#[test]
fn unknown_and_unowned_coverage_are_rejected() {
    let (_directory, root) = test_root();
    let mut ledger = valid_ledger();
    ledger
        .families
        .first_mut()
        .expect("family")
        .coverage
        .push("format:unknown".to_string());
    ledger
        .families
        .first_mut()
        .expect("family")
        .coverage
        .retain(|member| member != "route:sample-output/01");
    let message = validation_message(
        validate(&root, &ledger, &route_only_expected()).expect_err("coverage errors"),
    );
    assert!(message.contains("unknown coverage member format:unknown"));
    assert!(message.contains("coverage member route:sample-output/01 has no family owner"));
}

#[test]
fn unsafe_and_missing_stim_references_are_rejected() {
    let (_directory, root) = test_root();
    let mut ledger = valid_ledger();
    ledger.families.first_mut().expect("family").stim_refs =
        vec!["../outside.md".to_string(), "doc/missing.md".to_string()];
    let message = validation_message(
        validate(&root, &ledger, &route_only_expected()).expect_err("unsafe references"),
    );
    assert!(message.contains("unsafe Stim reference ../outside.md"));
    assert!(message.contains("missing or unsafe Stim reference doc/missing.md"));
}

#[cfg(unix)]
#[test]
fn stim_reference_symlinks_are_rejected() {
    use std::os::unix::fs::symlink;

    let (_directory, root) = test_root();
    symlink(
        root.path.join("vendor/stim/doc/source.md"),
        root.path.join("vendor/stim/doc/link.md"),
    )
    .expect("reference symlink");
    let mut ledger = valid_ledger();
    ledger.families.first_mut().expect("family").stim_refs = vec!["doc/link.md".to_string()];
    let message = validation_message(
        validate(&root, &ledger, &route_only_expected()).expect_err("symlink reference"),
    );
    assert!(message.contains("missing or unsafe Stim reference doc/link.md"));
}

#[test]
fn malformed_format_route_is_rejected() {
    let (_directory, root) = test_root();
    let mut ledger = valid_ledger();
    let route = ledger.format_routes.first_mut().expect("format route");
    route.command = "sample".to_string();
    drop(route.accepted_formats.pop());
    let message = validation_message(
        validate(&root, &ledger, &route_only_expected()).expect_err("route shape"),
    );
    assert!(message.contains("must classify each of the six Stim result formats"));
    assert!(message.contains("inconsistent with pinned Stim"));
}

#[test]
fn format_route_rejects_invented_dets_observable_ordering() {
    let (_directory, root) = test_root();
    let mut ledger = valid_ledger();
    let route = ledger
        .format_routes
        .iter_mut()
        .find(|route| route.id == "convert-output")
        .expect("convert output route");
    route.dets_observable_order = Some(DetsObservableOrder::PrependByDefault);
    let message = validation_message(
        validate(&root, &ledger, &route_only_expected()).expect_err("route ordering"),
    );
    assert!(message.contains("DETS observable ordering inconsistent with pinned Stim"));
}

#[test]
fn malformed_command_surface_is_rejected() {
    let (_directory, root) = test_root();
    let mut ledger = valid_ledger();
    ledger.command_surfaces.pop();
    let gen_surface = ledger
        .command_surfaces
        .iter_mut()
        .find(|surface| surface.command == "gen")
        .expect("gen command surface");
    gen_surface.options = vec!["out".to_string(), "--out".to_string(), "--out".to_string()];
    let message = validation_message(
        validate(&root, &ledger, &route_only_expected()).expect_err("command surface defects"),
    );
    assert!(message.contains("command surface sample_dem is missing"));
    assert!(message.contains("command surface gen has invalid option out"));
    assert!(message.contains("command surface gen repeats option --out"));
    assert!(message.contains("command surface gen options are not sorted at --out"));
    assert!(message.contains("command surface gen omits pinned option --in"));
    assert!(message.contains("command surface gen adds non-pinned option out"));
}

#[test]
fn completed_options_are_checked_against_the_live_stab_clap_schema() {
    let mut ledger = valid_ledger();
    ledger
        .command_surfaces
        .retain(|surface| surface.command == "sample");
    ledger.families = vec![Family {
        id: "cli.sample".to_string(),
        area: Area::Cli,
        contract: "The sample command exposes its frozen option surface.".to_string(),
        stim_refs: vec!["doc/source.md".to_string()],
        coverage: option_coverage(&ledger.command_surfaces),
        disposition: Disposition::Done {
            owner: "stab-cli".to_string(),
            evidence: Evidence::NeedsOwner {
                milestone: Milestone::P1,
            },
        },
    }];
    let incomplete = clap::Command::new("stab")
        .subcommand(clap::Command::new("sample").arg(clap::Arg::new("in").long("in")));
    let mut errors = Vec::new();
    commands::validate_stab_command_schema(&ledger, &incomplete, &mut errors);
    assert!(
        errors
            .iter()
            .any(|error| error.contains("command sample omits pinned option --shots")),
        "{errors:?}"
    );

    let mut errors = Vec::new();
    commands::validate_stab_command_schema(&ledger, &stab_cli::command_descriptor(), &mut errors);
    assert!(errors.is_empty(), "{errors:?}");

    ledger
        .families
        .first_mut()
        .expect("sample option family")
        .disposition = Disposition::Missing {
        owner: "stab-cli".to_string(),
        milestone: Milestone::P6,
    };
    let mut errors = Vec::new();
    commands::validate_stab_command_schema(&ledger, &incomplete, &mut errors);
    assert!(
        errors.is_empty(),
        "missing options must remain representable until implementation: {errors:?}"
    );
}

#[test]
fn format_route_outcomes_are_not_collapsed_into_codec_support() {
    let (_directory, root) = test_root();
    let mut ledger = valid_ledger();
    let route = ledger
        .format_routes
        .iter_mut()
        .find(|route| route.id == "convert-output")
        .expect("convert output route");
    route.stim_bug_divergences.clear();
    route.rejected_formats.push("ptb64".to_string());
    let message = validation_message(
        validate(&root, &ledger, &route_only_expected()).expect_err("route outcome"),
    );
    assert!(message.contains("rejection or Stim-bug behavior inconsistent with pinned Stim"));
}

#[test]
fn gate_document_extraction_is_exact() {
    let mut source = String::new();
    for index in 0..81 {
        source.push_str(&format!("### The 'G{index}' Gate\n"));
    }
    for index in 0..12 {
        source.push_str(&format!(
            "Alternate name: <a name=\"A{index}\"></a>`A{index}`\n"
        ));
    }
    let (gates, aliases) = parse_stim_gate_doc(&source).expect("frozen catalog");
    assert_eq!(gates.len(), 81);
    assert_eq!(aliases.len(), 12);

    source.push_str("### The 'EXTRA' Instruction\n");
    let message = validation_message(parse_stim_gate_doc(&source).expect_err("extra gate"));
    assert!(message.contains("82 canonical gates instead of 81"));
}

#[test]
fn exact_selector_listing_requires_one_match() {
    require_one_listing_match("selector", "owner", &["family"], "owner: test\n")
        .expect("one exact test");
    let zero = validation_message(
        require_one_listing_match("selector", "owner", &["family"], "")
            .expect_err("zero selector matches"),
    );
    assert!(zero.contains("resolved to 0 tests"));
    let benchmark_only = validation_message(
        require_one_listing_match("selector", "owner", &["family"], "owner: benchmark\n")
            .expect_err("benchmark is not a canonical behavior test"),
    );
    assert!(benchmark_only.contains("resolved to 0 tests"));
    let multiple = validation_message(
        require_one_listing_match(
            "selector",
            "owner",
            &["one", "two"],
            "owner: test\nowner: test\n",
        )
        .expect_err("multiple selector matches"),
    );
    assert!(multiple.contains("[one, two] resolved to 2 tests"));
}

#[test]
fn exact_owner_execution_includes_ignored_tests() {
    let owner = test_owner();
    assert!(owner.run_args().contains(&"--include-ignored"));
    assert!(owner.display().contains("--include-ignored"));
}

#[test]
fn stim_bug_route_cells_require_stim_bug_dispositions() {
    let (_directory, root) = test_root();
    let mut ledger = valid_ledger();
    ledger.families = vec![done_family("route.wrong")];
    ledger
        .families
        .first_mut()
        .expect("coverage owner")
        .coverage = route_coverage(&ledger.format_routes);
    let message = validation_message(
        validate(&root, &ledger, &route_only_expected()).expect_err("wrong disposition"),
    );
    assert!(message.contains("without a Stim-bug divergence disposition"));
}

#[test]
fn stim_bug_divergences_require_independent_reproductions() {
    let (_directory, root) = test_root();
    let mut ledger = valid_ledger();
    ledger.families.push(divergence_family(
        "zz.stim-bug",
        DivergenceKind::StimBug,
        None,
    ));
    let message = validation_message(
        validate(&root, &ledger, &route_only_expected()).expect_err("missing reproduction"),
    );
    assert!(message.contains("Stim-bug family zz.stim-bug has no independent pinned reproduction"));

    ledger.families.pop();
    ledger.families.push(divergence_family(
        "zz.resource-limit",
        DivergenceKind::ResourceLimit,
        Some(test_owner()),
    ));
    let message = validation_message(
        validate(&root, &ledger, &route_only_expected()).expect_err("foreign reproduction"),
    );
    assert!(message.contains(
        "non-bug divergence family zz.resource-limit cannot carry a Stim-bug reproduction"
    ));

    ledger.families.pop();
    ledger.families.push(divergence_family(
        "zz.stim-bug",
        DivergenceKind::StimBug,
        Some(test_owner()),
    ));
    let message = validation_message(
        validate(&root, &ledger, &route_only_expected()).expect_err("same reproduction"),
    );
    assert!(message.contains(
        "Stim-bug family zz.stim-bug reproduction must be distinct from its Stab regression"
    ));

    ledger.families.pop();
    let mut non_oracle = test_owner();
    non_oracle.package = "stab-cli".to_string();
    non_oracle.name = "independent_reproduction".to_string();
    ledger.families.push(divergence_family(
        "zz.stim-bug",
        DivergenceKind::StimBug,
        Some(non_oracle),
    ));
    let message = validation_message(
        validate(&root, &ledger, &route_only_expected()).expect_err("non-oracle reproduction"),
    );
    assert!(
        message.contains("Stim-bug family zz.stim-bug reproduction must be owned by stab-oracle")
    );
}

#[test]
fn implemented_non_bug_divergences_may_receive_their_lean_owner_in_p1() {
    let (_directory, root) = test_root();
    let mut ledger = valid_ledger();
    ledger.families.push(divergence_without_owner(
        "zz.resource-limit",
        DivergenceKind::ResourceLimit,
        Milestone::P1,
    ));
    validate(&root, &ledger, &route_only_expected()).expect("P1 owner debt is explicit");

    ledger.families.pop();
    ledger.families.push(divergence_without_owner(
        "zz.resource-limit",
        DivergenceKind::ResourceLimit,
        Milestone::P2,
    ));
    let message = validation_message(
        validate(&root, &ledger, &route_only_expected()).expect_err("late owner debt"),
    );
    assert!(message.contains("must receive its lean canonical owner in P1"));

    ledger.families.pop();
    ledger.families.push(divergence_without_owner(
        "zz.stim-bug",
        DivergenceKind::StimBug,
        Milestone::P1,
    ));
    let message = validation_message(
        validate(&root, &ledger, &route_only_expected()).expect_err("unproved Stim bug"),
    );
    assert!(message.contains("must have verified independent evidence"));
}

#[test]
fn shared_owner_selectors_are_deduplicated() {
    let mut ledger = valid_ledger();
    ledger.families = vec![done_family("one.owner"), done_family("two.owner")];
    let owners = collect_owner_tests(&ledger);
    assert_eq!(owners.len(), 1);
    assert_eq!(
        owners.values().next().expect("shared owner").1,
        ["one.owner", "two.owner"]
    );
}

#[test]
fn rendering_is_deterministic_and_escapes_tables() {
    let mut ledger = valid_ledger();
    ledger.families.first_mut().expect("family").contract = "A | B behavior.".to_string();
    let first = render(&ledger);
    let second = render(&ledger);
    assert_eq!(first, second);
    assert!(first.contains("A \\| B behavior."));
    assert!(first.contains("## Result Format Applicability"));
    assert!(first.contains("Finish in P1"));
}
