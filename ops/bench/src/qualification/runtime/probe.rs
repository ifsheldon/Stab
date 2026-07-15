use std::collections::BTreeSet;
use std::ffi::OsString;
use std::num::NonZeroU64;
use std::time::Duration;

use clap::{Args, ValueEnum};
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

const ADAPTER_PROBE_ID: &str = "pq1-adapter-protocol-smoke";
const CIRCUIT_PARSE_PROBE_ID: &str = "pq2-circuit-parse-adapter-smoke";
const CIRCUIT_CANONICAL_PRINT_PROBE_ID: &str = "pq2-circuit-canonical-print-adapter-smoke";
const PROCESS_PROBE_ID: &str = "pq1-process-contract-smoke";
const PROTOCOL_OUTPUT_LIMIT: usize = 1 << 20;
const EMPTY_INPUT_DIGEST: &str = "6a09e667f3bcc908bb67ae8584caa73b3c6ef372fe94f82ba54ff53a5f1d36f1";

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

    /// Semantic work items per worker iteration.
    #[arg(long, default_value = "4096")]
    work_items: NonZeroU64,

    /// Produce timing or separately classified memory evidence.
    #[arg(long, value_enum, default_value = "timing")]
    evidence_mode: ProbeEvidenceMode,
}

pub(super) fn run(root: &RepoRoot, args: ProbeArgs) -> Result<(), ProbeError> {
    match args.group {
        ProbeGroup::ProcessContract => run_process_probe(root, args),
        ProbeGroup::AdapterProtocol
        | ProbeGroup::CircuitParseAdapter
        | ProbeGroup::CircuitCanonicalPrintAdapter => run_adapter_probe(root, args),
    }
}

fn run_process_probe(root: &RepoRoot, args: ProbeArgs) -> Result<(), ProbeError> {
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
    let expected_work_count = expected_work_count(&args)?;
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

fn run_adapter_probe(root: &RepoRoot, args: ProbeArgs) -> Result<(), ProbeError> {
    let (probe_id, workload, measurement) = match args.group {
        ProbeGroup::AdapterProtocol => (ADAPTER_PROBE_ID, "protocol-smoke", "main"),
        ProbeGroup::CircuitParseAdapter => (CIRCUIT_PARSE_PROBE_ID, "circuit-parse", "parse"),
        ProbeGroup::CircuitCanonicalPrintAdapter => (
            CIRCUIT_CANONICAL_PRINT_PROBE_ID,
            "circuit-canonical-print",
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
    let common_arguments = vec![
        OsString::from("--workload"),
        OsString::from(workload),
        OsString::from("--measurement-id"),
        OsString::from(measurement),
        OsString::from("--iterations"),
        OsString::from(args.iterations.get().to_string()),
        OsString::from("--work-items"),
        OsString::from(args.work_items.get().to_string()),
        OsString::from("--evidence-mode"),
        OsString::from(args.evidence_mode.as_str()),
    ];
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
        program: current_exe,
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
    let expected_work_count = expected_work_count(&args)?;
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
        source_digest: worker_identity.source_digest,
        build_fingerprint: worker_identity.build_fingerprint,
    }
    .validate(&stab_rows)?;

    if args.evidence_mode == ProbeEvidenceMode::Timing {
        let pairs = pair_measurements(0, PairOrder::StimThenStab, &stim_rows, &stab_rows)?;
        let pair = pairs.first().ok_or_else(|| {
            ProbeError::Contract("paired protocol probe returned no row".to_string())
        })?;
        println!(
            "[stab-bench] probe={} mode=timing work={} stim_seconds={:.9} stab_seconds={:.9} diagnostic_ratio={:.6} stim_parent_peak_rss={} stab_parent_peak_rss={}",
            probe_id,
            pair.work_count,
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
    Ok(())
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
        OsString::from(args.work_items.get().to_string()),
        OsString::from("--evidence-mode"),
        OsString::from(args.evidence_mode.as_str()),
    ]
}

fn expected_work_count(args: &ProbeArgs) -> Result<u64, ProbeError> {
    args.iterations
        .get()
        .checked_mul(args.work_items.get())
        .ok_or(ProbeError::WorkOverflow)
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
    #[error("failed to resolve the current Stab qualification worker: {0}")]
    CurrentExecutable(std::io::Error),
    #[error("Stab qualification worker identity changed during the probe")]
    WorkerIdentityChanged,
    #[error("qualification probe semantic work count overflows u64")]
    WorkOverflow,
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
    }

    #[test]
    fn canonical_print_adapter_probe_is_registered() {
        assert!(ProbeGroup::from_str("pq2-circuit-canonical-print-adapter-smoke", true).is_ok());
    }
}
