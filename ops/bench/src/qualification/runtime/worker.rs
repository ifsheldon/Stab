use std::fmt::Write as _;
use std::hint::black_box;
use std::io::Read as _;
use std::io::Write as _;
use std::num::NonZeroU64;
use std::sync::atomic::{Ordering, compiler_fence};
use std::time::Instant;

use clap::{Args, ValueEnum};
use sha2::{Digest as _, Sha256};
use thiserror::Error;

use super::protocol::{
    EvidenceMode, GitCommit, Implementation, InputDigest, PROTOCOL_SCHEMA_VERSION, ProtocolId,
    SemanticDigest, Sha256Digest, WorkerMeasurement,
};
use crate::config::STIM_COMMIT;

const WORKER_SOURCE: &[u8] = include_bytes!("worker.rs");
const DIAGNOSTIC_BUILD_FINGERPRINT: &str =
    "0000000000000000000000000000000000000000000000000000000000000000";
const MAX_CIRCUIT_PARSE_INSTRUCTIONS: u64 = 1_000_000;
const GATE_HASH_NAME_COUNT: u64 = 82;
const POPCOUNT_ALIGNMENT_BITS: u64 = 256;
const POPCOUNT_MIN_BITS: u64 = 512;
const POPCOUNT_MAX_BITS: u64 = 268_435_456;
const POPCOUNT_TOGGLE_BIT: usize = 300;
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
}

