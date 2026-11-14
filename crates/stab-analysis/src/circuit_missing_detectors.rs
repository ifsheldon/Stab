use std::collections::{BTreeMap, BTreeSet};

use stab_algebra::Flow;
use stab_model::advanced::FoldedCircuitEvent;
use stab_model::{
    Cancellation, Circuit, CircuitInstruction, CircuitItem, GateKind, MeasureRecordOffset,
    MemoryBudget, ParseLimits, QubitId, Target,
};

use crate::folded_walk::walk_folded;
use crate::{
    AnalysisError, AnalysisResult, FlowGeneratorOptions, ResourceKind, ResourceLimitError,
    ResourceOperation, circuit_flow::flow_record_index, circuit_flow_generators_with_options,
};

mod final_repeat;

/// Above this many expanded instructions the count-independent folded final-repeat proof is
/// tried first. It selects an algorithm; it does not reject work.
const FOLDED_FINAL_REPEAT_THRESHOLD: u64 = 1_000_000;
/// Conservative retained bytes of one declared row before its targets, and per row target.
const DECLARED_ROW_BYTES: u64 = 64;
const DECLARED_ROW_TARGET_BYTES: u64 = 48;
/// Declared rows are retained twice: once as declared and once reduced.
const DECLARED_ROW_COPIES: u64 = 2;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MissingDetectorOptions {
    pub ignore_non_deterministic_measurements: bool,
    /// Budget for the retained declared rows and the projected flow rows, also passed to the
    /// flow generators.
    pub memory_budget: MemoryBudget,
    /// Signal polled while flow generators are computed over the expanded circuit.
    pub cancellation: Cancellation,
}

impl MissingDetectorOptions {
    fn flow_generator_options(&self) -> FlowGeneratorOptions {
        FlowGeneratorOptions {
            memory_budget: self.memory_budget,
            cancellation: self.cancellation.clone(),
        }
    }
}

/// Finds deterministic measurement parities not covered by detectors or record-only observables.
///
/// Detector rows and measurement invariants share one GF(2) elimination. The invariants come from
/// [`crate::circuit_flow_generators`], which is also the canonical owner for reset, measurement,
/// feedback, heralded-record, annotation, and compact-repeat semantics.
pub fn missing_detectors(
    circuit: &Circuit,
    options: MissingDetectorOptions,
) -> AnalysisResult<Circuit> {
    let qubit_count = stab_model::advanced::circuit_simulated_qubit_count(circuit);
    if declaration_scan(circuit)?.instructions > FOLDED_FINAL_REPEAT_THRESHOLD
        && let Some(output) =
            final_repeat::try_missing_detectors_folded_final_repeat(circuit, &options, qubit_count)?
    {
        return Ok(output);
    }
    if !contains_measurement_row_instruction(circuit) {
        return Ok(Circuit::new());
    }
    missing_detectors_bounded(circuit, &options, qubit_count)
}

pub(super) fn contains_measurement_row_instruction(circuit: &Circuit) -> bool {
    circuit.items().iter().any(|item| match item {
        CircuitItem::Instruction(instruction) => {
            instruction.gate().produces_measurements()
                || matches!(
                    instruction.gate().kind(),
                    GateKind::DETECTOR | GateKind::OBSERVABLE_INCLUDE
                )
        }
        CircuitItem::RepeatBlock(repeat) => contains_measurement_row_instruction(repeat.body()),
    })
}

pub(super) fn missing_detectors_bounded(
    circuit: &Circuit,
    options: &MissingDetectorOptions,
    qubit_count: usize,
) -> AnalysisResult<Circuit> {
    let measurement_count = usize::try_from(circuit.count_measurements()?).map_err(|_| {
        AnalysisError::invalid_detector_error_model(
            "measurement count does not fit usize during missing-detector analysis",
        )
    })?;
    validate_declaration_scan(circuit, options.memory_budget)?;
    if measurement_count == 0 {
        collect_declared_rows(circuit, measurement_count)?;
        return Ok(Circuit::new());
    }
    validate_missing_detector_flow_storage(qubit_count, measurement_count, options.memory_budget)?;
    let generator_circuit = circuit_with_known_input_resets(circuit, options, qubit_count)?;
    let invariants = collect_measurement_invariants(
        circuit_flow_generators_with_options(
            &generator_circuit,
            &options.flow_generator_options(),
        )?,
        measurement_count,
    )?;
    let declarations = collect_declared_rows(circuit, measurement_count)?;
    build_missing_detector_output(declarations, invariants, measurement_count)
}

