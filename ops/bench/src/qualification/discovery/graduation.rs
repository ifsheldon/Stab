use std::path::Path;

use super::super::model::{
    ComparatorSource, CorrectnessBinding, EvidenceState, FixtureLocator, InputByteCount,
    MemoryMethod, MemoryPolicy, OutputContract, Phase, QualificationGroup, QualificationStatus,
    RunnerFidelity, ScalePoint, SizeClass, ThresholdPolicy, TimingBatchPolicy, WorkloadFamily,
};
use crate::error::BenchError;
use crate::root::RepoRoot;

mod curated;
mod derived;

pub(super) use curated::groups as curated_api_groups;
pub(super) use derived::additional_groups;
use derived::apply_sparse_xor;

const CIRCUIT_PARSE_GROUP_ID: &str = "PERFQ-M4-CIRCUIT-PARSE";
const CIRCUIT_CANONICAL_PRINT_GROUP_ID: &str = "PERFQ-M4-CIRCUIT-CANONICAL-PRINT";
const GATE_NAME_HASH_GROUP_ID: &str = "PERFQ-M4-GATE-LOOKUP";
const SIMD_WORD_POPCOUNT_GROUP_ID: &str = "PERFQ-M5-SIMD-WORD";
const SIMD_BITS_XOR_GROUP_ID: &str = "PERFQ-M5-SIMD-BITS";
const SIMD_BITS_NOT_ZERO_EARLY_GROUP_ID: &str = "PERFQ-M5-SIMD-BITS-NOT-ZERO-EARLY";
const SIMD_BITS_NOT_ZERO_ALL_ZERO_GROUP_ID: &str = "PERFQ-M5-SIMD-BITS-NOT-ZERO-ALL-ZERO";
const SIMD_BITS_NOT_ZERO_LATE_GROUP_ID: &str = "PERFQ-M5-SIMD-BITS-NOT-ZERO-LATE";
const SPARSE_XOR_ROW_GROUP_ID: &str = "PERFQ-M5-SPARSE-XOR";
const SPARSE_XOR_ITEM_GROUP_ID: &str = "PERFQ-M5-SPARSE-XOR-ITEM";
const BIT_MATRIX_TRANSPOSE_IN_PLACE_GROUP_ID: &str = "PERFQ-M5-BIT-MATRIX-TRANSPOSE-IN-PLACE";
const BIT_MATRIX_TRANSPOSE_ALLOCATING_GROUP_ID: &str = "PERFQ-M5-BIT-MATRIX-TRANSPOSE-ALLOCATING";
const PAULI_STRING_MULTIPLY_GROUP_ID: &str = "PERFQ-M6-PAULI-STRING";
const PAULI_STRING_ITER_RANGE_GROUP_ID: &str = "PERFQ-M6-PAULI-ITER";
const PAULI_STRING_ITER_SINGLETON_GROUP_ID: &str = "PERFQ-M6-PAULI-ITER-SINGLETON";
const CLIFFORD_STRING_IDENTITY_GROUP_ID: &str = "PERFQ-M6-CLIFFORD-STRING";
const CLIFFORD_STRING_NON_IDENTITY_GROUP_ID: &str = "PERFQ-M6-CLIFFORD-STRING-NON-IDENTITY";
const DEM_PARSE_GROUP_ID: &str = "PERFQ-M10-DEM-PARSE-CONTRACT";
const DEM_CANONICAL_PRINT_GROUP_ID: &str = "PERFQ-M10-DEM-PRINT-CONTRACT";
const STIM_ADAPTER_SOURCE: &str = "benchmarks/stim_adapter/main.cc";
const SIMD_WORD_POPCOUNT_COMPARATOR_SOURCE: &str =
    "benchmarks/stim_adapter/simd_word_popcount_contract.h";
const SIMD_BITS_XOR_COMPARATOR_SOURCE: &str = "benchmarks/stim_adapter/simd_bits_xor_contract.h";
const SIMD_BITS_NOT_ZERO_COMPARATOR_SOURCE: &str =
    "benchmarks/stim_adapter/simd_bits_not_zero_contract.h";
const SPARSE_XOR_COMPARATOR_SOURCE: &str = "benchmarks/stim_adapter/sparse_xor_contract.h";
const BIT_MATRIX_TRANSPOSE_COMPARATOR_SOURCE: &str =
    "benchmarks/stim_adapter/bit_matrix_transpose_contract.h";
const PAULI_STRING_MULTIPLY_COMPARATOR_SOURCE: &str =
    "benchmarks/stim_adapter/pauli_string_multiply_contract.h";
const PAULI_STRING_ITER_COMPARATOR_SOURCE: &str =
    "benchmarks/stim_adapter/pauli_string_iter_contract.h";
const CLIFFORD_STRING_COMPARATOR_SOURCE: &str =
    "benchmarks/stim_adapter/clifford_string_contract.h";
const DEM_MODEL_COMPARATOR_SOURCE: &str = "benchmarks/stim_adapter/dem_model_contract.h";
const CLIFFORD_VECTOR_PATH: &str = "benchmarks/fixtures/pq2-clifford-string-vectors.json";
const CIRCUIT_PARSE_CORRECTNESS_CASES: [&str; 2] = [
    "cq-evidence-qualification-633fa529edf5f549",
    "cq-evidence-qualification-e660819ae9a223c6",
];
const CIRCUIT_CANONICAL_PRINT_CORRECTNESS_CASES: [&str; 2] = [
    "cq-evidence-qualification-e660819ae9a223c6",
    "cq-evidence-qualification-ef933925fb901877",
];
const GATE_NAME_HASH_CORRECTNESS_CASE: &str = "cq-evidence-qualification-bd20a013e903a05f";
const SIMD_WORD_POPCOUNT_CORRECTNESS_CASES: [&str; 3] = [
    "cq-evidence-qualification-5118006702599a45",
    "cq-evidence-qualification-b1530dc4e48e942d",
    "cq-evidence-qualification-ba252d42660a41ce",
];
const SIMD_BITS_XOR_CORRECTNESS_CASES: [&str; 2] = [
    "cq-evidence-qualification-b1530dc4e48e942d",
    "cq-evidence-qualification-ba252d42660a41ce",
];
const SPARSE_XOR_CORRECTNESS_CASE: &str = "cq-evidence-qualification-bea77c19e9ae0b24";
const BIT_MATRIX_TRANSPOSE_CORRECTNESS_CASES: [&str; 2] = [
    "cq-evidence-qualification-4d0291febfd22b68",
    "cq-evidence-qualification-66e29faafe5f2856",
];
const PAULI_STRING_MULTIPLY_CORRECTNESS_CASES: [&str; 2] = [
    "cq-evidence-qualification-3bab0f51237445f6",
    "cq-evidence-qualification-489e6445120743c2",
];
const PAULI_STRING_ITER_CORRECTNESS_CASES: [&str; 3] = [
    "cq-evidence-qualification-0a4be178ce1c903b",
    "cq-evidence-qualification-489e6445120743c2",
    "cq-evidence-qualification-5331280b58fd49c7",
];
const CLIFFORD_STRING_CORRECTNESS_CASES: [&str; 3] = [
    "cq-evidence-qualification-40e5ad2f2f4c4fd4",
    "cq-evidence-qualification-510e746ec36e7d1c",
    "cq-evidence-qualification-ae9390dd6a207cb6",
];
const DEM_MODEL_CORRECTNESS_CASES: [&str; 4] = [
    "cq-evidence-qualification-0908c21b917526e3",
    "cq-evidence-qualification-2f6a06e3fffcf5c0",
    "cq-evidence-qualification-966ab53fd109b7b3",
    "cq-evidence-qualification-ae2cf29058be84b0",
];
const EMPTY_INPUT_DIGEST: &str = "6a09e667f3bcc908bb67ae8584caa73b3c6ef372fe94f82ba54ff53a5f1d36f1";

