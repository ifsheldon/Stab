use std::path::Path;

use super::*;

fn contract_identity() -> WorkerContractIdentityEvidence {
    let digest = |value: char| {
        Sha256Digest::try_new(value.to_string().repeat(64)).expect("contract identity digest")
    };
    WorkerContractIdentityEvidence {
        stim_source_sha256: digest('a'),
        stim_build_fingerprint: digest('b'),
        stim_binary_sha256: digest('c'),
        stab_source_sha256: digest('d'),
        stab_build_fingerprint: digest('e'),
        stab_binary_sha256: digest('f'),
    }
}

fn report_identity(
    identity: &WorkerContractIdentityEvidence,
    contract_preflight_sha256: String,
) -> WorkerIdentityEvidence {
    WorkerIdentityEvidence {
        stim_source_sha256: identity.stim_source_sha256.as_str().to_string(),
        stim_build_fingerprint: identity.stim_build_fingerprint.as_str().to_string(),
        stim_binary_sha256: identity.stim_binary_sha256.as_str().to_string(),
        stab_source_sha256: identity.stab_source_sha256.as_str().to_string(),
        stab_build_fingerprint: identity.stab_build_fingerprint.as_str().to_string(),
        stab_binary_sha256: identity.stab_binary_sha256.as_str().to_string(),
        contract_preflight_sha256,
    }
}

