use std::collections::BTreeSet;
use std::ffi::OsString;
use std::path::Path;

use super::super::adapter::AdapterExecutable;
use super::super::process::{ProcessRequest, ProcessResult, run_bounded_process};
use super::super::protocol::{
    EvidenceMode, GitCommit, Implementation, InputDigest, ProtocolExpectation, ProtocolId,
    SemanticDigest, parse_worker_json_lines,
};
use super::super::worker::WorkerIdentity;
use super::super::worker::dem_model::{DEM_CYCLE_ITEMS, DemFamily, DemFixture, parse, serialize};
use super::{ProbeError, ProbeGroup, checked_process, probe_environment, probe_limits};
use crate::config::STIM_COMMIT;
use crate::root::RepoRoot;

pub(super) const MEDIUM_ITEMS: u64 = 4_096;
const SMALL_ITEMS: u64 = 64;
const LARGE_ITEMS: u64 = 65_536;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DemProbeKind {
    Parse,
    Serialize,
}

#[derive(Clone, Copy, Debug)]
enum DemGuardRejection {
    Zero,
    IncompleteCycle,
    WrongMeasurement,
    WorkOverflow,
}

impl DemProbeKind {
    fn workload(self) -> &'static str {
        match self {
            Self::Parse => "dem-parse",
            Self::Serialize => "dem-canonical-print",
        }
    }

    fn measurement(self) -> &'static str {
        match self {
            Self::Parse => "parse",
            Self::Serialize => "serialize",
        }
    }
}

pub(super) fn validate_work_items(group: ProbeGroup, items: u64) -> Result<(), ProbeError> {
    if kind(group).is_some()
        && (items == 0
            || items > DemFamily::FlatErrors.maximum_items()
            || !items.is_multiple_of(DEM_CYCLE_ITEMS))
    {
        return Err(ProbeError::Contract(format!(
            "DEM model probe work count {items} is not a positive complete flat-errors cycle through {} items",
            DemFamily::FlatErrors.maximum_items()
        )));
    }
    Ok(())
}

pub(super) fn append_default_family_arguments(group: ProbeGroup, arguments: &mut Vec<OsString>) {
    if kind(group).is_some() {
        arguments.extend([
            OsString::from("--input-family"),
            OsString::from(DemFamily::FlatErrors.id()),
        ]);
    }
}

pub(super) fn validate_boundaries(
    root: &RepoRoot,
    group: ProbeGroup,
    adapter: &AdapterExecutable,
    worker_program: &Path,
    worker_identity: &WorkerIdentity,
) -> Result<(), ProbeError> {
    let Some(kind) = kind(group) else {
        return Ok(());
    };
    for family in DemFamily::ALL {
        for items in [SMALL_ITEMS, MEDIUM_ITEMS, LARGE_ITEMS] {
            validate_accepted_fixture(
                root,
                kind,
                family,
                adapter,
                worker_program,
                worker_identity,
                1,
                items,
            )?;
        }
        validate_accepted_fixture(
            root,
            kind,
            family,
            adapter,
            worker_program,
            worker_identity,
            2,
            SMALL_ITEMS,
        )?;
        validate_accepted_fixture(
            root,
            kind,
            family,
            adapter,
            worker_program,
            worker_identity,
            1,
            family.maximum_items(),
        )?;
        validate_guard_rejections(root, kind, family, adapter, worker_program)?;
        validate_first_rejection(root, kind, family, adapter, worker_program)?;
    }
    Ok(())
}

#[allow(
    clippy::too_many_arguments,
    reason = "the exact worker protocol receipt is intentionally explicit"
)]
fn validate_accepted_fixture(
    root: &RepoRoot,
    kind: DemProbeKind,
    family: DemFamily,
    adapter: &AdapterExecutable,
    worker_program: &Path,
    worker_identity: &WorkerIdentity,
    iterations: u64,
    items: u64,
) -> Result<(), ProbeError> {
    let fixture = DemFixture::prepare(family, items)
        .map_err(|error| ProbeError::Contract(error.to_string()))?;
    let model = parse(1, &fixture).map_err(|error| ProbeError::Contract(error.to_string()))?;
    let canonical = serialize(1, &model);
    let output_digest = fixture
        .validate_canonical(&canonical)
        .map_err(|error| ProbeError::Contract(error.to_string()))?;
    validate_accepted(
        root,
        kind,
        family,
        adapter,
        worker_program,
        worker_identity,
        iterations,
        items,
        fixture
            .input_bytes()
            .map_err(|error| ProbeError::Contract(error.to_string()))?,
        &fixture.input_digest(),
        &output_digest,
    )
}