pub(super) fn terminal_state_signature(
    circuit: &Circuit,
    options: &MissingDetectorOptions,
    qubit_count: usize,
) -> AnalysisResult<Option<Vec<Flow>>> {
    let measurement_count = usize::try_from(circuit.count_measurements()?).map_err(|_| {
        AnalysisError::invalid_detector_error_model(
            "measurement count does not fit usize during missing-detector state proof",
        )
    })?;
    validate_missing_detector_flow_storage(qubit_count, measurement_count, options.memory_budget)?;
    let generator_circuit = circuit_with_known_input_resets(circuit, options, qubit_count)?;
    let mut signature = Vec::new();
    for flow in
        circuit_flow_generators_with_options(&generator_circuit, &options.flow_generator_options())?
    {
        if !flow.input().has_no_pauli_terms() || flow.output().has_no_pauli_terms() {
            continue;
        }
        if flow.measurements().next().is_some() || flow.observables().next().is_some() {
            return Ok(None);
        }
        signature.push(flow);
    }
    signature.sort();
    Ok(Some(signature))
}

struct DeclaredRows {
    known: Vec<MeasurementRow>,
    original: Vec<MeasurementRow>,
}

fn collect_declared_rows(
    circuit: &Circuit,
    measurement_count: usize,
) -> AnalysisResult<DeclaredRows> {
    let mut measurement_offset = 0usize;
    let mut detector_rows = Vec::new();
    let mut logical_rows = BTreeMap::<u64, MeasurementRow>::new();
    let mut ignored_logical_rows = BTreeSet::new();

    for instruction in circuit.iter_flattened_instructions() {
        if instruction.gate().produces_measurements() {
            measurement_offset = measurement_offset
                .checked_add(instruction.target_groups().len())
                .ok_or_else(|| {
                    AnalysisError::invalid_detector_error_model(
                        "measurement offset overflowed during missing-detector analysis",
                    )
                })?;
        }

        match instruction.gate().kind() {
            GateKind::DETECTOR => detector_rows.push(row_from_record_targets(
                instruction,
                measurement_offset,
                false,
                &mut ignored_logical_rows,
            )?),
            GateKind::OBSERVABLE_INCLUDE => {
                let observable = instruction
                    .observable_id_argument()?
                    .ok_or_else(|| {
                        AnalysisError::invalid_detector_error_model(
                            "OBSERVABLE_INCLUDE is missing an observable id argument",
                        )
                    })?
                    .get();
                let delta = row_from_record_targets(
                    instruction,
                    measurement_offset,
                    true,
                    &mut ignored_logical_rows,
                )?;
                logical_rows
                    .entry(observable)
                    .or_default()
                    .xor_assign(&delta);
                if instruction.targets().iter().any(Target::is_pauli_target) {
                    ignored_logical_rows.insert(observable);
                }
            }
            GateKind::TICK
            | GateKind::QUBIT_COORDS
            | GateKind::SHIFT_COORDS
            | GateKind::REPEAT
            | GateKind::MPAD
            | GateKind::MX
            | GateKind::MY
            | GateKind::M
            | GateKind::MRX
            | GateKind::MRY
            | GateKind::MR
            | GateKind::RX
            | GateKind::RY
            | GateKind::R
            | GateKind::XCX
            | GateKind::XCY
            | GateKind::XCZ
            | GateKind::YCX
            | GateKind::YCY
            | GateKind::YCZ
            | GateKind::CX
            | GateKind::CY
            | GateKind::CZ
            | GateKind::H
            | GateKind::H_XY
            | GateKind::H_YZ
            | GateKind::H_NXY
            | GateKind::H_NXZ
            | GateKind::H_NYZ
            | GateKind::DEPOLARIZE1
            | GateKind::DEPOLARIZE2
            | GateKind::X_ERROR
            | GateKind::Y_ERROR
            | GateKind::Z_ERROR
            | GateKind::I_ERROR
            | GateKind::II_ERROR
            | GateKind::PAULI_CHANNEL_1
            | GateKind::PAULI_CHANNEL_2
            | GateKind::E
            | GateKind::ELSE_CORRELATED_ERROR
            | GateKind::HERALDED_ERASE
            | GateKind::HERALDED_PAULI_CHANNEL_1
            | GateKind::I
            | GateKind::X
            | GateKind::Y
            | GateKind::Z
            | GateKind::C_XYZ
            | GateKind::C_ZYX
            | GateKind::C_NXYZ
            | GateKind::C_XNYZ
            | GateKind::C_XYNZ
            | GateKind::C_NZYX
            | GateKind::C_ZNYX
            | GateKind::C_ZYNX
            | GateKind::SQRT_X
            | GateKind::SQRT_X_DAG
            | GateKind::SQRT_Y
            | GateKind::SQRT_Y_DAG
            | GateKind::S
            | GateKind::S_DAG
            | GateKind::II
            | GateKind::SQRT_XX
            | GateKind::SQRT_XX_DAG
            | GateKind::SQRT_YY
            | GateKind::SQRT_YY_DAG
            | GateKind::SQRT_ZZ
            | GateKind::SQRT_ZZ_DAG
            | GateKind::MPP
            | GateKind::SPP
            | GateKind::SPP_DAG
            | GateKind::SWAP
            | GateKind::ISWAP
            | GateKind::CXSWAP
            | GateKind::SWAPCX
            | GateKind::CZSWAP
            | GateKind::ISWAP_DAG
            | GateKind::MXX
            | GateKind::MYY
            | GateKind::MZZ => {}
        }
    }
    if measurement_offset != measurement_count {
        return Err(AnalysisError::invalid_detector_error_model(format!(
            "missing-detector scan counted {measurement_offset} measurements but the circuit reports {measurement_count}"
        )));
    }

    let mut known = detector_rows;
    for (observable, row) in logical_rows {
        if !ignored_logical_rows.contains(&observable) {
            known.push(row);
        }
    }
    Ok(DeclaredRows {
        original: known.clone(),
        known,
    })
}

