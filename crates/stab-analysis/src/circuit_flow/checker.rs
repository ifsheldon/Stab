use std::collections::BTreeSet;

use stab_algebra::{
    AlgebraMemoryBudget, Flow, FlowMeasurementIndex, PauliBasis, PauliSign, PauliString,
    StabilizerError, Tableau,
};
use stab_model::advanced::FoldedCircuitEvent;
use stab_model::{
    Cancellation, Circuit, CircuitItem, DemTarget, GateCategory, MemoryBudget, ObservableId,
    ParseLimits, QubitId, Target,
};

use super::check_algorithm::{FlowCheckAlgorithm, FlowCheckCost};
use crate::{
    AnalysisError, AnalysisOperation, AnalysisResult, CircuitToTableauOptions, ResourceKind,
    ResourceLimitError, circuit_to_tableau_with_options,
    folded_walk::walk_folded,
    sparse_rev_frame_tracker::{AnticommutationPolicy, SparseReverseFrameTracker},
};

/// Options for the unsigned stabilizer flow checks, such as
/// [`check_if_circuit_has_unsigned_stabilizer_flows_with_options`].
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct UnsignedFlowCheckOptions {
    /// Budget for the dense Tableau of the circuit, the sparse tracker's live sensitivity sets,
    /// and each Pauli string the sparse tracker extracts. A Tableau the budget refuses is not an
    /// error: the check falls back to the sparse tracker, which is how circuits too wide for a
    /// Tableau are checked.
    pub memory_budget: MemoryBudget,
    /// Signal polled once per instruction, Tableau composition, seeded flow term, and checked flow.
    pub cancellation: Cancellation,
}

impl UnsignedFlowCheckOptions {
    fn algebra_budget(&self) -> AlgebraMemoryBudget {
        AlgebraMemoryBudget::from_bytes(self.memory_budget.bytes())
    }
}

/// Checks unsigned stabilizer flows with the default memory budget, reporting `false` for a flow
/// the checker cannot decide.
///
/// See [`check_if_circuit_has_unsigned_stabilizer_flows_with_options`].
pub fn check_if_circuit_has_unsigned_stabilizer_flows(
    circuit: &Circuit,
    flows: &[Flow],
) -> Vec<bool> {
    check_if_circuit_has_unsigned_stabilizer_flows_with_options(
        circuit,
        flows,
        &UnsignedFlowCheckOptions::default(),
    )
    .unwrap_or_else(|_| vec![false; flows.len()])
}

/// Checks unsigned stabilizer flows against the supported unitary and sparse-tracker subsets.
///
/// Pauli-only flows of a measurement-free circuit are checked either against one dense Tableau or
/// with the sparse reverse tracker, whichever an internal cost model estimates to be cheaper; both
/// return the same result. A flow outside the supported subset is reported as `false`.
///
/// # Errors
///
/// Returns [`AnalysisError::Cancelled`] once the options' signal is observed, never a partial or
/// `false` result, and [`AnalysisError::ResourceLimit`] when the sparse tracker's live sensitivity
/// sets or an extracted Pauli string exceed the options' memory budget.
pub fn check_if_circuit_has_unsigned_stabilizer_flows_with_options(
    circuit: &Circuit,
    flows: &[Flow],
    options: &UnsignedFlowCheckOptions,
) -> AnalysisResult<Vec<bool>> {
    options.cancellation.check()?;
    if flows.is_empty() {
        return Ok(Vec::new());
    }
    check_unsigned_flows_using(
        select_flow_check_algorithm(circuit, flows),
        circuit,
        flows,
        options,
    )
}

/// Chooses the cheaper algorithm for a flow query, or the sparse tracker when only it applies.
fn select_flow_check_algorithm(circuit: &Circuit, flows: &[Flow]) -> FlowCheckAlgorithm {
    if dense_tableau_can_check(circuit, flows) {
        FlowCheckCost::measure(circuit, flows).cheaper_algorithm()
    } else {
        FlowCheckAlgorithm::SparseTracker
    }
}