#[allow(
    clippy::too_many_arguments,
    reason = "the exact worker protocol receipt is intentionally explicit"
)]
fn validate_accepted(
    root: &RepoRoot,
    kind: DemProbeKind,
    family: DemFamily,
    adapter: &AdapterExecutable,
    worker_program: &Path,
    worker_identity: &WorkerIdentity,
    iterations: u64,
    items: u64,
    input_bytes: u64,
    input_digest: &str,
    output_digest: &str,
) -> Result<(), ProbeError> {
    let workload_id = ProtocolId::try_new(kind.workload())?;
    let measurement_ids = BTreeSet::from([ProtocolId::try_new(kind.measurement())?]);
    let expected_input_digest = InputDigest::try_new(input_digest)?;
    let expected_output_digest = SemanticDigest::try_new(output_digest)?;
    let stim_commit = GitCommit::try_new(STIM_COMMIT)?;
    for implementation in [Implementation::Stim, Implementation::Stab] {
        let output = checked_process(
            run_bounded_process(&request(
                root,
                implementation,
                adapter,
                worker_program,
                kind,
                family,
                kind.measurement(),
                iterations,
                items,
                false,
            ))?,
            match implementation {
                Implementation::Stim => "Stim DEM model boundary probe",
                Implementation::Stab => "Stab DEM model boundary probe",
            },
        )?;
        let rows = parse_worker_json_lines(&output.stdout)?;
        let (source_digest, build_fingerprint) = match implementation {
            Implementation::Stim => (
                adapter.source_digest.clone(),
                adapter.build_fingerprint.clone(),
            ),
            Implementation::Stab => (
                worker_identity.source_digest.clone(),
                worker_identity.build_fingerprint.clone(),
            ),
        };
        ProtocolExpectation {
            implementation,
            evidence_mode: EvidenceMode::Timing,
            workload_id: workload_id.clone(),
            measurement_ids: measurement_ids.clone(),
            iteration_count: iterations,
            expected_work_count: iterations
                .checked_mul(items)
                .ok_or(ProbeError::WorkOverflow)?,
            expected_input_bytes: input_bytes,
            expected_input_digest: expected_input_digest.clone(),
            expected_output_digest: Some(expected_output_digest.clone()),
            affinity_cpu: None,
            stim_commit: stim_commit.clone(),
            source_digest,
            build_fingerprint,
        }
        .validate(&rows)?;
    }
    Ok(())
}

fn validate_guard_rejections(
    root: &RepoRoot,
    kind: DemProbeKind,
    family: DemFamily,
    adapter: &AdapterExecutable,
    worker_program: &Path,
) -> Result<(), ProbeError> {
    for implementation in [Implementation::Stim, Implementation::Stab] {
        let mut classes = vec![
            DemGuardRejection::Zero,
            DemGuardRejection::WrongMeasurement,
            DemGuardRejection::WorkOverflow,
        ];
        if family.cycle_items() > 1 {
            classes.push(DemGuardRejection::IncompleteCycle);
        }
        for class in classes {
            let (measurement, iterations, items) = guard_request(kind, family, class);
            let output = run_bounded_process(&request(
                root,
                implementation,
                adapter,
                worker_program,
                kind,
                family,
                measurement,
                iterations,
                items,
                true,
            ))?;
            verify_guard_rejection(&output, implementation, kind, family, class)?;
        }
    }
    Ok(())
}

fn validate_first_rejection(
    root: &RepoRoot,
    kind: DemProbeKind,
    family: DemFamily,
    adapter: &AdapterExecutable,
    worker_program: &Path,
) -> Result<(), ProbeError> {
    for implementation in [Implementation::Stim, Implementation::Stab] {
        let output = run_bounded_process(&request(
            root,
            implementation,
            adapter,
            worker_program,
            kind,
            family,
            kind.measurement(),
            1,
            family.maximum_items() + 1,
            true,
        ))?;
        verify_rejection(&output, implementation, family)?;
    }
    Ok(())
}

