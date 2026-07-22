use std::collections::BTreeSet;
use std::ffi::OsString;
use std::num::NonZeroU64;
use std::time::Duration;

use clap::{Args, ValueEnum};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::adapter::prepare_adapter;
use super::process::{ProcessLimits, ProcessRequest, ProcessResult, run_bounded_process};
use super::protocol::{
    EvidenceMode, GitCommit, Implementation, InputDigest, ProtocolExpectation, ProtocolId,
    parse_worker_json_lines,
};
use super::statistics::{PairOrder, pair_measurements};
use super::worker;
use crate::config::STIM_COMMIT;
use crate::root::RepoRoot;

mod clifford_string;
mod dem_model;
mod pauli_iter;

const ADAPTER_PROBE_ID: &str = "pq1-adapter-protocol-smoke";
const CIRCUIT_PARSE_PROBE_ID: &str = "pq2-circuit-parse-adapter-smoke";
const CIRCUIT_CANONICAL_PRINT_PROBE_ID: &str = "pq2-circuit-canonical-print-adapter-smoke";
const GATE_NAME_HASH_PROBE_ID: &str = "pq2-gate-name-hash-adapter-smoke";
const SIMD_BITS_XOR_PROBE_ID: &str = "pq2-simd-bits-xor-adapter-smoke";
const SIMD_BITS_NOT_ZERO_EARLY_PROBE_ID: &str = "pq2-simd-bits-not-zero-early-adapter-smoke";
const SIMD_BITS_NOT_ZERO_ALL_ZERO_PROBE_ID: &str = "pq2-simd-bits-not-zero-all-zero-adapter-smoke";
const SIMD_BITS_NOT_ZERO_LATE_PROBE_ID: &str = "pq2-simd-bits-not-zero-late-adapter-smoke";
const SPARSE_XOR_ROW_PROBE_ID: &str = "pq2-sparse-xor-row-adapter-smoke";
const SPARSE_XOR_ITEM_PROBE_ID: &str = "pq2-sparse-xor-item-adapter-smoke";
const BIT_MATRIX_TRANSPOSE_IN_PLACE_PROBE_ID: &str =
    "pq2-bit-matrix-transpose-in-place-adapter-smoke";
const BIT_MATRIX_TRANSPOSE_ALLOCATING_PROBE_ID: &str =
    "pq2-bit-matrix-transpose-allocating-adapter-smoke";
const PAULI_STRING_MULTIPLY_PROBE_ID: &str = "pq2-pauli-string-multiply-adapter-smoke";
const PAULI_STRING_ITER_RANGE_PROBE_ID: &str = "pq2-pauli-iter-range-adapter-smoke";
const PAULI_STRING_ITER_SINGLETON_PROBE_ID: &str = "pq2-pauli-iter-singleton-adapter-smoke";
const DEM_PARSE_PROBE_ID: &str = "pq2-dem-parse-adapter-smoke";
const DEM_CANONICAL_PRINT_PROBE_ID: &str = "pq2-dem-canonical-print-adapter-smoke";
const SIMD_WORD_POPCOUNT_PROBE_ID: &str = "pq2-simd-word-popcount-adapter-smoke";
const PROCESS_PROBE_ID: &str = "pq1-process-contract-smoke";
const PROTOCOL_OUTPUT_LIMIT: usize = 1 << 20;
const DEFAULT_PROBE_WORK_ITEMS: u64 = 4_096;
const DEFAULT_GATE_HASH_WORK_ITEMS: u64 = 5_248;
const DEFAULT_POPCOUNT_WORK_ITEMS: u64 = 262_144;
const DEFAULT_NOT_ZERO_WORK_ITEMS: u64 = 10_000;
const DEFAULT_TRANSPOSE_WORK_ITEMS: u64 = 65_536;
const DEFAULT_PAULI_WORK_ITEMS: u64 = 10_000;
const DEFAULT_PAULI_ITER_RANGE_WORK_ITEMS: u64 = 232;
const DEFAULT_PAULI_ITER_SINGLETON_WORK_ITEMS: u64 = 3_000;
const DEFAULT_CLIFFORD_WORK_ITEMS: u64 = 10_000;
const GATE_HASH_NAME_COUNT: u64 = 82;
const POPCOUNT_ALIGNMENT_BITS: u64 = 256;
const POPCOUNT_MIN_BITS: u64 = 512;
const POPCOUNT_MAX_BITS: u64 = 268_435_456;
const XOR_ALIGNMENT_BITS: u64 = 256;
const XOR_MIN_BITS: u64 = 256;
const XOR_MAX_BITS: u64 = 268_435_456;
const NOT_ZERO_MIN_BITS: u64 = 64;
const NOT_ZERO_MAX_BITS: u64 = 268_435_456;
const SPARSE_XOR_ROW_BASE_WORK_ITEMS: u64 = 1_997;
const SPARSE_XOR_ROW_MAX_WORK_ITEMS: u64 = 8_179_712;
const SPARSE_XOR_ITEM_BASE_WORK_ITEMS: u64 = 7;
const SPARSE_XOR_ITEM_MAX_WORK_ITEMS: u64 = 28_672;
const TRANSPOSE_MIN_DIMENSION: u64 = 256;
const TRANSPOSE_MAX_DIMENSION: u64 = 16_384;
const TRANSPOSE_DIMENSION_ALIGNMENT: u64 = 256;
const PAULI_MIN_QUBITS: u64 = 1;
const PAULI_MAX_QUBITS: u64 = 1_048_576;
const EMPTY_INPUT_DIGEST: &str = "6a09e667f3bcc908bb67ae8584caa73b3c6ef372fe94f82ba54ff53a5f1d36f1";
const CIRCUIT_PARSE_RUNTIME_GROUP_ID: &str = "PERFQ-M4-CIRCUIT-PARSE";
const CIRCUIT_CANONICAL_PRINT_RUNTIME_GROUP_ID: &str = "PERFQ-M4-CIRCUIT-CANONICAL-PRINT";
const GATE_NAME_HASH_RUNTIME_GROUP_ID: &str = "PERFQ-M4-GATE-LOOKUP";
const SIMD_WORD_POPCOUNT_RUNTIME_GROUP_ID: &str = "PERFQ-M5-SIMD-WORD";
const SIMD_BITS_XOR_RUNTIME_GROUP_ID: &str = "PERFQ-M5-SIMD-BITS";
const SIMD_BITS_NOT_ZERO_EARLY_RUNTIME_GROUP_ID: &str = "PERFQ-M5-SIMD-BITS-NOT-ZERO-EARLY";
const SIMD_BITS_NOT_ZERO_ALL_ZERO_RUNTIME_GROUP_ID: &str = "PERFQ-M5-SIMD-BITS-NOT-ZERO-ALL-ZERO";
const SIMD_BITS_NOT_ZERO_LATE_RUNTIME_GROUP_ID: &str = "PERFQ-M5-SIMD-BITS-NOT-ZERO-LATE";
const SPARSE_XOR_ROW_RUNTIME_GROUP_ID: &str = "PERFQ-M5-SPARSE-XOR";
const SPARSE_XOR_ITEM_RUNTIME_GROUP_ID: &str = "PERFQ-M5-SPARSE-XOR-ITEM";
const BIT_MATRIX_TRANSPOSE_IN_PLACE_RUNTIME_GROUP_ID: &str =
    "PERFQ-M5-BIT-MATRIX-TRANSPOSE-IN-PLACE";
const BIT_MATRIX_TRANSPOSE_ALLOCATING_RUNTIME_GROUP_ID: &str =
    "PERFQ-M5-BIT-MATRIX-TRANSPOSE-ALLOCATING";