/// Reports whether one dense Tableau can check every flow: the circuit is repeat-free and acts on
/// Pauli-only flows as a unitary, and every flow is a Pauli-only flow no wider than the circuit.
/// Repeat blocks stay on the sparse tracker, which folds them without raising a Tableau to a power.
fn dense_tableau_can_check(circuit: &Circuit, flows: &[Flow]) -> bool {
    let circuit_qubits = stab_model::advanced::circuit_simulated_qubit_count(circuit);
    circuit
        .items()
        .iter()
        .all(|item| matches!(item, CircuitItem::Instruction(_)))
        && circuit_acts_unitarily_on_pauli_flows(circuit)
        && flows.iter().all(|flow| {
            is_pauli_only_flow(flow)
                && flow.input().len() <= circuit_qubits
                && flow.output().len() <= circuit_qubits
        })
}

/// Reports whether every instruction either is a unitary over qubit targets or cannot act on a
/// Pauli-only flow (annotations without record targets and noise without heralds), so the circuit
/// maps each Pauli-only flow input to exactly one output Pauli string.
fn circuit_acts_unitarily_on_pauli_flows(circuit: &Circuit) -> bool {
    circuit.items().iter().all(|item| match item {
        CircuitItem::Instruction(instruction) => {
            let gate = instruction.gate();
            !gate.produces_measurements()
                && !gate.is_reset()
                && !instruction
                    .targets()
                    .iter()
                    .any(Target::is_classical_bit_target)
                && (gate.is_unitary()
                    || matches!(
                        gate.category(),
                        GateCategory::Annotation | GateCategory::Noise
                    ))
        }
        CircuitItem::RepeatBlock(repeat) => circuit_acts_unitarily_on_pauli_flows(repeat.body()),
    })
}

fn is_pauli_only_flow(flow: &Flow) -> bool {
    flow.measurements().next().is_none() && flow.observables().next().is_none()
}

/// Checks flows with the given algorithm. The dense Tableau falls back to the sparse tracker when
/// the circuit cannot be converted, so either algorithm returns the same result for any query.
pub(crate) fn check_unsigned_flows_using(
    algorithm: FlowCheckAlgorithm,
    circuit: &Circuit,
    flows: &[Flow],
    options: &UnsignedFlowCheckOptions,
) -> AnalysisResult<Vec<bool>> {
    if algorithm == FlowCheckAlgorithm::DenseTableau
        && let Some(tableau) = dense_flow_tableau(circuit, flows, options)?
    {
        return flows
            .iter()
            .map(|flow| {
                options.cancellation.check()?;
                Ok(dense_flow_image(&tableau, flow.input(), options)?
                    .is_some_and(|actual| paulis_match_unsigned(&actual, flow.output())))
            })
            .collect();
    }
    match check_unsigned_flows_with_sparse_tracker(circuit, flows, options) {
        Err(error) if !stops_flow_check(&error) => Ok(vec![false; flows.len()]),
        result => result,
    }
}

/// Reports whether a failure stops the whole check instead of failing the flows closed: the
/// caller's signal or budget, as opposed to a query outside the supported subset.
pub(crate) fn stops_flow_check(error: &AnalysisError) -> bool {
    matches!(
        error,
        AnalysisError::Cancelled(_) | AnalysisError::ResourceLimit(_)
    )
}

/// Builds the dense Tableau when it can check every flow. Noise is skipped because unsigned flow
/// checking ignores it, exactly as the sparse tracker does. A Tableau that cannot be built, for
/// example because the budget refuses it, yields `None` so the caller falls back to the sparse
/// tracker; only cancellation stops the check.
fn dense_flow_tableau(
    circuit: &Circuit,
    flows: &[Flow],
    options: &UnsignedFlowCheckOptions,
) -> AnalysisResult<Option<Tableau>> {
    if !dense_tableau_can_check(circuit, flows) {
        return Ok(None);
    }
    let tableau_options = CircuitToTableauOptions {
        ignore_noise: true,
        memory_budget: options.memory_budget,
        cancellation: options.cancellation.clone(),
        ..CircuitToTableauOptions::default()
    };
    match circuit_to_tableau_with_options(circuit, &tableau_options) {
        Ok(tableau) => Ok(Some(tableau)),
        Err(AnalysisError::Cancelled(cancelled)) => Err(cancelled.into()),
        Err(_) => Ok(None),
    }
}