#[allow(
    clippy::too_many_arguments,
    reason = "the worker protocol shape is explicit"
)]
fn request(
    root: &RepoRoot,
    implementation: Implementation,
    adapter: &AdapterExecutable,
    worker_program: &Path,
    kind: DemProbeKind,
    family: DemFamily,
    measurement: &str,
    iterations: u64,
    items: u64,
    start_barrier: bool,
) -> ProcessRequest {
    let mut args = Vec::with_capacity(13);
    if implementation == Implementation::Stab {
        args.push(OsString::from("qualification-worker"));
    }
    args.extend([
        OsString::from("--workload"),
        OsString::from(kind.workload()),
        OsString::from("--measurement-id"),
        OsString::from(measurement),
        OsString::from("--iterations"),
        OsString::from(iterations.to_string()),
        OsString::from("--work-items"),
        OsString::from(items.to_string()),
        OsString::from("--input-family"),
        OsString::from(family.id()),
        OsString::from("--evidence-mode"),
        OsString::from("timing"),
        OsString::from("--start-barrier"),
        OsString::from(start_barrier.to_string()),
    ]);
    ProcessRequest {
        program: match implementation {
            Implementation::Stim => adapter.path.clone(),
            Implementation::Stab => worker_program.to_path_buf(),
        },
        args,
        stdin: Vec::new(),
        working_directory: root.path.clone(),
        environment: probe_environment().into(),
        affinity_cpu: None,
        limits: probe_limits(),
    }
}

fn guard_request(
    kind: DemProbeKind,
    family: DemFamily,
    class: DemGuardRejection,
) -> (&'static str, u64, u64) {
    match class {
        DemGuardRejection::Zero => (kind.measurement(), 1, 0),
        DemGuardRejection::IncompleteCycle => (kind.measurement(), 1, family.cycle_items() + 1),
        DemGuardRejection::WrongMeasurement => ("wrong", 1, SMALL_ITEMS),
        DemGuardRejection::WorkOverflow => (kind.measurement(), u64::MAX, family.cycle_items() * 2),
    }
}

