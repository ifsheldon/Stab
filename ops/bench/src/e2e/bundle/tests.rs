use super::*;
use crate::e2e::data::{OutputKey, OutputWitness, RecordCensus};
use crate::e2e::host::{HostProfile, SwapSnapshot};
use crate::e2e::model::{
    BOOTSTRAP_RESAMPLES, BOOTSTRAP_SEED, COMMAND_TIMEOUT_SECONDS, CONFIDENCE_LEVEL, Case,
    Comparator, DataRecipe, Family, MAXIMUM_CASE_RSS_BYTES, MAXIMUM_OUTPUT_BYTES,
    MAXIMUM_TEMPERATURE_MILLIDEGREES, OutputContract, Policy, RecordFormat, RunnerKind,
    STIM_COMMIT, STIM_VERSION, SemanticWork, SizeClass, StimIdentity, TierPolicy, WorkUnit,
    Workload,
};
use crate::e2e::report::{
    CaseCorrectness, CaseSamples, RUN_SCHEMA_VERSION, SAMPLES_SCHEMA_VERSION,
};
use crate::e2e::statistics::{PairOrder, PairedTiming, StabTiming};

#[test]
fn replay_verifies_every_source_byte_and_rejects_extra_entries() {
    let directory = tempfile::tempdir().expect("bundle directory");
    publish_fixture(directory.path());
    let verified = verify_bundle(directory.path()).expect("verified bundle");
    let mut inconsistent = verified.metadata.clone();
    inconsistent.formal = true;
    assert!(validate_metadata(&verified.suite, &inconsistent).is_err());

    fs::write(directory.path().join("unexpected"), b"extra").expect("extra entry");
    assert!(verify_bundle(directory.path()).is_err());
    fs::remove_file(directory.path().join("unexpected")).expect("remove extra entry");

    let samples = directory.path().join("samples.json");
    let mut bytes = fs::read(&samples).expect("samples");
    bytes.push(b' ');
    fs::write(samples, bytes).expect("tamper samples");
    assert!(verify_bundle(directory.path()).is_err());
}

#[cfg(unix)]
#[test]
fn replay_rejects_a_symlinked_bundle_member() {
    let directory = tempfile::tempdir().expect("bundle directory");
    publish_fixture(directory.path());
    let samples = directory.path().join("samples.json");
    fs::remove_file(&samples).expect("remove samples");
    std::os::unix::fs::symlink("report.json", samples).expect("symlink samples");
    assert!(verify_bundle(directory.path()).is_err());
}

#[test]
fn release_pointer_and_descendant_scope_fail_closed() {
    let valid = CurrentEvidence {
        schema_version: 1,
        measured_source_commit: "a".repeat(40),
        full_bundle: PathBuf::from("benchmarks/evidence/aarch64/run/full"),
        soak_bundle: PathBuf::from("benchmarks/evidence/aarch64/run/soak"),
    };
    assert!(valid.validate().is_ok());
    let mut invalid = valid;
    invalid.soak_bundle = invalid.full_bundle.clone();
    assert!(invalid.validate().is_err());

    assert!(release_descendant_path(
        "benchmarks/evidence/aarch64/run/full/report.json"
    ));
    assert!(release_descendant_path("benchmarks/suite.toml"));
    assert!(!release_descendant_path("crates/stab-engine/src/lib.rs"));
    assert!(!release_descendant_path("Cargo.toml"));
}

#[test]
fn changed_paths_report_both_sides_of_a_rename_and_odd_names() {
    let repository = tempfile::tempdir().expect("throwaway repository");
    let git = |args: &[&str]| {
        let mut full = vec![
            "-c",
            "user.name=stab-bench test",
            "-c",
            "user.email=stab-bench@invalid",
            "-c",
            "commit.gpgsign=false",
        ];
        full.extend_from_slice(args);
        super::super::commands::command_output(
            Path::new("git"),
            &full,
            b"",
            repository.path(),
            1 << 20,
            None,
            60,
        )
        .expect("git command")
    };
    git(&["init", "-q"]);
    fs::create_dir_all(repository.path().join("crates")).expect("source directory");
    fs::write(repository.path().join("crates/lib.rs"), b"fn source() {}\n").expect("source");
    git(&["add", "-A"]);
    git(&["commit", "-q", "-m", "measured"]);
    let measured = String::from_utf8(git(&["rev-parse", "HEAD"])).expect("commit id");
    fs::create_dir_all(repository.path().join("benchmarks/evidence/aarch64"))
        .expect("evidence directory");
    fs::rename(
        repository.path().join("crates/lib.rs"),
        repository.path().join("benchmarks/evidence/aarch64/lib.rs"),
    )
    .expect("rename source into evidence");
    fs::write(repository.path().join("odd\nname"), b"x").expect("odd name");
    git(&["add", "-A"]);
    git(&["commit", "-q", "-m", "descendant"]);

    let range = format!("{}..HEAD", measured.trim());
    let paths = changed_paths(repository.path(), &range).expect("changed paths");

    assert_eq!(
        paths,
        [
            "benchmarks/evidence/aarch64/lib.rs",
            "crates/lib.rs",
            "odd\nname"
        ]
    );
    assert!(paths.iter().any(|path| !release_descendant_path(path)));
}