pub(super) fn apply(root: &RepoRoot, group: &mut QualificationGroup) -> Result<(), BenchError> {
    match group.id.as_str() {
        CIRCUIT_PARSE_GROUP_ID => apply_circuit_parse(group),
        CIRCUIT_CANONICAL_PRINT_GROUP_ID => apply_circuit_canonical_print(group),
        GATE_NAME_HASH_GROUP_ID => apply_gate_name_hash(group),
        SIMD_WORD_POPCOUNT_GROUP_ID => apply_simd_word_popcount(root, group)?,
        SIMD_BITS_XOR_GROUP_ID => apply_simd_bits_xor(root, group)?,
        SPARSE_XOR_ROW_GROUP_ID => apply_sparse_xor(root, group, false)?,
        BIT_MATRIX_TRANSPOSE_IN_PLACE_GROUP_ID => {
            apply_bit_matrix_transpose(root, group, false)?;
        }
        BIT_MATRIX_TRANSPOSE_ALLOCATING_GROUP_ID => {
            apply_bit_matrix_transpose(root, group, true)?;
        }
        PAULI_STRING_MULTIPLY_GROUP_ID => apply_pauli_string_multiply(root, group)?,
        PAULI_STRING_ITER_RANGE_GROUP_ID => apply_pauli_string_iter(root, group, false)?,
        PAULI_STRING_ITER_SINGLETON_GROUP_ID => apply_pauli_string_iter(root, group, true)?,
        CLIFFORD_STRING_IDENTITY_GROUP_ID => apply_clifford_string(root, group, false)?,
        CLIFFORD_STRING_NON_IDENTITY_GROUP_ID => apply_clifford_string(root, group, true)?,
        DEM_PARSE_GROUP_ID => apply_dem_model(root, group, false)?,
        DEM_CANONICAL_PRINT_GROUP_ID => apply_dem_model(root, group, true)?,
        _ => {}
    }
    Ok(())
}

fn apply_dem_model(
    root: &RepoRoot,
    group: &mut QualificationGroup,
    serialize: bool,
) -> Result<(), BenchError> {
    group.phase = if serialize {
        Phase::Serialize
    } else {
        Phase::Parse
    };
    group.runner_fidelity = RunnerFidelity::AdapterLibrary;
    group.correctness_cases = DEM_MODEL_CORRECTNESS_CASES
        .into_iter()
        .map(str::to_string)
        .collect();
    group.correctness_binding = CorrectnessBinding::ExactCases;
    group.planned_correctness_case_id = None;
    group.workload_family = dem_model_workload_family();
    group.work_unit = "top-level DEM items".to_string();
    group.output_contract = OutputContract {
        expected_shape: "Exact source fixture byte count and digest plus exact normalized final canonical DEM byte count and semantic digest."
            .to_string(),
        digest_state: EvidenceState::Existing,
        sink_policy: if serialize {
            "Both workers generate and parse the exact source-owned fixture before timing, repeatedly produce and optimizer-consume an owned canonical string in timing, retain the final string, normalize only one terminal newline, and compare its exact bytes and digest outside timing."
        } else {
            "Both workers generate the exact source-owned fixture before timing, repeatedly construct and optimizer-consume a fresh owned model in timing, retain the final model, serialize it outside timing, normalize only one terminal newline, and compare its exact bytes and digest."
        }
        .to_string(),
        comparator_sources: [STIM_ADAPTER_SOURCE, DEM_MODEL_COMPARATOR_SOURCE]
            .into_iter()
            .map(|path| comparator_source(root, path))
            .collect::<Result<_, _>>()?,
    };
    group.memory_policy = dem_memory_policy();
    group.threshold_policy = ThresholdPolicy::Primary1_25;
    group.owner = if serialize {
        "stab-core/dem-printer"
    } else {
        "stab-core/dem-parser"
    }
    .to_string();
    group.reason = if serialize {
        "Implemented paired pinned-Stim and Rust direct canonical DEM serialization with exact CQ2, fixture, output, scale, timing, hostile-boundary, and receipt contracts."
    } else {
        "Implemented paired pinned-Stim and Rust direct DEM text parsing with exact CQ2, fixture, output, scale, timing, hostile-boundary, and receipt contracts."
    }
    .to_string();
    group.status = QualificationStatus::Implemented;
    Ok(())
}

fn dem_memory_policy() -> MemoryPolicy {
    MemoryPolicy {
        method: MemoryMethod::ProcessRss,
        scale_ids: ["flat-errors", "coordinate-sparse", "folded-repeats"]
            .into_iter()
            .flat_map(|family| {
                ["small", "medium", "large"]
                    .into_iter()
                    .map(move |size| format!("{family}-{size}"))
            })
            .collect(),
        expected_growth: "The generated text and selected setup model remain live as applicable, and setup and peak process RSS are reported independently for each family and size. Both workers capture peak RSS immediately after timed dispatch and before post-timing serialization, digest construction, or validation."
            .to_string(),
    }
}

