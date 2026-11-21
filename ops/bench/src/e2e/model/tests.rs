use super::*;

fn minimal_suite() -> Suite {
    Suite {
        schema_version: SUITE_SCHEMA_VERSION,
        stim: StimIdentity {
            version: STIM_VERSION.to_string(),
            commit: STIM_COMMIT.to_string(),
        },
        policy: Policy {
            timing_boundary: TIMING_BOUNDARY.to_string(),
            parity_max_ratio: 1.25,
            self_regression_max_ratio: 1.15,
            bootstrap_seed: BOOTSTRAP_SEED,
            bootstrap_resamples: BOOTSTRAP_RESAMPLES,
            confidence_level: CONFIDENCE_LEVEL,
            maximum_temperature_millidegrees: MAXIMUM_TEMPERATURE_MILLIDEGREES,
            maximum_output_bytes: MAXIMUM_OUTPUT_BYTES,
            command_timeout_seconds: COMMAND_TIMEOUT_SECONDS,
        },
        tiers: BTreeMap::from([
            (
                Tier::Smoke,
                TierPolicy {
                    warmups: 0,
                    samples: 1,
                },
            ),
            (
                Tier::Full,
                TierPolicy {
                    warmups: 2,
                    samples: 9,
                },
            ),
            (
                Tier::Soak,
                TierPolicy {
                    warmups: 3,
                    samples: 21,
                },
            ),
        ]),
        expected_release_families: 1,
        expected_release_cases: 1,
        self_baselines: Vec::new(),
        families: vec![Family {
            id: "sample".to_string(),
            description: "sample records".to_string(),
            runner: RunnerKind::Cli,
            comparator: Comparator::StimCli,
            prerequisites: vec!["cli.sample".to_string()],
            cases: vec![Case {
                id: "small".to_string(),
                size_class: SizeClass::Small,
                batch: 1,
                work: SemanticWork {
                    amount: 64,
                    unit: WorkUnit::Shots,
                },
                maximum_stab_peak_rss_bytes: MAXIMUM_CASE_RSS_BYTES,
                workload: Workload::Cli {
                    args: vec![
                        "sample".to_string(),
                        "--shots".to_string(),
                        "64".to_string(),
                    ],
                    stdin: DataRecipe::Empty,
                    files: Vec::new(),
                    stdout: OutputContract::Records {
                        format: RecordFormat::B8,
                        records: 64,
                        bits: 1,
                        minimum_one_bits: 0,
                        maximum_one_fraction: 1.0,
                    },
                },
            }],
        }],
    }
}

#[test]
fn suite_rejects_duplicate_ids_caps_and_inconsistent_work() {
    let mut suite = minimal_suite();
    suite.validate().expect("minimal suite is valid");

    let duplicate = suite.families[0].cases[0].clone();
    suite.families[0].cases.push(duplicate);
    suite.expected_release_cases = 2;
    let error = suite.validate().expect_err("duplicate case").to_string();
    assert!(error.contains("duplicate case id"));

    let mut suite = minimal_suite();
    if let Workload::Cli { args, .. } = &mut suite.families[0].cases[0].workload {
        args.clear();
    }
    assert!(
        suite
            .validate()
            .expect_err("empty command")
            .to_string()
            .contains("named subcommand")
    );
}

#[test]
fn semantic_work_must_match_cli_records_and_shots() {
    let mut suite = minimal_suite();
    suite.families[0].cases[0].work.amount = 63;
    let error = suite.validate().expect_err("mismatched shots").to_string();
    assert!(error.contains("--shots"));
    assert!(error.contains("output record count"));

    let case = &mut suite.families[0].cases[0];
    case.work.unit = WorkUnit::Records;
    let error = suite
        .validate()
        .expect_err("records without an input recipe")
        .to_string();
    assert!(error.contains("input recipe"));
}

#[test]
fn contract_digest_ignores_baseline_values_but_owns_workloads() {
    let mut suite = minimal_suite();
    let initial = suite.contract_digest().expect("initial digest");
    let case_digest = suite.families[0].cases[0]
        .digest("sample")
        .expect("case digest");
    suite.self_baselines.push(SelfBaseline {
        architecture: "aarch64".to_string(),
        cpu_model: "test".to_string(),
        rustc: "rustc test".to_string(),
        target: "aarch64-test".to_string(),
        timing_boundary: TIMING_BOUNDARY.to_string(),
        case_id: CaseId::new("sample", "small"),
        case_digest,
        median_seconds_per_work: 0.1,
        upper_seconds_per_work: 0.2,
    });
    assert_eq!(suite.contract_digest().expect("seeded digest"), initial);

    suite.families[0].cases[0].batch = 2;
    assert_ne!(suite.contract_digest().expect("changed digest"), initial);
}

#[test]
fn suite_rejects_weakened_evidence_policy() {
    let mut suite = minimal_suite();
    suite.stim.commit = "a".repeat(40);
    suite.tiers.get_mut(&Tier::Full).expect("full tier").samples = 1;
    suite.policy.bootstrap_resamples = 1;
    suite.families[0].cases[0].maximum_stab_peak_rss_bytes = MAXIMUM_CASE_RSS_BYTES + 1;
    let error = suite.validate().expect_err("weakened policy").to_string();
    assert!(error.contains("Stim identity"));
    assert!(error.contains("tier full"));
    assert!(error.contains("bootstrap_resamples"));
    assert!(error.contains("RSS policy cap"));
}
