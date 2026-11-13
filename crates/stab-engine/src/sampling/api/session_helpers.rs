use rand::Rng;

use stab_records::BitPlane64Batch;

use super::{ReferenceSampleMode, SamplingExecutionError, SamplingPlanInner, SessionFrame};
use crate::detection::frame::{CircuitProgram, PAULI_FRAME_BATCH_SHOTS, PauliFrameSamplingState};
use crate::sampling::reference_run::ReferenceWorkspace;

pub(super) fn validate_session_storage(
    plan: &SamplingPlanInner,
    reference_mode: ReferenceSampleMode,
) -> Result<(), SamplingExecutionError> {
    let estimated_bytes = session_storage_bytes(plan, reference_mode);
    if !plan.memory_budget.admits(estimated_bytes) {
        return Err(SamplingExecutionError::SessionStorageLimit {
            estimated_bytes,
            limit_bytes: plan.memory_budget.bytes(),
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
    if reference_mode == ReferenceSampleMode::UseReferenceSample {
        estimated_bytes = estimated_bytes.saturating_add(measurements);
    }
    estimated_bytes.saturating_add(PauliFrameSamplingState::storage_bytes(&plan.program))
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
    let needs_snapshot = plan.uses_reference_state_snapshot();
    crate::sampling::validate_simulator_work_storage(
        plan.qubit_count,
        plan.measurement_count,
        needs_snapshot,
        plan.memory_budget,
    )?;
    let mut output = try_bool_buffer(plan.measurement_count, "reference measurement output")?;
    ReferenceWorkspace::try_new(plan.qubit_count, needs_snapshot)?.run(
        &plan.program,
        plan.reference_sample_loop_policy,
        &[],
        &mut output,
    )?;
    validate_reference_width(&output, plan.measurement_count)?;
    Ok(output)
}

pub(in crate::sampling) fn validate_reference_width(
    output: &[bool],
    measurement_count: usize,
) -> Result<(), SamplingExecutionError> {
    if output.len() == measurement_count {
        Ok(())
    } else {
        Err(SamplingExecutionError::InternalInvariant {
            message: format!(
                "reference simulation produced {} measurements but {measurement_count} were compiled",
                output.len()
            ),
        })
    }
}

/// The generator-list reference path, iterating every repeat, kept as a differential baseline.
#[cfg(test)]
pub(in crate::sampling) fn compute_stabilizer_frame_reference_sample(
    plan: &SamplingPlanInner,
) -> Result<Vec<bool>, SamplingExecutionError> {
    use rand::SeedableRng as _;
    use rand::rngs::SmallRng;

    use crate::sampling::execute::{ExecutionBuffers, ExecutionMode, execute_program};
    use crate::sampling::stabilizer_frame::StabilizerFrame;

    let mut record = try_bool_buffer(plan.measurement_count, "reference measurement record")?;
    let mut output = try_bool_buffer(plan.measurement_count, "reference measurement output")?;
    let mut rng = SmallRng::seed_from_u64(0);
    let mut frame = StabilizerFrame::try_new(plan.qubit_count).map_err(|error| {
        SamplingExecutionError::SessionStorageAllocation {
            message: error.to_string(),
        }
    })?;
    let mut correlated_error_occurred = false;
    let mut buffers = ExecutionBuffers {
        frame: &mut frame,
        record: &mut record,
        output: &mut output,
        correlated_error_occurred: &mut correlated_error_occurred,
    };
    execute_program(
        &plan.program,
        &mut buffers,
        &mut rng,
        ExecutionMode::ReferenceSample,
        &[],
    )?;
    Ok(output)
}

pub(super) fn fill_pauli_frame_batch(
    program: &CircuitProgram,
    frame: &mut SessionFrame,
    batch: &mut BitPlane64Batch,
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
    let mut output_start = 0_usize;
    while output_start < shot_count {
        if *pending_count == 0 {
            state.sample_batch(program, rng).map_err(|error| {
                SamplingExecutionError::InternalInvariant {
                    message: error.to_string(),
                }
            })?;
            *pending_start = 0;
            *pending_count = PAULI_FRAME_BATCH_SHOTS;
        }
        let take = (*pending_count).min(shot_count - output_start);
        for bit_index in 0..state.measurement_count() {
            let reference_mask = reference
                .and_then(|sample| sample.get(bit_index))
                .copied()
                .map_or(0, |bit| if bit { u64::MAX } else { 0 });
            let segment = state
                .measurement_segment(bit_index, *pending_start, take)
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
