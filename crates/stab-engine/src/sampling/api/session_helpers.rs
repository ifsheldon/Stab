use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng as _};

use super::{
    MAX_SAMPLING_SESSION_STORAGE_BYTES, ReferenceSampleMode, SamplingExecutionError,
    SamplingPlanInner, SamplingPlanKind, SessionBatch, SessionFrame,
};
use crate::detection::frame::{PAULI_FRAME_BATCH_SHOTS, PauliFrameSamplingPlan};
use crate::sampling::ExecutionMode;
use crate::sampling::execute::{
    ExecutionBuffers, execute_operations, execute_reference_operations,
};
use crate::sampling::operation::SampleProgram;
use crate::sampling::stabilizer_frame::{StabilizerFrame, StabilizerStateSnapshot};

pub(super) fn validate_session_storage(
    plan: &SamplingPlanInner,
    reference_mode: ReferenceSampleMode,
) -> Result<(), SamplingExecutionError> {
    let estimated_bytes = session_storage_bytes(plan, reference_mode);
    if estimated_bytes > u128::from(MAX_SAMPLING_SESSION_STORAGE_BYTES) {
        return Err(SamplingExecutionError::SessionStorageLimit {
            estimated_bytes,
            limit_bytes: MAX_SAMPLING_SESSION_STORAGE_BYTES,
        });
    }
    Ok(())
}

pub(super) fn session_storage_bytes(
    plan: &SamplingPlanInner,
    reference_mode: ReferenceSampleMode,
) -> u128 {
    if plan.measurement_count == 0 {
        return 0;
    }
    let measurements = plan.measurement_count as u128;
    let mut estimated_bytes = measurements.saturating_mul(size_of::<u64>() as u128);
    if !matches!(
        plan.kind,
        SamplingPlanKind::DirectZ(_) | SamplingPlanKind::PauliFrame(_)
    ) {
        estimated_bytes = estimated_bytes.saturating_add(measurements.saturating_mul(2));
    }
    let stores_reference = match (&plan.kind, reference_mode) {
        (SamplingPlanKind::PauliFrame(_), ReferenceSampleMode::UseReferenceSample) => true,
        (SamplingPlanKind::PauliFrame(_), ReferenceSampleMode::SkipReferenceSample) => false,
        (_, ReferenceSampleMode::SkipReferenceSample) => true,
        (_, ReferenceSampleMode::UseReferenceSample) => false,
    };
    if stores_reference {
        estimated_bytes = estimated_bytes.saturating_add(measurements);
    }
    if let SamplingPlanKind::PauliFrame(pauli) = &plan.kind {
        estimated_bytes = estimated_bytes.saturating_add(pauli.state_storage_bytes());
    } else if matches!(plan.kind, SamplingPlanKind::GeneralFrame)
        || (reference_mode == ReferenceSampleMode::SkipReferenceSample
            && !matches!(plan.kind, SamplingPlanKind::DirectZ(_)))
    {
        let qubits = plan.qubit_count as u128;
        estimated_bytes = estimated_bytes
            .saturating_add(qubits.saturating_mul(qubits).saturating_mul(4))
            .saturating_add(qubits.saturating_mul(256));
    }
    estimated_bytes
}

pub(in crate::sampling) fn try_bool_buffer(
    capacity: usize,
    label: &'static str,
) -> Result<Vec<bool>, SamplingExecutionError> {
    let mut buffer = Vec::new();
    buffer.try_reserve_exact(capacity).map_err(|error| {
        SamplingExecutionError::SessionStorageAllocation {
            message: format!("{label} capacity {capacity}: {error}"),
        }
    })?;
    Ok(buffer)
}

