use std::fmt::Write as _;
use std::hint::black_box;
use std::io::Read as _;
use std::io::Write as _;
use std::num::NonZeroU64;
use std::sync::atomic::{Ordering, compiler_fence};
use std::time::Instant;

use super::protocol::{
    EvidenceMode, GitCommit, Implementation, InputDigest, PROTOCOL_SCHEMA_VERSION, ProtocolId,
    SemanticDigest, Sha256Digest, WorkerMeasurement,
};
use crate::config::STIM_COMMIT;
use clap::{Args, ValueEnum};
use sha2::{Digest as _, Sha256};

mod bits;
mod error;
mod not_zero;
mod sparse_xor;
mod transpose;

pub(super) use error::WorkerError;

#[cfg(test)]
use bits::{
    DENSE_XOR_ALIGNMENT_BITS, DENSE_XOR_MAX_BITS, POPCOUNT_ALIGNMENT_BITS, POPCOUNT_MAX_BITS,
};
use bits::{
    POPCOUNT_TOGGLE_BIT, dense_xor, dense_xor_fixture, dense_xor_output_digest, popcount_fixture,
    popcount_output_digest, simd_word_popcount,
};
use not_zero::{NotZeroPattern, not_zero_fixture, not_zero_output_digest, simd_bits_not_zero};
use sparse_xor::{SparseXorFixture, SparseXorKind};
use transpose::{TransposeFixture, TransposeKind};

const WORKER_SOURCES: [(&str, &[u8]); 6] = [
    ("worker.rs", include_bytes!("worker.rs")),
    ("worker/bits.rs", include_bytes!("worker/bits.rs")),
    ("worker/not_zero.rs", include_bytes!("worker/not_zero.rs")),
    (
        "worker/sparse_xor.rs",
        include_bytes!("worker/sparse_xor.rs"),
    ),
    ("worker/transpose.rs", include_bytes!("worker/transpose.rs")),
    ("worker/error.rs", include_bytes!("worker/error.rs")),
];
const DIAGNOSTIC_BUILD_FINGERPRINT: &str =
    "0000000000000000000000000000000000000000000000000000000000000000";
const MAX_CIRCUIT_PARSE_INSTRUCTIONS: u64 = 1_000_000;
const GATE_HASH_NAME_COUNT: u64 = 82;
const CIRCUIT_INSTRUCTION_CYCLE: [&str; 6] = [
    "H 0\n",
    "S 1\n",
    "CX 0 1\n",
    "M 0\n",
    "DETECTOR rec[-1]\n",
    "TICK\n",
];

#[derive(Clone, Debug)]
pub(super) struct WorkerIdentity {
    pub(super) source_digest: Sha256Digest,
    pub(super) build_fingerprint: Sha256Digest,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub(crate) enum WorkerWorkload {
    ProtocolSmoke,
    CircuitParse,
    CircuitCanonicalPrint,
    GateNameHash,
    SimdWordPopcount,
    SimdBitsXor,
    SimdBitsNotZeroEarly,
    SimdBitsNotZeroZero,
    SimdBitsNotZeroLate,
    SparseXorRow,
    SparseXorItem,
    BitMatrixTransposeInPlace,
    BitMatrixTransposeAllocating,
}

impl WorkerWorkload {
    fn id(self) -> &'static str {
        match self {
            Self::ProtocolSmoke => "protocol-smoke",
            Self::CircuitParse => "circuit-parse",
            Self::CircuitCanonicalPrint => "circuit-canonical-print",
            Self::GateNameHash => "gate-name-hash",
            Self::SimdWordPopcount => "simd-word-popcount",
            Self::SimdBitsXor => "simd-bits-xor",
            Self::SimdBitsNotZeroEarly => "simd-bits-not-zero-early",
            Self::SimdBitsNotZeroZero => "simd-bits-not-zero-zero",
            Self::SimdBitsNotZeroLate => "simd-bits-not-zero-late",
            Self::SparseXorRow => "sparse-xor-row",
            Self::SparseXorItem => "sparse-xor-item",
            Self::BitMatrixTransposeInPlace => "bit-matrix-transpose-in-place",
            Self::BitMatrixTransposeAllocating => "bit-matrix-transpose-allocating",
        }
    }

