//! Admission and validation for measurement-rich flow reversal: projected size, supported
//! instructions, flow terms the reversal cannot map, and the final check of the reversed flows.

use std::collections::BTreeMap;

use stab_algebra::Flow;
use stab_model::advanced::FoldedCircuitEvent;
use stab_model::{
    Cancellation, Circuit, CircuitInstruction, CircuitItem, FoldedCount, GateKind, MemoryBudget,
    ParseLimits, Target,
};

use super::{
    absolute_measurement_index, any_nested_instruction, instruction_text, invalid_flow_request,
};
use crate::circuit_flow::transitions::{ReverseFlowTransition, reverse_flow_transition};
use crate::circuit_flow::{
    UnsignedFlowCheckOptions, check_unsigned_flows_with_sparse_tracker, stops_flow_check,
};
use crate::controlled_pauli::{ControlSlot, controlled_pauli_form};
use crate::{AnalysisError, AnalysisOperation, AnalysisResult, ResourceKind, ResourceLimitError};

/// Measurement-rich reversal expands every repeat into the inverted circuit, its detector and
/// coordinate maps, and the finished copy.
const MEASUREMENT_RICH_RETAINED_COPIES: u64 = 3;

pub(super) fn validate_general_reversal(
    circuit: &Circuit,
    budget: MemoryBudget,
) -> AnalysisResult<()> {
    let projected = projected_reversal_bytes(circuit)?
        .checked_mul(MEASUREMENT_RICH_RETAINED_COPIES)
        .ok_or_else(|| {
            AnalysisError::count_overflow(
                AnalysisOperation::TimeReversal,
                "measurement-rich repeat expansion size overflowed",
            )
        })?;
    if !budget.admits(u128::from(projected)) {
        return Err(ResourceLimitError::memory_budget(
            AnalysisOperation::TimeReversal,
            ResourceKind::MaterializedBytes,
            projected,
            budget.bytes(),
        )
        .into());
    }
    validate_instructions(circuit)
}

/// Conservative retained bytes of one expanded copy of `circuit`, with checked arithmetic.
fn projected_reversal_bytes(circuit: &Circuit) -> AnalysisResult<u64> {
    let overflow = || {
        AnalysisError::count_overflow(
            AnalysisOperation::TimeReversal,
            "measurement-rich repeat expansion size overflowed",
        )
    };
    let mut total = 0_u64;
    crate::folded_walk::walk_folded(
        circuit,
        ParseLimits::DEFAULT_REPEAT_NESTING,
        &Cancellation::never(),
        |depth, limit| {
            AnalysisError::unsupported(
                AnalysisOperation::TimeReversal,
                None,
                format!(
                    "measurement-rich repeat expansion nests repeat blocks {depth} deep, beyond the {limit}-level envelope"
                ),
            )
        },
        |event| {
            let FoldedCircuitEvent::Instruction {
                instruction,
                executions,
            } = event
            else {
                return Ok(());
            };
            let bytes = (size_of::<CircuitItem>() as u64)
                .checked_add(
                    (instruction.targets().len() as u64)
                        .checked_mul(size_of::<Target>() as u64)
                        .ok_or_else(overflow)?,
                )
                .and_then(|bytes| bytes.checked_add(instruction.args().len() as u64 * 8))
                .ok_or_else(overflow)?;
            let expanded = FoldedCount::new(bytes)
                .times(executions)
                .exact()
                .ok_or_else(overflow)?;
            total = total.checked_add(expanded).ok_or_else(overflow)?;
            Ok(())
        },
    )?;
    Ok(total)
}

fn validate_instructions(circuit: &Circuit) -> AnalysisResult<()> {
    let mut failure = None;
    any_nested_instruction(circuit, |instruction| {
        failure = validate_instruction(instruction).err();
        failure.is_some()
    });
    failure.map_or(Ok(()), Err)
}