fn verify_guard_rejection(
    output: &ProcessResult,
    implementation: Implementation,
    kind: DemProbeKind,
    family: DemFamily,
    class: DemGuardRejection,
) -> Result<(), ProbeError> {
    let (expected_status, expected_stderr) = guard_expectation(implementation, kind, family, class);
    if output.status == Some(expected_status)
        && output.stdout.is_empty()
        && output.stderr == expected_stderr.as_bytes()
    {
        Ok(())
    } else {
        Err(ProbeError::Contract(format!(
            "{implementation} did not reject the {class:?} DEM guard before the start barrier; status={:?}; stdout={:?}; stderr={:?}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        )))
    }
}

fn guard_expectation(
    implementation: Implementation,
    kind: DemProbeKind,
    family: DemFamily,
    class: DemGuardRejection,
) -> (i32, String) {
    let message = match (implementation, class) {
        (Implementation::Stim, DemGuardRejection::Zero) => {
            "work-items must be positive".to_string()
        }
        (Implementation::Stab, DemGuardRejection::Zero) => {
            return (
                2,
                "error: invalid value '0' for '--work-items <WORK_ITEMS>': number would be zero for non-zero type\n\nFor more information, try '--help'.\n"
                    .to_string(),
            );
        }
        (Implementation::Stim, DemGuardRejection::IncompleteCycle) => {
            "DEM model work count is not a positive complete fixture cycle".to_string()
        }
        (Implementation::Stab, DemGuardRejection::IncompleteCycle) => {
            format!(
                "DEM model work count {} is not a positive multiple of {}",
                family.cycle_items() + 1,
                family.cycle_items()
            )
        }
        (Implementation::Stim, DemGuardRejection::WrongMeasurement) => {
            "adapter workload and measurement are not a registered pair".to_string()
        }
        (Implementation::Stab, DemGuardRejection::WrongMeasurement) => format!(
            "qualification workload {} requires measurement {}, got wrong",
            kind.workload(),
            kind.measurement()
        ),
        (_, DemGuardRejection::WorkOverflow) => {
            if implementation == Implementation::Stim {
                "adapter semantic work count overflows u64".to_string()
            } else {
                "qualification worker semantic work count overflows u64".to_string()
            }
        }
    };
    match implementation {
        Implementation::Stim => (2, format!("stim qualification adapter: {message}\n")),
        Implementation::Stab => (
            1,
            format!(
                "[stab-bench] ERROR: performance qualification validation failed:\n{message}\n"
            ),
        ),
    }
}

fn verify_rejection(
    output: &ProcessResult,
    implementation: Implementation,
    family: DemFamily,
) -> Result<(), ProbeError> {
    let (expected_status, expected) = match implementation {
        Implementation::Stim => (
            2,
            "stim qualification adapter: DEM model work count exceeds the source-owned limit\n"
                .to_string(),
        ),
        Implementation::Stab => (
            1,
            format!(
                "[stab-bench] ERROR: performance qualification validation failed:\nDEM model work count {} exceeds maximum {}\n",
                family.maximum_items() + 1,
                family.maximum_items()
            ),
        ),
    };
    if output.status == Some(expected_status)
        && output.stdout.is_empty()
        && output.stderr == expected.as_bytes()
    {
        Ok(())
    } else {
        Err(ProbeError::Contract(format!(
            "{implementation} did not reject the first unsupported {} DEM item count before the start barrier; status={:?}; stdout={:?}; stderr={:?}",
            family.id(),
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        )))
    }
}

const fn kind(group: ProbeGroup) -> Option<DemProbeKind> {
    match group {
        ProbeGroup::DemParseAdapter => Some(DemProbeKind::Parse),
        ProbeGroup::DemCanonicalPrintAdapter => Some(DemProbeKind::Serialize),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_groups_map_to_distinct_contracts() {
        assert_eq!(kind(ProbeGroup::DemParseAdapter), Some(DemProbeKind::Parse));
        assert_eq!(
            kind(ProbeGroup::DemCanonicalPrintAdapter),
            Some(DemProbeKind::Serialize)
        );
        assert_eq!(kind(ProbeGroup::CircuitParseAdapter), None);
    }

    #[test]
    fn probe_width_validation_requires_complete_bounded_cycles() {
        assert!(validate_work_items(ProbeGroup::DemParseAdapter, MEDIUM_ITEMS).is_ok());
        assert!(
            validate_work_items(
                ProbeGroup::DemCanonicalPrintAdapter,
                DemFamily::FlatErrors.maximum_items(),
            )
            .is_ok()
        );
        assert!(validate_work_items(ProbeGroup::DemParseAdapter, 0).is_err());
        assert!(validate_work_items(ProbeGroup::DemParseAdapter, 65).is_err());
        assert!(
            validate_work_items(
                ProbeGroup::DemParseAdapter,
                DemFamily::FlatErrors.maximum_items() + 1,
            )
            .is_err()
        );
    }

    #[test]
    fn probe_ids_and_semantic_work_overflow_are_bound_before_process_setup() {
        assert!(ProtocolId::try_new(super::super::DEM_PARSE_PROBE_ID).is_ok());
        assert!(ProtocolId::try_new(super::super::DEM_CANONICAL_PRINT_PROBE_ID).is_ok());
        let args = super::super::ProbeArgs {
            group: ProbeGroup::DemParseAdapter,
            iterations: std::num::NonZeroU64::new(u64::MAX).expect("positive iterations"),
            work_items: std::num::NonZeroU64::new(DEM_CYCLE_ITEMS),
            evidence_mode: super::super::ProbeEvidenceMode::Timing,
        };
        assert!(matches!(
            super::super::expected_work_count(&args),
            Err(ProbeError::WorkOverflow)
        ));
    }

    #[test]
    fn guard_rejections_require_exact_failures_before_the_barrier() {
        let output = |status, stderr: String| ProcessResult {
            status: Some(status),
            stdout: Vec::new(),
            stderr: stderr.into_bytes(),
            parent_observed_peak_rss_bytes: None,
            wall_elapsed: std::time::Duration::from_millis(1),
        };
        for implementation in [Implementation::Stim, Implementation::Stab] {
            for class in [
                DemGuardRejection::Zero,
                DemGuardRejection::IncompleteCycle,
                DemGuardRejection::WrongMeasurement,
                DemGuardRejection::WorkOverflow,
            ] {
                let (status, stderr) = guard_expectation(
                    implementation,
                    DemProbeKind::Parse,
                    DemFamily::FlatErrors,
                    class,
                );
                verify_guard_rejection(
                    &output(status, stderr),
                    implementation,
                    DemProbeKind::Parse,
                    DemFamily::FlatErrors,
                    class,
                )
                .expect("exact guard rejection");
            }
        }
        assert!(matches!(
            verify_guard_rejection(
                &output(
                    2,
                    "stim qualification adapter: start barrier must contain exactly one newline\n"
                        .to_string(),
                ),
                Implementation::Stim,
                DemProbeKind::Parse,
                DemFamily::FlatErrors,
                DemGuardRejection::IncompleteCycle,
            ),
            Err(ProbeError::Contract(_))
        ));
    }
}