/// Applies the Tableau to a flow input no wider than it, padding the input with identities.
///
/// Returns `Ok(None)` when the Tableau cannot act on the input, and an error only for a padded
/// input the budget refuses.
fn dense_flow_image(
    tableau: &Tableau,
    input: &PauliString,
    options: &UnsignedFlowCheckOptions,
) -> AnalysisResult<Option<PauliString>> {
    dense_flow_image_or_reason(tableau, input, options).map(Result::ok)
}

/// Like [`dense_flow_image`], keeping the reason the Tableau could not act on the input.
fn dense_flow_image_or_reason(
    tableau: &Tableau,
    input: &PauliString,
    options: &UnsignedFlowCheckOptions,
) -> AnalysisResult<Result<PauliString, String>> {
    if input.len() == tableau.len() {
        return Ok(tableau.apply(input).map_err(|error| error.to_string()));
    }
    let padded = match PauliString::from_bases_with_budget(
        PauliSign::Plus,
        (0..tableau.len()).map(|index| input.get(index).unwrap_or(PauliBasis::I)),
        options.algebra_budget(),
    ) {
        Ok(padded) => padded,
        Err(StabilizerError::MemoryBudgetExceeded {
            requested_bytes,
            budget_bytes,
            ..
        }) => {
            return Err(ResourceLimitError::memory_budget(
                AnalysisOperation::FlowCheck,
                ResourceKind::MaterializedBytes,
                u64::try_from(requested_bytes).unwrap_or(u64::MAX),
                budget_bytes,
            )
            .into());
        }
        Err(error) => return Ok(Err(error.to_string())),
    };
    Ok(tableau.apply(&padded).map_err(|error| error.to_string()))
}

fn check_unsigned_flows_with_tableau(
    tableau: &Tableau,
    flows: &[Flow],
    options: &UnsignedFlowCheckOptions,
) -> AnalysisResult<Vec<UnsignedStabilizerFlowCheck>> {
    flows
        .iter()
        .map(|flow| {
            options.cancellation.check()?;
            Ok(
                match dense_flow_image_or_reason(tableau, flow.input(), options)? {
                    Ok(actual) if paulis_match_unsigned(&actual, flow.output()) => {
                        UnsignedStabilizerFlowCheck::passed()
                    }
                    Ok(actual) => UnsignedStabilizerFlowCheck::failed(
                        UnsignedStabilizerFlowFailure::OutputMismatch {
                            expected_output: unsigned_pauli(flow.output()),
                            actual_output: unsigned_pauli(&actual),
                        },
                    ),
                    Err(reason) => UnsignedStabilizerFlowCheck::unsupported(reason),
                },
            )
        })
        .collect()
}

/// Diagnostic result for one unsigned stabilizer flow query.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnsignedStabilizerFlowCheck {
    has_flow: bool,
    failure: Option<UnsignedStabilizerFlowFailure>,
}

/// Reason an unsigned stabilizer flow query failed for the supported diagnostic subset.
#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UnsignedStabilizerFlowFailure {
    /// A circuit of unitaries, annotations, and noise mapped the input of a Pauli-only flow to a
    /// different output Pauli string, whichever algorithm checked it.
    OutputMismatch {
        expected_output: PauliString,
        actual_output: PauliString,
    },
    /// A circuit with collapsing or classically controlled operations, or a flow with measurement
    /// or observable terms, pulled the requested output, measurement, and observable terms back to
    /// a different input Pauli string.
    InputMismatch {
        expected_input: PauliString,
        actual_input: PauliString,
    },
    /// A flow measurement term referenced a measurement record outside the circuit's measurement
    /// range.
    MeasurementRecordOutOfRange {
        record: FlowMeasurementIndex,
        measurement_count: usize,
    },
    /// A collapse operation anti-commuted with the tracked flow region.
    CollapseAnticommutation,
    /// The circuit or flow fell outside the supported unsigned checker subset.
    UnsupportedCircuit { reason: String },
}