fn validate_missing_detector_flow_storage(
    qubit_count: usize,
    measurement_count: usize,
    budget: MemoryBudget,
) -> AnalysisResult<()> {
    let projected = crate::circuit_flow::measurement_rich_flow_generator_projected_bytes(
        qubit_count,
        measurement_count,
    );
    admit_missing_detector_bytes(budget, ResourceKind::ProjectedPayloadBytes, projected)
}

fn admit_missing_detector_bytes(
    budget: MemoryBudget,
    resource: ResourceKind,
    projected: u64,
) -> AnalysisResult<()> {
    if !budget.admits(u128::from(projected)) {
        return Err(ResourceLimitError::memory_budget(
            ResourceOperation::MissingDetectorDiscovery,
            resource,
            projected,
            budget.bytes(),
        )
        .into());
    }
    Ok(())
}

/// Saturating expanded totals of one arithmetic declaration scan.
#[derive(Clone, Copy, Debug, Default)]
struct DeclarationScan {
    instructions: u64,
    row_bytes: u64,
}

fn declaration_scan(circuit: &Circuit) -> AnalysisResult<DeclarationScan> {
    let mut totals = DeclarationScan::default();
    walk_folded(
        circuit,
        ParseLimits::DEFAULT_REPEAT_NESTING,
        &Cancellation::never(),
        |depth, limit| {
            ResourceLimitError::missing_detector_discovery(
                ResourceKind::RepeatNesting,
                depth,
                limit,
            )
            .into()
        },
        |event| {
            let FoldedCircuitEvent::Instruction {
                instruction,
                executions,
            } = event
            else {
                return Ok(());
            };
            let multiplier = executions.saturating();
            totals.instructions = totals.instructions.saturating_add(multiplier);
            if matches!(
                instruction.gate().kind(),
                GateKind::DETECTOR | GateKind::OBSERVABLE_INCLUDE
            ) {
                let row = DECLARED_ROW_BYTES.saturating_add(
                    (instruction.targets().len() as u64).saturating_mul(DECLARED_ROW_TARGET_BYTES),
                );
                totals.row_bytes = totals.row_bytes.saturating_add(
                    row.saturating_mul(DECLARED_ROW_COPIES)
                        .saturating_mul(multiplier),
                );
            }
            Ok(())
        },
    )?;
    Ok(totals)
}

