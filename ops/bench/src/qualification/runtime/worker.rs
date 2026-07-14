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

#[derive(Clone, Debug)]
pub(super) struct WorkerIdentity {
    pub(super) source_digest: Sha256Digest,
    pub(super) build_fingerprint: Sha256Digest,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub(crate) enum WorkerWorkload {
    ProtocolSmoke,
}

impl WorkerWorkload {
    fn id(self) -> &'static str {
        match self {
            Self::ProtocolSmoke => "protocol-smoke",
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
    let measurement_id = ProtocolId::try_new(args.measurement_id)?;
    let workload_id = ProtocolId::try_new(args.workload.id())?;
    let identity = current_identity()?;
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
    let digest = match args.workload {
        WorkerWorkload::ProtocolSmoke => {
            protocol_smoke(args.iterations.get(), args.work_items.get())
        }
    };
    let elapsed_seconds = started.elapsed().as_secs_f64();
    if elapsed_seconds <= 0.0 || !elapsed_seconds.is_finite() {
        return Err(WorkerError::InvalidElapsed(elapsed_seconds));
    }
    let peak_rss_bytes = peak_rss_bytes()?.max(current_rss_bytes()?);
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

fn protocol_smoke(iterations: u64, work_items: u64) -> String {
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
}