impl WorkerWorkload {
    fn id(self) -> &'static str {
        match self {
            Self::ProtocolSmoke => "protocol-smoke",
            Self::CircuitParse => "circuit-parse",
            Self::CircuitCanonicalPrint => "circuit-canonical-print",
            Self::GateNameHash => "gate-name-hash",
            Self::SimdWordPopcount => "simd-word-popcount",
        }
    }

    fn measurement_id(self) -> &'static str {
        match self {
            Self::ProtocolSmoke => "main",
            Self::CircuitParse => "parse",
            Self::CircuitCanonicalPrint => "serialize",
            Self::GateNameHash => "hash-all-names",
            Self::SimdWordPopcount => "toggle-popcount",
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
        | WorkerWorkload::SimdWordPopcount => None,
        WorkerWorkload::CircuitParse | WorkerWorkload::CircuitCanonicalPrint => {
            Some(circuit_parse_fixture(args.work_items.get())?)
        }
    };
    let mut popcount_fixture = match args.workload {
        WorkerWorkload::SimdWordPopcount => Some(popcount_fixture(args.work_items.get())?),
        WorkerWorkload::ProtocolSmoke
        | WorkerWorkload::CircuitParse
        | WorkerWorkload::CircuitCanonicalPrint
        | WorkerWorkload::GateNameHash => None,
    };
    let (input_bytes, input_digest_state) = if let Some(fixture) = &popcount_fixture {
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
        | WorkerWorkload::SimdWordPopcount => None,
    };
    let gate_hash_names = match args.workload {
        WorkerWorkload::GateNameHash => Some(gate_hash_names()?),
        WorkerWorkload::ProtocolSmoke
        | WorkerWorkload::CircuitParse
        | WorkerWorkload::CircuitCanonicalPrint
        | WorkerWorkload::SimdWordPopcount => None,
    };
    let gate_hash_sweeps = match args.workload {
        WorkerWorkload::GateNameHash => Some(gate_hash_sweeps(args.work_items.get())?),
        WorkerWorkload::ProtocolSmoke
        | WorkerWorkload::CircuitParse
        | WorkerWorkload::CircuitCanonicalPrint
        | WorkerWorkload::SimdWordPopcount => None,
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
    sha256_bytes(WORKER_SOURCE)
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

#[derive(Clone)]
struct PopcountFixture {
    bits: stab_core::BitVec,
    input_bytes: u64,
    input_digest: [u64; 4],
}

fn popcount_fixture(bit_count: u64) -> Result<PopcountFixture, WorkerError> {
    let word_count = validate_popcount_width(bit_count)?;
    let bit_count_usize =
        usize::try_from(bit_count).map_err(|_| WorkerError::PopcountWidthRange(bit_count))?;
    let mut words = Vec::new();
    words
        .try_reserve_exact(word_count)
        .map_err(WorkerError::PopcountFixtureAllocation)?;
    for index in 0..word_count {
        let index = u64::try_from(index).map_err(|_| WorkerError::PopcountWordIndexRange)?;
        words.push(splitmix64_word(index));
    }
    let input_bytes = u64::try_from(word_count)
        .ok()
        .and_then(|count| count.checked_mul(u64::BITS as u64 / 8))
        .ok_or(WorkerError::InputSizeRange)?;
    let input_digest = byte_digest_words(&words);
    Ok(PopcountFixture {
        bits: stab_core::BitVec::from_words_truncated(bit_count_usize, words),
        input_bytes,
        input_digest,
    })
}

fn validate_popcount_width(bit_count: u64) -> Result<usize, WorkerError> {
    if bit_count < POPCOUNT_MIN_BITS {
        return Err(WorkerError::PopcountWidthMinimum {
            actual: bit_count,
            minimum: POPCOUNT_MIN_BITS,
        });
    }
    if bit_count > POPCOUNT_MAX_BITS {
        return Err(WorkerError::PopcountWidthLimit {
            actual: bit_count,
            maximum: POPCOUNT_MAX_BITS,
        });
    }
    if !bit_count.is_multiple_of(POPCOUNT_ALIGNMENT_BITS) {
        return Err(WorkerError::PopcountWidthAlignment {
            actual: bit_count,
            alignment: POPCOUNT_ALIGNMENT_BITS,
        });
    }
    usize::try_from(bit_count / u64::BITS as u64)
        .map_err(|_| WorkerError::PopcountWidthRange(bit_count))
}

fn splitmix64_word(index: u64) -> u64 {
    let mut value = index.wrapping_add(0x9e37_79b9_7f4a_7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

fn simd_word_popcount(
    iterations: u64,
    fixture: &mut PopcountFixture,
    toggle_state: &mut bool,
) -> Result<u64, WorkerError> {
    let mut checksum = 0_u64;
    for _ in 0..iterations {
        compiler_fence(Ordering::SeqCst);
        *toggle_state = !*toggle_state;
        fixture.bits.set(POPCOUNT_TOGGLE_BIT, *toggle_state)?;
        let count =
            u64::try_from(fixture.bits.popcount()).map_err(|_| WorkerError::PopcountResultRange)?;
        checksum = checksum.wrapping_add(count);
    }
    Ok(checksum)
}

fn popcount_output_digest(
    checksum: u64,
    iterations: u64,
    work_items: u64,
    input_digest: [u64; 4],
    final_bit: bool,
) -> [u64; 4] {
    byte_digest_words(&[
        checksum,
        iterations,
        work_items,
        input_digest[0],
        input_digest[1],
        input_digest[2],
        input_digest[3],
        u64::from(final_bit),
    ])
}

fn byte_digest(bytes: &[u8]) -> [u64; 4] {
    byte_digest_iter(bytes.iter().copied())
}

fn byte_digest_words(words: &[u64]) -> [u64; 4] {
    byte_digest_iter(words.iter().flat_map(|word| word.to_le_bytes()))
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

fn sha256_bytes(bytes: &[u8]) -> Result<Sha256Digest, WorkerError> {
    let digest = Sha256::digest(bytes);
    Sha256Digest::try_new(hex_bytes(&digest)?).map_err(WorkerError::Protocol)
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

#[derive(Debug, Error)]
pub(super) enum WorkerError {
    #[error("qualification workers require Linux RSS and process contracts")]
    UnsupportedHost,
    #[error(transparent)]
    Protocol(#[from] super::protocol::ProtocolError),
    #[error(transparent)]
    Circuit(#[from] stab_core::CircuitError),
    #[error(transparent)]
    Bits(#[from] stab_core::BitError),
    #[error("qualification workload {workload} requires measurement {expected}, got {actual}")]
    MeasurementMismatch {
        workload: &'static str,
        expected: &'static str,
        actual: String,
    },
    #[error("circuit-parse scale has {actual} instructions, maximum {maximum}")]
    CircuitScaleLimit { actual: u64, maximum: u64 },
    #[error("circuit-parse scale {0} cannot be represented on this host")]
    CircuitScaleRange(u64),
    #[error("circuit-parse fixture capacity overflows usize")]
    CircuitFixtureOverflow,
    #[error("qualification input byte count cannot be represented as u64")]
    InputSizeRange,
    #[error("circuit-parse workload was invoked without its prepared fixture")]
    MissingCircuitFixture,
    #[error("circuit-canonical-print workload was invoked without its prepared circuit")]
    MissingCanonicalPrintCircuit,
    #[error("gate-name-hash workload was invoked without its prepared name table")]
    MissingGateHashNames,
    #[error("gate-name-hash workload was invoked without its validated sweep count")]
    MissingGateHashSweeps,
    #[error("gate-name-hash workload was invoked without its prepared table digest")]
    MissingGateHashTableDigest,
    #[error("gate-name-hash registry has {actual} names, expected {expected}")]
    GateHashNameCount { actual: usize, expected: u64 },
    #[error("gate-name-hash value {actual} for {name:?} cannot be represented as u16")]
    GateHashValueRange { name: String, actual: usize },
    #[error("gate-name-hash work count {actual} is not a complete sweep of {name_count} names")]
    GateHashPartialSweep { actual: u64, name_count: u64 },
    #[error("simd-word-popcount width {actual} bits is below the minimum {minimum}")]
    PopcountWidthMinimum { actual: u64, minimum: u64 },
    #[error("simd-word-popcount width {actual} bits exceeds the maximum {maximum}")]
    PopcountWidthLimit { actual: u64, maximum: u64 },
    #[error("simd-word-popcount width {actual} bits is not a multiple of {alignment}")]
    PopcountWidthAlignment { actual: u64, alignment: u64 },
    #[error("simd-word-popcount width {0} cannot be represented on this host")]
    PopcountWidthRange(u64),
    #[error("simd-word-popcount word index cannot be represented as u64")]
    PopcountWordIndexRange,
    #[error("simd-word-popcount fixture allocation failed: {0}")]
    PopcountFixtureAllocation(std::collections::TryReserveError),
    #[error("simd-word-popcount fixture does not contain its toggle bit")]
    MissingPopcountToggleBit,
    #[error("simd-word-popcount workload was invoked without its prepared fixture")]
    MissingPopcountFixture,
    #[error("simd-word-popcount result cannot be represented as u64")]
    PopcountResultRange,
    #[error("qualification worker semantic work count overflows u64")]
    WorkOverflow,
    #[error("failed to read the qualification start barrier: {0}")]
    ReadStartBarrier(std::io::Error),
    #[error("qualification start barrier must contain exactly one newline")]
    InvalidStartBarrier,
    #[error("qualification CPU id cannot be represented on this host")]
    AffinityCpuRange,
    #[error("failed to read qualification worker CPU affinity: {0}")]
    ReadAffinity(rustix::io::Errno),
    #[error("qualification worker affinity is {actual:?}, expected only CPU {expected}")]
    AffinityMismatch { expected: usize, actual: Vec<usize> },
    #[error("qualification worker elapsed seconds are invalid: {0}")]
    InvalidElapsed(f64),
    #[error("failed to read /proc/self/status: {0}")]
    ReadStatus(std::io::Error),
    #[error("/proc/self/status is missing {0}")]
    MissingStatusField(&'static str),
    #[error("/proc/self/status has malformed {0}")]
    MalformedStatusField(&'static str),
    #[error("qualification worker resident memory overflows u64 bytes")]
    MemoryOverflow,
    #[error("qualification worker could not encode a SHA-256 digest")]
    DigestEncoding,
    #[error("failed to serialize qualification worker output: {0}")]
    Serialize(serde_json::Error),
    #[error("failed to write qualification worker output: {0}")]
    Write(std::io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protocol_smoke_digest_is_deterministic_and_work_sensitive() {
        assert_eq!(
            protocol_smoke(1, 1),
            [
                0x656c_7d8a_03ff_449d,
                0x0c24_8bde_f4c3_140b,
                0x0225_2abf_fcd7_61d6,
                0x68e9_bc4c_63e0_059d,
            ]
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