fn dem_model_workload_family() -> WorkloadFamily {
    WorkloadFamily {
        fixture: FixtureLocator::Generated {
            id: "dem-model-families-v2".to_string(),
        },
        source: "src/stim/dem/detector_error_model.test.cc".to_string(),
        deterministic_seed: "none; exact source-owned family cycles".to_string(),
        scales: [
            (
                "flat-errors",
                "small",
                64,
                1_368,
                "ac17cebf0983e626775012a7bab634d136669f36d706b3842b73a66b1c5684c5",
            ),
            (
                "flat-errors",
                "medium",
                4_096,
                87_552,
                "4c2832178c6c44c8ef895a7cc8a8043699d096f678f6c624039ae126e4541b60",
            ),
            (
                "flat-errors",
                "large",
                65_536,
                1_400_832,
                "756997bdca11be9e8d5f4e9470bea059ec516daf218e1a0b3b6c421dbb229de0",
            ),
            (
                "coordinate-sparse",
                "small",
                64,
                1_816,
                "1065bcd0f102d45a035aa841c27717cfb4e02ea6ea600bb833e3eb8c80f40780",
            ),
            (
                "coordinate-sparse",
                "medium",
                4_096,
                116_224,
                "f2d6a6395d630dae37ea9ef42f94604fbd42e6f98537390230f5f63173074fbb",
            ),
            (
                "coordinate-sparse",
                "large",
                65_536,
                1_859_584,
                "5a15e24913a40f53a1a10e7760009b3e68be8114a76e9f18a3d831ee1f4addc1",
            ),
            (
                "folded-repeats",
                "small",
                64,
                8_320,
                "c484e6f7c587a0b7edf9e3572ee745d2268ccde4f6d3d444d186b7cff5ae8db6",
            ),
            (
                "folded-repeats",
                "medium",
                4_096,
                532_480,
                "0a0ac94bf0297b57a62e400c64ccce954519c051b4ef482c9af05babd9ee17c2",
            ),
            (
                "folded-repeats",
                "large",
                65_536,
                8_519_680,
                "2dab06c78e9a7c41ceb5799b84fb83487db68d7e18b9ffd8796114e16fa4a77e",
            ),
        ]
        .into_iter()
        .map(
            |(family, size, items, input_bytes, input_digest)| ScalePoint {
                id: format!("{family}-{size}"),
                family_id: family.to_string(),
                size_class: SizeClass::from_scale_id(size),
                parameters: format!(
                    "generator=dem-model-families-v2; family={family}; top_level_items={items}"
                ),
                input_bytes: InputByteCount::Exact { bytes: input_bytes },
                semantic_work: Some(items),
                input_digest: Some(input_digest.to_string()),
            },
        )
        .collect(),
    }
}

fn apply_clifford_string(
    root: &RepoRoot,
    group: &mut QualificationGroup,
    non_identity: bool,
) -> Result<(), BenchError> {
    group.phase = Phase::Execute;
    group.runner_fidelity = RunnerFidelity::AdapterLibrary;
    group.correctness_cases = CLIFFORD_STRING_CORRECTNESS_CASES
        .into_iter()
        .map(str::to_string)
        .collect();
    group.correctness_binding = CorrectnessBinding::ExactCases;
    group.planned_correctness_case_id = None;
    group.workload_family = clifford_string_workload_family(root, non_identity)?;
    group.work_unit = "single-qubit products".to_string();
    group.output_contract = OutputContract {
        expected_shape: "SHA-256 over exactly sixteen little-endian u64 fields binding iteration count, checked semantic work, width, workload marker, observed left and right non-identity counts, successful public callback count, result-derived execution witness, four final-left gate-sequence digest lanes, and four unchanged-right digest lanes."
            .to_string(),
        digest_state: EvidenceState::Existing,
        sink_policy: "Both workers construct equal-width operands and an independent Tableau-derived scalar expectation before the barrier, reset callback and witness state immediately before it, time only the public in-place operation behind matching sequentially consistent compiler fences and optimizer-opaque references, derive the successful-call witness from the mutated left operand, retain the final left operand, and validate every left and right gate plus both SHA-256 sequence digests outside timing."
            .to_string(),
        comparator_sources: [STIM_ADAPTER_SOURCE, CLIFFORD_STRING_COMPARATOR_SOURCE]
            .into_iter()
            .map(|path| comparator_source(root, path))
            .collect::<Result<_, _>>()?,
    };
    group.memory_policy = circuit_memory_policy(
        "Two equal-width Clifford strings and the untimed scalar expectation remain live during timing. Stab allocation instrumentation proves zero calls and zero bytes for the direct public callback at small, medium, large, and accepted-maximum widths for both workload contracts; Stim allocation counts remain unclaimed and PQ6 owns cross-scale RSS acceptance.",
    );
    group.threshold_policy = ThresholdPolicy::Primary1_25;
    group.timing_policy.batch_policy = if non_identity {
        TimingBatchPolicy::CommonIterations
    } else {
        TimingBatchPolicy::IndependentThroughput
    };
    group.owner = "stab-core/stabilizers".to_string();
    group.reason = if non_identity {
        "Implemented paired pinned-Stim and Rust public in-place Clifford-string multiplication over the complete deterministic 24-by-23 non-identity composition cycle with exact CQ2, immutable-right, output, zero-allocation, hostile-input, scale, timing, and receipt contracts."
    } else {
        "Implemented the exact pinned identity-by-identity CliffordString benchmark as an independent public in-place workload with exact CQ2, immutable-right, result-witness, zero-allocation, scale, timing, and bounded-worker contracts."
    }
    .to_string();
    group.status = QualificationStatus::Implemented;
    if !non_identity {
        group.public_api_items.clear();
    }
    Ok(())
}