const PAULI_STRING_MULTIPLY_RUNTIME_GROUP_ID: &str = "PERFQ-M6-PAULI-STRING";
const PAULI_STRING_ITER_RANGE_RUNTIME_GROUP_ID: &str = "PERFQ-M6-PAULI-ITER";
const PAULI_STRING_ITER_SINGLETON_RUNTIME_GROUP_ID: &str = "PERFQ-M6-PAULI-ITER-SINGLETON";
const DEM_PARSE_RUNTIME_GROUP_ID: &str = "PERFQ-M10-DEM-PARSE-CONTRACT";
const DEM_CANONICAL_PRINT_RUNTIME_GROUP_ID: &str = "PERFQ-M10-DEM-PRINT-CONTRACT";

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum ProbeGroup {
    #[value(name = "pq1-process-contract-smoke")]
    ProcessContract,
    #[value(name = "pq1-adapter-protocol-smoke")]
    AdapterProtocol,
    #[value(name = "pq2-circuit-parse-adapter-smoke")]
    CircuitParseAdapter,
    #[value(name = "pq2-circuit-canonical-print-adapter-smoke")]
    CircuitCanonicalPrintAdapter,
    #[value(name = "pq2-gate-name-hash-adapter-smoke")]
    GateNameHashAdapter,
    #[value(name = "pq2-simd-word-popcount-adapter-smoke")]
    SimdWordPopcountAdapter,
    #[value(name = "pq2-simd-bits-xor-adapter-smoke")]
    SimdBitsXorAdapter,
    #[value(name = "pq2-simd-bits-not-zero-early-adapter-smoke")]
    SimdBitsNotZeroEarlyAdapter,
    #[value(name = "pq2-simd-bits-not-zero-all-zero-adapter-smoke")]
    SimdBitsNotZeroAllZeroAdapter,
    #[value(name = "pq2-simd-bits-not-zero-late-adapter-smoke")]
    SimdBitsNotZeroLateAdapter,
    #[value(name = "pq2-sparse-xor-row-adapter-smoke")]
    SparseXorRowAdapter,
    #[value(name = "pq2-sparse-xor-item-adapter-smoke")]
    SparseXorItemAdapter,
    #[value(name = "pq2-bit-matrix-transpose-in-place-adapter-smoke")]
    BitMatrixTransposeInPlaceAdapter,
    #[value(name = "pq2-bit-matrix-transpose-allocating-adapter-smoke")]
    BitMatrixTransposeAllocatingAdapter,
    #[value(name = "pq2-pauli-string-multiply-adapter-smoke")]
    PauliStringMultiplyAdapter,
    #[value(name = "pq2-pauli-iter-range-adapter-smoke")]
    PauliStringIterRangeAdapter,
    #[value(name = "pq2-pauli-iter-singleton-adapter-smoke")]
    PauliStringIterSingletonAdapter,
    #[value(name = "pq2-clifford-string-identity-adapter-smoke")]
    CliffordStringIdentityAdapter,
    #[value(name = "pq2-clifford-string-non-identity-adapter-smoke")]
    CliffordStringNonIdentityAdapter,
    #[value(name = "pq2-dem-parse-adapter-smoke")]
    DemParseAdapter,
    #[value(name = "pq2-dem-canonical-print-adapter-smoke")]
    DemCanonicalPrintAdapter,
}

impl ProbeGroup {
    fn runtime_group_id(self) -> Option<&'static str> {
        match self {
            Self::ProcessContract => None,
            Self::AdapterProtocol => Some(ADAPTER_PROBE_ID),
            Self::CircuitParseAdapter => Some(CIRCUIT_PARSE_RUNTIME_GROUP_ID),
            Self::CircuitCanonicalPrintAdapter => Some(CIRCUIT_CANONICAL_PRINT_RUNTIME_GROUP_ID),
            Self::GateNameHashAdapter => Some(GATE_NAME_HASH_RUNTIME_GROUP_ID),
            Self::SimdWordPopcountAdapter => Some(SIMD_WORD_POPCOUNT_RUNTIME_GROUP_ID),
            Self::SimdBitsXorAdapter => Some(SIMD_BITS_XOR_RUNTIME_GROUP_ID),
            Self::SimdBitsNotZeroEarlyAdapter => Some(SIMD_BITS_NOT_ZERO_EARLY_RUNTIME_GROUP_ID),
            Self::SimdBitsNotZeroAllZeroAdapter => {
                Some(SIMD_BITS_NOT_ZERO_ALL_ZERO_RUNTIME_GROUP_ID)
            }
            Self::SimdBitsNotZeroLateAdapter => Some(SIMD_BITS_NOT_ZERO_LATE_RUNTIME_GROUP_ID),
            Self::SparseXorRowAdapter => Some(SPARSE_XOR_ROW_RUNTIME_GROUP_ID),
            Self::SparseXorItemAdapter => Some(SPARSE_XOR_ITEM_RUNTIME_GROUP_ID),
            Self::BitMatrixTransposeInPlaceAdapter => {
                Some(BIT_MATRIX_TRANSPOSE_IN_PLACE_RUNTIME_GROUP_ID)
            }
            Self::BitMatrixTransposeAllocatingAdapter => {
                Some(BIT_MATRIX_TRANSPOSE_ALLOCATING_RUNTIME_GROUP_ID)
            }
            Self::PauliStringMultiplyAdapter => Some(PAULI_STRING_MULTIPLY_RUNTIME_GROUP_ID),
            Self::PauliStringIterRangeAdapter => Some(PAULI_STRING_ITER_RANGE_RUNTIME_GROUP_ID),
            Self::PauliStringIterSingletonAdapter => {
                Some(PAULI_STRING_ITER_SINGLETON_RUNTIME_GROUP_ID)
            }
            Self::CliffordStringIdentityAdapter => Some(clifford_string::IDENTITY_RUNTIME_GROUP_ID),
            Self::CliffordStringNonIdentityAdapter => {
                Some(clifford_string::NON_IDENTITY_RUNTIME_GROUP_ID)
            }
            Self::DemParseAdapter => Some(DEM_PARSE_RUNTIME_GROUP_ID),
            Self::DemCanonicalPrintAdapter => Some(DEM_CANONICAL_PRINT_RUNTIME_GROUP_ID),
        }
    }

    fn for_runtime_group(group_id: &str) -> Option<Self> {
        match group_id {
            ADAPTER_PROBE_ID => Some(Self::AdapterProtocol),
            CIRCUIT_PARSE_RUNTIME_GROUP_ID => Some(Self::CircuitParseAdapter),
            CIRCUIT_CANONICAL_PRINT_RUNTIME_GROUP_ID => Some(Self::CircuitCanonicalPrintAdapter),
            GATE_NAME_HASH_RUNTIME_GROUP_ID => Some(Self::GateNameHashAdapter),
            SIMD_WORD_POPCOUNT_RUNTIME_GROUP_ID => Some(Self::SimdWordPopcountAdapter),
            SIMD_BITS_XOR_RUNTIME_GROUP_ID => Some(Self::SimdBitsXorAdapter),
            SIMD_BITS_NOT_ZERO_EARLY_RUNTIME_GROUP_ID => Some(Self::SimdBitsNotZeroEarlyAdapter),
            SIMD_BITS_NOT_ZERO_ALL_ZERO_RUNTIME_GROUP_ID => {
                Some(Self::SimdBitsNotZeroAllZeroAdapter)
            }
            SIMD_BITS_NOT_ZERO_LATE_RUNTIME_GROUP_ID => Some(Self::SimdBitsNotZeroLateAdapter),
            SPARSE_XOR_ROW_RUNTIME_GROUP_ID => Some(Self::SparseXorRowAdapter),
            SPARSE_XOR_ITEM_RUNTIME_GROUP_ID => Some(Self::SparseXorItemAdapter),
            BIT_MATRIX_TRANSPOSE_IN_PLACE_RUNTIME_GROUP_ID => {
                Some(Self::BitMatrixTransposeInPlaceAdapter)
            }
            BIT_MATRIX_TRANSPOSE_ALLOCATING_RUNTIME_GROUP_ID => {
                Some(Self::BitMatrixTransposeAllocatingAdapter)
            }
            PAULI_STRING_MULTIPLY_RUNTIME_GROUP_ID => Some(Self::PauliStringMultiplyAdapter),
            PAULI_STRING_ITER_RANGE_RUNTIME_GROUP_ID => Some(Self::PauliStringIterRangeAdapter),
            PAULI_STRING_ITER_SINGLETON_RUNTIME_GROUP_ID => {
                Some(Self::PauliStringIterSingletonAdapter)
            }
            clifford_string::IDENTITY_RUNTIME_GROUP_ID => Some(Self::CliffordStringIdentityAdapter),
            clifford_string::NON_IDENTITY_RUNTIME_GROUP_ID => {
                Some(Self::CliffordStringNonIdentityAdapter)
            }
            DEM_PARSE_RUNTIME_GROUP_ID => Some(Self::DemParseAdapter),
            DEM_CANONICAL_PRINT_RUNTIME_GROUP_ID => Some(Self::DemCanonicalPrintAdapter),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum ProbeEvidenceMode {
    Timing,
    Memory,
}

impl ProbeEvidenceMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Timing => "timing",
            Self::Memory => "memory",
        }
    }
}