#[test]
fn committed_release_evidence_replays_under_the_shared_derivation() {
    let root = RepoRoot::resolve(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .as_path(),
    )
    .expect("repository root");
    let pointer = CurrentEvidence::read(&root).expect("current evidence pointer");
    let full = verify_bundle(&evidence_bundle_path(&root, &pointer.full_bundle).expect("full"))
        .expect("committed full bundle replays");
    let soak = verify_bundle(&evidence_bundle_path(&root, &pointer.soak_bundle).expect("soak"))
        .expect("committed soak bundle replays");
    validate_candidate_sources(&full, &soak).expect("committed bundles pair");
}

#[test]
fn baseline_eligibility_accepts_only_unfailed_formal_gate_states() {
    let directory = tempfile::tempdir().expect("bundle directory");
    publish_fixture(directory.path());
    let mut report = verify_bundle(directory.path())
        .expect("verified bundle")
        .report;
    report.parity = GateOutcome::Passed;
    report.self_regression = GateOutcome::Unseeded;
    report.memory = GateOutcome::Passed;
    assert!(eligible_for_baseline(&report));

    report.self_regression = GateOutcome::Failed;
    assert!(!eligible_for_baseline(&report));
    report.self_regression = GateOutcome::Unseeded;
    report.parity = GateOutcome::Diagnostic;
    assert!(!eligible_for_baseline(&report));
}

fn publish_fixture(directory: &Path) {
    let case = Case {
        id: "small".to_string(),
        size_class: SizeClass::Small,
        batch: 1,
        work: SemanticWork {
            amount: 64,
            unit: WorkUnit::Shots,
        },
        maximum_stab_peak_rss_bytes: MAXIMUM_CASE_RSS_BYTES,
        workload: Workload::RustPipeline {
            shots: 64,
            minimum_logical_failures: 1,
            maximum_logical_failures: 10,
            seed: 7,
        },
    };
    let family = Family {
        id: "pipeline".to_string(),
        description: "reusable pipeline".to_string(),
        runner: RunnerKind::Rust,
        comparator: Comparator::SelfOnly,
        prerequisites: vec!["sampling.test".to_string()],
        cases: vec![case],
    };
    let stab_outputs = BTreeMap::from([(
        OutputKey::Pipeline,
        OutputWitness {
            bytes: 4,
            sha256: hex::encode(Sha256::digest(b"64:1")),
            one_bits: None,
            census: None,
        },
    )]);
    let stab_only = vec![StabTiming {
        index: 0,
        seconds: 0.01,
        work: 64,
        peak_rss_bytes: 1 << 20,
        output_bytes: 4,
    }];
    publish_single_case(
        directory,
        family,
        BTreeMap::new(),
        stab_outputs,
        Vec::new(),
        stab_only,
    );
}

/// Publishes a smoke bundle holding one case with the given witnesses and samples. The report is
/// derived from the witnesses without their censuses, so a fixture can carry censuses that the
/// replay derivation must reject.
fn publish_single_case(
    directory: &Path,
    family: Family,
    stim_outputs: BTreeMap<OutputKey, OutputWitness>,
    stab_outputs: BTreeMap<OutputKey, OutputWitness>,
    paired: Vec<PairedTiming>,
    stab_only: Vec<StabTiming>,
) {
    assert_eq!(family.cases.len(), 1, "a single-case fixture");
    let case = &family.cases[0];
    let case_id = crate::e2e::model::CaseId::new(&family.id, &case.id);
    let digest = case.digest(&family.id).expect("case digest");
    let suite = Suite {
        schema_version: 1,
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
        families: vec![family],
    };
    suite.validate().expect("fixture suite");
    let suite_bytes = toml::to_string_pretty(&suite)
        .expect("suite TOML")
        .into_bytes();
    let host = HostProfile {
        architecture: "aarch64".to_string(),
        cpu_model: "test".to_string(),
        logical_cpus: 4,
        affinity_cpu: Some(2),
        kernel_release: "test".to_string(),
        thermal: Vec::new(),
        swap: SwapSnapshot {
            configured: Vec::new(),
            pages_in: 0,
            pages_out: 0,
        },
    };
    let metadata = RunMetadata {
        schema_version: RUN_SCHEMA_VERSION,
        suite_sha256: hex::encode(Sha256::digest(&suite_bytes)),
        source_commit: "b".repeat(40),
        source_clean: false,
        stim_commit: STIM_COMMIT.to_string(),
        stim_binary_sha256: "1".repeat(64),
        stab_binary_sha256: "2".repeat(64),
        bench_binary_sha256: "3".repeat(64),
        rustc: "rustc test".to_string(),
        target: "aarch64-test".to_string(),
        tier: Tier::Smoke,
        formal: false,
        host_before: host.clone(),
        host_after: host,
        selected_cases: vec![case_id.clone()],
        warmups: Some(0),
    };
    let correctness = CorrectnessReport {
        schema_version: CORRECTNESS_SCHEMA_VERSION,
        cases: vec![CaseCorrectness {
            case_id: case_id.clone(),
            input_sha256: hex::encode(Sha256::digest([])),
            stim_outputs,
            stab_outputs,
        }],
    };
    let samples = RawSamples {
        schema_version: SAMPLES_SCHEMA_VERSION,
        cases: vec![CaseSamples {
            case_id,
            case_digest: digest,
            paired,
            stab_only,
        }],
    };
    let mut without_census = correctness.clone();
    for case in &mut without_census.cases {
        for witness in case
            .stim_outputs
            .values_mut()
            .chain(case.stab_outputs.values_mut())
        {
            witness.census = None;
        }
    }
    let report = DerivedReport::derive(&suite, &metadata, &without_census, &samples)
        .expect("derived report");
    publish_bundle(
        directory,
        &suite_bytes,
        &metadata,
        &correctness,
        &samples,
        &report,
    )
    .expect("publish bundle");
}

