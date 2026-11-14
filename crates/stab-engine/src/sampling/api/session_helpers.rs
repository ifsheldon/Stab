use super::{ReferenceSampleMode, SamplingExecutionError, SamplingPlanInner};
use crate::detection::frame::FrameBatchState;
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
    let mut estimated_bytes = FrameBatchState::storage_bytes(
        plan.program.frame_qubit_count(),
        plan.measurement_count,
        0,
        0,
    );
    if reference_mode == ReferenceSampleMode::UseReferenceSample {
        estimated_bytes = estimated_bytes.saturating_add(plan.measurement_count as u128);
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
        &plan.cancellation,
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
