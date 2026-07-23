use std::ffi::OsString;

use super::super::clifford_vectors::{CliffordRequestVector, checked_file, request_for_runtime};
use super::super::process::ProcessResult;
use super::super::protocol::Implementation;
use super::super::worker::clifford_string::{
    CLIFFORD_FIXTURE_SCHEMA, CLIFFORD_GATE_COUNT, CLIFFORD_NON_IDENTITY_CYCLE, CLIFFORD_PUBLIC_CAP,
    CliffordDescriptor, CliffordWorkloadKind,
};
use super::InvocationError;

pub(in crate::qualification::runtime) const CLIFFORD_IDENTITY_GROUP_ID: &str =
    "PERFQ-M6-CLIFFORD-STRING";
pub(in crate::qualification::runtime) const CLIFFORD_NON_IDENTITY_GROUP_ID: &str =
    "PERFQ-M6-CLIFFORD-STRING-NON-IDENTITY";

pub(super) fn runtime_descriptor(
    group_id: &str,
    workload: &str,
    width: u64,
) -> Result<Option<String>, InvocationError> {
    if group_id != CLIFFORD_IDENTITY_GROUP_ID && group_id != CLIFFORD_NON_IDENTITY_GROUP_ID {
        return Ok(None);
    }
    let file = checked_file().map_err(InvocationError::CliffordVectorContract)?;
    let request = request_for_runtime(file, workload, width)
        .map_err(InvocationError::CliffordVectorContract)?;
    Ok(Some(request.descriptor_hex.clone()))
}

pub(in crate::qualification::runtime) fn clifford_arguments(
    request: &CliffordRequestVector,
) -> Vec<OsString> {
    vec![
        OsString::from("--workload"),
        OsString::from(&request.workload),
        OsString::from("--measurement-id"),
        OsString::from(&request.measurement_id),
        OsString::from("--iterations"),
        OsString::from(request.iterations.to_string()),
        OsString::from("--work-items"),
        OsString::from(request.work_items.to_string()),
        OsString::from("--input-descriptor-hex"),
        OsString::from(&request.descriptor_hex),
        OsString::from("--evidence-mode"),
        OsString::from("contract"),
        OsString::from("--start-barrier"),
        OsString::from("true"),
    ]
}