fn clifford_string_workload_family(
    root: &RepoRoot,
    non_identity: bool,
) -> Result<WorkloadFamily, BenchError> {
    let fixture_source =
        super::read_repo_text_bounded(root, &root.path.join(Path::new(CLIFFORD_VECTOR_PATH)))?;
    let (marker, cycle, span, input_digests) = if non_identity {
        (
            3_551_455_952_266_415_171_u64,
            23,
            552,
            [
                "6e9792d9f06e4a183bd73eeba556cd4cbc87b0c176bf4cb90a54849120cac96d",
                "0427b1f905f1fce379ca809029cbc6f90aae1a56f7fbb3acdeeb96bfee576b44",
                "e47454166c98afb2c2bc19b2701b346c097bd9ea04481e250361b2e15faf1ce6",
            ],
        )
    } else {
        (
            3_550_043_079_824_723_011_u64,
            0,
            0,
            [
                "8daac0ca1000f1d8cb6545d611d5f3e7b289bf403d8d5dcf529af28e7b573329",
                "5fc473c86b0d3bb66e1994ecff910a324cb705666c4b032b74bace09fdf2e90a",
                "cfc386ccbfc3a9220c49b2b17fab281350c2b26c669893e6e3a0beba1b6675aa",
            ],
        )
    };
    Ok(WorkloadFamily {
        fixture: FixtureLocator::RepositoryFile {
            path: CLIFFORD_VECTOR_PATH.to_string(),
            sha256: super::sha256_hex(fixture_source.as_bytes()),
        },
        source: "src/stim/stabilizers/clifford_string.perf.cc".to_string(),
        deterministic_seed: "none; descriptor-schema=1; canonical-gate-order=stim-v1.16.0"
            .to_string(),
        scales: [("small", 10_000_u64), ("medium", 100_000), ("large", 1_000_000)]
            .into_iter()
            .zip(input_digests)
            .map(|((id, width), input_digest)| ScalePoint {
                id: id.to_string(),
                    family_id: "default".to_string(),
                    size_class: SizeClass::from_scale_id(id),
                parameters: format!(
                    "generator=pq2-clifford-string-vectors-v1; width={width}; marker={marker}; canonical_gates=24; right_cycle={cycle}; complete_span={span}; public_cap=1048576"
                ),
                input_bytes: InputByteCount::Exact { bytes: 64 },
                semantic_work: Some(width),
                input_digest: Some(input_digest.to_string()),
            })
            .collect(),
    })
}

fn apply_pauli_string_iter(
    root: &RepoRoot,
    group: &mut QualificationGroup,
    singleton: bool,
) -> Result<(), BenchError> {
    group.phase = Phase::Execute;
    group.runner_fidelity = RunnerFidelity::AdapterLibrary;
    group.correctness_cases = PAULI_STRING_ITER_CORRECTNESS_CASES
        .into_iter()
        .map(str::to_string)
        .collect();
    group.correctness_binding = CorrectnessBinding::ExactCases;
    group.planned_correctness_case_id = None;
    group.workload_family = pauli_string_iter_workload_family(singleton);
    group.work_unit = "Pauli strings".to_string();
    group.output_contract = OutputContract {
        expected_shape: "Exactly eighteen little-endian u64 fields bind iteration count, checked semantic work, width, workload marker, minimum and maximum weight, allowed-axis mask, outputs per traversal, observed output count, observed total result-width checksum, four canonical-input digest lanes, and four last-yielded-result digest lanes."
            .to_string(),
        digest_state: EvidenceState::Existing,
        sink_policy: "Both workers validate one complete traversal outside timing, then each timed callback constructs one public iterator, repeatedly advances its borrowed result, consumes every result width and output count, and destroys the iterator. The observed counters are optimizer-opaque and the last yielded validation result is hashed outside timing."
            .to_string(),
        comparator_sources: [STIM_ADAPTER_SOURCE, PAULI_STRING_ITER_COMPARATOR_SOURCE]
            .into_iter()
            .map(|path| comparator_source(root, path))
            .collect::<Result<_, _>>()?,
    };
    group.memory_policy = circuit_memory_policy(if singleton {
        "Each Stab callback performs exactly four allocation calls. Requested bytes equal two packed result planes plus 40 bytes of traversal state: 296 bytes at small, 8,040 at medium, 250,040 at large, and 262,184 at the accepted maximum. Stim allocation counts remain unclaimed; setup and peak RSS remain report-only until PQ6."
    } else {
        "Each Stab callback performs exactly five allocation calls requesting 120 bytes at all three range scales, including the accepted maximum. Stim allocation counts remain unclaimed; setup and peak RSS remain report-only until PQ6."
    });
    group.threshold_policy = ThresholdPolicy::Primary1_25;
    group.owner = "stab-core/stabilizers".to_string();
    group.reason = if singleton {
        "Implemented paired pinned-Stim and Rust construction plus complete borrowed X/Y/Z singleton traversal with exact CQ2, semantic output, allocation, scale, timing, and bounded-worker contracts."
    } else {
        "Implemented paired pinned-Stim and Rust construction plus complete borrowed X/Z weight-range traversal with exact API ownership, CQ2, semantic output, allocation, scale, timing, and bounded-worker contracts."
    }
    .to_string();
    group.status = QualificationStatus::Implemented;
    Ok(())
}

fn pauli_string_iter_workload_family(singleton: bool) -> WorkloadFamily {
    let (fixture, marker, scales) = if singleton {
        (
            "pauli-iterator-xyz-singleton-v1",
            7,
            [
                (
                    "small",
                    1_000_u64,
                    3_000_u64,
                    "d8d6b42d21392b7ab593f2b09cb9673e261381aa2d93c8f15b8c4ac52a97235b",
                ),
                (
                    "medium",
                    32_000,
                    96_000,
                    "802dc4fd7b6e4d21c2eef73174aa24ee6cb81bc00be978d223a4e4c2242d89f9",
                ),
                (
                    "large",
                    1_000_000,
                    3_000_000,
                    "394634d1a0abfaace26d4f3c07b81fe797d60c474314e625fd7f02f64d25fd0d",
                ),
            ],
        )
    } else {
        (
            "pauli-iterator-xz-weight-range-v1",
            6,
            [
                (
                    "small",
                    5_u64,
                    232_u64,
                    "315732711c88257f9f4b2be3dfc3dd01785be01b86bdb7338e663945a90070d5",
                ),
                (
                    "medium",
                    11,
                    21_604,
                    "d5c711573168f39a388aa386b1fb66b4b9d063f2a070610cd4543442548f4102",
                ),
                (
                    "large",
                    22,
                    972_972,
                    "85017fcee6d99c399676aac24ff1945f03363f316352eb10d707b51c66f506bc",
                ),
            ],
        )
    };
    WorkloadFamily {
        fixture: FixtureLocator::Generated {
            id: fixture.to_string(),
        },
        source: "src/stim/stabilizers/pauli_string_iter.perf.cc".to_string(),
        deterministic_seed: format!("source-owned-enumeration;marker={marker}"),
        scales: scales
            .into_iter()
            .map(|(id, width, outputs, input_digest)| ScalePoint {
                id: id.to_string(),
                    family_id: "default".to_string(),
                    size_class: SizeClass::from_scale_id(id),
                parameters: if singleton {
                    format!(
                        "generator={fixture}; qubits={width}; min_weight=1; max_weight=1; axes=XYZ; outputs={outputs}; marker={marker}"
                    )
                } else {
                    format!(
                        "generator={fixture}; qubits={width}; min_weight=2; max_weight=5; axes=XZ; outputs={outputs}; marker={marker}"
                    )
                },
                input_bytes: InputByteCount::Exact { bytes: 64 },
                semantic_work: Some(outputs),
                input_digest: Some(input_digest.to_string()),
            })
            .collect(),
    }
}

