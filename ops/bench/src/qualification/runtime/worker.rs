use std::fmt::Write as _;
use std::hint::black_box;
use std::io::Read as _;
use std::io::Write as _;
use std::num::NonZeroU64;
use std::time::Instant;

use clap::{Args, ValueEnum};
use sha2::{Digest as _, Sha256};
use thiserror::Error;

use super::protocol::{
    EvidenceMode, GitCommit, Implementation, PROTOCOL_SCHEMA_VERSION, ProtocolId, SemanticDigest,
    Sha256Digest, WorkerMeasurement,
};
use crate::config::STIM_COMMIT;

const WORKER_SOURCE: &[u8] = include_bytes!("worker.rs");
const DIAGNOSTIC_BUILD_FINGERPRINT: &str =
    "0000000000000000000000000000000000000000000000000000000000000000";
const MAX_CIRCUIT_PARSE_INSTRUCTIONS: u64 = 1_000_000;
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
}

impl WorkerWorkload {
    fn id(self) -> &'static str {
        match self {
            Self::ProtocolSmoke => "protocol-smoke",
            Self::CircuitParse => "circuit-parse",
        }
    }

    fn measurement_id(self) -> &'static str {
        match self {
            Self::ProtocolSmoke => "main",
            Self::CircuitParse => "parse",
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
        WorkerWorkload::ProtocolSmoke => None,
        WorkerWorkload::CircuitParse => Some(circuit_parse_fixture(args.work_items.get())?),
    };
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

    let started = Instant::now();
    let output = match args.workload {
        WorkerWorkload::ProtocolSmoke => WorkloadOutput::DigestState(protocol_smoke(
            args.iterations.get(),
            args.work_items.get(),
        )),
        WorkerWorkload::CircuitParse => WorkloadOutput::Circuit(circuit_parse(
            args.iterations.get(),
            circuit_fixture
                .as_deref()
                .ok_or(WorkerError::MissingCircuitFixture)?,
        )?),
    };
    let elapsed_seconds = started.elapsed().as_secs_f64();
    if elapsed_seconds <= 0.0 || !elapsed_seconds.is_finite() {
        return Err(WorkerError::InvalidElapsed(elapsed_seconds));
    }
    let peak_rss_bytes = peak_rss_bytes()?.max(current_rss_bytes()?);
    let digest = output.semantic_digest();
    let row = WorkerMeasurement {
        schema_version: PROTOCOL_SCHEMA_VERSION,
        implementation: Implementation::Stab,
        evidence_mode: args.evidence_mode.into(),
        workload_id,
        measurement_id,
        iteration_count: args.iterations.get(),
        elapsed_seconds,
        work_count,
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
}

impl WorkloadOutput {
    fn semantic_digest(self) -> String {
        match self {
            Self::DigestState(state) => semantic_digest(state),
            Self::Circuit(circuit) => {
                let canonical = circuit.to_stim_string();
                let canonical = canonical.strip_suffix('\n').unwrap_or(&canonical);
                semantic_digest(byte_digest(canonical.as_bytes()))
            }
        }
    }
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

fn byte_digest(bytes: &[u8]) -> [u64; 4] {
    let mut state = [
        0x6a09_e667_f3bc_c908_u64,
        0xbb67_ae85_84ca_a73b_u64,
        0x3c6e_f372_fe94_f82b_u64,
        0xa54f_f53a_5f1d_36f1_u64,
    ];
    for (index, byte) in bytes.iter().copied().enumerate() {
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
    #[error("circuit-parse workload was invoked without its prepared fixture")]
    MissingCircuitFixture,
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
        assert_eq!(protocol_smoke(2, 8), protocol_smoke(2, 8));
        assert_ne!(protocol_smoke(2, 8), protocol_smoke(2, 9));
        assert_ne!(protocol_smoke(2, 8), protocol_smoke(3, 8));
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
    fn circuit_parse_fixture_rejects_the_first_unsupported_scale() {
        assert!(circuit_parse_fixture(MAX_CIRCUIT_PARSE_INSTRUCTIONS).is_ok());
        assert!(matches!(
            circuit_parse_fixture(MAX_CIRCUIT_PARSE_INSTRUCTIONS + 1),
            Err(WorkerError::CircuitScaleLimit { .. })
        ));
    }
}
