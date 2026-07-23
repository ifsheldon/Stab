use super::{
    BenchError, CorrectnessBinding, EvidenceState, OutputContract,
    PAULI_STRING_ITER_RANGE_GROUP_ID, PAULI_STRING_ITER_SINGLETON_GROUP_ID, Phase,
    QualificationGroup, QualificationStatus, RepoRoot, RunnerFidelity,
    SIMD_BITS_NOT_ZERO_ALL_ZERO_GROUP_ID, SIMD_BITS_NOT_ZERO_COMPARATOR_SOURCE,
    SIMD_BITS_NOT_ZERO_EARLY_GROUP_ID, SIMD_BITS_NOT_ZERO_LATE_GROUP_ID,
    SIMD_BITS_XOR_CORRECTNESS_CASES, SIMD_BITS_XOR_GROUP_ID, SPARSE_XOR_COMPARATOR_SOURCE,
    SPARSE_XOR_CORRECTNESS_CASE, SPARSE_XOR_ITEM_GROUP_ID, SPARSE_XOR_ROW_GROUP_ID,
    STIM_ADAPTER_SOURCE, ThresholdPolicy, apply_pauli_string_iter, circuit_memory_policy,
    comparator_source, simd_bits_not_zero_workload_family, sparse_xor_workload_family,
};