pub(in crate::sampling) fn compute_reference_sample(
    plan: &SamplingPlanInner,
) -> Result<Vec<bool>, SamplingExecutionError> {
    if plan.measurement_count == 0 {
        return Ok(Vec::new());
    }
    if let SamplingPlanKind::DirectZ(direct) = &plan.kind {
        let mut output = try_bool_buffer(1, "direct reference sample")?;
        output.push(direct.reference_bit());
        return Ok(output);
    }
    let needs_snapshot = plan.uses_reference_state_snapshot();
    crate::sampling::validate_general_frame_work_storage(
        plan.qubit_count,
        plan.measurement_count,
        needs_snapshot,
    )?;
    let mut rng = SmallRng::seed_from_u64(0);
    let mut frame = StabilizerFrame::try_new(plan.qubit_count).map_err(|error| {
        SamplingExecutionError::SessionStorageAllocation {
            message: error.to_string(),
        }
    })?;
    let mut snapshot = needs_snapshot
        .then(|| StabilizerStateSnapshot::try_new(plan.qubit_count))
        .transpose()
        .map_err(|error| SamplingExecutionError::SessionStorageAllocation {
            message: error.to_string(),
        })?;
    let mut record = try_bool_buffer(plan.measurement_count, "reference measurement record")?;
    let mut output = try_bool_buffer(plan.measurement_count, "reference measurement output")?;
    frame.reset_to_z_basis();
    let mut correlated_error_occurred = false;
    let mut buffers = ExecutionBuffers {
        frame: &mut frame,
        record: &mut record,
        output: &mut output,
        correlated_error_occurred: &mut correlated_error_occurred,
    };
    execute_reference_operations(
        &plan.operations,
        &mut buffers,
        &mut rng,
        &[],
        plan.reference_sample_loop_policy,
        snapshot.as_mut(),
    )?;
    Ok(output)
}

pub(super) fn fill_pauli_frame_batch(
    plan: &PauliFrameSamplingPlan,
    frame: &mut SessionFrame,
    batch: &mut SessionBatch,
    reference: Option<&[bool]>,
    rng: &mut impl Rng,
    shot_count: usize,
) -> Result<(), SamplingExecutionError> {
    let SessionFrame::Pauli {
        state,
        pending_start,
        pending_count,
    } = frame
    else {
        return Err(SamplingExecutionError::InternalInvariant {
            message: "Pauli sampling plan did not own Pauli-frame state".to_owned(),
        });
    };
    let SessionBatch::BitPlanes(batch) = batch else {
        return Err(SamplingExecutionError::InternalInvariant {
            message: "Pauli sampling plan did not own bit-plane batch storage".to_owned(),
        });
    };
    let mut output_start = 0_usize;
    while output_start < shot_count {
        if *pending_count == 0 {
            plan.sample_batch(state, rng).map_err(|error| {
                SamplingExecutionError::InternalInvariant {
                    message: error.to_string(),
                }
            })?;
            *pending_start = 0;
            *pending_count = PAULI_FRAME_BATCH_SHOTS;
        }
        let take = (*pending_count).min(shot_count - output_start);
        for bit_index in 0..plan.measurement_count() {
            let reference_mask = reference
                .and_then(|sample| sample.get(bit_index))
                .copied()
                .map_or(0, |bit| if bit { u64::MAX } else { 0 });
            let segment = plan
                .measurement_segment(state, bit_index, *pending_start, take)
                .map_err(|error| SamplingExecutionError::InternalInvariant {
                    message: error.to_string(),
                })?
                ^ (reference_mask & low_bits_mask(take));
            let existing = if output_start == 0 {
                0
            } else {
                batch
                    .plane(bit_index)
                    .map_err(|error| SamplingExecutionError::InternalInvariant {
                        message: error.to_string(),
                    })?
                    .words()
                    .first()
                    .copied()
                    .unwrap_or(0)
            };
            batch
                .copy_plane_from_word(bit_index, existing | (segment << output_start))
                .map_err(|error| SamplingExecutionError::InternalInvariant {
                    message: error.to_string(),
                })?;
        }
        *pending_start += take;
        *pending_count -= take;
        output_start += take;
    }
    Ok(())
}

const fn low_bits_mask(bit_count: usize) -> u64 {
    if bit_count >= u64::BITS as usize {
        u64::MAX
    } else if bit_count == 0 {
        0
    } else {
        (1_u64 << bit_count) - 1
    }
}

pub(super) fn sample_general_into(
    operations: &SampleProgram,
    frame: &mut StabilizerFrame,
    record: &mut Vec<bool>,
    output: &mut Vec<bool>,
    reference: Option<&[bool]>,
    rng: &mut impl Rng,
) -> Result<(), SamplingExecutionError> {
    frame.reset_to_z_basis();
    record.clear();
    output.clear();
    let mut correlated_error_occurred = false;
    let mut buffers = ExecutionBuffers {
        frame,
        record,
        output,
        correlated_error_occurred: &mut correlated_error_occurred,
    };
    execute_operations(operations, &mut buffers, rng, ExecutionMode::Sample, &[])?;
    if let Some(reference) = reference {
        for (bit, reference_bit) in output.iter_mut().zip(reference) {
            *bit ^= *reference_bit;
        }
    }
    Ok(())
}
