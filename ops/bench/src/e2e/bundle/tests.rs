use super::*;
use crate::e2e::data::OutputWitness;
use crate::e2e::host::{HostProfile, SwapSnapshot};
use crate::e2e::model::{
    BOOTSTRAP_RESAMPLES, BOOTSTRAP_SEED, COMMAND_TIMEOUT_SECONDS, CONFIDENCE_LEVEL, Case,
    Comparator, Family, MAXIMUM_CASE_RSS_BYTES, MAXIMUM_OUTPUT_BYTES,
    MAXIMUM_TEMPERATURE_MILLIDEGREES, Policy, RunnerKind, STIM_COMMIT, STIM_VERSION, SemanticWork,
    SizeClass, StimIdentity, TierPolicy, WorkUnit, Workload,
};
use crate::e2e::report::{
    CaseCorrectness, CaseSamples, RUN_SCHEMA_VERSION, SAMPLES_SCHEMA_VERSION,
};
use crate::e2e::statistics::StabTiming;

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
        families: vec![Family {
            id: "pipeline".to_string(),
            description: "reusable pipeline".to_string(),
            runner: RunnerKind::Rust,
            comparator: Comparator::SelfOnly,
            prerequisites: vec!["sampling.test".to_string()],
            cases: vec![case.clone()],
        }],
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
    let case_id = crate::e2e::model::CaseId::new("pipeline", "small");
    let digest = case.digest("pipeline").expect("case digest");
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
            stim_outputs: BTreeMap::new(),
            stab_outputs: BTreeMap::from([(
                crate::e2e::data::OutputKey::Pipeline,
                OutputWitness {
                    bytes: 4,
                    sha256: hex::encode(Sha256::digest(b"64:1")),
                    one_bits: None,
                },
            )]),
        }],
    };
    let samples = RawSamples {
        schema_version: SAMPLES_SCHEMA_VERSION,
        cases: vec![CaseSamples {
            case_id,
            case_digest: digest,
            paired: Vec::new(),
            stab_only: vec![StabTiming {
                index: 0,
                seconds: 0.01,
                work: 64,
                peak_rss_bytes: 1 << 20,
                output_bytes: 4,
            }],
        }],
    };
    let report =
        DerivedReport::derive(&suite, &metadata, &correctness, &samples).expect("derived report");
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