pub(in crate::qualification::runtime) fn checked_clifford_rejection(
    output: &ProcessResult,
    implementation: Implementation,
    request: &CliffordRequestVector,
) -> Result<(), InvocationError> {
    let (expected_status, expected_stderr) =
        clifford_rejection_expectation(implementation, request)?;
    if output.status != Some(expected_status)
        || !output.stdout.is_empty()
        || output.stderr != expected_stderr.as_bytes()
    {
        return Err(InvocationError::CliffordWorkRejection {
            implementation,
            case_id: request.id.clone(),
            status: output.status,
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }
    Ok(())
}

pub(super) fn clifford_rejection_expectation(
    implementation: Implementation,
    request: &CliffordRequestVector,
) -> Result<(i32, String), InvocationError> {
    let class = request.expected_rejection_class.as_deref().ok_or_else(|| {
        InvocationError::CliffordVectorContract(format!(
            "rejected Clifford request {} lacks a rejection class",
            request.id
        ))
    })?;
    if class == "malformed-descriptor-hex" {
        return Ok(match implementation {
            Implementation::Stim => (
                2,
                "stim qualification adapter: Clifford descriptor contains a non-hexadecimal character\n"
                    .to_string(),
            ),
            Implementation::Stab => (
                2,
                format!(
                    "error: invalid value '{}' for '--input-descriptor-hex <INPUT_DESCRIPTOR_HEX>': Clifford descriptor contains a non-hexadecimal character\n\nFor more information, try '--help'.\n",
                    request.descriptor_hex
                ),
            ),
        });
    }
    let kind = kind_from_workload(&request.workload)?;
    let descriptor = request
        .descriptor_hex
        .parse::<CliffordDescriptor>()
        .map_err(|error| InvocationError::CliffordVectorContract(error.to_string()))?;
    let [
        width,
        marker,
        schema,
        gate_count,
        cycle_count,
        complete_span,
        public_cap,
        reserved,
    ] = descriptor.fields();
    let message = match class {
        "width-limit" => {
            format!("Clifford-string width {width} exceeds maximum {CLIFFORD_PUBLIC_CAP}")
        }
        "zero-width" => "Clifford-string width must be positive".to_string(),
        "unknown-marker" => {
            format!("Clifford-string descriptor has unknown workload marker {marker}")
        }
        "wrong-measurement" => match implementation {
            Implementation::Stim => {
                "adapter workload and measurement are not a registered pair".to_string()
            }
            Implementation::Stab => format!(
                "qualification workload {} requires measurement {}, got {}",
                kind.workload(),
                kind.measurement(),
                request.measurement_id
            ),
        },
        "fixture-schema" => field_message("fixture schema", schema, CLIFFORD_FIXTURE_SCHEMA),
        "gate-count" => field_message("canonical gate count", gate_count, CLIFFORD_GATE_COUNT),
        "cycle-count" => field_message(
            "right-cycle count",
            cycle_count,
            match kind {
                CliffordWorkloadKind::Identity => 0,
                CliffordWorkloadKind::NonIdentity => CLIFFORD_NON_IDENTITY_CYCLE,
            },
        ),
        "cross-product-span" => field_message(
            "complete cross-product span",
            complete_span,
            match kind {
                CliffordWorkloadKind::Identity => 0,
                CliffordWorkloadKind::NonIdentity => 552,
            },
        ),
        "public-cap" => field_message("public Clifford-qubit cap", public_cap, CLIFFORD_PUBLIC_CAP),
        "reserved" => field_message("reserved field", reserved, 0),
        "work-overflow" => match implementation {
            Implementation::Stim => "adapter semantic work count overflows u64".to_string(),
            Implementation::Stab => {
                "qualification worker semantic work count overflows u64".to_string()
            }
        },
        "workload-marker-mismatch" => match implementation {
            Implementation::Stim => {
                "Clifford-string descriptor workload marker mismatch".to_string()
            }
            Implementation::Stab => format!(
                "{} does not accept Clifford-string workload marker {marker}",
                kind.workload()
            ),
        },
        "width-work-mismatch" => format!(
            "Clifford-string descriptor width {width} differs from work-items {}",
            request.work_items
        ),
        _ => {
            return Err(InvocationError::CliffordVectorContract(format!(
                "unknown Clifford rejection class {class}"
            )));
        }
    };
    Ok(match implementation {
        Implementation::Stim => (2, format!("stim qualification adapter: {message}\n")),
        Implementation::Stab => (
            1,
            format!(
                "[stab-bench] ERROR: performance qualification validation failed:\n{message}\n"
            ),
        ),
    })
}

fn kind_from_workload(workload: &str) -> Result<CliffordWorkloadKind, InvocationError> {
    if workload == CliffordWorkloadKind::Identity.workload() {
        Ok(CliffordWorkloadKind::Identity)
    } else if workload == CliffordWorkloadKind::NonIdentity.workload() {
        Ok(CliffordWorkloadKind::NonIdentity)
    } else {
        Err(InvocationError::CliffordVectorContract(format!(
            "unknown Clifford workload {workload}"
        )))
    }
}

fn field_message(name: &str, actual: u64, expected: u64) -> String {
    format!("Clifford-string descriptor {name} is {actual}, expected {expected}")
}

#[cfg(test)]
mod tests {
    use super::super::super::clifford_vectors::CliffordRequestResult;
    use super::*;

    #[test]
    fn runtime_descriptor_is_exactly_the_checked_scale_descriptor() {
        for (group, kind) in [
            (CLIFFORD_IDENTITY_GROUP_ID, CliffordWorkloadKind::Identity),
            (
                CLIFFORD_NON_IDENTITY_GROUP_ID,
                CliffordWorkloadKind::NonIdentity,
            ),
        ] {
            for width in [10_000, 100_000, 1_000_000] {
                assert_eq!(
                    runtime_descriptor(group, kind.workload(), width).expect("descriptor"),
                    Some(CliffordDescriptor::canonical(kind, width).to_string())
                );
            }
        }
    }

    #[test]
    fn rejection_expectations_cover_every_checked_rejection() {
        let file = checked_file().expect("checked vectors");
        for request in file
            .requests
            .iter()
            .filter(|request| request.result == CliffordRequestResult::Rejected)
        {
            for implementation in [Implementation::Stab, Implementation::Stim] {
                let (status, stderr) =
                    clifford_rejection_expectation(implementation, request).expect("expectation");
                assert_eq!(
                    status,
                    if implementation == Implementation::Stab
                        && request.expected_rejection_class.as_deref()
                            != Some("malformed-descriptor-hex")
                    {
                        1
                    } else {
                        2
                    }
                );
                assert!(!stderr.is_empty());
            }
        }
    }
}
