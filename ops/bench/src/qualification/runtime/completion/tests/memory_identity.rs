use super::*;
use crate::qualification::runtime::{
    git::RepositoryState,
    host::HostEvidence,
    probe::{AdapterProbeReceipt, DemMemoryReceiptEvidence},
    protocol::Sha256Digest,
};

fn memory_receipt_evidence(shared: &RollupReplayEvidence) -> DemMemoryReceiptEvidence {
    DemMemoryReceiptEvidence {
        path: PathBuf::from("target/benchmarks/qualification/dem-parse-memory"),
        report_sha256: "9".repeat(64),
        repository: RepositoryEvidence {
            commit_before: shared.stab_commit.clone(),
            commit_after: shared.stab_commit.clone(),
            local_modifications_before: false,
            local_modifications_after: false,
        },
        runtime_group_id: DEM_PARSE_GROUP.to_string(),
        host: HostEvidence {
            policy_sha256: shared.host_policy_sha256.clone(),
            profile_id: shared.host_profile_id.clone(),
            operating_system: shared.operating_system.clone(),
            architecture: shared.architecture.clone(),
            allowed_cpus: vec![0],
            logical_cpu_count: 1,
            selected_cpu: 0,
            cpu_identity: shared.cpu_identity.clone(),
            load_one_before: 0.0,
            load_one_after: 0.0,
            maximum_load_one: 1.0,
            available_memory_before_bytes: 1,
            available_memory_after_bytes: 1,
            minimum_available_memory_bytes: 1,
            swap_in_before: 0,
            swap_in_after: 0,
            swap_out_before: 0,
            swap_out_after: 0,
            frequency_governor_before: None,
            frequency_governor_after: None,
            frequency_khz_before: None,
            frequency_khz_after: None,
            maximum_temperature_millidegrees_celsius: 100_000,
            thermal_readings_before: Vec::new(),
            thermal_readings_after: Vec::new(),
            thermal_probe_available: false,
            verified: true,
            violations: Vec::new(),
        },
        probe: {
            let mut probe = AdapterProbeReceipt::test_fixture(DEM_PARSE_GROUP);
            probe.stim_source_sha256 = digest(&shared.workers.stim_source_sha256);
            probe.stim_build_fingerprint = digest(&shared.workers.stim_build_fingerprint);
            probe.stim_binary_sha256 = digest(&shared.workers.stim_binary_sha256);
            probe.stab_source_sha256 = digest(&shared.workers.stab_source_sha256);
            probe.stab_build_fingerprint = digest(&shared.workers.stab_build_fingerprint);
            probe.stab_binary_sha256 = digest(&shared.workers.stab_binary_sha256);
            probe
        },
    }
}

fn digest(value: &str) -> Sha256Digest {
    Sha256Digest::try_new(value).expect("SHA-256 digest")
}

#[test]
fn completion_rejects_each_mismatched_private_worker_memory_identity() {
    let shared = replay_evidence();
    let repository_state = RepositoryState {
        commit: shared.stab_commit.clone(),
        local_modifications: false,
    };
    validate_memory_receipt_identity(
        DEM_PARSE_GROUP,
        &memory_receipt_evidence(&shared),
        &shared,
        &repository_state,
    )
    .expect("matching memory and rollup identities");

    let assert_rejected = |receipt: &DemMemoryReceiptEvidence| {
        assert!(matches!(
            validate_memory_receipt_identity(
                DEM_PARSE_GROUP,
                receipt,
                &shared,
                &repository_state
            ),
            Err(CompletionError::MemoryReceiptIdentity(group_id)) if group_id == DEM_PARSE_GROUP
        ));
    };

    let mut wrong_source = memory_receipt_evidence(&shared);
    wrong_source.probe.stab_source_sha256 = digest(&"0".repeat(64));
    assert_rejected(&wrong_source);

    let mut wrong_build = memory_receipt_evidence(&shared);
    wrong_build.probe.stab_build_fingerprint = digest(&"0".repeat(64));
    assert_rejected(&wrong_build);

    let mut wrong_binary = memory_receipt_evidence(&shared);
    wrong_binary.probe.stab_binary_sha256 = digest(&"0".repeat(64));
    assert_rejected(&wrong_binary);
}