/// Checks unsigned stabilizer flows and reports why unsupported or unsatisfied flows failed.
///
/// This is the diagnostic counterpart to
/// [`check_if_circuit_has_unsigned_stabilizer_flows`]. It preserves the same supported subset and
/// fail-closed semantics, but keeps the first local reason for a false row.
///
/// The mismatch kind depends only on the query, never on the algorithm that checked it. A
/// Pauli-only flow of a circuit whose instructions are unitaries, annotations, or noise reports
/// [`UnsignedStabilizerFlowFailure::OutputMismatch`] with the circuit's image of the flow input,
/// as wide as the widest of the circuit, the input, and the output. Every other Pauli mismatch
/// reports [`UnsignedStabilizerFlowFailure::InputMismatch`].
///
/// The default memory budget applies, and a check the budget refuses reports every flow as
/// [`UnsignedStabilizerFlowFailure::UnsupportedCircuit`] with the refusal as its reason; see
/// [`check_unsigned_stabilizer_flows_with_diagnostics_with_options`].
pub fn check_unsigned_stabilizer_flows_with_diagnostics(
    circuit: &Circuit,
    flows: &[Flow],
) -> Vec<UnsignedStabilizerFlowCheck> {
    check_unsigned_stabilizer_flows_with_diagnostics_with_options(
        circuit,
        flows,
        &UnsignedFlowCheckOptions::default(),
    )
    .unwrap_or_else(|error| {
        vec![UnsignedStabilizerFlowCheck::unsupported(error.to_string()); flows.len()]
    })
}

/// Checks unsigned stabilizer flows with diagnostics under the given options.
///
/// This is [`check_unsigned_stabilizer_flows_with_diagnostics`] with a caller budget and
/// cancellation signal, as [`check_if_circuit_has_unsigned_stabilizer_flows_with_options`].
///
/// # Errors
///
/// Returns [`AnalysisError::Cancelled`] once the options' signal is observed and
/// [`AnalysisError::ResourceLimit`] when live sensitivity sets or an extracted Pauli string exceed
/// the budget.
pub fn check_unsigned_stabilizer_flows_with_diagnostics_with_options(
    circuit: &Circuit,
    flows: &[Flow],
    options: &UnsignedFlowCheckOptions,
) -> AnalysisResult<Vec<UnsignedStabilizerFlowCheck>> {
    options.cancellation.check()?;
    if flows.is_empty() {
        return Ok(Vec::new());
    }
    diagnose_unsigned_flows_using(
        select_flow_check_algorithm(circuit, flows),
        circuit,
        flows,
        options,
    )
}

/// Diagnoses flows with the given algorithm; either algorithm returns the same diagnostics.
pub(crate) fn diagnose_unsigned_flows_using(
    algorithm: FlowCheckAlgorithm,
    circuit: &Circuit,
    flows: &[Flow],
    options: &UnsignedFlowCheckOptions,
) -> AnalysisResult<Vec<UnsignedStabilizerFlowCheck>> {
    if algorithm == FlowCheckAlgorithm::DenseTableau
        && let Some(tableau) = dense_flow_tableau(circuit, flows, options)?
    {
        return check_unsigned_flows_with_tableau(&tableau, flows, options);
    }
    let checks = match diagnose_unsigned_flows_with_sparse_tracker(circuit, flows, options) {
        Ok(checks) => checks,
        Err(error) if stops_flow_check(&error) => return Err(error),
        Err(error) => {
            return Ok(vec![
                UnsignedStabilizerFlowCheck::unsupported(
                    error.to_string()
                );
                flows.len()
            ]);
        }
    };
    if circuit_acts_unitarily_on_pauli_flows(circuit) {
        report_unitary_mismatches_as_outputs(circuit, flows, checks, options)
    } else {
        Ok(checks)
    }
}