    fn measurement_id(self) -> &'static str {
        match self {
            Self::ProtocolSmoke => "main",
            Self::CircuitParse => "parse",
            Self::CircuitCanonicalPrint => "serialize",
            Self::GateNameHash => "hash-all-names",
            Self::SimdWordPopcount => "toggle-popcount",
            Self::SimdBitsXor => "xor-complete-vector",
            Self::SimdBitsNotZeroEarly | Self::SimdBitsNotZeroZero | Self::SimdBitsNotZeroLate => {
                "not-zero"
            }
            Self::SparseXorRow => "row-xor",
            Self::SparseXorItem => "xor-item",
            Self::BitMatrixTransposeInPlace => TransposeKind::InPlace.measurement(),
            Self::BitMatrixTransposeAllocating => TransposeKind::Allocating.measurement(),
        }
    }

    const fn not_zero_pattern(self) -> Option<NotZeroPattern> {
        match self {
            Self::SimdBitsNotZeroEarly => Some(NotZeroPattern::Early),
            Self::SimdBitsNotZeroZero => Some(NotZeroPattern::Zero),
            Self::SimdBitsNotZeroLate => Some(NotZeroPattern::Late),
            Self::ProtocolSmoke
            | Self::CircuitParse
            | Self::CircuitCanonicalPrint
            | Self::GateNameHash
            | Self::SimdWordPopcount
            | Self::SimdBitsXor
            | Self::SparseXorRow
            | Self::SparseXorItem
            | Self::BitMatrixTransposeInPlace
            | Self::BitMatrixTransposeAllocating => None,
        }
    }

    const fn sparse_xor_kind(self) -> Option<SparseXorKind> {
        match self {
            Self::SparseXorRow => Some(SparseXorKind::Row),
            Self::SparseXorItem => Some(SparseXorKind::Item),
            _ => None,
        }
    }

    const fn transpose_kind(self) -> Option<TransposeKind> {
        match self {
            Self::BitMatrixTransposeInPlace => Some(TransposeKind::InPlace),
            Self::BitMatrixTransposeAllocating => Some(TransposeKind::Allocating),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub(crate) enum WorkerEvidenceMode {
    Timing,
    Memory,
}

impl From<WorkerEvidenceMode> for EvidenceMode {
    fn from(value: WorkerEvidenceMode) -> Self {
        match value {
            WorkerEvidenceMode::Timing => Self::Timing,
            WorkerEvidenceMode::Memory => Self::Memory,
        }
    }
}

#[derive(Clone, Debug, Args)]
pub(crate) struct WorkerArgs {
    /// Source-owned worker workload.
    #[arg(long, value_enum)]
    workload: WorkerWorkload,

    /// Exact named measurement emitted by the workload.
    #[arg(long, default_value = "main")]
    measurement_id: String,

    /// Number of times to execute the timed workload body.
    #[arg(long)]
    iterations: NonZeroU64,

    /// Semantic work items processed by each iteration.
    #[arg(long)]
    work_items: NonZeroU64,

    /// Whether this invocation produces timing or separately instrumented memory evidence.
    #[arg(long, value_enum, default_value = "timing")]
    evidence_mode: WorkerEvidenceMode,

    /// Wait for one newline on stdin before entering the measured workload.
    #[arg(long, default_value_t = false, action = clap::ArgAction::Set)]
    start_barrier: bool,

    /// Require the process affinity mask to contain exactly this CPU.
    #[arg(long)]
    expected_cpu: Option<u32>,
}

pub(super) fn run(args: WorkerArgs) -> Result<(), WorkerError> {
    ensure_linux()?;
    if args.measurement_id != args.workload.measurement_id() {
        return Err(WorkerError::MeasurementMismatch {
            workload: args.workload.id(),
            expected: args.workload.measurement_id(),
            actual: args.measurement_id,
        });
    }
    let measurement_id = ProtocolId::try_new(args.measurement_id)?;
    let workload_id = ProtocolId::try_new(args.workload.id())?;
    let identity = current_identity()?;
    let circuit_fixture = match args.workload {
        WorkerWorkload::ProtocolSmoke
        | WorkerWorkload::GateNameHash
        | WorkerWorkload::SimdWordPopcount
        | WorkerWorkload::SimdBitsXor
        | WorkerWorkload::SimdBitsNotZeroEarly
        | WorkerWorkload::SimdBitsNotZeroZero
        | WorkerWorkload::SimdBitsNotZeroLate
        | WorkerWorkload::SparseXorRow
        | WorkerWorkload::SparseXorItem
        | WorkerWorkload::BitMatrixTransposeInPlace
        | WorkerWorkload::BitMatrixTransposeAllocating => None,
        WorkerWorkload::CircuitParse | WorkerWorkload::CircuitCanonicalPrint => {
            Some(circuit_parse_fixture(args.work_items.get())?)
        }
    };
    let mut popcount_fixture = match args.workload {
        WorkerWorkload::SimdWordPopcount => Some(popcount_fixture(args.work_items.get())?),
        WorkerWorkload::ProtocolSmoke
        | WorkerWorkload::CircuitParse
        | WorkerWorkload::CircuitCanonicalPrint
        | WorkerWorkload::GateNameHash
        | WorkerWorkload::SimdBitsXor
        | WorkerWorkload::SimdBitsNotZeroEarly
        | WorkerWorkload::SimdBitsNotZeroZero
        | WorkerWorkload::SimdBitsNotZeroLate
        | WorkerWorkload::SparseXorRow
        | WorkerWorkload::SparseXorItem
        | WorkerWorkload::BitMatrixTransposeInPlace
        | WorkerWorkload::BitMatrixTransposeAllocating => None,
    };
    let mut dense_xor_fixture = match args.workload {
        WorkerWorkload::SimdBitsXor => Some(dense_xor_fixture(args.work_items.get())?),
        WorkerWorkload::ProtocolSmoke
        | WorkerWorkload::CircuitParse
        | WorkerWorkload::CircuitCanonicalPrint
        | WorkerWorkload::GateNameHash
        | WorkerWorkload::SimdWordPopcount
        | WorkerWorkload::SimdBitsNotZeroEarly
        | WorkerWorkload::SimdBitsNotZeroZero
        | WorkerWorkload::SimdBitsNotZeroLate
        | WorkerWorkload::SparseXorRow
        | WorkerWorkload::SparseXorItem
        | WorkerWorkload::BitMatrixTransposeInPlace
        | WorkerWorkload::BitMatrixTransposeAllocating => None,
    };
    let not_zero_fixture = args
        .workload
        .not_zero_pattern()
        .map(|pattern| not_zero_fixture(args.work_items.get(), pattern))
        .transpose()?;
    let mut sparse_xor_fixture = args
        .workload
        .sparse_xor_kind()
        .map(|kind| SparseXorFixture::prepare(kind, args.work_items.get()))
        .transpose()?;
    let mut transpose_fixture = args
        .workload
        .transpose_kind()
        .map(|kind| TransposeFixture::prepare(kind, args.work_items.get()))
        .transpose()?;
    let (input_bytes, input_digest_state) = if let Some(fixture) = &popcount_fixture {
        (fixture.input_bytes, fixture.input_digest)
    } else if let Some(fixture) = &dense_xor_fixture {
        (fixture.input_bytes, fixture.input_digest)
    } else if let Some(fixture) = &not_zero_fixture {
        (fixture.input_bytes, fixture.input_digest)
    } else if let Some(fixture) = &sparse_xor_fixture {
        (fixture.input_bytes, fixture.input_digest)
    } else if let Some(fixture) = &transpose_fixture {
        (fixture.input_bytes, fixture.input_digest)
    } else {
        let input = circuit_fixture.as_deref().unwrap_or_default().as_bytes();
        (
            u64::try_from(input.len()).map_err(|_| WorkerError::InputSizeRange)?,
            byte_digest(input),
        )
    };
    let input_digest = InputDigest::try_new(semantic_digest(input_digest_state))?;
    let mut popcount_toggle_state = if let Some(fixture) = &popcount_fixture {
        Some(
            fixture
                .bits
                .get(POPCOUNT_TOGGLE_BIT)
                .ok_or(WorkerError::MissingPopcountToggleBit)?,
        )
    } else {
        None
    };
    let canonical_print_circuit = match args.workload {
        WorkerWorkload::CircuitCanonicalPrint => Some(stab_core::Circuit::from_stim_str(
            circuit_fixture
                .as_deref()
                .ok_or(WorkerError::MissingCircuitFixture)?,
        )?),
        WorkerWorkload::ProtocolSmoke
        | WorkerWorkload::CircuitParse
        | WorkerWorkload::GateNameHash
        | WorkerWorkload::SimdWordPopcount
        | WorkerWorkload::SimdBitsXor
        | WorkerWorkload::SimdBitsNotZeroEarly
        | WorkerWorkload::SimdBitsNotZeroZero
        | WorkerWorkload::SimdBitsNotZeroLate
        | WorkerWorkload::SparseXorRow
        | WorkerWorkload::SparseXorItem
        | WorkerWorkload::BitMatrixTransposeInPlace
        | WorkerWorkload::BitMatrixTransposeAllocating => None,
    };
    let gate_hash_names = match args.workload {
        WorkerWorkload::GateNameHash => Some(gate_hash_names()?),
        WorkerWorkload::ProtocolSmoke
        | WorkerWorkload::CircuitParse
        | WorkerWorkload::CircuitCanonicalPrint
        | WorkerWorkload::SimdWordPopcount
        | WorkerWorkload::SimdBitsXor
        | WorkerWorkload::SimdBitsNotZeroEarly
        | WorkerWorkload::SimdBitsNotZeroZero
        | WorkerWorkload::SimdBitsNotZeroLate
        | WorkerWorkload::SparseXorRow
        | WorkerWorkload::SparseXorItem
        | WorkerWorkload::BitMatrixTransposeInPlace
        | WorkerWorkload::BitMatrixTransposeAllocating => None,
    };
    let gate_hash_sweeps = match args.workload {
        WorkerWorkload::GateNameHash => Some(gate_hash_sweeps(args.work_items.get())?),
        WorkerWorkload::ProtocolSmoke
        | WorkerWorkload::CircuitParse
        | WorkerWorkload::CircuitCanonicalPrint
        | WorkerWorkload::SimdWordPopcount
        | WorkerWorkload::SimdBitsXor
        | WorkerWorkload::SimdBitsNotZeroEarly
        | WorkerWorkload::SimdBitsNotZeroZero
        | WorkerWorkload::SimdBitsNotZeroLate
        | WorkerWorkload::SparseXorRow
        | WorkerWorkload::SparseXorItem
        | WorkerWorkload::BitMatrixTransposeInPlace
        | WorkerWorkload::BitMatrixTransposeAllocating => None,
    };
    let gate_hash_table_digest = gate_hash_names
        .as_deref()
        .map(gate_table_digest)
        .transpose()?;
    if args.start_barrier {
        wait_for_start_barrier()?;
    }
    verify_affinity(args.expected_cpu)?;
    let setup_rss_bytes = current_rss_bytes()?;
    let work_count = args
        .iterations
        .get()
        .checked_mul(args.work_items.get())
        .ok_or(WorkerError::WorkOverflow)?;

    let (output, elapsed_seconds) = match args.workload {
        WorkerWorkload::ProtocolSmoke => measure_workload(|| {
            Ok(TimedWorkloadOutput::Complete(WorkloadOutput::DigestState(
                protocol_smoke(args.iterations.get(), args.work_items.get()),
            )))
        })?,
        WorkerWorkload::CircuitParse => {
            let fixture = circuit_fixture
                .as_deref()
                .ok_or(WorkerError::MissingCircuitFixture)?;
            measure_workload(|| {
                circuit_parse(args.iterations.get(), fixture)
                    .map(WorkloadOutput::Circuit)
                    .map(TimedWorkloadOutput::Complete)
            })?
        }
        WorkerWorkload::CircuitCanonicalPrint => {
            let circuit = canonical_print_circuit
                .as_ref()
                .ok_or(WorkerError::MissingCanonicalPrintCircuit)?;
            measure_workload(|| {
                Ok(TimedWorkloadOutput::Complete(
                    WorkloadOutput::CanonicalCircuitText(circuit_canonical_print(
                        args.iterations.get(),
                        circuit,
                    )),
                ))
            })?
        }
        WorkerWorkload::GateNameHash => {
            let sweeps = gate_hash_sweeps.ok_or(WorkerError::MissingGateHashSweeps)?;
            let names = gate_hash_names
                .as_deref()
                .ok_or(WorkerError::MissingGateHashNames)?;
            let table_digest =
                gate_hash_table_digest.ok_or(WorkerError::MissingGateHashTableDigest)?;
            measure_workload(|| {
                Ok(TimedWorkloadOutput::Complete(WorkloadOutput::DigestState(
                    gate_name_hash(
                        args.iterations.get(),
                        args.work_items.get(),
                        sweeps,
                        names,
                        table_digest,
                    ),
                )))
            })?
        }
        WorkerWorkload::SimdWordPopcount => {
            let fixture = popcount_fixture
                .as_mut()
                .ok_or(WorkerError::MissingPopcountFixture)?;
            let toggle_state = popcount_toggle_state
                .as_mut()
                .ok_or(WorkerError::MissingPopcountToggleBit)?;
            measure_workload(|| {
                simd_word_popcount(args.iterations.get(), fixture, toggle_state)
                    .map(TimedWorkloadOutput::PopcountChecksum)
            })?
        }
        WorkerWorkload::SimdBitsXor => {
            let fixture = dense_xor_fixture
                .as_mut()
                .ok_or(WorkerError::MissingDenseXorFixture)?;
            measure_workload(|| {
                dense_xor(args.iterations.get(), fixture)
                    .map(|()| TimedWorkloadOutput::DenseXorComplete)
            })?
        }
        WorkerWorkload::SimdBitsNotZeroEarly
        | WorkerWorkload::SimdBitsNotZeroZero
        | WorkerWorkload::SimdBitsNotZeroLate => {
            let fixture = not_zero_fixture
                .as_ref()
                .ok_or(WorkerError::MissingNotZeroFixture)?;
            measure_workload(|| {
                Ok(TimedWorkloadOutput::NotZeroChecksum(simd_bits_not_zero(
                    args.iterations.get(),
                    fixture,
                )))
            })?
        }
        WorkerWorkload::SparseXorRow | WorkerWorkload::SparseXorItem => {
            let fixture = sparse_xor_fixture
                .as_mut()
                .ok_or(WorkerError::MissingSparseXorFixture)?;
            measure_workload(|| {
                fixture.execute(args.iterations.get());
                Ok(TimedWorkloadOutput::SparseXorComplete)
            })?
        }
        WorkerWorkload::BitMatrixTransposeInPlace
        | WorkerWorkload::BitMatrixTransposeAllocating => {
            let fixture = transpose_fixture
                .as_mut()
                .ok_or(WorkerError::MissingTransposeFixture)?;
            measure_workload(|| {
                fixture.execute(args.iterations.get())?;
                Ok(TimedWorkloadOutput::TransposeComplete)
            })?
        }
    };
    if elapsed_seconds <= 0.0 || !elapsed_seconds.is_finite() {
        return Err(WorkerError::InvalidElapsed(elapsed_seconds));
    }
    let peak_rss_bytes = peak_rss_bytes()?.max(current_rss_bytes()?);
    let digest = match output {
        TimedWorkloadOutput::Complete(output) => output.semantic_digest(),
        TimedWorkloadOutput::PopcountChecksum(checksum) => {
            let fixture = popcount_fixture
                .as_ref()
                .ok_or(WorkerError::MissingPopcountFixture)?;
            let final_bit = fixture
                .bits
                .get(POPCOUNT_TOGGLE_BIT)
                .ok_or(WorkerError::MissingPopcountToggleBit)?;
            semantic_digest(popcount_output_digest(
                checksum,
                args.iterations.get(),
                args.work_items.get(),
                fixture.input_digest,
                final_bit,
            ))
        }
        TimedWorkloadOutput::DenseXorComplete => {
            let fixture = dense_xor_fixture
                .as_ref()
                .ok_or(WorkerError::MissingDenseXorFixture)?;
            semantic_digest(dense_xor_output_digest(
                fixture,
                args.iterations.get(),
                args.work_items.get(),
            ))
        }
        TimedWorkloadOutput::NotZeroChecksum(checksum) => {
            let fixture = not_zero_fixture
                .as_ref()
                .ok_or(WorkerError::MissingNotZeroFixture)?;
            semantic_digest(not_zero_output_digest(
                checksum,
                args.iterations.get(),
                args.work_items.get(),
                fixture,
            ))
        }
        TimedWorkloadOutput::SparseXorComplete => {
            let fixture = sparse_xor_fixture
                .as_ref()
                .ok_or(WorkerError::MissingSparseXorFixture)?;
            semantic_digest(fixture.output_digest(args.iterations.get(), args.work_items.get())?)
        }
        TimedWorkloadOutput::TransposeComplete => {
            let fixture = transpose_fixture
                .as_ref()
                .ok_or(WorkerError::MissingTransposeFixture)?;
            semantic_digest(fixture.output_digest(args.iterations.get(), args.work_items.get())?)
        }
    };
    let row = WorkerMeasurement {
        schema_version: PROTOCOL_SCHEMA_VERSION,
        implementation: Implementation::Stab,
        evidence_mode: args.evidence_mode.into(),
        workload_id,
        measurement_id,
        iteration_count: args.iterations.get(),
        elapsed_seconds,
        work_count,
        input_bytes,
        input_digest,
        output_digest: SemanticDigest::try_new(digest)?,
        setup_rss_bytes: Some(setup_rss_bytes),
        peak_rss_bytes: Some(peak_rss_bytes.max(setup_rss_bytes)),
        affinity_cpu: args.expected_cpu,
        stim_commit: GitCommit::try_new(STIM_COMMIT)?,
        source_digest: identity.source_digest,
        build_fingerprint: identity.build_fingerprint,
    };
    let stdout = std::io::stdout();
    let mut stdout = stdout.lock();
    serde_json::to_writer(&mut stdout, &row).map_err(WorkerError::Serialize)?;
    stdout.write_all(b"\n").map_err(WorkerError::Write)?;
    stdout.flush().map_err(WorkerError::Write)?;
    Ok(())
}

pub(super) fn current_identity() -> Result<WorkerIdentity, WorkerError> {
    Ok(WorkerIdentity {
        source_digest: source_digest()?,
        build_fingerprint: Sha256Digest::try_new(
            option_env!("STAB_PQ1_BUILD_FINGERPRINT")
                .unwrap_or(DIAGNOSTIC_BUILD_FINGERPRINT)
                .to_string(),
        )?,
    })
}

pub(super) fn source_digest() -> Result<Sha256Digest, WorkerError> {
    let mut digest = Sha256::new();
    for (path, source) in WORKER_SOURCES {
        digest.update(
            u64::try_from(path.len())
                .map_err(|_| WorkerError::InputSizeRange)?
                .to_le_bytes(),
        );
        digest.update(path.as_bytes());
        digest.update(
            u64::try_from(source.len())
                .map_err(|_| WorkerError::InputSizeRange)?
                .to_le_bytes(),
        );
        digest.update(source);
    }
    Sha256Digest::try_new(hex_bytes(&digest.finalize())?).map_err(WorkerError::Protocol)
}

fn protocol_smoke(iterations: u64, work_items: u64) -> [u64; 4] {
    let mut state = [
        0x243f_6a88_85a3_08d3_u64,
        0x1319_8a2e_0370_7344_u64,
        0xa409_3822_299f_31d0_u64,
        0x082e_fa98_ec4e_6c89_u64,
    ];
    for iteration in 0..iterations {
        for item in 0..work_items {
            let value = item
                .wrapping_mul(0x9e37_79b9_7f4a_7c15)
                .wrapping_add(iteration.rotate_left(17));
            for (lane_state, lane) in state.iter_mut().zip(0_u32..) {
                *lane_state ^= value.rotate_left(lane * 11);
                *lane_state = lane_state
                    .wrapping_mul(0x0100_0000_01b3_u64.wrapping_add(u64::from(lane) * 2))
                    .rotate_left(7 + lane);
            }
        }
    }
    black_box(state);
    state
}

enum WorkloadOutput {
    DigestState([u64; 4]),
    Circuit(stab_core::Circuit),
    CanonicalCircuitText(String),
}

enum TimedWorkloadOutput {
    Complete(WorkloadOutput),
    PopcountChecksum(u64),
    DenseXorComplete,
    NotZeroChecksum(u64),
    SparseXorComplete,
    TransposeComplete,
}

impl WorkloadOutput {
    fn semantic_digest(self) -> String {
        match self {
            Self::DigestState(state) => semantic_digest(state),
            Self::Circuit(circuit) => {
                let canonical = circuit.to_stim_string();
                canonical_circuit_digest(&canonical)
            }
            Self::CanonicalCircuitText(canonical) => canonical_circuit_digest(&canonical),
        }
    }
}

fn canonical_circuit_digest(canonical: &str) -> String {
    let canonical = canonical.strip_suffix('\n').unwrap_or(canonical);
    semantic_digest(byte_digest(canonical.as_bytes()))
}

fn measure_workload<T>(
    operation: impl FnOnce() -> Result<T, WorkerError>,
) -> Result<(T, f64), WorkerError> {
    let started = Instant::now();
    let output = operation()?;
    Ok((output, started.elapsed().as_secs_f64()))
}

fn circuit_parse_fixture(work_items: u64) -> Result<String, WorkerError> {
    if work_items > MAX_CIRCUIT_PARSE_INSTRUCTIONS {
        return Err(WorkerError::CircuitScaleLimit {
            actual: work_items,
            maximum: MAX_CIRCUIT_PARSE_INSTRUCTIONS,
        });
    }
    let instruction_count =
        usize::try_from(work_items).map_err(|_| WorkerError::CircuitScaleRange(work_items))?;
    let capacity = instruction_count
        .checked_mul(12)
        .ok_or(WorkerError::CircuitFixtureOverflow)?;
    let mut fixture = String::with_capacity(capacity);
    for instruction in CIRCUIT_INSTRUCTION_CYCLE
        .iter()
        .cycle()
        .take(instruction_count)
    {
        fixture.push_str(instruction);
    }
    Ok(fixture)
}

fn circuit_parse(iterations: u64, fixture: &str) -> Result<stab_core::Circuit, WorkerError> {
    let mut parsed = stab_core::Circuit::new();
    for _ in 0..iterations {
        parsed = stab_core::Circuit::from_stim_str(fixture)?;
    }
    Ok(parsed)
}

fn circuit_canonical_print(iterations: u64, circuit: &stab_core::Circuit) -> String {
    let mut canonical = String::new();
    for _ in 0..iterations {
        canonical = black_box(black_box(circuit).to_stim_string());
    }
    canonical
}

fn gate_hash_names() -> Result<Vec<String>, WorkerError> {
    let names = std::iter::once("NOT_A_GATE")
        .chain(stab_core::Gate::all().map(stab_core::Gate::canonical_name))
        .map(str::to_string)
        .collect::<Vec<_>>();
    if u64::try_from(names.len()) != Ok(GATE_HASH_NAME_COUNT) {
        return Err(WorkerError::GateHashNameCount {
            actual: names.len(),
            expected: GATE_HASH_NAME_COUNT,
        });
    }
    Ok(names)
}

fn gate_hash_sweeps(work_items: u64) -> Result<u64, WorkerError> {
    if !work_items.is_multiple_of(GATE_HASH_NAME_COUNT) {
        return Err(WorkerError::GateHashPartialSweep {
            actual: work_items,
            name_count: GATE_HASH_NAME_COUNT,
        });
    }
    Ok(work_items / GATE_HASH_NAME_COUNT)
}

fn gate_table_digest(names: &[String]) -> Result<u64, WorkerError> {
    let mut digest = 0xcbf2_9ce4_8422_2325_u64;
    for name in names {
        for byte in name.bytes().chain(std::iter::once(0)) {
            digest ^= u64::from(byte);
            digest = digest.wrapping_mul(0x0000_0100_0000_01b3);
        }
        let hash = stab_core::Gate::stim_name_hash(name);
        let hash = u16::try_from(hash).map_err(|_| WorkerError::GateHashValueRange {
            name: name.clone(),
            actual: hash,
        })?;
        for byte in hash.to_le_bytes() {
            digest ^= u64::from(byte);
            digest = digest.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    Ok(digest)
}

fn gate_name_hash(
    iterations: u64,
    work_items: u64,
    sweeps: u64,
    names: &[String],
    table_digest: u64,
) -> [u64; 4] {
    let mut checksum = 0_u64;
    for _ in 0..iterations {
        for _ in 0..sweeps {
            compiler_fence(Ordering::SeqCst);
            for name in names {
                checksum = checksum.wrapping_add(stab_core::Gate::stim_name_hash(name) as u64);
            }
        }
    }
    [checksum, iterations, work_items, table_digest]
}

fn byte_digest(bytes: &[u8]) -> [u64; 4] {
    byte_digest_iter(bytes.iter().copied())
}

fn byte_digest_words(words: &[u64]) -> [u64; 4] {
    byte_digest_iter(words.iter().flat_map(|word| word.to_le_bytes()))
}

fn byte_digest_word_pair(first: &[u64], second: &[u64]) -> [u64; 4] {
    byte_digest_iter(
        first
            .iter()
            .chain(second)
            .flat_map(|word| word.to_le_bytes()),
    )
}

fn byte_digest_iter(bytes: impl IntoIterator<Item = u8>) -> [u64; 4] {
    let mut state = [
        0x6a09_e667_f3bc_c908_u64,
        0xbb67_ae85_84ca_a73b_u64,
        0x3c6e_f372_fe94_f82b_u64,
        0xa54f_f53a_5f1d_36f1_u64,
    ];
    for (index, byte) in bytes.into_iter().enumerate() {
        let value =
            u64::from(byte).wrapping_add((index as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15));
        for (lane_state, lane) in state.iter_mut().zip(0_u32..) {
            *lane_state ^= value.rotate_left(lane * 13);
            *lane_state = lane_state
                .wrapping_mul(0x0100_0000_01b3_u64.wrapping_add(u64::from(lane) * 2))
                .rotate_left(9 + lane);
        }
    }
    state
}

fn semantic_digest(state: [u64; 4]) -> String {
    format!(
        "{:016x}{:016x}{:016x}{:016x}",
        state[0], state[1], state[2], state[3]
    )
}

fn hex_bytes(bytes: &[u8]) -> Result<String, WorkerError> {
    let mut output = String::with_capacity(bytes.len().saturating_mul(2));
    for byte in bytes {
        write!(&mut output, "{byte:02x}").map_err(|_| WorkerError::DigestEncoding)?;
    }
    Ok(output)
}

fn current_rss_bytes() -> Result<u64, WorkerError> {
    status_kib("VmRSS:")
}

fn peak_rss_bytes() -> Result<u64, WorkerError> {
    status_kib("VmHWM:")
}

fn wait_for_start_barrier() -> Result<(), WorkerError> {
    let stdin = std::io::stdin();
    let mut stdin = stdin.lock();
    let mut byte = [0_u8; 1];
    stdin
        .read_exact(&mut byte)
        .map_err(WorkerError::ReadStartBarrier)?;
    if byte != *b"\n" {
        return Err(WorkerError::InvalidStartBarrier);
    }
    let mut extra = [0_u8; 1];
    if stdin
        .read(&mut extra)
        .map_err(WorkerError::ReadStartBarrier)?
        != 0
    {
        return Err(WorkerError::InvalidStartBarrier);
    }
    Ok(())
}

fn verify_affinity(expected_cpu: Option<u32>) -> Result<(), WorkerError> {
    let Some(expected_cpu) = expected_cpu else {
        return Ok(());
    };
    let expected_cpu = usize::try_from(expected_cpu).map_err(|_| WorkerError::AffinityCpuRange)?;
    let set = rustix::thread::sched_getaffinity(None).map_err(WorkerError::ReadAffinity)?;
    let actual = (0..rustix::thread::CpuSet::MAX_CPU)
        .filter(|cpu| set.is_set(*cpu))
        .collect::<Vec<_>>();
    if actual == [expected_cpu] {
        Ok(())
    } else {
        Err(WorkerError::AffinityMismatch {
            expected: expected_cpu,
            actual,
        })
    }
}

fn status_kib(field: &'static str) -> Result<u64, WorkerError> {
    let status = std::fs::read_to_string("/proc/self/status").map_err(WorkerError::ReadStatus)?;
    let line = status
        .lines()
        .find(|line| line.starts_with(field))
        .ok_or(WorkerError::MissingStatusField(field))?;
    let mut fields = line.split_ascii_whitespace();
    if fields.next() != Some(field) {
        return Err(WorkerError::MalformedStatusField(field));
    }
    let kib = fields
        .next()
        .ok_or(WorkerError::MalformedStatusField(field))?
        .parse::<u64>()
        .map_err(|_| WorkerError::MalformedStatusField(field))?;
    if fields.next() != Some("kB") || fields.next().is_some() {
        return Err(WorkerError::MalformedStatusField(field));
    }
    kib.checked_mul(1024).ok_or(WorkerError::MemoryOverflow)
}

fn ensure_linux() -> Result<(), WorkerError> {
    if cfg!(target_os = "linux") {
        Ok(())
    } else {
        Err(WorkerError::UnsupportedHost)
    }
}

#[cfg(test)]
mod dense_xor_tests;

#[cfg(test)]
mod not_zero_tests;

#[cfg(test)]
mod sparse_xor_tests;

#[cfg(test)]
mod transpose_tests;

#[cfg(test)]
mod tests {
    use super::super::contract::{
        PROTOCOL_SMOKE_ITERATIONS, PROTOCOL_SMOKE_OUTPUT_LANES, PROTOCOL_SMOKE_WORK_ITEMS,
    };
    use super::*;

    #[test]
    fn protocol_smoke_digest_is_deterministic_and_work_sensitive() {
        assert_eq!(
            protocol_smoke(PROTOCOL_SMOKE_ITERATIONS, PROTOCOL_SMOKE_WORK_ITEMS),
            PROTOCOL_SMOKE_OUTPUT_LANES,
        );
        assert_eq!(protocol_smoke(2, 8), protocol_smoke(2, 8));
        assert_ne!(protocol_smoke(2, 8), protocol_smoke(2, 9));
        assert_ne!(protocol_smoke(2, 8), protocol_smoke(3, 8));
    }

    #[test]
    fn canonical_print_workload_is_registered() {
        assert!(WorkerWorkload::from_str("circuit-canonical-print", true).is_ok());
    }

    #[test]
    fn gate_name_hash_covers_complete_stim_registry_sweeps() {
        let names = gate_hash_names().expect("pinned gate names");
        let table_digest = gate_table_digest(&names).expect("pinned gate digest");
        let once = gate_name_hash(1, GATE_HASH_NAME_COUNT, 1, &names, table_digest);
        let repeated = gate_name_hash(2, GATE_HASH_NAME_COUNT, 1, &names, table_digest);
        let wider = gate_name_hash(1, GATE_HASH_NAME_COUNT * 2, 2, &names, table_digest);

        assert_ne!(table_digest, 0);
        assert_eq!(once[3], table_digest);
        assert_eq!(repeated[3], table_digest);
        assert_eq!(wider[3], table_digest);
        assert_eq!(repeated[0], once[0] * 2);
        assert_eq!(wider[0], once[0] * 2);
        assert!(matches!(
            gate_hash_sweeps(GATE_HASH_NAME_COUNT + 1),
            Err(WorkerError::GateHashPartialSweep { .. })
        ));
    }

    #[test]
    fn simd_word_popcount_fixture_binds_exact_scales() {
        let small = popcount_fixture(4_096).expect("small fixture");
        let medium = popcount_fixture(262_144).expect("medium fixture");
        let large = popcount_fixture(16_777_216).expect("large fixture");
        assert_eq!(small.input_bytes, 512);
        assert_eq!(medium.input_bytes, 32_768);
        assert_eq!(large.input_bytes, 2_097_152);
        assert_eq!(
            semantic_digest(small.input_digest),
            "101e05fc22ce0676c277e9b16363a38750079d12e0b93f3c687ed95457b79d1c"
        );
        assert_eq!(
            semantic_digest(medium.input_digest),
            "b33ad442a544ef4b367ab3b2e9a47d65676791ed7661ad7fa2529b5249bfea77"
        );
        assert_eq!(
            semantic_digest(large.input_digest),
            "b1e7afd7d73691441ea033a9eb9496d02fa12bc4d3bcf059856c089112dae368"
        );
    }

    #[test]
    fn simd_word_popcount_accumulates_and_binds_odd_and_even_final_state() {
        let mut odd = popcount_fixture(4_096).expect("odd fixture");
        let initial_count = odd.bits.popcount() as u64;
        let initial_bit = odd.bits.get(POPCOUNT_TOGGLE_BIT).expect("toggle bit");
        let expected_count = if initial_bit {
            initial_count - 1
        } else {
            initial_count + 1
        };
        let mut odd_toggle = initial_bit;
        let odd_checksum =
            simd_word_popcount(1, &mut odd, &mut odd_toggle).expect("odd popcount workload");
        let odd_final = odd.bits.get(POPCOUNT_TOGGLE_BIT).expect("odd final bit");
        assert_eq!(odd_checksum, expected_count);
        assert_eq!(odd_toggle, !initial_bit);
        assert_eq!(odd_final, odd_toggle);
        assert_eq!(
            semantic_digest(popcount_output_digest(
                odd_checksum,
                1,
                4_096,
                odd.input_digest,
                odd_final,
            )),
            "b7c42176f3f0246013376d1d65756b9b6092f0aed397cb2afefd29eba663acf9"
        );

        let mut even = popcount_fixture(4_096).expect("even fixture");
        let mut even_toggle = initial_bit;
        let even_checksum =
            simd_word_popcount(2, &mut even, &mut even_toggle).expect("even popcount workload");
        let even_final = even.bits.get(POPCOUNT_TOGGLE_BIT).expect("even final bit");
        assert_eq!(even_checksum, expected_count + initial_count);
        assert_eq!(even_toggle, initial_bit);
        assert_eq!(even_final, even_toggle);
        assert_eq!(
            semantic_digest(popcount_output_digest(
                even_checksum,
                2,
                4_096,
                even.input_digest,
                even_final,
            )),
            "b29b34efb75f68c6c751edd91d96fecacef5d5032644a76bb36973ca427ea649"
        );
    }

    #[test]
    fn simd_word_popcount_constructs_and_executes_the_accepted_maximum() {
        let mut maximum = popcount_fixture(POPCOUNT_MAX_BITS).expect("maximum fixture");
        assert_eq!(maximum.input_bytes, POPCOUNT_MAX_BITS / 8);
        assert_eq!(
            semantic_digest(maximum.input_digest),
            "cf5061f39d456d884fbdbcebfc53e04c47c29c872830a6a424f55d2e1e3d8ab4"
        );
        let initial_bit = maximum
            .bits
            .get(POPCOUNT_TOGGLE_BIT)
            .expect("maximum toggle bit");
        let mut toggle_state = initial_bit;
        let checksum = simd_word_popcount(1, &mut maximum, &mut toggle_state)
            .expect("maximum popcount workload");
        assert!(checksum > 0);
        assert_eq!(toggle_state, !initial_bit);
        assert_eq!(maximum.bits.get(POPCOUNT_TOGGLE_BIT), Some(toggle_state));
        assert_eq!(
            semantic_digest(popcount_output_digest(
                checksum,
                1,
                POPCOUNT_MAX_BITS,
                maximum.input_digest,
                toggle_state,
            )),
            "72b158a2870c2bca123553e5aca970f39107a3c7448bdbdda1512a9bcdfa33aa"
        );
    }

    #[test]
    fn simd_word_popcount_fixture_rejects_invalid_widths_before_allocation() {
        assert!(matches!(
            popcount_fixture(256),
            Err(WorkerError::PopcountWidthMinimum { .. })
        ));
        assert!(matches!(
            popcount_fixture(513),
            Err(WorkerError::PopcountWidthAlignment { .. })
        ));
        assert!(matches!(
            popcount_fixture(POPCOUNT_MAX_BITS + POPCOUNT_ALIGNMENT_BITS),
            Err(WorkerError::PopcountWidthLimit { .. })
        ));
    }

    #[test]
    fn circuit_parse_fixture_and_digest_are_work_sensitive() {
        let small = circuit_parse_fixture(64).expect("small fixture");
        let larger = circuit_parse_fixture(65).expect("larger fixture");
        assert_eq!(small.lines().count(), 64);
        assert_eq!(larger.lines().count(), 65);
        let small = circuit_parse(1, &small).expect("parse small fixture");
        let larger = circuit_parse(1, &larger).expect("parse larger fixture");
        let small = WorkloadOutput::Circuit(small).semantic_digest();
        let larger = WorkloadOutput::Circuit(larger).semantic_digest();
        assert_eq!(small.len(), 64);
        assert_ne!(small, larger);
    }

    #[test]
    fn canonical_print_matches_the_parsed_circuit_digest() {
        let fixture = circuit_parse_fixture(64).expect("fixture");
        let circuit = circuit_parse(1, &fixture).expect("parse fixture");
        let printed = circuit_canonical_print(2, &circuit);

        assert_eq!(printed.lines().count(), 64);
        assert_eq!(
            WorkloadOutput::CanonicalCircuitText(printed).semantic_digest(),
            WorkloadOutput::Circuit(circuit).semantic_digest()
        );
    }

    #[test]
    fn canonically_equivalent_parse_inputs_have_distinct_input_digests() {
        let canonical = circuit_parse_fixture(64).expect("fixture");
        let whitespace_variant = canonical.replacen("H 0\n", "H  0\n", 1);
        let canonical_output =
            WorkloadOutput::Circuit(circuit_parse(1, &canonical).expect("canonical parse"))
                .semantic_digest();
        let variant_output =
            WorkloadOutput::Circuit(circuit_parse(1, &whitespace_variant).expect("variant parse"))
                .semantic_digest();

        assert_eq!(canonical_output, variant_output);
        assert_ne!(
            semantic_digest(byte_digest(canonical.as_bytes())),
            semantic_digest(byte_digest(whitespace_variant.as_bytes()))
        );
    }

    #[test]
    fn circuit_parse_fixture_rejects_the_first_unsupported_scale() {
        assert!(circuit_parse_fixture(MAX_CIRCUIT_PARSE_INSTRUCTIONS).is_ok());
        assert!(matches!(
            circuit_parse_fixture(MAX_CIRCUIT_PARSE_INSTRUCTIONS + 1),
            Err(WorkerError::CircuitScaleLimit { .. })
        ));
    }
}