impl From<ProbeEvidenceMode> for EvidenceMode {
    fn from(value: ProbeEvidenceMode) -> Self {
        match value {
            ProbeEvidenceMode::Timing => Self::Timing,
            ProbeEvidenceMode::Memory => Self::Memory,
        }
    }
}

#[derive(Clone, Debug, Args)]
pub(crate) struct ProbeArgs {
    /// Exact source-owned probe group.
    #[arg(long, value_enum)]
    group: ProbeGroup,

    /// Worker iterations for the bounded protocol probe.
    #[arg(long, default_value = "4")]
    iterations: NonZeroU64,

    /// Semantic work items per worker iteration; defaults to a group-valid smoke scale.
    #[arg(long)]
    work_items: Option<NonZeroU64>,

    /// Produce timing or separately classified memory evidence.
    #[arg(long, value_enum, default_value = "timing")]
    evidence_mode: ProbeEvidenceMode,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct AdapterProbeReceipt {
    pub(super) probe_id: String,
    pub(super) runtime_group_id: String,
    pub(super) evidence_mode: String,
    pub(super) iteration_count: u64,
    pub(super) work_items: u64,
    pub(super) work_count: u64,
    pub(super) input_bytes: u64,
    pub(super) input_digest: String,
    pub(super) output_digest: String,
    pub(super) stim_source_sha256: String,
    pub(super) stim_build_fingerprint: String,
    pub(super) stim_binary_sha256: String,
    pub(super) stab_source_sha256: String,
    pub(super) stab_build_fingerprint: String,
}

pub(super) fn run(root: &RepoRoot, args: ProbeArgs) -> Result<(), ProbeError> {
    validate_probe_work_items(args.group, probe_work_items(&args))?;
    match args.group {
        ProbeGroup::ProcessContract => run_process_probe(root, args),
        ProbeGroup::AdapterProtocol
        | ProbeGroup::CircuitParseAdapter
        | ProbeGroup::CircuitCanonicalPrintAdapter
        | ProbeGroup::GateNameHashAdapter
        | ProbeGroup::SimdWordPopcountAdapter
        | ProbeGroup::SimdBitsXorAdapter
        | ProbeGroup::SimdBitsNotZeroEarlyAdapter
        | ProbeGroup::SimdBitsNotZeroAllZeroAdapter
        | ProbeGroup::SimdBitsNotZeroLateAdapter
        | ProbeGroup::SparseXorRowAdapter
        | ProbeGroup::SparseXorItemAdapter
        | ProbeGroup::BitMatrixTransposeInPlaceAdapter
        | ProbeGroup::BitMatrixTransposeAllocatingAdapter
        | ProbeGroup::PauliStringMultiplyAdapter
        | ProbeGroup::PauliStringIterRangeAdapter
        | ProbeGroup::PauliStringIterSingletonAdapter
        | ProbeGroup::CliffordStringIdentityAdapter
        | ProbeGroup::CliffordStringNonIdentityAdapter
        | ProbeGroup::DemParseAdapter
        | ProbeGroup::DemCanonicalPrintAdapter => run_adapter_probe(root, args).map(|_| ()),
    }
}

pub(super) fn run_source_owned_adapter_probe(
    root: &RepoRoot,
    runtime_group_id: &str,
) -> Result<AdapterProbeReceipt, ProbeError> {
    let group = ProbeGroup::for_runtime_group(runtime_group_id)
        .ok_or_else(|| ProbeError::MissingRuntimeGroup(runtime_group_id.to_string()))?;
    let iterations = NonZeroU64::new(4)
        .ok_or_else(|| ProbeError::Contract("probe iteration count must be nonzero".to_string()))?;
    let args = ProbeArgs {
        group,
        iterations,
        work_items: None,
        evidence_mode: ProbeEvidenceMode::Timing,
    };
    validate_probe_work_items(args.group, probe_work_items(&args))?;
    run_adapter_probe(root, args)
}

fn run_process_probe(root: &RepoRoot, args: ProbeArgs) -> Result<(), ProbeError> {
    let expected_work_count = expected_work_count(&args)?;
    let identity = worker::current_identity()?;
    let current_exe = std::env::current_exe().map_err(ProbeError::CurrentExecutable)?;
    let request = ProcessRequest {
        program: current_exe,
        args: worker_arguments(&args),
        stdin: Vec::new(),
        working_directory: root.path.clone(),
        environment: probe_environment(),
        affinity_cpu: None,
        limits: probe_limits(),
    };
    let output = checked_process(run_bounded_process(&request)?, "Stab worker")?;
    let rows = parse_worker_json_lines(&output.stdout)?;
    ProtocolExpectation {
        implementation: Implementation::Stab,
        evidence_mode: args.evidence_mode.into(),
        workload_id: ProtocolId::try_new("protocol-smoke")?,
        measurement_ids: BTreeSet::from([ProtocolId::try_new("main")?]),
        iteration_count: args.iterations.get(),
        expected_work_count,
        expected_input_bytes: 0,
        expected_input_digest: InputDigest::try_new(EMPTY_INPUT_DIGEST)?,
        expected_output_digest: None,
        affinity_cpu: None,
        stim_commit: GitCommit::try_new(STIM_COMMIT)?,
        source_digest: identity.source_digest.clone(),
        build_fingerprint: identity.build_fingerprint.clone(),
    }
    .validate(&rows)?;
    let current = worker::current_identity()?;
    if current.source_digest != identity.source_digest
        || current.build_fingerprint != identity.build_fingerprint
    {
        return Err(ProbeError::WorkerIdentityChanged);
    }
    let row = rows
        .first()
        .ok_or_else(|| ProbeError::Contract("process probe returned no row".to_string()))?;
    println!(
        "[stab-bench] probe={} mode={} work={} measured_seconds={:.9} wall_seconds={:.9} parent_peak_rss={}",
        PROCESS_PROBE_ID,
        args.evidence_mode.as_str(),
        row.work_count,
        row.elapsed_seconds,
        output.wall_elapsed.as_secs_f64(),
        display_rss(output.parent_observed_peak_rss_bytes),
    );
    Ok(())
}

fn run_adapter_probe(root: &RepoRoot, args: ProbeArgs) -> Result<AdapterProbeReceipt, ProbeError> {
    let expected_work_count = expected_work_count(&args)?;
    let (probe_id, workload, measurement) = match args.group {
        ProbeGroup::AdapterProtocol => (ADAPTER_PROBE_ID, "protocol-smoke", "main"),
        ProbeGroup::CircuitParseAdapter => (CIRCUIT_PARSE_PROBE_ID, "circuit-parse", "parse"),
        ProbeGroup::CircuitCanonicalPrintAdapter => (
            CIRCUIT_CANONICAL_PRINT_PROBE_ID,
            "circuit-canonical-print",
            "serialize",
        ),
        ProbeGroup::GateNameHashAdapter => {
            (GATE_NAME_HASH_PROBE_ID, "gate-name-hash", "hash-all-names")
        }
        ProbeGroup::SimdWordPopcountAdapter => (
            SIMD_WORD_POPCOUNT_PROBE_ID,
            "simd-word-popcount",
            "toggle-popcount",
        ),
        ProbeGroup::SimdBitsXorAdapter => (
            SIMD_BITS_XOR_PROBE_ID,
            "simd-bits-xor",
            "xor-complete-vector",
        ),
        ProbeGroup::SimdBitsNotZeroEarlyAdapter => (
            SIMD_BITS_NOT_ZERO_EARLY_PROBE_ID,
            "simd-bits-not-zero-early",
            "not-zero",
        ),
        ProbeGroup::SimdBitsNotZeroAllZeroAdapter => (
            SIMD_BITS_NOT_ZERO_ALL_ZERO_PROBE_ID,
            "simd-bits-not-zero-zero",
            "not-zero",
        ),
        ProbeGroup::SimdBitsNotZeroLateAdapter => (
            SIMD_BITS_NOT_ZERO_LATE_PROBE_ID,
            "simd-bits-not-zero-late",
            "not-zero",
        ),
        ProbeGroup::SparseXorRowAdapter => (SPARSE_XOR_ROW_PROBE_ID, "sparse-xor-row", "row-xor"),
        ProbeGroup::SparseXorItemAdapter => {
            (SPARSE_XOR_ITEM_PROBE_ID, "sparse-xor-item", "xor-item")
        }
        ProbeGroup::BitMatrixTransposeInPlaceAdapter => (
            BIT_MATRIX_TRANSPOSE_IN_PLACE_PROBE_ID,
            "bit-matrix-transpose-in-place",
            "in-place-transpose",
        ),
        ProbeGroup::BitMatrixTransposeAllocatingAdapter => (
            BIT_MATRIX_TRANSPOSE_ALLOCATING_PROBE_ID,
            "bit-matrix-transpose-allocating",
            "allocating-transpose",
        ),
        ProbeGroup::PauliStringMultiplyAdapter => (
            PAULI_STRING_MULTIPLY_PROBE_ID,
            "pauli-string-right-multiply",
            "right-multiply-in-place",
        ),
        ProbeGroup::PauliStringIterRangeAdapter => (
            PAULI_STRING_ITER_RANGE_PROBE_ID,
            "pauli-string-iter-range",
            "construct-and-iterate-borrowed",
        ),
        ProbeGroup::PauliStringIterSingletonAdapter => (
            PAULI_STRING_ITER_SINGLETON_PROBE_ID,
            "pauli-string-iter-singleton",
            "construct-and-iterate-borrowed",
        ),
        ProbeGroup::CliffordStringIdentityAdapter
        | ProbeGroup::CliffordStringNonIdentityAdapter => {
            clifford_string::probe_contract(args.group).ok_or_else(|| {
                ProbeError::Contract("missing Clifford probe contract".to_string())
            })?
        }
        ProbeGroup::DemParseAdapter => (DEM_PARSE_PROBE_ID, "dem-parse", "parse"),
        ProbeGroup::DemCanonicalPrintAdapter => (
            DEM_CANONICAL_PRINT_PROBE_ID,
            "dem-canonical-print",
            "serialize",
        ),
        ProbeGroup::ProcessContract => {
            return Err(ProbeError::Contract(
                "process-only probe cannot use the adapter path".to_string(),
            ));
        }
    };
    let repository = super::git::repository_state(root)?;
    let adapter = prepare_adapter(root, &repository.commit)?;
    let worker_identity = worker::current_identity()?;
    let current_exe = std::env::current_exe().map_err(ProbeError::CurrentExecutable)?;
    let mut common_arguments = vec![
        OsString::from("--workload"),
        OsString::from(workload),
        OsString::from("--measurement-id"),
        OsString::from(measurement),
        OsString::from("--iterations"),
        OsString::from(args.iterations.get().to_string()),
        OsString::from("--work-items"),
        OsString::from(probe_work_items(&args).to_string()),
        OsString::from("--evidence-mode"),
        OsString::from(args.evidence_mode.as_str()),
    ];
    clifford_string::append_descriptor_arguments(
        args.group,
        probe_work_items(&args),
        &mut common_arguments,
    )?;
    let adapter_request = ProcessRequest {
        program: adapter.path.clone(),
        args: common_arguments.clone(),
        stdin: Vec::new(),
        working_directory: root.path.clone(),
        environment: probe_environment(),
        affinity_cpu: None,
        limits: probe_limits(),
    };
    let mut worker_arguments = vec![OsString::from("qualification-worker")];
    worker_arguments.extend(common_arguments);
    let worker_request = ProcessRequest {
        program: current_exe.clone(),
        args: worker_arguments,
        stdin: Vec::new(),
        working_directory: root.path.clone(),
        environment: probe_environment(),
        affinity_cpu: None,
        limits: probe_limits(),
    };

    let stim_output = checked_process(run_bounded_process(&adapter_request)?, "Stim adapter")?;
    let stab_output = checked_process(run_bounded_process(&worker_request)?, "Stab worker")?;
    adapter.verify()?;
    let post_worker_identity = worker::current_identity()?;
    if post_worker_identity.source_digest != worker_identity.source_digest
        || post_worker_identity.build_fingerprint != worker_identity.build_fingerprint
    {
        return Err(ProbeError::WorkerIdentityChanged);
    }

    let stim_rows = parse_worker_json_lines(&stim_output.stdout)?;
    let stab_rows = parse_worker_json_lines(&stab_output.stdout)?;
    let workload_id = ProtocolId::try_new(workload)?;
    let measurement_id = ProtocolId::try_new(measurement)?;
    let measurement_ids = BTreeSet::from([measurement_id.clone()]);
    let stim_commit = GitCommit::try_new(STIM_COMMIT)?;
    let stim_input = stim_rows
        .first()
        .ok_or_else(|| ProbeError::Contract("Stim probe returned no row".to_string()))?;
    let expected_input_bytes = stim_input.input_bytes;
    let expected_input_digest = stim_input.input_digest.clone();
    ProtocolExpectation {
        implementation: Implementation::Stim,
        evidence_mode: args.evidence_mode.into(),
        workload_id: workload_id.clone(),
        measurement_ids: measurement_ids.clone(),
        iteration_count: args.iterations.get(),
        expected_work_count,
        expected_input_bytes,
        expected_input_digest: expected_input_digest.clone(),
        expected_output_digest: None,
        affinity_cpu: None,
        stim_commit: stim_commit.clone(),
        source_digest: adapter.source_digest.clone(),
        build_fingerprint: adapter.build_fingerprint.clone(),
    }
    .validate(&stim_rows)?;
    ProtocolExpectation {
        implementation: Implementation::Stab,
        evidence_mode: args.evidence_mode.into(),
        workload_id,
        measurement_ids,
        iteration_count: args.iterations.get(),
        expected_work_count,
        expected_input_bytes,
        expected_input_digest,
        expected_output_digest: None,
        affinity_cpu: None,
        stim_commit,
        source_digest: worker_identity.source_digest.clone(),
        build_fingerprint: worker_identity.build_fingerprint.clone(),
    }
    .validate(&stab_rows)?;
    pauli_iter::validate_boundaries(root, args.group, &adapter, &current_exe, &worker_identity)?;
    clifford_string::validate_boundaries(
        root,
        args.group,
        &adapter,
        &current_exe,
        &worker_identity,
    )?;
    dem_model::validate_boundaries(root, args.group, &adapter, &current_exe, &worker_identity)?;

    if args.evidence_mode == ProbeEvidenceMode::Timing {
        let pairs = pair_measurements(0, PairOrder::StimThenStab, &stim_rows, &stab_rows)?;
        let pair = pairs.first().ok_or_else(|| {
            ProbeError::Contract("paired protocol probe returned no row".to_string())
        })?;
        println!(
            "[stab-bench] probe={} mode=timing work={} stim_seconds={:.9} stab_seconds={:.9} diagnostic_ratio={:.6} stim_parent_peak_rss={} stab_parent_peak_rss={}",
            probe_id,
            pair.stim_work_count,
            pair.stim_elapsed_seconds,
            pair.stab_elapsed_seconds,
            pair.ratio,
            display_rss(stim_output.parent_observed_peak_rss_bytes),
            display_rss(stab_output.parent_observed_peak_rss_bytes),
        );
    } else {
        let stim = stim_rows
            .first()
            .ok_or_else(|| ProbeError::Contract("Stim memory probe returned no row".to_string()))?;
        let stab = stab_rows
            .first()
            .ok_or_else(|| ProbeError::Contract("Stab memory probe returned no row".to_string()))?;
        if stim.output_digest != stab.output_digest || stim.work_count != stab.work_count {
            return Err(ProbeError::Contract(
                "memory probe work or semantic output differs".to_string(),
            ));
        }
        println!(
            "[stab-bench] probe={} mode=memory work={} stim_setup_rss={} stim_peak_rss={} stab_setup_rss={} stab_peak_rss={}",
            probe_id,
            stim.work_count,
            display_rss(stim.setup_rss_bytes),
            display_rss(stim.peak_rss_bytes),
            display_rss(stab.setup_rss_bytes),
            display_rss(stab.peak_rss_bytes),
        );
    }
    let stim = stim_rows
        .first()
        .ok_or_else(|| ProbeError::Contract("Stim probe returned no row".to_string()))?;
    let stab = stab_rows
        .first()
        .ok_or_else(|| ProbeError::Contract("Stab probe returned no row".to_string()))?;
    let runtime_group_id = args
        .group
        .runtime_group_id()
        .ok_or_else(|| ProbeError::Contract("adapter probe has no runtime group".to_string()))?;
    Ok(AdapterProbeReceipt {
        probe_id: probe_id.to_string(),
        runtime_group_id: runtime_group_id.to_string(),
        evidence_mode: args.evidence_mode.as_str().to_string(),
        iteration_count: args.iterations.get(),
        work_items: probe_work_items(&args),
        work_count: stim.work_count,
        input_bytes: stim.input_bytes,
        input_digest: stim.input_digest.as_str().to_string(),
        output_digest: stim.output_digest.as_str().to_string(),
        stim_source_sha256: stim.source_digest.as_str().to_string(),
        stim_build_fingerprint: stim.build_fingerprint.as_str().to_string(),
        stim_binary_sha256: adapter.binary_digest.as_str().to_string(),
        stab_source_sha256: stab.source_digest.as_str().to_string(),
        stab_build_fingerprint: stab.build_fingerprint.as_str().to_string(),
    })
}

fn worker_arguments(args: &ProbeArgs) -> Vec<OsString> {
    vec![
        OsString::from("qualification-worker"),
        OsString::from("--workload"),
        OsString::from("protocol-smoke"),
        OsString::from("--measurement-id"),
        OsString::from("main"),
        OsString::from("--iterations"),
        OsString::from(args.iterations.get().to_string()),
        OsString::from("--work-items"),
        OsString::from(probe_work_items(args).to_string()),
        OsString::from("--evidence-mode"),
        OsString::from(args.evidence_mode.as_str()),
    ]
}

fn expected_work_count(args: &ProbeArgs) -> Result<u64, ProbeError> {
    args.iterations
        .get()
        .checked_mul(probe_work_items(args))
        .ok_or(ProbeError::WorkOverflow)
}

fn probe_work_items(args: &ProbeArgs) -> u64 {
    args.work_items.map_or_else(
        || match args.group {
            ProbeGroup::GateNameHashAdapter => DEFAULT_GATE_HASH_WORK_ITEMS,
            ProbeGroup::SimdWordPopcountAdapter | ProbeGroup::SimdBitsXorAdapter => {
                DEFAULT_POPCOUNT_WORK_ITEMS
            }
            ProbeGroup::SimdBitsNotZeroEarlyAdapter
            | ProbeGroup::SimdBitsNotZeroAllZeroAdapter
            | ProbeGroup::SimdBitsNotZeroLateAdapter => DEFAULT_NOT_ZERO_WORK_ITEMS,
            ProbeGroup::SparseXorRowAdapter => SPARSE_XOR_ROW_BASE_WORK_ITEMS,
            ProbeGroup::SparseXorItemAdapter => SPARSE_XOR_ITEM_BASE_WORK_ITEMS,
            ProbeGroup::BitMatrixTransposeInPlaceAdapter
            | ProbeGroup::BitMatrixTransposeAllocatingAdapter => DEFAULT_TRANSPOSE_WORK_ITEMS,
            ProbeGroup::PauliStringMultiplyAdapter => DEFAULT_PAULI_WORK_ITEMS,
            ProbeGroup::PauliStringIterRangeAdapter => DEFAULT_PAULI_ITER_RANGE_WORK_ITEMS,
            ProbeGroup::PauliStringIterSingletonAdapter => DEFAULT_PAULI_ITER_SINGLETON_WORK_ITEMS,
            ProbeGroup::CliffordStringIdentityAdapter
            | ProbeGroup::CliffordStringNonIdentityAdapter => DEFAULT_CLIFFORD_WORK_ITEMS,
            ProbeGroup::DemParseAdapter | ProbeGroup::DemCanonicalPrintAdapter => {
                dem_model::MEDIUM_ITEMS
            }
            ProbeGroup::ProcessContract
            | ProbeGroup::AdapterProtocol
            | ProbeGroup::CircuitParseAdapter
            | ProbeGroup::CircuitCanonicalPrintAdapter => DEFAULT_PROBE_WORK_ITEMS,
        },
        NonZeroU64::get,
    )
}

fn validate_probe_work_items(group: ProbeGroup, work_items: u64) -> Result<(), ProbeError> {
    clifford_string::validate_work_items(group, work_items)?;
    pauli_iter::validate_work_items(group, work_items)?;
    dem_model::validate_work_items(group, work_items)?;
    if group == ProbeGroup::GateNameHashAdapter && !work_items.is_multiple_of(GATE_HASH_NAME_COUNT)
    {
        return Err(ProbeError::Contract(format!(
            "gate-name-hash probe work count {work_items} is not a complete sweep of {GATE_HASH_NAME_COUNT} names"
        )));
    }
    if group == ProbeGroup::SimdWordPopcountAdapter
        && !(POPCOUNT_MIN_BITS..=POPCOUNT_MAX_BITS).contains(&work_items)
    {
        return Err(ProbeError::Contract(format!(
            "simd-word-popcount probe width {work_items} is outside {POPCOUNT_MIN_BITS}..={POPCOUNT_MAX_BITS} bits"
        )));
    }
    if group == ProbeGroup::SimdWordPopcountAdapter
        && !work_items.is_multiple_of(POPCOUNT_ALIGNMENT_BITS)
    {
        return Err(ProbeError::Contract(format!(
            "simd-word-popcount probe width {work_items} is not a multiple of {POPCOUNT_ALIGNMENT_BITS} bits"
        )));
    }
    if group == ProbeGroup::SimdBitsXorAdapter
        && !(XOR_MIN_BITS..=XOR_MAX_BITS).contains(&work_items)
    {
        return Err(ProbeError::Contract(format!(
            "simd-bits-xor probe width {work_items} is outside {XOR_MIN_BITS}..={XOR_MAX_BITS} bits"
        )));
    }
    if group == ProbeGroup::SimdBitsXorAdapter && !work_items.is_multiple_of(XOR_ALIGNMENT_BITS) {
        return Err(ProbeError::Contract(format!(
            "simd-bits-xor probe width {work_items} is not a multiple of {XOR_ALIGNMENT_BITS} bits"
        )));
    }
    if is_not_zero_probe(group) && !(NOT_ZERO_MIN_BITS..=NOT_ZERO_MAX_BITS).contains(&work_items) {
        return Err(ProbeError::Contract(format!(
            "simd-bits-not-zero probe width {work_items} is outside {NOT_ZERO_MIN_BITS}..={NOT_ZERO_MAX_BITS} bits"
        )));
    }
    if group == ProbeGroup::SparseXorRowAdapter
        && (work_items == 0
            || work_items > SPARSE_XOR_ROW_MAX_WORK_ITEMS
            || !work_items.is_multiple_of(SPARSE_XOR_ROW_BASE_WORK_ITEMS))
    {
        return Err(ProbeError::Contract(format!(
            "sparse-XOR row probe work count {work_items} is not a positive complete callback through {SPARSE_XOR_ROW_MAX_WORK_ITEMS} row XORs"
        )));
    }
    if group == ProbeGroup::SparseXorItemAdapter
        && (work_items == 0
            || work_items > SPARSE_XOR_ITEM_MAX_WORK_ITEMS
            || !work_items.is_multiple_of(SPARSE_XOR_ITEM_BASE_WORK_ITEMS))
    {
        return Err(ProbeError::Contract(format!(
            "sparse-XOR item probe work count {work_items} is not a positive complete callback through {SPARSE_XOR_ITEM_MAX_WORK_ITEMS} item toggles"
        )));
    }
    if is_transpose_probe(group) {
        let dimension = work_items.isqrt();
        if dimension.saturating_mul(dimension) != work_items {
            return Err(ProbeError::Contract(format!(
                "bit-matrix transpose probe work count {work_items} is not a perfect square"
            )));
        }
        if !(TRANSPOSE_MIN_DIMENSION..=TRANSPOSE_MAX_DIMENSION).contains(&dimension) {
            return Err(ProbeError::Contract(format!(
                "bit-matrix transpose probe dimension {dimension} is outside {TRANSPOSE_MIN_DIMENSION}..={TRANSPOSE_MAX_DIMENSION}"
            )));
        }
        if !dimension.is_multiple_of(TRANSPOSE_DIMENSION_ALIGNMENT) {
            return Err(ProbeError::Contract(format!(
                "bit-matrix transpose probe dimension {dimension} is not a multiple of {TRANSPOSE_DIMENSION_ALIGNMENT}"
            )));
        }
    }
    if group == ProbeGroup::PauliStringMultiplyAdapter
        && !(PAULI_MIN_QUBITS..=PAULI_MAX_QUBITS).contains(&work_items)
    {
        return Err(ProbeError::Contract(format!(
            "Pauli multiplication probe width {work_items} is outside {PAULI_MIN_QUBITS}..={PAULI_MAX_QUBITS} qubits"
        )));
    }
    Ok(())
}

const fn is_transpose_probe(group: ProbeGroup) -> bool {
    matches!(
        group,
        ProbeGroup::BitMatrixTransposeInPlaceAdapter
            | ProbeGroup::BitMatrixTransposeAllocatingAdapter
    )
}

const fn is_not_zero_probe(group: ProbeGroup) -> bool {
    matches!(
        group,
        ProbeGroup::SimdBitsNotZeroEarlyAdapter
            | ProbeGroup::SimdBitsNotZeroAllZeroAdapter
            | ProbeGroup::SimdBitsNotZeroLateAdapter
    )
}

fn checked_process(output: ProcessResult, name: &'static str) -> Result<ProcessResult, ProbeError> {
    if output.status != Some(0) {
        return Err(ProbeError::Contract(format!(
            "{name} failed with status {:?}\nstdout:\n{}\nstderr:\n{}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    if !output.stderr.is_empty() {
        return Err(ProbeError::Contract(format!(
            "{name} emitted unexpected stderr:\n{}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    Ok(output)
}

fn probe_limits() -> ProcessLimits {
    ProcessLimits {
        stdin_bytes: 0,
        stdout_bytes: PROTOCOL_OUTPUT_LIMIT,
        stderr_bytes: 64 << 10,
        regular_file_bytes: None,
        timeout: Duration::from_secs(30),
    }
}

fn probe_environment() -> Vec<(OsString, OsString)> {
    vec![
        (OsString::from("LANG"), OsString::from("C")),
        (OsString::from("LC_ALL"), OsString::from("C")),
        (OsString::from("TZ"), OsString::from("UTC")),
    ]
}

fn display_rss(value: Option<u64>) -> String {
    value.map_or_else(|| "unobserved".to_string(), |value| value.to_string())
}

#[derive(Debug, Error)]
pub(super) enum ProbeError {
    #[error(transparent)]
    Adapter(#[from] super::adapter::AdapterError),
    #[error(transparent)]
    Git(#[from] super::git::GitError),
    #[error(transparent)]
    Worker(#[from] super::worker::WorkerError),
    #[error(transparent)]
    Process(#[from] super::process::ProcessError),
    #[error(transparent)]
    Protocol(#[from] super::protocol::ProtocolError),
    #[error(transparent)]
    Statistics(#[from] super::statistics::StatisticsError),
    #[error(transparent)]
    Invocation(#[from] super::invocation::InvocationError),
    #[error("failed to resolve the current Stab qualification worker: {0}")]
    CurrentExecutable(std::io::Error),
    #[error("Stab qualification worker identity changed during the probe")]
    WorkerIdentityChanged,
    #[error("qualification probe semantic work count overflows u64")]
    WorkOverflow,
    #[error("runtime group {0} has no source-owned adapter probe")]
    MissingRuntimeGroup(String),
    #[error("qualification probe contract failed: {0}")]
    Contract(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_limits_bound_every_protocol_stream() {
        let limits = probe_limits();
        assert_eq!(limits.stdin_bytes, 0);
        assert_eq!(limits.stdout_bytes, PROTOCOL_OUTPUT_LIMIT);
        assert!(limits.stderr_bytes > 0);
        assert!(limits.timeout > Duration::ZERO);
    }

    #[test]
    fn probe_ids_are_valid_protocol_ids() {
        assert!(ProtocolId::try_new(PROCESS_PROBE_ID).is_ok());
        assert!(ProtocolId::try_new(ADAPTER_PROBE_ID).is_ok());
        assert!(ProtocolId::try_new(CIRCUIT_CANONICAL_PRINT_PROBE_ID).is_ok());
        assert!(ProtocolId::try_new(GATE_NAME_HASH_PROBE_ID).is_ok());
        assert!(ProtocolId::try_new(SIMD_WORD_POPCOUNT_PROBE_ID).is_ok());
        assert!(ProtocolId::try_new(SIMD_BITS_XOR_PROBE_ID).is_ok());
        assert!(ProtocolId::try_new(SIMD_BITS_NOT_ZERO_EARLY_PROBE_ID).is_ok());
        assert!(ProtocolId::try_new(SIMD_BITS_NOT_ZERO_ALL_ZERO_PROBE_ID).is_ok());
        assert!(ProtocolId::try_new(SIMD_BITS_NOT_ZERO_LATE_PROBE_ID).is_ok());
        assert!(ProtocolId::try_new(SPARSE_XOR_ROW_PROBE_ID).is_ok());
        assert!(ProtocolId::try_new(SPARSE_XOR_ITEM_PROBE_ID).is_ok());
        assert!(ProtocolId::try_new(BIT_MATRIX_TRANSPOSE_IN_PLACE_PROBE_ID).is_ok());
        assert!(ProtocolId::try_new(BIT_MATRIX_TRANSPOSE_ALLOCATING_PROBE_ID).is_ok());
        assert!(ProtocolId::try_new(PAULI_STRING_MULTIPLY_PROBE_ID).is_ok());
        assert!(ProtocolId::try_new(PAULI_STRING_ITER_RANGE_PROBE_ID).is_ok());
        assert!(ProtocolId::try_new(PAULI_STRING_ITER_SINGLETON_PROBE_ID).is_ok());
    }

    #[test]
    fn canonical_print_adapter_probe_is_registered() {
        assert!(ProbeGroup::from_str("pq2-circuit-canonical-print-adapter-smoke", true).is_ok());
    }

    #[test]
    fn gate_name_hash_adapter_probe_is_registered() {
        assert!(ProbeGroup::from_str("pq2-gate-name-hash-adapter-smoke", true).is_ok());
    }

    #[test]
    fn gate_name_hash_probe_default_is_a_complete_table_sweep() {
        assert!(
            validate_probe_work_items(
                ProbeGroup::GateNameHashAdapter,
                DEFAULT_GATE_HASH_WORK_ITEMS
            )
            .is_ok()
        );
        assert!(
            validate_probe_work_items(ProbeGroup::GateNameHashAdapter, DEFAULT_PROBE_WORK_ITEMS)
                .is_err()
        );
    }

    #[test]
    fn simd_word_popcount_probe_enforces_bounded_aligned_widths() {
        assert!(ProbeGroup::from_str("pq2-simd-word-popcount-adapter-smoke", true).is_ok());
        assert!(
            validate_probe_work_items(
                ProbeGroup::SimdWordPopcountAdapter,
                DEFAULT_POPCOUNT_WORK_ITEMS
            )
            .is_ok()
        );
        assert!(validate_probe_work_items(ProbeGroup::SimdWordPopcountAdapter, 513).is_err());
        assert!(validate_probe_work_items(ProbeGroup::SimdWordPopcountAdapter, 256).is_err());
        assert!(
            validate_probe_work_items(
                ProbeGroup::SimdWordPopcountAdapter,
                POPCOUNT_MAX_BITS + POPCOUNT_ALIGNMENT_BITS
            )
            .is_err()
        );
    }

    #[test]
    fn simd_bits_xor_probe_enforces_bounded_aligned_widths() {
        assert!(ProbeGroup::from_str("pq2-simd-bits-xor-adapter-smoke", true).is_ok());
        assert!(
            validate_probe_work_items(ProbeGroup::SimdBitsXorAdapter, DEFAULT_POPCOUNT_WORK_ITEMS)
                .is_ok()
        );
        assert!(validate_probe_work_items(ProbeGroup::SimdBitsXorAdapter, 257).is_err());
        assert!(validate_probe_work_items(ProbeGroup::SimdBitsXorAdapter, 0).is_err());
        assert!(
            validate_probe_work_items(
                ProbeGroup::SimdBitsXorAdapter,
                XOR_MAX_BITS + XOR_ALIGNMENT_BITS
            )
            .is_err()
        );
    }

    #[test]
    fn simd_bits_not_zero_probes_accept_logical_widths_and_enforce_bounds() {
        for group in [
            ProbeGroup::SimdBitsNotZeroEarlyAdapter,
            ProbeGroup::SimdBitsNotZeroAllZeroAdapter,
            ProbeGroup::SimdBitsNotZeroLateAdapter,
        ] {
            assert!(validate_probe_work_items(group, DEFAULT_NOT_ZERO_WORK_ITEMS).is_ok());
            assert!(validate_probe_work_items(group, 65).is_ok());
            assert!(validate_probe_work_items(group, NOT_ZERO_MIN_BITS - 1).is_err());
            assert!(validate_probe_work_items(group, NOT_ZERO_MAX_BITS + 1).is_err());
        }
    }

    #[test]
    fn sparse_xor_probes_are_distinct_and_map_to_their_runtime_groups() {
        let row = ProbeGroup::from_str(SPARSE_XOR_ROW_PROBE_ID, true).expect("row probe");
        let item = ProbeGroup::from_str(SPARSE_XOR_ITEM_PROBE_ID, true).expect("item probe");
        assert_ne!(row, item);
        assert_eq!(
            row.runtime_group_id(),
            Some(SPARSE_XOR_ROW_RUNTIME_GROUP_ID)
        );
        assert_eq!(
            item.runtime_group_id(),
            Some(SPARSE_XOR_ITEM_RUNTIME_GROUP_ID)
        );
        assert_eq!(
            ProbeGroup::for_runtime_group(SPARSE_XOR_ROW_RUNTIME_GROUP_ID),
            Some(row)
        );
        assert_eq!(
            ProbeGroup::for_runtime_group(SPARSE_XOR_ITEM_RUNTIME_GROUP_ID),
            Some(item)
        );
    }

    #[test]
    fn sparse_xor_probes_require_complete_bounded_callbacks() {
        for work_items in [
            SPARSE_XOR_ROW_BASE_WORK_ITEMS,
            SPARSE_XOR_ROW_MAX_WORK_ITEMS,
        ] {
            assert!(validate_probe_work_items(ProbeGroup::SparseXorRowAdapter, work_items).is_ok());
        }
        for work_items in [
            0,
            SPARSE_XOR_ROW_BASE_WORK_ITEMS + 1,
            SPARSE_XOR_ROW_MAX_WORK_ITEMS + SPARSE_XOR_ROW_BASE_WORK_ITEMS,
        ] {
            assert!(
                validate_probe_work_items(ProbeGroup::SparseXorRowAdapter, work_items).is_err()
            );
        }
        for work_items in [
            SPARSE_XOR_ITEM_BASE_WORK_ITEMS,
            SPARSE_XOR_ITEM_MAX_WORK_ITEMS,
        ] {
            assert!(
                validate_probe_work_items(ProbeGroup::SparseXorItemAdapter, work_items).is_ok()
            );
        }
        for work_items in [
            0,
            SPARSE_XOR_ITEM_BASE_WORK_ITEMS + 1,
            SPARSE_XOR_ITEM_MAX_WORK_ITEMS + SPARSE_XOR_ITEM_BASE_WORK_ITEMS,
        ] {
            assert!(
                validate_probe_work_items(ProbeGroup::SparseXorItemAdapter, work_items).is_err()
            );
        }
    }

    #[test]
    fn transpose_probes_are_distinct_mapped_and_fully_bounded() {
        let in_place = ProbeGroup::from_str(BIT_MATRIX_TRANSPOSE_IN_PLACE_PROBE_ID, true)
            .expect("in-place probe");
        let allocating = ProbeGroup::from_str(BIT_MATRIX_TRANSPOSE_ALLOCATING_PROBE_ID, true)
            .expect("allocating probe");
        assert_ne!(in_place, allocating);
        assert_eq!(
            in_place.runtime_group_id(),
            Some(BIT_MATRIX_TRANSPOSE_IN_PLACE_RUNTIME_GROUP_ID)
        );
        assert_eq!(
            allocating.runtime_group_id(),
            Some(BIT_MATRIX_TRANSPOSE_ALLOCATING_RUNTIME_GROUP_ID)
        );
        for group in [in_place, allocating] {
            for work_items in [DEFAULT_TRANSPOSE_WORK_ITEMS, 4_194_304, 268_435_456] {
                assert!(validate_probe_work_items(group, work_items).is_ok());
            }
            for work_items in [65_025, 65_537, 66_049, 276_889_600] {
                assert!(validate_probe_work_items(group, work_items).is_err());
            }
        }
    }

    #[test]
    fn transpose_probe_rejects_semantic_work_overflow_before_process_setup() {
        let args = ProbeArgs {
            group: ProbeGroup::BitMatrixTransposeInPlaceAdapter,
            iterations: NonZeroU64::new(1_u64 << 48).expect("nonzero overflow iterations"),
            work_items: NonZeroU64::new(DEFAULT_TRANSPOSE_WORK_ITEMS),
            evidence_mode: ProbeEvidenceMode::Timing,
        };
        assert!(
            validate_probe_work_items(args.group, probe_work_items(&args)).is_ok(),
            "overflow regression must use an otherwise valid transpose shape"
        );
        assert!(matches!(
            expected_work_count(&args),
            Err(ProbeError::WorkOverflow)
        ));
    }

    #[test]
    fn pauli_probe_maps_to_its_runtime_group_and_enforces_public_bounds() {
        let group = ProbeGroup::from_str(PAULI_STRING_MULTIPLY_PROBE_ID, true)
            .expect("Pauli multiplication probe");
        assert_eq!(
            group.runtime_group_id(),
            Some(PAULI_STRING_MULTIPLY_RUNTIME_GROUP_ID)
        );
        assert_eq!(
            ProbeGroup::for_runtime_group(PAULI_STRING_MULTIPLY_RUNTIME_GROUP_ID),
            Some(group)
        );
        for work_items in [PAULI_MIN_QUBITS, DEFAULT_PAULI_WORK_ITEMS, PAULI_MAX_QUBITS] {
            assert!(validate_probe_work_items(group, work_items).is_ok());
        }
        for work_items in [0, PAULI_MAX_QUBITS + 1] {
            assert!(validate_probe_work_items(group, work_items).is_err());
        }
    }

    #[test]
    fn pauli_probe_rejects_semantic_work_overflow_before_process_setup() {
        let args = ProbeArgs {
            group: ProbeGroup::PauliStringMultiplyAdapter,
            iterations: NonZeroU64::new(1_u64 << 44).expect("nonzero overflow iterations"),
            work_items: NonZeroU64::new(PAULI_MAX_QUBITS),
            evidence_mode: ProbeEvidenceMode::Timing,
        };
        assert!(
            validate_probe_work_items(args.group, probe_work_items(&args)).is_ok(),
            "overflow regression must use an otherwise valid Pauli width"
        );
        assert!(matches!(
            expected_work_count(&args),
            Err(ProbeError::WorkOverflow)
        ));
    }

    #[test]
    fn pauli_iterator_probes_are_distinct_and_map_to_their_runtime_groups() {
        let range = ProbeGroup::from_str(PAULI_STRING_ITER_RANGE_PROBE_ID, true)
            .expect("Pauli iterator range probe");
        let singleton = ProbeGroup::from_str(PAULI_STRING_ITER_SINGLETON_PROBE_ID, true)
            .expect("Pauli iterator singleton probe");
        assert_ne!(range, singleton);
        assert_eq!(
            range.runtime_group_id(),
            Some(PAULI_STRING_ITER_RANGE_RUNTIME_GROUP_ID)
        );
        assert_eq!(
            singleton.runtime_group_id(),
            Some(PAULI_STRING_ITER_SINGLETON_RUNTIME_GROUP_ID)
        );
        assert_eq!(
            ProbeGroup::for_runtime_group(PAULI_STRING_ITER_RANGE_RUNTIME_GROUP_ID),
            Some(range)
        );
        assert_eq!(
            ProbeGroup::for_runtime_group(PAULI_STRING_ITER_SINGLETON_RUNTIME_GROUP_ID),
            Some(singleton)
        );
    }

    #[test]
    fn pauli_iterator_probes_require_complete_bounded_traversals() {
        for work_items in [DEFAULT_PAULI_ITER_RANGE_WORK_ITEMS, 21_604, 972_972] {
            assert!(
                validate_probe_work_items(ProbeGroup::PauliStringIterRangeAdapter, work_items)
                    .is_ok()
            );
        }
        for work_items in [0, 233, pauli_iter::RANGE_OUTPUT_CAP, 1_233_628] {
            assert!(
                validate_probe_work_items(ProbeGroup::PauliStringIterRangeAdapter, work_items)
                    .is_err()
            );
        }
        for work_items in [
            DEFAULT_PAULI_ITER_SINGLETON_WORK_ITEMS,
            96_000,
            3_000_000,
            pauli_iter::SINGLETON_OUTPUT_CAP,
        ] {
            assert!(
                validate_probe_work_items(ProbeGroup::PauliStringIterSingletonAdapter, work_items)
                    .is_ok()
            );
        }
        for work_items in [0, 3_001, pauli_iter::SINGLETON_OUTPUT_CAP + 3] {
            assert!(
                validate_probe_work_items(ProbeGroup::PauliStringIterSingletonAdapter, work_items)
                    .is_err()
            );
        }
    }

    #[test]
    fn pauli_iterator_probes_reject_semantic_work_overflow_before_process_setup() {
        for (group, work_items) in [
            (
                ProbeGroup::PauliStringIterRangeAdapter,
                DEFAULT_PAULI_ITER_RANGE_WORK_ITEMS,
            ),
            (
                ProbeGroup::PauliStringIterSingletonAdapter,
                DEFAULT_PAULI_ITER_SINGLETON_WORK_ITEMS,
            ),
        ] {
            let args = ProbeArgs {
                group,
                iterations: NonZeroU64::new(u64::MAX).expect("nonzero overflow iterations"),
                work_items: NonZeroU64::new(work_items),
                evidence_mode: ProbeEvidenceMode::Timing,
            };
            assert!(validate_probe_work_items(group, work_items).is_ok());
            assert!(matches!(
                expected_work_count(&args),
                Err(ProbeError::WorkOverflow)
            ));
        }
    }
}