fn validate_declaration_scan(circuit: &Circuit, budget: MemoryBudget) -> AnalysisResult<()> {
    admit_missing_detector_bytes(
        budget,
        ResourceKind::MaterializedBytes,
        declaration_scan(circuit)?.row_bytes,
    )
}

fn row_from_record_targets(
    instruction: &CircuitInstruction,
    measurement_offset: usize,
    allow_pauli: bool,
    ignored_logical_rows: &mut BTreeSet<u64>,
) -> AnalysisResult<MeasurementRow> {
    let mut row = MeasurementRow::default();
    for target in instruction.targets() {
        if let Some(offset) = target.measurement_record_offset() {
            if offset.is_negative_zero() {
                continue;
            }
            row.toggle(absolute_record_index(offset, measurement_offset)?);
        } else if allow_pauli && target.is_pauli_target() {
            if let Some(observable) = instruction.observable_id_argument()? {
                ignored_logical_rows.insert(observable.get());
            }
        } else {
            return Err(AnalysisError::invalid_detector_error_model(format!(
                "{} target {target} is not supported by missing-detector analysis",
                instruction.gate().canonical_name()
            )));
        }
    }
    Ok(row)
}

fn absolute_record_index(
    offset: MeasureRecordOffset,
    measurement_offset: usize,
) -> AnalysisResult<usize> {
    let current = i64::try_from(measurement_offset).map_err(|_| {
        AnalysisError::invalid_detector_error_model(
            "measurement offset does not fit i64 during missing-detector analysis",
        )
    })?;
    let index = current
        .checked_add(i64::from(offset.get()))
        .ok_or_else(|| {
            AnalysisError::invalid_detector_error_model(
                "measurement record offset overflowed during missing-detector analysis",
            )
        })?;
    if index < 0 || index >= current {
        return Err(AnalysisError::invalid_detector_error_model(format!(
            "measurement record target rec[{}] is outside missing-detector analysis history",
            offset.stim_text()
        )));
    }
    usize::try_from(index).map_err(|_| {
        AnalysisError::invalid_detector_error_model(
            "measurement record index does not fit usize during missing-detector analysis",
        )
    })
}

fn circuit_with_known_input_resets(
    circuit: &Circuit,
    options: &MissingDetectorOptions,
    qubit_count: usize,
) -> AnalysisResult<Circuit> {
    if options.ignore_non_deterministic_measurements {
        return Ok(circuit.clone());
    }
    if qubit_count == 0 {
        return Ok(circuit.clone());
    }
    let mut targets = Vec::with_capacity(qubit_count);
    for index in 0..qubit_count {
        let id = u32::try_from(index)
            .ok()
            .and_then(|index| QubitId::new(index).ok())
            .ok_or_else(|| {
                AnalysisError::invalid_detector_error_model(format!(
                    "qubit index {index} is outside the supported target range"
                ))
            })?;
        targets.push(Target::qubit(id, false));
    }
    let mut with_resets = Circuit::new();
    with_resets.append_instruction(CircuitInstruction::new(
        GateKind::R.gate(),
        Vec::new(),
        targets,
        None,
    )?);
    with_resets.append_circuit(circuit);
    Ok(with_resets)
}

fn collect_measurement_invariants(
    generators: Vec<Flow>,
    measurement_count: usize,
) -> AnalysisResult<Vec<MeasurementRow>> {
    let mut invariants = Vec::new();
    for generator in generators {
        if !generator.input().has_no_pauli_terms()
            || !generator.output().has_no_pauli_terms()
            || generator.observables().next().is_some()
        {
            continue;
        }
        let mut row = MeasurementRow::default();
        for record in generator.measurements() {
            let index = flow_record_index(record, measurement_count).ok_or_else(|| {
                AnalysisError::invalid_detector_error_model(format!(
                    "flow generator record {record} is outside {measurement_count} measurements"
                ))
            })?;
            row.toggle(index);
        }
        invariants.push(row);
    }
    Ok(invariants)
}

