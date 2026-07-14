use super::*;

#[cfg(all(
    target_os = "linux",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
fn policy_fixture() -> (tempfile::TempDir, RepoRoot, HostEvidence) {
    let directory = tempfile::tempdir().expect("temporary repository");
    let benchmark_directory = directory.path().join("benchmarks");
    std::fs::create_dir(&benchmark_directory).expect("benchmark directory");
    let policy = serde_json::json!({
        "schema_version": HOST_POLICY_SCHEMA_VERSION,
        "profiles": [{
            "id": "test-current-host",
            "operating_system": std::env::consts::OS,
            "architecture": std::env::consts::ARCH,
            "cpu_selection": "lowest-allowed",
            "max_load_per_allowed_cpu": "1000",
            "minimum_available_memory_bytes": 1024,
            "require_no_swap_activity": true,
            "require_frequency_governor": false,
            "allowed_frequency_governors": [],
            "require_thermal_probe": false,
            "maximum_temperature_millidegrees_celsius": 100000
        }]
    });
    let mut policy_bytes = serde_json::to_vec_pretty(&policy).expect("policy JSON");
    policy_bytes.push(b'\n');
    std::fs::write(
        benchmark_directory.join("qualification-host-policy.json"),
        &policy_bytes,
    )
    .expect("host policy");
    let root = RepoRoot::resolve(directory.path()).expect("repository root");
    let allowed_cpus = allowed_cpus().expect("current CPU affinity");
    let maximum_load_one = 1000.0 * allowed_cpus.len() as f64;
    let selected_cpu = allowed_cpus.first().copied().expect("selected CPU");
    let evidence = HostEvidence {
        policy_sha256: sha256_hex(&policy_bytes),
        profile_id: "test-current-host".to_string(),
        operating_system: std::env::consts::OS.to_string(),
        architecture: std::env::consts::ARCH.to_string(),
        logical_cpu_count: allowed_cpus.len(),
        allowed_cpus,
        selected_cpu,
        cpu_identity: cpu_identity().expect("CPU identity"),
        load_one_before: 0.0,
        load_one_after: 0.0,
        maximum_load_one,
        available_memory_before_bytes: 2048,
        available_memory_after_bytes: 2048,
        minimum_available_memory_bytes: 1024,
        swap_in_before: 10,
        swap_in_after: 10,
        swap_out_before: 20,
        swap_out_after: 20,
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
    };
    (directory, root, evidence)
}

#[cfg(all(
    target_os = "linux",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
#[test]
fn report_host_evidence_replays_source_owned_policy() {
    let (_directory, root, evidence) = policy_fixture();
    evidence
        .validate_against_policy(&root)
        .expect("valid host evidence");

    let mut unverified = evidence.clone();
    unverified.load_one_after = unverified.maximum_load_one + 1.0;
    unverified.verified = false;
    unverified.violations = vec![format!(
        "one-minute load average after the run is {:.3}, exceeding {:.3}",
        unverified.load_one_after, unverified.maximum_load_one
    )];
    unverified
        .validate_against_policy(&root)
        .expect("truthfully unverified evidence");
}

#[cfg(all(
    target_os = "linux",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
#[test]
fn report_host_evidence_rejects_hidden_policy_violations() {
    let (_directory, root, evidence) = policy_fixture();
    let mut mutations = Vec::new();

    let mut load = evidence.clone();
    load.load_one_after = load.maximum_load_one + 1.0;
    mutations.push(load);

    let mut memory = evidence.clone();
    memory.available_memory_after_bytes = 512;
    mutations.push(memory);

    let mut swap = evidence.clone();
    swap.swap_in_after += 1;
    mutations.push(swap);

    let mut governor = evidence.clone();
    governor.frequency_governor_before = Some("powersave".to_string());
    governor.frequency_governor_after = Some("powersave".to_string());
    governor.frequency_khz_before = Some(1);
    governor.frequency_khz_after = Some(1);
    mutations.push(governor);

    let overheated = ThermalReading {
        zone: "thermal_zone0".to_string(),
        kind: "cpu".to_string(),
        millidegrees_celsius: 110_000,
    };
    let mut thermal = evidence.clone();
    thermal.thermal_readings_before = vec![overheated.clone()];
    thermal.thermal_readings_after = vec![overheated];
    thermal.thermal_probe_available = true;
    mutations.push(thermal);

    let mut digest = evidence.clone();
    digest.policy_sha256 = "0".repeat(64);
    mutations.push(digest);

    let mut fabricated = evidence;
    fabricated.verified = false;
    fabricated.violations = vec!["fabricated violation".to_string()];
    mutations.push(fabricated);

    for mutation in mutations {
        assert!(matches!(
            mutation.validate_against_policy(&root),
            Err(HostError::EvidenceMismatch)
        ));
    }
}