/// Rewrites the input mismatches of Pauli-only flows of a unitary circuit as output mismatches, so
/// the sparse tracker reports what the dense Tableau reports. The circuit's images of the failing
/// inputs are found by tracking them backwards through the inverse of the circuit's unitary part,
/// so this costs one more sparse pass over the failing flows only.
fn report_unitary_mismatches_as_outputs(
    circuit: &Circuit,
    flows: &[Flow],
    mut checks: Vec<UnsignedStabilizerFlowCheck>,
    options: &UnsignedFlowCheckOptions,
) -> AnalysisResult<Vec<UnsignedStabilizerFlowCheck>> {
    let failing = flows
        .iter()
        .zip(&checks)
        .enumerate()
        .filter(|(_, (flow, check))| {
            is_pauli_only_flow(flow)
                && matches!(
                    check.failure,
                    Some(UnsignedStabilizerFlowFailure::InputMismatch { .. })
                )
        })
        .map(|(index, (flow, _))| (index, flow))
        .collect::<Vec<_>>();
    if failing.is_empty() {
        return Ok(checks);
    }
    let inputs = failing
        .iter()
        .map(|(_, flow)| flow.input().clone())
        .collect::<Vec<_>>();
    let images = crate::circuit_inverse::unitary_part_inverse(circuit)
        .and_then(|inverse| pull_back_paulis_with_sparse_tracker(&inverse, &inputs, options));
    if let Err(error) = &images
        && stops_flow_check(error)
    {
        return Err(error.clone());
    }
    let circuit_qubits = stab_model::advanced::circuit_simulated_qubit_count(circuit);
    for (position, (index, flow)) in failing.into_iter().enumerate() {
        let failure = match &images {
            Ok(images) => match images.get(position) {
                Some(image) => {
                    let width = circuit_qubits
                        .max(flow.input().len())
                        .max(flow.output().len());
                    let mut bases = vec![PauliBasis::I; width];
                    xor_region(&mut bases, image, &options.cancellation)?;
                    UnsignedStabilizerFlowFailure::OutputMismatch {
                        expected_output: unsigned_pauli(flow.output()),
                        actual_output: stab_algebra::advanced::pauli_from_bases_unbudgeted(
                            PauliSign::Plus,
                            bases,
                        ),
                    }
                }
                None => UnsignedStabilizerFlowFailure::UnsupportedCircuit {
                    reason: "sparse reverse tracking returned fewer images than flows".to_owned(),
                },
            },
            Err(error) => UnsignedStabilizerFlowFailure::UnsupportedCircuit {
                reason: error.to_string(),
            },
        };
        if let Some(check) = checks.get_mut(index) {
            *check = UnsignedStabilizerFlowCheck::failed(failure);
        }
    }
    Ok(checks)
}

/// Tracks each Pauli string backwards through `circuit` and returns the Pauli strings it pulls
/// back to, unsigned and only as wide as their support.
fn pull_back_paulis_with_sparse_tracker(
    circuit: &Circuit,
    paulis: &[PauliString],
    options: &UnsignedFlowCheckOptions,
) -> AnalysisResult<Vec<PauliString>> {
    let qubit_count = paulis.iter().map(PauliString::len).fold(
        stab_model::advanced::circuit_simulated_qubit_count(circuit),
        usize::max,
    );
    let mut tracker =
        SparseReverseFrameTracker::new(qubit_count, 0, 0, AnticommutationPolicy::Record);
    tracker.set_cancellation(options.cancellation.clone());
    tracker.set_memory_budget(
        AnalysisOperation::FlowCheck,
        ResourceKind::LiveStateUnits,
        options.memory_budget,
    );
    let targets = flow_labels(circuit, paulis.len(), &options.cancellation)?;
    for (pauli, target) in paulis.iter().zip(&targets) {
        seed_flow_pauli_output(&mut tracker, pauli, *target)?;
        tracker.check_live_budget()?;
    }
    tracker.undo_circuit(circuit)?;
    targets
        .into_iter()
        .map(|target| {
            tracker.check_cancelled()?;
            Ok(tracker
                .compact_region_for_target(target, options.algebra_budget())?
                .value()
                .clone())
        })
        .collect()
}

impl UnsignedStabilizerFlowCheck {
    /// Returns true when the flow was satisfied by the circuit under unsigned semantics.
    pub fn has_flow(&self) -> bool {
        self.has_flow
    }

    /// Returns the failure reason when the flow was not satisfied.
    pub fn failure(&self) -> Option<&UnsignedStabilizerFlowFailure> {
        self.failure.as_ref()
    }

    fn passed() -> Self {
        Self {
            has_flow: true,
            failure: None,
        }
    }

    fn failed(failure: UnsignedStabilizerFlowFailure) -> Self {
        Self {
            has_flow: false,
            failure: Some(failure),
        }
    }

    fn unsupported(reason: String) -> Self {
        Self::failed(UnsignedStabilizerFlowFailure::UnsupportedCircuit { reason })
    }
}

/// Returns true when the circuit has the given unsigned stabilizer flow, under the default memory
/// budget.
///
/// See [`circuit_has_unsigned_stabilizer_flow_with_options`].
pub fn circuit_has_unsigned_stabilizer_flow(circuit: &Circuit, flow: &Flow) -> bool {
    check_if_circuit_has_unsigned_stabilizer_flows(circuit, std::slice::from_ref(flow))
        .into_iter()
        .next()
        .unwrap_or(false)
}