fn validate_instruction(instruction: &CircuitInstruction) -> AnalysisResult<()> {
    if instruction.args().iter().any(|arg| !arg.is_finite()) {
        return Err(AnalysisError::unsupported(
            AnalysisOperation::TimeReversal,
            instruction.gate().kind(),
            format!(
                "time reversal requires finite instruction arguments: {}",
                instruction_text(instruction)
            ),
        ));
    }
    if matches!(
        reverse_flow_transition(instruction),
        ReverseFlowTransition::ControlledPauli(_)
    ) && instruction
        .targets()
        .iter()
        .any(Target::is_measurement_record_target)
    {
        return Err(AnalysisError::unsupported(
            AnalysisOperation::TimeReversal,
            instruction.gate().kind(),
            format!(
                "time-reversing feedback isn't supported yet; found feedback in: {}",
                instruction_text(instruction)
            ),
        ));
    }
    validate_sweep_target_order(instruction)?;
    if instruction.gate().kind() == GateKind::ELSE_CORRELATED_ERROR
        || matches!(
            reverse_flow_transition(instruction),
            ReverseFlowTransition::Unsupported
        )
    {
        return Err(AnalysisError::unsupported(
            AnalysisOperation::TimeReversal,
            instruction.gate().kind(),
            format!(
                "don't know how to time-reverse {}",
                instruction_text(instruction)
            ),
        ));
    }
    Ok(())
}

fn validate_sweep_target_order(instruction: &CircuitInstruction) -> AnalysisResult<()> {
    // A sweep bit is valid only in the gate's classical-control slot.
    let invalid_side = match controlled_pauli_form(instruction.gate()).map(|form| form.slot) {
        Some(ControlSlot::First) => SweepInvalidSide::Right,
        Some(ControlSlot::Second) => SweepInvalidSide::Left,
        Some(ControlSlot::Either) | None => return Ok(()),
    };
    for group in instruction.target_groups() {
        let [left, right] = group else {
            continue;
        };
        let target = match invalid_side {
            SweepInvalidSide::Left => left,
            SweepInvalidSide::Right => right,
        };
        if target.is_sweep_bit_target() {
            return Err(AnalysisError::unsupported(
                AnalysisOperation::TimeReversal,
                instruction.gate().kind(),
                format!(
                    "time reversal requires gate-valid sweep target ordering; {} has sweep target {target} on its qubit-only side",
                    instruction_text(instruction)
                ),
            ));
        }
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum SweepInvalidSide {
    Left,
    Right,
}

pub(super) fn reject_flow_observable_terms(flows: &[Flow]) -> AnalysisResult<()> {
    for (flow_index, flow) in flows.iter().enumerate() {
        if let Some(observable) = flow.observables().next() {
            return Err(invalid_flow_request(format!(
                "flow {flow_index} contains obs[{observable}], but time reversal does not define how observable dependencies map into returned flows"
            )));
        }
    }
    Ok(())
}

pub(super) fn reject_measurement_record_aliases(
    flow_index: usize,
    flow: &Flow,
    measurement_count: usize,
) -> AnalysisResult<()> {
    let mut resolved = BTreeMap::new();
    for measurement in flow.measurements() {
        let absolute = absolute_measurement_index(measurement, measurement_count)?;
        if let Some(previous) = resolved.insert(absolute, measurement)
            && previous != measurement
        {
            return Err(invalid_flow_request(format!(
                "flow {flow_index} contains distinct measurement terms rec[{previous}] and rec[{measurement}] that alias absolute record {absolute}; pinned Stim rejects this flow instead of XOR-cancelling the aliases"
            )));
        }
    }
    Ok(())
}

pub(super) fn validate_output_flows(
    circuit: &Circuit,
    flows: &[Flow],
    options: &UnsignedFlowCheckOptions,
) -> AnalysisResult<()> {
    let checks =
        check_unsigned_flows_with_sparse_tracker(circuit, flows, options).map_err(|error| {
            if stops_flow_check(&error) {
                error
            } else {
                AnalysisError::internal(
                    AnalysisOperation::TimeReversal,
                    format!("failed to validate reversed flows: {error}"),
                )
            }
        })?;
    for (index, (flow, satisfied)) in flows.iter().zip(checks).enumerate() {
        if !satisfied {
            return Err(AnalysisError::internal(
                AnalysisOperation::TimeReversal,
                format!("reversed flow {index} is not satisfied by the reversed circuit: {flow}"),
            ));
        }
    }
    Ok(())
}