struct NotZeroGroupSpec {
    id: &'static str,
    manifest_row: &'static str,
    pattern: &'static str,
    seed: &'static str,
    input_digests: [&'static str; 3],
}

pub(in super::super) fn additional_groups(
    root: &RepoRoot,
    groups: &[QualificationGroup],
) -> Result<Vec<QualificationGroup>, BenchError> {
    let dense_xor = groups
        .iter()
        .find(|group| group.id == SIMD_BITS_XOR_GROUP_ID)
        .ok_or_else(|| {
            BenchError::Qualification(
                "source-owned not-zero groups require the dense-XOR bit-kernel owner".to_string(),
            )
        })?;
    let sparse_xor_row = groups
        .iter()
        .find(|group| group.id == SPARSE_XOR_ROW_GROUP_ID)
        .ok_or_else(|| {
            BenchError::Qualification(
                "source-owned sparse-XOR item group requires the row-XOR owner".to_string(),
            )
        })?;
    let pauli_iter_range = groups
        .iter()
        .find(|group| group.id == PAULI_STRING_ITER_RANGE_GROUP_ID)
        .ok_or_else(|| {
            BenchError::Qualification(
                "source-owned Pauli singleton iterator group requires the range iterator owner"
                    .to_string(),
            )
        })?;
    let mut additional = [
        NotZeroGroupSpec {
            id: SIMD_BITS_NOT_ZERO_EARLY_GROUP_ID,
            manifest_row: "pq2-simd-bits-not-zero-early",
            pattern: "early",
            seed: "single-bit-at-3-of-50-v1",
            input_digests: [
                "652aebf153201450c8fe9d3707aed8cb0ee9fee8f5332d88e2001c56cfd0838f",
                "f2af8de388713368d12e7bf4188e96c030bf1c3e2906250672e2f2eee9370aa8",
                "84118644943bed7c2aa82daafc7e8b8f2358d0e38ab07fd140c8aba466fb3ba4",
            ],
        },
        NotZeroGroupSpec {
            id: SIMD_BITS_NOT_ZERO_ALL_ZERO_GROUP_ID,
            manifest_row: "pq2-simd-bits-not-zero-all-zero",
            pattern: "zero",
            seed: "all-zero-v1",
            input_digests: [
                "b6286dfe1dca80e14e17bbc6a371565900665697e8f4f2b22d30a303f804b537",
                "60aace21d864e2176a3f43edcd21a970c401e36a0223c24d09a8d482e075aae0",
                "080543f5fd6fe5ca816fbfc568988f74eb08c7477f433ccbdecbc16d62790ec8",
            ],
        },
        NotZeroGroupSpec {
            id: SIMD_BITS_NOT_ZERO_LATE_GROUP_ID,
            manifest_row: "pq2-simd-bits-not-zero-late",
            pattern: "late",
            seed: "single-bit-at-logical-end-v1",
            input_digests: [
                "76618d8f234d913b3b6f99be0c83fca1e8a6eb3c5cdb6f622c06dccc7aaa2cc0",
                "61aace21da17e2176a3f445b0d21a9b0c41d536a0223c24deda8d482e075aae6",
                "0b0543f60288e5ca816fc551a8988eb4e96d37477f433ccbe2cbc16d62790f06",
            ],
        },
    ]
    .into_iter()
    .map(|spec| not_zero_group(root, dense_xor, spec))
    .collect::<Result<Vec<_>, _>>()?;
    let mut sparse_xor_item = sparse_xor_row.clone();
    sparse_xor_item.id = SPARSE_XOR_ITEM_GROUP_ID.to_string();
    sparse_xor_item.manifest_row = "pq2-sparse-xor-item".to_string();
    sparse_xor_item.row_origin = super::super::super::model::RowOrigin::Planned;
    apply_sparse_xor(root, &mut sparse_xor_item, true)?;
    additional.push(sparse_xor_item);
    let mut pauli_iter_singleton = pauli_iter_range.clone();
    pauli_iter_singleton.id = PAULI_STRING_ITER_SINGLETON_GROUP_ID.to_string();
    pauli_iter_singleton.manifest_row = "pq2-pauli-string-iter-singleton".to_string();
    pauli_iter_singleton.row_origin = super::super::super::model::RowOrigin::Planned;
    pauli_iter_singleton.public_api_items.clear();
    apply_pauli_string_iter(root, &mut pauli_iter_singleton, true)?;
    additional.push(pauli_iter_singleton);
    Ok(additional)
}

pub(super) fn apply_sparse_xor(
    root: &RepoRoot,
    group: &mut QualificationGroup,
    item_workload: bool,
) -> Result<(), BenchError> {
    group.phase = Phase::Execute;
    group.runner_fidelity = RunnerFidelity::AdapterLibrary;
    group.correctness_cases = vec![SPARSE_XOR_CORRECTNESS_CASE.to_string()];
    group.correctness_binding = CorrectnessBinding::ExactCases;
    group.planned_correctness_case_id = None;
    group.workload_family = sparse_xor_workload_family(item_workload);
    group.work_unit = if item_workload {
        "item-toggles"
    } else {
        "row-xors"
    }
    .to_string();
    group.output_contract = OutputContract {
        expected_shape: "Exact canonical input bytes plus a digest over iteration count, declared work, workload marker, callback work, all four input-fingerprint lanes, and all four final-state-fingerprint lanes."
            .to_string(),
        digest_state: EvidenceState::Existing,
        sink_policy: "Both workers prepare the exact pinned-Stim sparse fixture, execute two untimed complete callbacks to restore canonical state while retaining capacity, time only complete callbacks behind matching compiler barriers and optimizer-opaque mutable references, and encode final state outside timing."
            .to_string(),
        comparator_sources: [STIM_ADAPTER_SOURCE, SPARSE_XOR_COMPARATOR_SOURCE]
            .into_iter()
            .map(|path| comparator_source(root, path))
            .collect::<Result<_, _>>()?,
    };
    group.memory_policy = circuit_memory_policy(
        "One capacity-primed sparse fixture remains live during timing and setup and peak process RSS are report-only observations at every scale. Stab's timed callbacks allocate nothing at all scales and the accepted maximum; PQ6 owns cross-scale RSS and Stim allocation acceptance.",
    );
    group.threshold_policy = ThresholdPolicy::Primary1_25;
    group.owner = "stab-core/bits".to_string();
    group.reason = if item_workload {
        "Implemented paired pinned-Stim and Rust seven-item sparse toggles with exact CQ2, complete-callback, canonical-state, allocation, scale, timing, and bounded-worker contracts."
    } else {
        "Implemented paired pinned-Stim and Rust 1,000-row sparse symmetric differences with exact CQ2, actual-operation, canonical-state, allocation, scale, timing, and bounded-worker contracts."
    }
    .to_string();
    group.status = QualificationStatus::Implemented;
    Ok(())
}

fn not_zero_group(
    root: &RepoRoot,
    dense_xor: &QualificationGroup,
    spec: NotZeroGroupSpec,
) -> Result<QualificationGroup, BenchError> {
    let mut group = dense_xor.clone();
    group.id = spec.id.to_string();
    group.manifest_row = spec.manifest_row.to_string();
    group.row_origin = super::super::super::model::RowOrigin::Planned;
    group.correctness_cases = SIMD_BITS_XOR_CORRECTNESS_CASES
        .into_iter()
        .map(str::to_string)
        .collect();
    group.workload_family =
        simd_bits_not_zero_workload_family(spec.pattern, spec.seed, spec.input_digests);
    group.output_contract = OutputContract {
        expected_shape: "Exact generated logical-word fixture bytes plus a canonical digest over checksum, iteration count, logical bit width, pattern marker, and all four input-fingerprint lanes."
            .to_string(),
        digest_state: EvidenceState::Existing,
        sink_policy: "Both workers prepare the same logical u64 words outside timing, obtain the immutable input through matching optimizer-opaque references behind compiler fences, invoke only not_zero, accumulate the Boolean result, and digest the semantic output outside timing."
            .to_string(),
        comparator_sources: [STIM_ADAPTER_SOURCE, SIMD_BITS_NOT_ZERO_COMPARATOR_SOURCE]
            .into_iter()
            .map(|path| comparator_source(root, path))
            .collect::<Result<_, _>>()?,
    };
    group.memory_policy = circuit_memory_policy(
        "One logical bit vector remains live during timing and setup and peak process RSS are report-only observations at every scale. Timed scans allocate nothing; PQ6 owns explicit cross-scale RSS and allocation slack.",
    );
    group.reason = format!(
        "Implemented paired pinned-Stim and Rust not_zero scans for the {} pattern with independent exact CQ2, deterministic input, semantic output, scale, timing, and bounded-worker contracts.",
        spec.pattern,
    );
    Ok(group)
}