fn apply_pauli_string_multiply(
    root: &RepoRoot,
    group: &mut QualificationGroup,
) -> Result<(), BenchError> {
    group.phase = Phase::Execute;
    group.runner_fidelity = RunnerFidelity::AdapterLibrary;
    group.correctness_cases = PAULI_STRING_MULTIPLY_CORRECTNESS_CASES
        .into_iter()
        .map(str::to_string)
        .collect();
    group.correctness_binding = CorrectnessBinding::ExactCases;
    group.planned_correctness_case_id = None;
    group.workload_family = pauli_string_multiply_workload_family();
    group.work_unit = "qubits".to_string();
    group.output_contract = OutputContract {
        expected_shape: "Exactly seventeen little-endian u64 fields bind iteration count, checked semantic work, width, workload marker, consumed phase checksum, four input-digest lanes, four final-left-digest lanes, and four unchanged-right-digest lanes."
            .to_string(),
        digest_state: EvidenceState::Existing,
        sink_policy: "Both workers build the same equal-width non-identity operands outside timing, execute two untimed public multiplications to restore the left operand, time only direct public in-place calls behind matching compiler fences and optimizer-opaque references, consume every returned phase, retain the final left operand, and hash both operands outside timing."
            .to_string(),
        comparator_sources: [STIM_ADAPTER_SOURCE, PAULI_STRING_MULTIPLY_COMPARATOR_SOURCE]
            .into_iter()
            .map(|path| comparator_source(root, path))
            .collect::<Result<_, _>>()?,
    };
    group.memory_policy = circuit_memory_policy(
        "Two equal-width Pauli strings remain live during timing. Stab allocation instrumentation proves zero calls and zero bytes for each direct public in-place multiplication at every scale and the accepted maximum; setup and peak RSS remain report-only until PQ6.",
    );
    group.threshold_policy = ThresholdPolicy::Primary1_25;
    group.owner = "stab-core/stabilizers".to_string();
    group.reason = "Implemented paired pinned-Stim and Rust direct in-place Pauli multiplication with exact API ownership, CQ2, deterministic non-identity operands, phase and state digests, zero-allocation, scale, timing, and bounded-worker contracts."
        .to_string();
    group.status = QualificationStatus::Implemented;
    Ok(())
}

fn pauli_string_multiply_workload_family() -> WorkloadFamily {
    WorkloadFamily {
        fixture: FixtureLocator::Generated {
            id: "pauli-right-multiply-splitmix64-v1".to_string(),
        },
        source: "src/stim/stabilizers/pauli_string.perf.cc".to_string(),
        deterministic_seed: "left=0x243f6a8885a308d3;right=0x13198a2e03707344".to_string(),
        scales: [
            (
                "small",
                10_000_u64,
                5_056_u64,
                "401b897ceb9c02fec1c57b15f76cdc45045fd551354c3dc5ae499e791aef22f4",
            ),
            (
                "medium",
                100_000,
                50_048,
                "51b8460e6069c3590ce2e25ee912a0ef92dfe1000a28aa4a1aa3b644ba0d402f",
            ),
            (
                "large",
                1_000_000,
                500_032,
                "5babb5f0de800c6ed6c644d103b7a0d01ab22fa7696a363e9120c7cac8157fd9",
            ),
        ]
        .into_iter()
        .map(|(id, qubits, input_bytes, input_digest)| ScalePoint {
            id: id.to_string(),
                    family_id: "default".to_string(),
                    size_class: SizeClass::from_scale_id(id),
            parameters: format!(
                "generator=pauli-right-multiply-splitmix64-v1; qubits={qubits}; marker=5; left_sign=plus; right_sign=minus"
            ),
            input_bytes: InputByteCount::Exact { bytes: input_bytes },
            semantic_work: Some(qubits),
            input_digest: Some(input_digest.to_string()),
        })
        .collect(),
    }
}

fn apply_bit_matrix_transpose(
    root: &RepoRoot,
    group: &mut QualificationGroup,
    allocating: bool,
) -> Result<(), BenchError> {
    group.phase = Phase::Execute;
    group.runner_fidelity = RunnerFidelity::AdapterLibrary;
    group.correctness_cases = BIT_MATRIX_TRANSPOSE_CORRECTNESS_CASES
        .into_iter()
        .map(str::to_string)
        .collect();
    group.correctness_binding = CorrectnessBinding::ExactCases;
    group.planned_correctness_case_id = None;
    group.workload_family = bit_matrix_transpose_workload_family();
    group.work_unit = "transposed-bits".to_string();
    group.output_contract = OutputContract {
        expected_shape: if allocating {
            "Exactly sixteen little-endian u64 fields bind iteration count, declared work, dimension, allocating marker, four input-digest lanes, four result-digest lanes, and four unchanged-source-digest lanes."
        } else {
            "Exactly twelve little-endian u64 fields bind iteration count, declared work, dimension, in-place marker, four input-digest lanes, and four final-state-digest lanes."
        }
        .to_string(),
        digest_state: EvidenceState::Existing,
        sink_policy: if allocating {
            "Both workers keep one immutable source live, execute and discard two untimed public allocating transposes, time one public allocation and transpose per iteration while destroying the preceding result inside the timed body, retain the final result, and hash both result and unchanged source outside timing."
        } else {
            "Both workers execute two untimed public in-place transposes to restore the canonical matrix, time only public in-place calls behind matching compiler fences and optimizer-opaque mutable references, retain the final matrix, and hash it outside timing."
        }
        .to_string(),
        comparator_sources: [STIM_ADAPTER_SOURCE, BIT_MATRIX_TRANSPOSE_COMPARATOR_SOURCE]
            .into_iter()
            .map(|path| comparator_source(root, path))
            .collect::<Result<_, _>>()?,
    };
    group.memory_policy = circuit_memory_policy(if allocating {
        "One immutable source and one retained result remain live after timing. Stab allocation instrumentation proves exactly one result-data allocation of dimension squared divided by eight bytes per public call at every scale and the accepted maximum; setup and peak RSS are report-only until PQ6."
    } else {
        "One mutable matrix remains live during timing. Stab allocation instrumentation proves zero calls and zero bytes for public in-place transpose at every scale and the accepted maximum; setup and peak RSS are report-only until PQ6."
    });
    group.threshold_policy = ThresholdPolicy::Primary1_25;
    group.owner = "stab-core/bits".to_string();
    group.reason = if allocating {
        "Implemented paired pinned-Stim and Rust public allocating square transpose with exact API ownership, CQ2, deterministic non-symmetric input, semantic output, one-allocation, scale, timing, and bounded-worker contracts."
    } else {
        "Implemented paired pinned-Stim and Rust public in-place square transpose with exact API ownership, CQ2, deterministic non-symmetric input, semantic output, zero-allocation, scale, timing, and bounded-worker contracts."
    }
    .to_string();
    group.status = QualificationStatus::Implemented;
    Ok(())
}