fn source_contracts() -> (RepoRoot, Vec<super::super::group::GroupContract>) {
    let root = RepoRoot::resolve(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
        .expect("repository root");
    let contracts =
        super::super::group::load_groups(&root, crate::qualification::EXPECTED_FROZEN_DIGEST)
            .expect("runtime contracts");
    (root, contracts)
}

fn expected_current_probes(
    contracts: &[super::super::group::GroupContract],
) -> Vec<WorkerContractProbeEvidence> {
    let output_digest = "9".repeat(64);
    let mut probes = Vec::new();
    for contract in contracts {
        let scale = contract.scales.first().expect("smallest scale");
        let case_id = preflight::accepted_case_id(contract, scale).expect("case id");
        for implementation in [Implementation::Stim, Implementation::Stab] {
            probes.push(
                preflight::expected_accepted_probe(
                    &case_id,
                    implementation,
                    1,
                    scale.work_items.get(),
                    scale.input_bytes,
                    scale.input_digest.as_str(),
                    &output_digest,
                )
                .expect("accepted receipt"),
            );
        }
    }
    for (case_id, expectation) in [
        (
            CIRCUIT_CAP_CASE_ID,
            cap_rejection_expectation as fn(Implementation) -> (i32, &'static str),
        ),
        (
            GATE_PARTIAL_SWEEP_CASE_ID,
            gate_partial_sweep_rejection_expectation,
        ),
        (POPCOUNT_CAP_CASE_ID, popcount_cap_rejection_expectation),
    ] {
        for implementation in [Implementation::Stim, Implementation::Stab] {
            let (status, stderr) = expectation(implementation);
            probes.push(
                preflight::expected_rejected_probe(case_id, implementation, status, stderr)
                    .expect("rejection receipt"),
            );
        }
    }
    probes
}

#[test]
fn canonical_worker_contract_preflight_binds_actual_receipts() {
    let (_, contracts) = source_contracts();
    let probes = expected_current_probes(&contracts);
    assert!(probes.iter().all(|probe| match probe {
        WorkerContractProbeEvidence::Accepted { evidence_mode, .. }
        | WorkerContractProbeEvidence::Rejected { evidence_mode, .. } => {
            *evidence_mode == EvidenceMode::Contract
        }
    }));
    let evidence = WorkerContractPreflightEvidence::from_actual_probes(
        contract_identity(),
        crate::qualification::EXPECTED_FROZEN_DIGEST,
        &contracts,
        probes,
    )
    .expect("valid contract evidence");
    assert_eq!(evidence.probe_count(), contracts.len() * 2 + 6);
    assert_eq!(evidence.probe_count(), 46);
    assert!(
        evidence
            .validates_source_contract(crate::qualification::EXPECTED_FROZEN_DIGEST, &contracts)
    );
    let encoded = serde_json::to_vec(&evidence).expect("serialize preflight evidence");
    assert!(
        encoded
            .windows(b"\"probes\"".len())
            .any(|window| window == b"\"probes\"")
    );
    assert!(
        encoded
            .windows(b"\"worker_identity\"".len())
            .any(|window| window == b"\"worker_identity\"")
    );
    let decoded: WorkerContractPreflightEvidence =
        serde_json::from_slice(&encoded).expect("deserialize preflight evidence");
    assert_eq!(decoded, evidence);

    let mut tampered = evidence;
    tampered.sha256 = "0".repeat(64);
    assert!(
        !tampered
            .validates_source_contract(crate::qualification::EXPECTED_FROZEN_DIGEST, &contracts)
    );

    let mut refingerprinted = WorkerContractPreflightEvidence::from_actual_probes(
        contract_identity(),
        crate::qualification::EXPECTED_FROZEN_DIGEST,
        &contracts,
        expected_current_probes(&contracts),
    )
    .expect("valid contract evidence");
    let first_probe = refingerprinted.probes.first_mut();
    assert!(matches!(
        first_probe,
        Some(WorkerContractProbeEvidence::Accepted { .. })
    ));
    if let Some(WorkerContractProbeEvidence::Accepted { work_count, .. }) = first_probe {
        *work_count += 1;
    }
    refingerprinted.sha256 = worker_contract_preflight_digest(
        &refingerprinted.performance_inventory_sha256,
        &refingerprinted.worker_identity,
        &refingerprinted.probes,
    )
    .expect("tampered digest");
    assert!(
        !refingerprinted
            .validates_source_contract(crate::qualification::EXPECTED_FROZEN_DIGEST, &contracts)
    );

    let mut wrong_mode = WorkerContractPreflightEvidence::from_actual_probes(
        contract_identity(),
        crate::qualification::EXPECTED_FROZEN_DIGEST,
        &contracts,
        expected_current_probes(&contracts),
    )
    .expect("valid contract evidence");
    assert!(!wrong_mode.probes.is_empty());
    if let Some(
        WorkerContractProbeEvidence::Accepted { evidence_mode, .. }
        | WorkerContractProbeEvidence::Rejected { evidence_mode, .. },
    ) = wrong_mode.probes.first_mut()
    {
        *evidence_mode = EvidenceMode::Timing;
    }
    wrong_mode.sha256 = worker_contract_preflight_digest(
        &wrong_mode.performance_inventory_sha256,
        &wrong_mode.worker_identity,
        &wrong_mode.probes,
    )
    .expect("wrong-mode digest");
    assert!(
        !wrong_mode
            .validates_source_contract(crate::qualification::EXPECTED_FROZEN_DIGEST, &contracts)
    );
}

#[test]
fn worker_contract_preflight_rejects_receipt_and_contract_drift() {
    let (_, contracts) = source_contracts();
    let evidence = WorkerContractPreflightEvidence::from_actual_probes(
        contract_identity(),
        crate::qualification::EXPECTED_FROZEN_DIGEST,
        &contracts,
        expected_current_probes(&contracts),
    )
    .expect("valid contract evidence");

    let refingerprint = |mut changed: WorkerContractPreflightEvidence| {
        changed.sha256 = worker_contract_preflight_digest(
            &changed.performance_inventory_sha256,
            &changed.worker_identity,
            &changed.probes,
        )
        .expect("preflight digest");
        changed
    };

    let mut missing = evidence.clone();
    missing.probes.remove(0);
    let missing = refingerprint(missing);
    assert!(
        !missing
            .validates_source_contract(crate::qualification::EXPECTED_FROZEN_DIGEST, &contracts)
    );

    let mut extra = evidence.clone();
    let first_probe = extra.probes.first().expect("first probe").clone();
    extra.probes.push(first_probe);
    let extra = refingerprint(extra);
    assert!(
        !extra.validates_source_contract(crate::qualification::EXPECTED_FROZEN_DIGEST, &contracts)
    );

    let mut wrong_implementation = evidence.clone();
    if let WorkerContractProbeEvidence::Accepted { implementation, .. } = wrong_implementation
        .probes
        .first_mut()
        .expect("first probe")
    {
        *implementation = Implementation::Stab;
    }
    let wrong_implementation = refingerprint(wrong_implementation);
    assert!(
        !wrong_implementation
            .validates_source_contract(crate::qualification::EXPECTED_FROZEN_DIGEST, &contracts)
    );

    let mut wrong_rejection = evidence.clone();
    let final_probe = wrong_rejection
        .probes
        .last_mut()
        .expect("shared rejection receipt");
    if let WorkerContractProbeEvidence::Rejected { exit_status, .. } = final_probe {
        *exit_status += 1;
    }
    let wrong_rejection = refingerprint(wrong_rejection);
    assert!(
        !wrong_rejection
            .validates_source_contract(crate::qualification::EXPECTED_FROZEN_DIGEST, &contracts)
    );

    let mut stale_contracts = contracts.clone();
    let stale_scale = stale_contracts
        .first_mut()
        .expect("first contract")
        .scales
        .first_mut()
        .expect("first scale");
    stale_scale.work_items =
        NonZeroU64::new(stale_scale.work_items.get() + 1).expect("positive work");
    assert!(!evidence.validates_source_contract(
        crate::qualification::EXPECTED_FROZEN_DIGEST,
        &stale_contracts
    ));
    assert!(!evidence.validates_source_contract(&"0".repeat(64), &contracts));
}

#[test]
fn worker_contract_preflight_enforces_the_global_receipt_cap() {
    let (_, contracts) = source_contracts();
    let oversized = vec![contracts.first().expect("first contract").clone(); 62];
    assert!(matches!(
        WorkerContractPreflightEvidence::from_actual_probes(
            contract_identity(),
            crate::qualification::EXPECTED_FROZEN_DIGEST,
            &oversized,
            Vec::new(),
        ),
        Err(InvocationError::ContractPreflightDefinition)
    ));
}

#[test]
fn report_replay_rejects_refingerprinted_preflight_from_another_worker_pair() {
    let (root, contracts) = source_contracts();
    let evidence = WorkerContractPreflightEvidence::from_actual_probes(
        contract_identity(),
        crate::qualification::EXPECTED_FROZEN_DIGEST,
        &contracts,
        expected_current_probes(&contracts),
    )
    .expect("valid contract evidence");
    let workers = report_identity(&evidence.worker_identity, evidence.sha256.clone());
    super::super::report::validate_worker_contract_preflight(
        &evidence,
        &workers,
        &root,
        crate::qualification::EXPECTED_FROZEN_DIGEST,
    )
    .expect("matching worker-bound preflight");

    let mut transplanted = evidence;
    transplanted.worker_identity.stim_binary_sha256 =
        Sha256Digest::try_new("0".repeat(64)).expect("different binary digest");
    transplanted.sha256 = worker_contract_preflight_digest(
        &transplanted.performance_inventory_sha256,
        &transplanted.worker_identity,
        &transplanted.probes,
    )
    .expect("refingerprinted preflight");
    let refingerprinted_workers =
        report_identity(&contract_identity(), transplanted.sha256.clone());

    assert!(matches!(
        super::super::report::validate_worker_contract_preflight(
            &transplanted,
            &refingerprinted_workers,
            &root,
            crate::qualification::EXPECTED_FROZEN_DIGEST,
        ),
        Err(super::super::report::ReportError::WorkerReceipt)
    ));
}

#[test]
fn parent_rejects_semantic_work_overflow_before_invocation() {
    let maximum = NonZeroU64::new(u64::MAX).expect("positive maximum");
    let two = NonZeroU64::new(2).expect("positive two");
    assert!(matches!(
        checked_work_count(maximum, two),
        Err(InvocationError::WorkOverflow)
    ));
}

#[test]
fn reproducibility_requires_one_clean_unchanged_commit() {
    let state = |commit: char, dirty| super::super::git::RepositoryState {
        commit: commit.to_string().repeat(40),
        local_modifications: dirty,
    };
    assert!(matches!(
        require_reproducibility_repository(&state('a', true), &state('a', false)),
        Err(InvocationError::DirtyReproducibilityRepository)
    ));
    assert!(matches!(
        require_reproducibility_repository(&state('a', false), &state('b', false)),
        Err(InvocationError::ReproducibilityRepositoryChanged { before, after })
            if before == "a".repeat(40) && after == "b".repeat(40)
    ));
}

#[test]
fn cap_rejection_requires_the_worker_limit_before_the_start_barrier() {
    let output = |status, stderr: &str| ProcessResult {
        status,
        stdout: Vec::new(),
        stderr: stderr.as_bytes().to_vec(),
        parent_observed_peak_rss_bytes: None,
        wall_elapsed: Duration::from_millis(1),
    };
    checked_cap_rejection(
        &output(
            Some(2),
            "stim qualification adapter: circuit-parse instruction count exceeds the source-owned limit\n",
        ),
        Implementation::Stim,
    )
    .expect("adapter cap rejection");
    checked_cap_rejection(
        &output(
            Some(1),
            "[stab-bench] ERROR: performance qualification validation failed:\ncircuit-parse scale has 1000001 instructions, maximum 1000000\n",
        ),
        Implementation::Stab,
    )
    .expect("Stab cap rejection");
    assert!(matches!(
        checked_cap_rejection(
            &output(
                Some(2),
                "stim qualification adapter error: start barrier must contain one newline\n"
            ),
            Implementation::Stim,
        ),
        Err(InvocationError::CapRejection { .. })
    ));
    let signaled = output(
        None,
        "stim qualification adapter: circuit-parse instruction count exceeds the source-owned limit\n",
    );
    assert!(matches!(
        checked_cap_rejection(&signaled, Implementation::Stim),
        Err(InvocationError::CapRejection { .. })
    ));
    assert!(matches!(
        checked_cap_rejection(
            &output(
                Some(2),
                "stim qualification adapter: circuit-parse instruction count exceeds the source-owned limit\nunrelated error\n"
            ),
            Implementation::Stim,
        ),
        Err(InvocationError::CapRejection { .. })
    ));
}

#[test]
fn partial_gate_sweep_rejection_must_precede_the_start_barrier() {
    let output = |status, stdout: &str, stderr: &str| ProcessResult {
        status,
        stdout: stdout.as_bytes().to_vec(),
        stderr: stderr.as_bytes().to_vec(),
        parent_observed_peak_rss_bytes: None,
        wall_elapsed: Duration::from_millis(1),
    };
    checked_gate_partial_sweep_rejection(
        &output(
            Some(2),
            "",
            "stim qualification adapter: gate-name-hash work count is not a complete gate-table sweep\n",
        ),
        Implementation::Stim,
    )
    .expect("adapter partial-sweep rejection");
    checked_gate_partial_sweep_rejection(
        &output(
            Some(1),
            "",
            "[stab-bench] ERROR: performance qualification validation failed:\ngate-name-hash work count 83 is not a complete sweep of 82 names\n",
        ),
        Implementation::Stab,
    )
    .expect("Stab partial-sweep rejection");

    for rejected in [
        output(
            Some(2),
            "",
            "stim qualification adapter: start barrier must contain one newline\n",
        ),
        output(
            Some(0),
            "",
            "stim qualification adapter: gate-name-hash work count is not a complete gate-table sweep\n",
        ),
        output(
            Some(2),
            "unexpected output\n",
            "stim qualification adapter: gate-name-hash work count is not a complete gate-table sweep\n",
        ),
    ] {
        assert!(matches!(
            checked_gate_partial_sweep_rejection(&rejected, Implementation::Stim),
            Err(InvocationError::GatePartialSweepRejection { .. })
        ));
    }
}

#[test]
fn invalid_popcount_width_rejections_must_precede_the_start_barrier() {
    let output = |status, stdout: &str, stderr: &str| ProcessResult {
        status,
        stdout: stdout.as_bytes().to_vec(),
        stderr: stderr.as_bytes().to_vec(),
        parent_observed_peak_rss_bytes: None,
        wall_elapsed: Duration::from_millis(1),
    };
    checked_popcount_cap_rejection(
        &output(
            Some(2),
            "",
            "stim qualification adapter: simd-word-popcount bit width exceeds the source-owned limit\n",
        ),
        Implementation::Stim,
    )
    .expect("adapter cap rejection");
    checked_popcount_cap_rejection(
        &output(
            Some(1),
            "",
            "[stab-bench] ERROR: performance qualification validation failed:\nsimd-word-popcount width 268435712 bits exceeds the maximum 268435456\n",
        ),
        Implementation::Stab,
    )
    .expect("Stab cap rejection");

    assert!(matches!(
        checked_popcount_cap_rejection(
            &output(
                Some(2),
                "",
                "stim qualification adapter: start barrier must contain exactly one newline\n",
            ),
            Implementation::Stim,
        ),
        Err(InvocationError::PopcountCapRejection { .. })
    ));
}

#[test]
#[cfg(target_os = "linux")]
#[ignore = "builds the pinned Stim adapter and Stab worker twice"]
fn private_worker_builds_are_byte_reproducible() {
    let root = RepoRoot::resolve(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
        .expect("repository root");
    let repository =
        super::super::artifact::RepositoryBinding::open(&root).expect("bind repository");
    let source_root = repository
        .descriptor_root(&root)
        .expect("descriptor-root view");
    let suite = crate::qualification::read(&source_root).expect("checked performance inventory");
    verify_private_worker_reproducibility(&source_root, &suite.semantic_digest)
        .expect("reproducible private workers");
}