/// Returns true when the circuit has the given unsigned stabilizer flow.
///
/// # Errors
///
/// Fails like [`check_if_circuit_has_unsigned_stabilizer_flows_with_options`].
pub fn circuit_has_unsigned_stabilizer_flow_with_options(
    circuit: &Circuit,
    flow: &Flow,
    options: &UnsignedFlowCheckOptions,
) -> AnalysisResult<bool> {
    Ok(check_if_circuit_has_unsigned_stabilizer_flows_with_options(
        circuit,
        std::slice::from_ref(flow),
        options,
    )?
    .into_iter()
    .next()
    .unwrap_or(false))
}

/// Returns true when the circuit has every requested unsigned stabilizer flow, under the default
/// memory budget.
///
/// This is the Rust unsigned counterpart of Stim's `has_all_flows` batch query for the supported
/// Stab flow-checker subset. Signs are ignored, matching
/// [`check_if_circuit_has_unsigned_stabilizer_flows`]. See
/// [`circuit_has_all_unsigned_stabilizer_flows_with_options`].
pub fn circuit_has_all_unsigned_stabilizer_flows(circuit: &Circuit, flows: &[Flow]) -> bool {
    check_if_circuit_has_unsigned_stabilizer_flows(circuit, flows)
        .into_iter()
        .all(|has_flow| has_flow)
}

/// Returns true when the circuit has every requested unsigned stabilizer flow.
///
/// # Errors
///
/// Fails like [`check_if_circuit_has_unsigned_stabilizer_flows_with_options`].
pub fn circuit_has_all_unsigned_stabilizer_flows_with_options(
    circuit: &Circuit,
    flows: &[Flow],
    options: &UnsignedFlowCheckOptions,
) -> AnalysisResult<bool> {
    Ok(
        check_if_circuit_has_unsigned_stabilizer_flows_with_options(circuit, flows, options)?
            .into_iter()
            .all(|has_flow| has_flow),
    )
}

pub(crate) fn check_unsigned_flows_with_sparse_tracker(
    circuit: &Circuit,
    flows: &[Flow],
    options: &UnsignedFlowCheckOptions,
) -> AnalysisResult<Vec<bool>> {
    Ok(
        diagnose_unsigned_flows_with_sparse_tracker(circuit, flows, options)?
            .into_iter()
            .map(|check| check.has_flow)
            .collect(),
    )
}