fn bit_matrix_transpose_workload_family() -> WorkloadFamily {
    WorkloadFamily {
        fixture: FixtureLocator::Generated {
            id: "bit-matrix-transpose-affine-splitmix64-v1".to_string(),
        },
        source: "src/stim/mem/simd_bit_table.perf.cc".to_string(),
        deterministic_seed: "0xd1b54a32d192ed03".to_string(),
        scales: [
            (
                "small",
                256_u64,
                65_536_u64,
                8_208_u64,
                "2a2a5f587d3c9fdb6fea43274c06ad453fcc76bbbcf6bcd9563991076cdf79da",
            ),
            (
                "medium",
                2_048,
                4_194_304,
                524_304,
                "15e610ea94b541a52446f7ff48ff9ca9560f8dbef5f96232806d0bcbff95f054",
            ),
            (
                "large",
                16_384,
                268_435_456,
                33_554_448,
                "d68c253c0ca01452ce0624f0fdeb67dd92c85b442034b4b0e574286f3c9f636e",
            ),
        ]
        .into_iter()
        .map(
            |(id, dimension, transposed_bits, input_bytes, input_digest)| ScalePoint {
                id: id.to_string(),
                    family_id: "default".to_string(),
                    size_class: SizeClass::from_scale_id(id),
                parameters: format!(
                    "generator=bit-matrix-transpose-affine-splitmix64-v1; dimension={dimension}; set_bits_per_row=8; seed=0xd1b54a32d192ed03"
                ),
                input_bytes: InputByteCount::Exact { bytes: input_bytes },
                semantic_work: Some(transposed_bits),
                input_digest: Some(input_digest.to_string()),
            },
        )
        .collect(),
    }
}

fn apply_simd_bits_xor(root: &RepoRoot, group: &mut QualificationGroup) -> Result<(), BenchError> {
    group.runner_fidelity = RunnerFidelity::AdapterLibrary;
    group.correctness_cases = SIMD_BITS_XOR_CORRECTNESS_CASES
        .into_iter()
        .map(str::to_string)
        .collect();
    group.correctness_binding = CorrectnessBinding::ExactCases;
    group.planned_correctness_case_id = None;
    group.workload_family = simd_bits_xor_workload_family();
    group.output_contract = OutputContract {
        expected_shape: "Exact paired deterministic fixture bytes plus a canonical digest over iteration count, bit width, all four input-fingerprint lanes, all four final-destination lanes, and all four unchanged-source lanes."
            .to_string(),
        digest_state: EvidenceState::Existing,
        sink_policy: "Both workers prepare identical aligned destination and source vectors outside timing, apply only complete-vector XOR behind matching compiler fences, and hash both final vectors outside timing."
            .to_string(),
        comparator_sources: [STIM_ADAPTER_SOURCE, SIMD_BITS_XOR_COMPARATOR_SOURCE]
            .into_iter()
            .map(|path| comparator_source(root, path))
            .collect::<Result<_, _>>()?,
    };
    group.memory_policy = circuit_memory_policy(
        "Two aligned bit vectors remain live during timing and setup and peak process RSS are report-only observations at every scale. Timed mutation reuses preallocated storage; PQ6 owns explicit cross-scale RSS and allocation slack.",
    );
    group.threshold_policy = ThresholdPolicy::Primary1_25;
    group.owner = "stab-core/bits".to_string();
    group.reason = "Implemented paired pinned-Stim and Rust complete-vector XOR work with exact CQ2, deterministic paired input, semantic output, scale, timing, and bounded-worker contracts. The legacy row's not-zero and unmatched logical operations remain separate."
        .to_string();
    group.status = QualificationStatus::Implemented;
    Ok(())
}

fn apply_simd_word_popcount(
    root: &RepoRoot,
    group: &mut QualificationGroup,
) -> Result<(), BenchError> {
    group.runner_fidelity = RunnerFidelity::AdapterLibrary;
    group.correctness_cases = SIMD_WORD_POPCOUNT_CORRECTNESS_CASES
        .into_iter()
        .map(str::to_string)
        .collect();
    group.correctness_binding = CorrectnessBinding::ExactCases;
    group.planned_correctness_case_id = None;
    group.workload_family = simd_word_popcount_workload_family();
    group.output_contract = OutputContract {
        expected_shape: "Exact deterministic fixture bytes plus a canonical digest over eight little-endian u64 fields in this order: accumulated popcount checksum, iteration count, bit width, all four input-fingerprint lanes, and final toggle-bit state."
            .to_string(),
        digest_state: EvidenceState::Existing,
        sink_policy: "Both workers prepare identical little-endian SplitMix64 words and initial toggle state outside timing, toggle bit 300 in the timed body, accumulate exact whole-vector popcounts using Stim ptr_simd[k].popcount() and Stab BitVec::popcount(), then read final state and construct the canonical output digest outside timing."
            .to_string(),
        comparator_sources: [STIM_ADAPTER_SOURCE, SIMD_WORD_POPCOUNT_COMPARATOR_SOURCE]
            .into_iter()
            .map(|path| comparator_source(root, path))
            .collect::<Result<_, _>>()?,
    };
    group.memory_policy = circuit_memory_policy(
        "The aligned bit vector is prepared before timing and setup and peak process RSS are report-only observations at every scale. This slice makes no linear-growth acceptance claim; PQ6 owns explicit cross-scale RSS and allocation slack.",
    );
    group.threshold_policy = ThresholdPolicy::Primary1_25;
    group.owner = "stab-core/bits".to_string();
    group.reason = "Implemented paired pinned-Stim and Rust toggle-plus-popcount work with exact CQ2, deterministic input, semantic output, scale, timing, and bounded-worker contracts."
        .to_string();
    group.status = QualificationStatus::Implemented;
    Ok(())
}

fn comparator_source(root: &RepoRoot, path: &str) -> Result<ComparatorSource, BenchError> {
    let source = super::read_repo_text_bounded(root, &root.path.join(Path::new(path)))?;
    Ok(ComparatorSource {
        path: path.to_string(),
        sha256: super::sha256_hex(source.as_bytes()),
    })
}