fn build_missing_detector_output(
    declarations: DeclaredRows,
    invariants: Vec<MeasurementRow>,
    measurement_count: usize,
) -> AnalysisResult<Circuit> {
    let mut rows = declarations
        .known
        .into_iter()
        .map(|row| EliminationRow {
            row,
            invariant: false,
        })
        .chain(invariants.into_iter().map(|row| EliminationRow {
            row,
            invariant: true,
        }))
        .collect::<Vec<_>>();
    eliminate_rows(&mut rows, measurement_count);

    let detector = GateKind::DETECTOR.gate();
    let mut result = Circuit::new();
    for row in &mut rows {
        if !row.invariant || row.row.is_empty() {
            continue;
        }
        for known in &declarations.original {
            if row.row.is_subset_of(known) {
                row.row.xor_assign(known);
            }
        }
        if row.row.is_empty() {
            continue;
        }
        let targets = row
            .row
            .iter()
            .map(|index| relative_offset(index, measurement_count).map(Target::measurement_record))
            .collect::<AnalysisResult<Vec<_>>>()?;
        result.append_instruction(CircuitInstruction::new(
            detector,
            Vec::new(),
            targets,
            None,
        )?);
    }
    Ok(result)
}

fn relative_offset(index: usize, total: usize) -> AnalysisResult<MeasureRecordOffset> {
    let index = i64::try_from(index).map_err(|_| {
        AnalysisError::invalid_detector_error_model(
            "measurement index does not fit i64 during missing-detector output",
        )
    })?;
    let total = i64::try_from(total).map_err(|_| {
        AnalysisError::invalid_detector_error_model(
            "measurement count does not fit i64 during missing-detector output",
        )
    })?;
    let offset = index.checked_sub(total).ok_or_else(|| {
        AnalysisError::invalid_detector_error_model(
            "measurement record offset underflowed during missing-detector output",
        )
    })?;
    let offset = i32::try_from(offset).map_err(|_| {
        AnalysisError::invalid_detector_error_model(format!(
            "measurement record offset {offset} does not fit i32"
        ))
    })?;
    MeasureRecordOffset::try_new(offset).map_err(AnalysisError::from)
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct MeasurementRow {
    bits: BTreeSet<usize>,
}

impl MeasurementRow {
    fn is_empty(&self) -> bool {
        self.bits.is_empty()
    }

    fn contains(&self, index: usize) -> bool {
        self.bits.contains(&index)
    }

    fn toggle(&mut self, index: usize) {
        if !self.bits.insert(index) {
            self.bits.remove(&index);
        }
    }

    fn xor_assign(&mut self, rhs: &Self) {
        for index in &rhs.bits {
            self.toggle(*index);
        }
    }

    fn is_subset_of(&self, rhs: &Self) -> bool {
        self.bits.is_subset(&rhs.bits)
    }

    fn iter(&self) -> impl Iterator<Item = usize> + '_ {
        self.bits.iter().copied()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct EliminationRow {
    row: MeasurementRow,
    invariant: bool,
}

fn eliminate_rows(rows: &mut [EliminationRow], measurement_count: usize) {
    let mut solved = 0usize;
    for column in 0..measurement_count {
        let pivot = (solved..rows.len())
            .find(|index| {
                rows.get(*index)
                    .is_some_and(|row| row.row.contains(column) && !row.invariant)
            })
            .or_else(|| {
                (solved..rows.len())
                    .find(|index| rows.get(*index).is_some_and(|row| row.row.contains(column)))
            });
        let Some(pivot) = pivot else {
            continue;
        };
        let Some(pivot_row) = rows.get(pivot).map(|row| row.row.clone()) else {
            continue;
        };
        for (index, row) in rows.iter_mut().enumerate() {
            if index != pivot && row.row.contains(column) {
                row.row.xor_assign(&pivot_row);
            }
        }
        rows.swap(pivot, solved);
        solved += 1;
    }
}