fn diagnose_unsigned_flows_with_sparse_tracker(
    circuit: &Circuit,
    flows: &[Flow],
    options: &UnsignedFlowCheckOptions,
) -> AnalysisResult<Vec<UnsignedStabilizerFlowCheck>> {
    if flows.is_empty() {
        return Ok(Vec::new());
    }
    let measurement_count = usize::try_from(circuit.count_measurements()?).map_err(|_| {
        AnalysisError::count_overflow(
            AnalysisOperation::FlowCheck,
            "circuit measurement count does not fit usize during flow checking",
        )
    })?;
    let detector_count = circuit.count_detectors()?;
    let flow_qubit_count = flows
        .iter()
        .flat_map(|flow| [flow.input().len(), flow.output().len()])
        .max()
        .unwrap_or(0);
    let qubit_count =
        stab_model::advanced::circuit_simulated_qubit_count(circuit).max(flow_qubit_count);
    let mut tracker = SparseReverseFrameTracker::new(
        qubit_count,
        measurement_count,
        detector_count,
        AnticommutationPolicy::Record,
    );
    tracker.set_cancellation(options.cancellation.clone());
    tracker.set_memory_budget(
        AnalysisOperation::FlowCheck,
        ResourceKind::LiveStateUnits,
        options.memory_budget,
    );

    let labels = flow_labels(circuit, flows.len(), &options.cancellation)?;
    let mut preliminary = Vec::with_capacity(flows.len());
    for (flow, &tracked_target) in flows.iter().zip(&labels) {
        match flow_record_indices(flow, measurement_count) {
            Ok(record_indices) => {
                seed_flow_pauli_output(&mut tracker, flow.output(), tracked_target)?;
                for record_index in record_indices {
                    tracker.toggle_record_target_absolute(record_index, tracked_target)?;
                }
                for observable in flow.observables() {
                    tracker
                        .toggle_observable_effect(ObservableId::from(observable), tracked_target)?;
                }
                tracker.check_live_budget()?;
                preliminary.push(None);
            }
            Err(record) => preliminary.push(Some(
                UnsignedStabilizerFlowFailure::MeasurementRecordOutOfRange {
                    record: FlowMeasurementIndex::new(record),
                    measurement_count,
                },
            )),
        }
    }
    tracker.undo_circuit(circuit)?;

    flows
        .iter()
        .enumerate()
        .map(|(flow_index, flow)| {
            if let Some(failure) = preliminary.get(flow_index).cloned().flatten() {
                return Ok(UnsignedStabilizerFlowCheck::failed(failure));
            }
            tracker.check_cancelled()?;
            let tracked_target = *labels.get(flow_index).ok_or_else(|| {
                AnalysisError::internal(
                    AnalysisOperation::FlowCheck,
                    "flow check lost a flow label",
                )
            })?;
            if tracker.target_anticommuted(tracked_target) {
                return Ok(UnsignedStabilizerFlowCheck::failed(
                    UnsignedStabilizerFlowFailure::CollapseAnticommutation,
                ));
            }
            let region =
                tracker.compact_region_for_target(tracked_target, options.algebra_budget())?;
            let region_pauli = region.value();
            let target_len = flow.input().len().max(region_pauli.len());
            let mut bases = Vec::new();
            let mut remaining = target_len;
            while remaining > 0 {
                options.cancellation.check()?;
                let step =
                    remaining.min(stab_model::advanced::CancellationCountdown::INTERVAL as usize);
                bases.resize(bases.len() + step, PauliBasis::I);
                remaining -= step;
            }
            xor_region(&mut bases, region_pauli, &options.cancellation)?;
            let actual =
                stab_algebra::advanced::pauli_from_bases_unbudgeted(PauliSign::Plus, bases);
            if paulis_match_unsigned_polled(&actual, flow.input(), &options.cancellation)? {
                Ok(UnsignedStabilizerFlowCheck::passed())
            } else {
                Ok(UnsignedStabilizerFlowCheck::failed(
                    UnsignedStabilizerFlowFailure::InputMismatch {
                        expected_input: unsigned_pauli(flow.input()),
                        actual_input: unsigned_pauli(&actual),
                    },
                ))
            }
        })
        .collect()
}

/// Returns one reverse-tracker label per flow: the smallest observable ids that no
/// `OBSERVABLE_INCLUDE` of `circuit` names.
///
/// A label therefore never meets a detector or observable the circuit itself adds, and, being an
/// observable, it is never renumbered by the tracker's shifted-loop folding or chosen as a gauge
/// pivot. The distinct ids collected are bounded by the circuit's own instruction count.
fn flow_labels(
    circuit: &Circuit,
    flow_count: usize,
    cancellation: &Cancellation,
) -> AnalysisResult<Vec<DemTarget>> {
    let mut included = BTreeSet::new();
    walk_folded(
        circuit,
        ParseLimits::DEFAULT_REPEAT_NESTING,
        cancellation,
        |depth, limit| {
            ResourceLimitError::fixed_operation(
                AnalysisOperation::FlowCheck,
                ResourceKind::RepeatNesting,
                depth,
                limit,
            )
            .into()
        },
        |event| {
            if let FoldedCircuitEvent::Instruction { instruction, .. } = event
                && let Some(observable) = instruction.observable_id_argument()?
            {
                included.insert(u64::from(observable.get()));
            }
            Ok(())
        },
    )?;
    let mut labels = Vec::with_capacity(flow_count);
    let mut candidate = 0_u64;
    while labels.len() < flow_count {
        cancellation.check()?;
        if !included.contains(&candidate) {
            labels.push(DemTarget::logical_observable(candidate).map_err(|_| {
                AnalysisError::unsupported(
                    AnalysisOperation::FlowCheck,
                    None,
                    "the circuit's observables and the flows exceed the observable id range",
                )
            })?);
        }
        candidate = candidate.checked_add(1).ok_or_else(|| {
            AnalysisError::count_overflow(
                AnalysisOperation::FlowCheck,
                "flow label search overflowed",
            )
        })?;
    }
    Ok(labels)
}