fn apply_gate_name_hash(group: &mut QualificationGroup) {
    group.runner_fidelity = RunnerFidelity::AdapterLibrary;
    group.correctness_cases = vec![GATE_NAME_HASH_CORRECTNESS_CASE.to_string()];
    group.correctness_binding = CorrectnessBinding::ExactCases;
    group.planned_correctness_case_id = None;
    group.workload_family = gate_name_hash_workload_family();
    group.output_contract = OutputContract {
        expected_shape: "Exact complete-table hash count plus matching final checksum, ordered table fingerprint, and semantic digest."
            .to_string(),
        digest_state: EvidenceState::Existing,
        sink_policy: "Both workers prepare the 82 Stim gate-table entries, including NOT_A_GATE, outside timing, hash only complete table sweeps in the timed body, and consume the final checksum outside timing."
            .to_string(),
        comparator_sources: Vec::new(),
    };
    group.memory_policy = circuit_memory_policy(
        "The immutable 82-name registry is prepared before timing; setup and peak process RSS are report-only observations at every scale. This slice makes no bounded-growth claim; PQ6 owns an explicit cross-scale RSS and allocation-growth rule.",
    );
    group.threshold_policy = ThresholdPolicy::Primary1_25;
    group.owner = "stab-core/gates".to_string();
    group.reason = "Implemented paired pinned-Stim and Rust all-gate-name hashing with exact CQ2, complete-sweep, output-digest, scale, timing, and memory bindings."
        .to_string();
    group.status = QualificationStatus::Implemented;
}

fn apply_circuit_parse(group: &mut QualificationGroup) {
    group.runner_fidelity = RunnerFidelity::AdapterLibrary;
    group.correctness_cases = CIRCUIT_PARSE_CORRECTNESS_CASES
        .into_iter()
        .map(str::to_string)
        .collect();
    group.correctness_binding = CorrectnessBinding::ExactCases;
    group.planned_correctness_case_id = None;
    group.workload_family = circuit_workload_family();
    group.output_contract = OutputContract {
        expected_shape:
            "Exact fixture byte count and digest plus canonical final-circuit semantic digest."
                .to_string(),
        digest_state: EvidenceState::Existing,
        sink_policy: "Both workers construct the source-owned fixture outside timing, bind its exact bytes, and digest the final parsed circuit outside timing."
            .to_string(),
        comparator_sources: Vec::new(),
    };
    group.memory_policy = circuit_memory_policy(
        "Process setup and peak RSS are reported separately at every timing scale; maximum accepted materialization and first rejection remain PQ6 resource evidence.",
    );
    group.threshold_policy = ThresholdPolicy::Primary1_25;
    group.owner = "stab-core/circuit-parser".to_string();
    group.reason = "Implemented paired adapter and Rust parser workload with exact CQ2, input, output, scale, timing, and memory bindings."
        .to_string();
    group.status = QualificationStatus::Implemented;
}

fn apply_circuit_canonical_print(group: &mut QualificationGroup) {
    group.runner_fidelity = RunnerFidelity::AdapterLibrary;
    group.correctness_cases = CIRCUIT_CANONICAL_PRINT_CORRECTNESS_CASES
        .into_iter()
        .map(str::to_string)
        .collect();
    group.correctness_binding = CorrectnessBinding::ExactCases;
    group.planned_correctness_case_id = None;
    group.workload_family = circuit_workload_family();
    group.output_contract = OutputContract {
        expected_shape:
            "Exact fixture byte count and digest plus final canonical circuit-text digest."
                .to_string(),
        digest_state: EvidenceState::Existing,
        sink_policy: "Both workers parse the source-owned fixture once outside timing, repeatedly serialize the resulting circuit while consuming each produced string, and compare the final canonical digest outside timing after normalizing only Stab's terminal newline."
            .to_string(),
        comparator_sources: Vec::new(),
    };
    group.memory_policy = circuit_memory_policy(
        "Process setup RSS includes the parsed circuit and peak RSS includes canonical output allocation at every timing scale; maximum accepted materialization and first rejection remain PQ6 resource evidence.",
    );
    group.threshold_policy = ThresholdPolicy::Primary1_25;
    group.owner = "stab-core/circuit-printer".to_string();
    group.reason = "Implemented paired adapter and Rust canonical circuit serialization workload with exact CQ2, input, output, scale, timing, and memory bindings."
        .to_string();
    group.status = QualificationStatus::Implemented;
}

fn circuit_workload_family() -> WorkloadFamily {
    WorkloadFamily {
        fixture: FixtureLocator::Generated {
            id: "circuit-parse-cycle-v1".to_string(),
        },
        source: "benchmarks/stim_adapter/main.cc".to_string(),
        deterministic_seed: "circuit-parse-cycle-v1".to_string(),
        scales: [
            (
                "small",
                64,
                429,
                "c3c0855f4f04402cd1768dee1ca0606d7d1ff8907d6a3a4e3b386fd78ff6c3b6",
            ),
            (
                "medium",
                4_096,
                27_981,
                "7c0a60d24fde2f776143003b987c30cd682d77fee5fd9f17bd9e9b5377a8ad04",
            ),
            (
                "large",
                65_536,
                447_821,
                "397e8db6accb8e66a826015e2d5db453271851fa2c49d40a0d98f91748219b60",
            ),
        ]
        .into_iter()
        .map(|(id, instructions, input_bytes, input_digest)| ScalePoint {
            id: id.to_string(),
            family_id: "default".to_string(),
            size_class: SizeClass::from_scale_id(id),
            parameters: format!("generator=circuit-parse-cycle-v1; instructions={instructions}"),
            input_bytes: InputByteCount::Exact { bytes: input_bytes },
            semantic_work: Some(instructions),
            input_digest: Some(input_digest.to_string()),
        })
        .collect(),
    }
}

fn circuit_memory_policy(expected_growth: &str) -> MemoryPolicy {
    MemoryPolicy {
        method: MemoryMethod::ProcessRss,
        scale_ids: ["small", "medium", "large"]
            .into_iter()
            .map(str::to_string)
            .collect(),
        expected_growth: expected_growth.to_string(),
    }
}

