#[cfg(target_os = "linux")]
use std::path::Path;

use super::*;

#[test]
fn canonical_worker_contract_preflight_binds_actual_receipts() {
    let probes = expected_contract_preflight_probes().expect("source-owned probes");
    let evidence = WorkerContractPreflightEvidence::from_actual_probes(probes)
        .expect("valid contract evidence");
    assert_eq!(evidence.probe_count(), 18);
    assert!(evidence.validates_source_contract());
    let encoded = serde_json::to_vec(&evidence).expect("serialize preflight evidence");
    assert!(
        encoded
            .windows(b"\"probes\"".len())
            .any(|window| window == b"\"probes\"")
    );
    let decoded: WorkerContractPreflightEvidence =
        serde_json::from_slice(&encoded).expect("deserialize preflight evidence");
    assert_eq!(decoded, evidence);

    let mut tampered = evidence;
    tampered.sha256 = "0".repeat(64);
    assert!(!tampered.validates_source_contract());

    let mut refingerprinted = WorkerContractPreflightEvidence::from_actual_probes(
        expected_contract_preflight_probes().expect("source-owned probes"),
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
    refingerprinted.sha256 =
        worker_contract_preflight_digest(&refingerprinted.probes).expect("tampered digest");
    assert!(!refingerprinted.validates_source_contract());
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
    checked_popcount_alignment_rejection(
        &output(
            Some(2),
            "",
            "stim qualification adapter: simd-word-popcount bit width is not a multiple of 256\n",
        ),
        Implementation::Stim,
    )
    .expect("adapter alignment rejection");
    checked_popcount_alignment_rejection(
        &output(
            Some(1),
            "",
            "[stab-bench] ERROR: performance qualification validation failed:\nsimd-word-popcount width 513 bits is not a multiple of 256\n",
        ),
        Implementation::Stab,
    )
    .expect("Stab alignment rejection");
    checked_popcount_minimum_rejection(
        &output(
            Some(2),
            "",
            "stim qualification adapter: simd-word-popcount bit width is below the source-owned minimum\n",
        ),
        Implementation::Stim,
    )
    .expect("adapter minimum rejection");
    checked_popcount_minimum_rejection(
        &output(
            Some(1),
            "",
            "[stab-bench] ERROR: performance qualification validation failed:\nsimd-word-popcount width 256 bits is below the minimum 512\n",
        ),
        Implementation::Stab,
    )
    .expect("Stab minimum rejection");

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
    assert!(matches!(
        checked_popcount_alignment_rejection(
            &output(
                Some(0),
                "",
                "stim qualification adapter: simd-word-popcount bit width is not a multiple of 256\n",
            ),
            Implementation::Stim,
        ),
        Err(InvocationError::PopcountAlignmentRejection { .. })
    ));
    assert!(matches!(
        checked_popcount_minimum_rejection(
            &output(
                Some(2),
                "",
                "stim qualification adapter: start barrier must contain exactly one newline\n",
            ),
            Implementation::Stim,
        ),
        Err(InvocationError::PopcountMinimumRejection { .. })
    ));
}

#[test]
#[cfg(target_os = "linux")]
#[ignore = "builds the pinned Stim adapter and Stab worker twice"]
fn private_worker_builds_are_byte_reproducible() {
    let root = RepoRoot::resolve(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
        .expect("repository root");
    verify_private_worker_reproducibility(&root).expect("reproducible private workers");
}