/// A Stim-paired smoke case whose stdout is 4096 one-bit B8 records.
fn publish_record_fixture(
    directory: &Path,
    stim: Option<RecordCensus>,
    stab: Option<RecordCensus>,
) {
    const RECORDS: u64 = 4096;
    const RECORD_COUNT: usize = 4096;
    let family = Family {
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
                amount: RECORDS,
                unit: WorkUnit::Shots,
            },
            maximum_stab_peak_rss_bytes: MAXIMUM_CASE_RSS_BYTES,
            workload: Workload::Cli {
                args: vec![
                    "sample".to_string(),
                    "--shots".to_string(),
                    RECORDS.to_string(),
                ],
                stdin: DataRecipe::Empty,
                files: Vec::new(),
                stdout: OutputContract::Records {
                    format: RecordFormat::B8,
                    records: RECORD_COUNT,
                    bits: 1,
                    minimum_one_bits: 0,
                    maximum_one_fraction: 1.0,
                },
            },
        }],
    };
    let witness = |census: Option<RecordCensus>, digit: char| {
        BTreeMap::from([(
            OutputKey::Stdout,
            OutputWitness {
                bytes: RECORDS,
                sha256: digit.to_string().repeat(64),
                one_bits: Some(census.map_or(2048, |census| census.ones)),
                census,
            },
        )])
    };
    let paired = vec![PairedTiming {
        index: 0,
        order: PairOrder::for_index(0),
        stim_seconds: 0.01,
        stab_seconds: 0.01,
        stim_work: RECORDS,
        stab_work: RECORDS,
        stim_peak_rss_bytes: 1 << 20,
        stab_peak_rss_bytes: 1 << 20,
        stim_output_bytes: RECORDS,
        stab_output_bytes: RECORDS,
    }];
    publish_single_case(
        directory,
        family,
        witness(stim, 'a'),
        witness(stab, 'b'),
        paired,
        Vec::new(),
    );
}

#[test]
fn replay_repeats_the_recorded_one_bit_census_comparison() {
    let census = |ones: u64| RecordCensus {
        records: 4096,
        ones,
        // One bit per record, so every per-record count is 0 or 1 and squares sum to the ones.
        sum_squares: u128::from(ones),
    };
    let replay = |stim: Option<RecordCensus>, stab: Option<RecordCensus>| {
        let directory = tempfile::tempdir().expect("bundle directory");
        publish_record_fixture(directory.path(), stim, stab);
        let correctness =
            fs::read_to_string(directory.path().join("correctness.json")).expect("correctness");
        (
            verify_bundle(directory.path()).map(|_| ()),
            correctness.contains("\"census\""),
        )
    };

    let (agreeing, written) = replay(Some(census(2048)), Some(census(2060)));
    agreeing.expect("a census inside six standard errors replays");
    assert!(written, "a recorded census is serialized");

    let (disagreeing, _) = replay(Some(census(2048)), Some(census(3000)));
    let error = disagreeing
        .expect_err("an edited census must fail replay")
        .to_string();
    assert!(
        error.contains("sample.small output stdout") && error.contains("standard errors"),
        "{error}"
    );

    let (one_sided, _) = replay(Some(census(2048)), None);
    let error = one_sided
        .expect_err("a census on one side only must fail replay")
        .to_string();
    assert!(error.contains("only the Stim witness"), "{error}");

    let (legacy, written) = replay(None, None);
    legacy.expect("bundles that predate the census field still verify");
    assert!(!written, "an absent census is not serialized");
}