fn flow_record_indices(flow: &Flow, measurement_count: usize) -> Result<Vec<usize>, i32> {
    let mut result = Vec::new();
    for measurement in flow.measurements() {
        let Some(record_index) = flow_record_index(measurement, measurement_count) else {
            return Err(measurement);
        };
        result.push(record_index);
    }
    Ok(result)
}

fn seed_flow_pauli_output(
    tracker: &mut SparseReverseFrameTracker,
    output: &PauliString,
    target: DemTarget,
) -> AnalysisResult<()> {
    for index in 0..output.len() {
        tracker.poll_cancellation()?;
        let basis = output.get(index).unwrap_or(PauliBasis::I);
        if basis == PauliBasis::I {
            continue;
        }
        let qubit = u32::try_from(index)
            .ok()
            .and_then(|index| QubitId::new(index).ok())
            .ok_or_else(|| {
                AnalysisError::internal(
                    AnalysisOperation::FlowCheck,
                    format!(
                        "flow output qubit index {index} is outside the supported target range"
                    ),
                )
            })?;
        tracker.toggle_pauli_target(qubit, basis, target)?;
    }
    Ok(())
}

pub fn flow_record_index(index: i32, measurement_count: usize) -> Option<usize> {
    if index >= 0 {
        return usize::try_from(index)
            .ok()
            .filter(|index| *index < measurement_count);
    }
    let measurement_count_i64 = i64::try_from(measurement_count).ok()?;
    let absolute = measurement_count_i64.checked_add(i64::from(index))?;
    usize::try_from(absolute)
        .ok()
        .filter(|index| *index < measurement_count)
}

fn xor_region(
    bases: &mut Vec<PauliBasis>,
    region: &PauliString,
    cancellation: &stab_model::Cancellation,
) -> AnalysisResult<()> {
    if region.len() > bases.len() {
        bases.resize(region.len(), PauliBasis::I);
    }
    let mut countdown = stab_model::advanced::CancellationCountdown::new();
    for index in 0..region.len() {
        countdown.tick(cancellation)?;
        let basis = region.get(index).unwrap_or(PauliBasis::I);
        if basis != PauliBasis::I
            && let Some(existing) = bases.get_mut(index)
        {
            *existing = xor_basis(*existing, basis);
        }
    }
    Ok(())
}

fn xor_basis(left: PauliBasis, right: PauliBasis) -> PauliBasis {
    PauliBasis::from_xz(left.x_bit() ^ right.x_bit(), left.z_bit() ^ right.z_bit())
}

fn paulis_match_unsigned(left: &PauliString, right: &PauliString) -> bool {
    word_planes_match(left.x_bits(), right.x_bits())
        && word_planes_match(left.z_bits(), right.z_bits())
}

fn paulis_match_unsigned_polled(
    left: &PauliString,
    right: &PauliString,
    cancellation: &stab_model::Cancellation,
) -> AnalysisResult<bool> {
    let mut countdown = stab_model::advanced::CancellationCountdown::new();
    Ok(
        word_planes_match_polled(left.x_bits(), right.x_bits(), cancellation, &mut countdown)?
            && word_planes_match_polled(
                left.z_bits(),
                right.z_bits(),
                cancellation,
                &mut countdown,
            )?,
    )
}

fn word_planes_match(left: &[u64], right: &[u64]) -> bool {
    let max_words = left.len().max(right.len());
    (0..max_words)
        .all(|word| left.get(word).copied().unwrap_or(0) == right.get(word).copied().unwrap_or(0))
}

fn word_planes_match_polled(
    left: &[u64],
    right: &[u64],
    cancellation: &stab_model::Cancellation,
    countdown: &mut stab_model::advanced::CancellationCountdown,
) -> AnalysisResult<bool> {
    let max_words = left.len().max(right.len());
    for word in 0..max_words {
        countdown.tick(cancellation)?;
        if left.get(word).copied().unwrap_or(0) != right.get(word).copied().unwrap_or(0) {
            return Ok(false);
        }
    }
    Ok(true)
}

fn unsigned_pauli(pauli: &PauliString) -> PauliString {
    pauli.with_sign(PauliSign::Plus)
}