fn gate_name_hash_workload_family() -> WorkloadFamily {
    WorkloadFamily {
        fixture: FixtureLocator::Generated {
            id: "stim-v1.16.0-gate-name-table".to_string(),
        },
        source: "src/stim/gates/gates.perf.cc".to_string(),
        deterministic_seed: "not-applicable-static-gate-table".to_string(),
        scales: [("small", 1_u64), ("medium", 64), ("large", 4_096)]
            .into_iter()
            .map(|(id, sweeps)| {
                let gate_hashes = sweeps * 82;
                ScalePoint {
                    id: id.to_string(),
                    family_id: "default".to_string(),
                    size_class: SizeClass::from_scale_id(id),
                    parameters: format!(
                        "generator=stim-v1.16.0-gate-name-table; names=82; complete_sweeps={sweeps}"
                    ),
                    input_bytes: InputByteCount::Exact { bytes: 0 },
                    semantic_work: Some(gate_hashes),
                    input_digest: Some(EMPTY_INPUT_DIGEST.to_string()),
                }
            })
            .collect(),
    }
}

fn simd_word_popcount_workload_family() -> WorkloadFamily {
    WorkloadFamily {
        fixture: FixtureLocator::Generated {
            id: "splitmix64-word-v1".to_string(),
        },
        source: "src/stim/mem/simd_word.perf.cc".to_string(),
        deterministic_seed: "splitmix64-word-v1".to_string(),
        scales: [
            (
                "small",
                4_096,
                512,
                "101e05fc22ce0676c277e9b16363a38750079d12e0b93f3c687ed95457b79d1c",
            ),
            (
                "medium",
                262_144,
                32_768,
                "b33ad442a544ef4b367ab3b2e9a47d65676791ed7661ad7fa2529b5249bfea77",
            ),
            (
                "large",
                16_777_216,
                2_097_152,
                "b1e7afd7d73691441ea033a9eb9496d02fa12bc4d3bcf059856c089112dae368",
            ),
        ]
        .into_iter()
        .map(|(id, bits, input_bytes, input_digest)| ScalePoint {
            id: id.to_string(),
            family_id: "default".to_string(),
            size_class: SizeClass::from_scale_id(id),
            parameters: format!(
                "generator=splitmix64-word-v1; bits={bits}; alignment_bits=256; toggle_bit=300"
            ),
            input_bytes: InputByteCount::Exact { bytes: input_bytes },
            semantic_work: Some(bits),
            input_digest: Some(input_digest.to_string()),
        })
        .collect(),
    }
}

fn simd_bits_xor_workload_family() -> WorkloadFamily {
    WorkloadFamily {
        fixture: FixtureLocator::Generated {
            id: "splitmix64-xor-pair-v1".to_string(),
        },
        source: "src/stim/mem/simd_bits.perf.cc".to_string(),
        deterministic_seed: "splitmix64-xor-pair-v1".to_string(),
        scales: [
            (
                "small",
                4_096,
                1_024,
                "d7fbfcc618ad7e3fd8a616be64f8b41949214afbbca6b58514d40fa5ea7ac498",
            ),
            (
                "medium",
                262_144,
                65_536,
                "7f2b0610db451711e538c7bea04e7cdbead09cc41c088ebfeb3da0788d53ca46",
            ),
            (
                "large",
                16_777_216,
                4_194_304,
                "43fe5c79be45a459124be3bd00a45b65dbc886a6915fe19b3a173d37abc088ee",
            ),
        ]
        .into_iter()
        .map(|(id, bits, input_bytes, input_digest)| ScalePoint {
            id: id.to_string(),
            family_id: "default".to_string(),
            size_class: SizeClass::from_scale_id(id),
            parameters: format!(
                "generator=splitmix64-xor-pair-v1; bits={bits}; alignment_bits=256"
            ),
            input_bytes: InputByteCount::Exact { bytes: input_bytes },
            semantic_work: Some(bits),
            input_digest: Some(input_digest.to_string()),
        })
        .collect(),
    }
}

fn simd_bits_not_zero_workload_family(
    pattern: &str,
    seed: &str,
    input_digests: [&str; 3],
) -> WorkloadFamily {
    WorkloadFamily {
        fixture: FixtureLocator::Generated {
            id: format!("simd-bits-not-zero-{pattern}-v1"),
        },
        source: "src/stim/mem/simd_bits.perf.cc".to_string(),
        deterministic_seed: seed.to_string(),
        scales: [
            ("small", 10_000_u64, 1_256_u64),
            ("medium", 640_000, 80_000),
            ("large", 40_960_000, 5_120_000),
        ]
        .into_iter()
        .zip(input_digests)
        .map(|((id, bits, input_bytes), input_digest)| ScalePoint {
            id: id.to_string(),
            family_id: "default".to_string(),
            size_class: SizeClass::from_scale_id(id),
            parameters: format!("generator=simd-bits-not-zero-v1; bits={bits}; pattern={pattern}"),
            input_bytes: InputByteCount::Exact { bytes: input_bytes },
            semantic_work: Some(bits),
            input_digest: Some(input_digest.to_string()),
        })
        .collect(),
    }
}

fn sparse_xor_workload_family(item_workload: bool) -> WorkloadFamily {
    let (fixture_id, seed, input_bytes, input_digest) = if item_workload {
        (
            "stim-v1.16.0-sparse-xor-item-sequence-v1",
            "items=2,5,9,5,3,6,10",
            36,
            "c2c1749b4bf4c7c355c1d0a8109ea53bba790034d116acea3755b533c1fb1059",
        )
    } else {
        (
            "stim-v1.16.0-sparse-xor-row-table-v1",
            "rows=1000; offsets=0,1,4,8,15",
            28_008,
            "9fdcaf10b6a6437d51afade0e21f39acdd1130ff18255e38c0751261f93df2a2",
        )
    };
    WorkloadFamily {
        fixture: FixtureLocator::Generated {
            id: fixture_id.to_string(),
        },
        source: "src/stim/mem/sparse_xor_vec.perf.cc".to_string(),
        deterministic_seed: seed.to_string(),
        scales: [("small", 1_u64), ("medium", 64), ("large", 4_096)]
            .into_iter()
            .map(|(id, sweeps)| {
                let base_work = if item_workload { 7 } else { 1_997 };
                let work_items = sweeps * base_work;
                ScalePoint {
                    id: id.to_string(),
                    family_id: "default".to_string(),
                    size_class: SizeClass::from_scale_id(id),
                    parameters: if item_workload {
                        format!(
                            "generator={fixture_id}; complete_callbacks={sweeps}; toggles_per_callback=7"
                        )
                    } else {
                        format!(
                            "generator={fixture_id}; complete_callbacks={sweeps}; rows=1000; actual_row_xors_per_callback=1997"
                        )
                    },
                    input_bytes: InputByteCount::Exact { bytes: input_bytes },
                    semantic_work: Some(work_items),
                    input_digest: Some(input_digest.to_string()),
                }
            })
            .collect(),
    }
}
