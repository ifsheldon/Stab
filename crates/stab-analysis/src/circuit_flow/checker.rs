use std::collections::BTreeSet;

use stab_algebra::{
    Flow, FlowMeasurementIndex, PauliBasis, PauliSign, PauliString, StabilizerError, Tableau,
};
use stab_model::advanced::{CancellationCountdown, FoldedCircuitEvent};
use stab_model::{
    Cancellation, Circuit, CircuitItem, DemTarget, GateArguments, GateCategory, MemoryBudget,
    ObservableId, ParseLimits, QubitId, Target,
};

use super::check_algorithm::{FlowCheckAlgorithm, FlowCheckCost};
use super::verdict::{FlowVerdict, FlowViolation};
use crate::{
    AnalysisError, AnalysisOperation, AnalysisResult, CircuitToTableauOptions, ResourceKind,
    ResourceLimitError, circuit_to_tableau,
    folded_walk::walk_folded,
    sparse_rev_frame_tracker::{AnticommutationPolicy, SparseReverseFrameTracker},
};

/// Options for [`check_unsigned_stabilizer_flows`].
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

/// The exclusive upper bound of the qubits a circuit target, and so the sparse tracker, can name.
const TRACKED_QUBIT_LIMIT: usize = stab_model::QubitId::MAX.get() as usize + 1;

/// Checks unsigned stabilizer flows, returning one verdict per flow in order.
///
/// Signs are ignored. Pauli-only flows of a measurement-free, repeat-free circuit are checked
/// either against one dense Tableau or with the sparse reverse tracker, whichever an internal cost
/// model estimates to be cheaper; both return the same verdicts. Every other query uses the sparse
/// tracker, which folds repeat blocks.
///
/// The violation kind depends only on the query, never on the algorithm that checked it. A
/// Pauli-only flow of a circuit whose instructions are unitaries, annotations, or noise is
/// violated with [`FlowViolation::OutputMismatch`], carrying the circuit's image of the flow input
/// as wide as the widest of the circuit, the input, and the output. Every other Pauli mismatch is
/// [`FlowViolation::InputMismatch`]. A flow measurement term naming a record the circuit does not
/// have is [`FlowViolation::MeasurementRecordOutOfRange`], and a collapse that anticommutes with
/// the tracked region is [`FlowViolation::CollapseAnticommutation`]. Every flow of a valid circuit
/// is decided, however wide: no circuit target names a qubit beyond [`stab_model::QubitId::MAX`],
/// so the circuit is the identity there and the reported mismatch strings carry the flow's own
/// terms at those qubits.
///
/// # Errors
///
/// Returns [`AnalysisError::Cancelled`] once the options' signal is observed, never a partial
/// result, and [`AnalysisError::ResourceLimit`] when the sparse tracker's live sensitivity sets or
/// an extracted or reported Pauli string exceeds the options' memory budget. A circuit without well-defined
/// flow semantics is an error for the whole batch: an anti-Hermitian `SPP`, `SPP_DAG`, or `MPP`
/// product is [`AnalysisError::AntiHermitianPauliProduct`], and a circuit measurement-record
/// target that looks back before the first measurement is
/// [`AnalysisError::MeasurementRecordOutOfRange`]. Counts the circuit implies that do not fit are
/// reported as count overflows or model errors.
pub fn check_unsigned_stabilizer_flows(
    circuit: &Circuit,
    flows: &[Flow],
    options: &UnsignedFlowCheckOptions,
) -> AnalysisResult<Vec<FlowVerdict>> {
    options.cancellation.check()?;
    if flows.is_empty() {
        return Ok(Vec::new());
    }
    flow_verdicts_using(
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
    let circuit_qubits = circuit.simulated_qubit_count();
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
/// the circuit cannot be converted, so either algorithm returns the same verdicts for any query.
pub(crate) fn flow_verdicts_using(
    algorithm: FlowCheckAlgorithm,
    circuit: &Circuit,
    flows: &[Flow],
    options: &UnsignedFlowCheckOptions,
) -> AnalysisResult<Vec<FlowVerdict>> {
    if algorithm == FlowCheckAlgorithm::DenseTableau
        && let Some(tableau) = dense_flow_tableau(circuit, flows, options)?
    {
        return dense_flow_verdicts(&tableau, flows, options);
    }
    let verdicts = sparse_flow_verdicts(circuit, flows, options)?;
    if circuit_acts_unitarily_on_pauli_flows(circuit) {
        report_unitary_mismatches_as_outputs(circuit, flows, verdicts, options)
    } else {
        Ok(verdicts)
    }
}

/// Reports whether a failure is the caller's signal or budget rather than a property of the query.
pub(crate) fn stops_flow_check(error: &AnalysisError) -> bool {
    matches!(
        error,
        AnalysisError::Cancelled(_) | AnalysisError::ResourceLimit(_)
    )
}

/// Builds the dense Tableau when it can check every flow. Noise is skipped because unsigned flow
/// checking ignores it, exactly as the sparse tracker does. A Tableau that cannot be built, for
/// example because the budget refuses it, yields `None` so the caller falls back to the sparse
/// tracker, which reports any failure that belongs to the query; only cancellation stops here.
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
    match circuit_to_tableau(circuit, &tableau_options) {
        Ok(tableau) => Ok(Some(tableau)),
        Err(AnalysisError::Cancelled(cancelled)) => Err(cancelled.into()),
        Err(_) => Ok(None),
    }
}

fn dense_flow_verdicts(
    tableau: &Tableau,
    flows: &[Flow],
    options: &UnsignedFlowCheckOptions,
) -> AnalysisResult<Vec<FlowVerdict>> {
    flows
        .iter()
        .map(|flow| {
            options.cancellation.check()?;
            let actual = dense_flow_image(tableau, flow.input(), options)?;
            Ok(if paulis_match_unsigned(&actual, flow.output()) {
                FlowVerdict::Holds
            } else {
                FlowVerdict::Violated(FlowViolation::OutputMismatch {
                    expected_output: unsigned_pauli(flow.output()),
                    actual_output: unsigned_pauli(&actual),
                })
            })
        })
        .collect()
}

/// Applies the Tableau to a flow input no wider than it, padding the input with identities.
///
/// The dense algorithm only runs on flows no wider than the circuit, so the only failure a caller
/// can cause is a padded input the budget refuses.
fn dense_flow_image(
    tableau: &Tableau,
    input: &PauliString,
    options: &UnsignedFlowCheckOptions,
) -> AnalysisResult<PauliString> {
    let image = if input.len() == tableau.len() {
        tableau.apply(input)
    } else {
        let padded = match PauliString::from_bases_with_budget(
            PauliSign::Plus,
            (0..tableau.len()).map(|index| input.get(index).unwrap_or(PauliBasis::I)),
            options.memory_budget,
        ) {
            Ok(padded) => padded,
            Err(StabilizerError::MemoryBudgetExceeded { exceeded, .. }) => {
                return Err(ResourceLimitError::budget_refusal(
                    AnalysisOperation::FlowCheck,
                    ResourceKind::MaterializedBytes,
                    exceeded,
                )
                .into());
            }
            Err(error) => return Err(dense_image_invariant(&error)),
        };
        tableau.apply(&padded)
    };
    image.map_err(|error| dense_image_invariant(&error))
}

fn dense_image_invariant(error: &StabilizerError) -> AnalysisError {
    AnalysisError::internal(
        AnalysisOperation::FlowCheck,
        format!("the circuit Tableau could not act on a flow input no wider than it: {error}"),
    )
}

/// Rewrites the input mismatches of Pauli-only flows of a unitary circuit as output mismatches, so
/// the sparse tracker reports what the dense Tableau reports. The circuit's images of the failing
/// inputs are found by tracking them backwards through the inverse of the circuit's unitary part,
/// so this costs one more sparse pass over the failing flows only.
fn report_unitary_mismatches_as_outputs(
    circuit: &Circuit,
    flows: &[Flow],
    mut verdicts: Vec<FlowVerdict>,
    options: &UnsignedFlowCheckOptions,
) -> AnalysisResult<Vec<FlowVerdict>> {
    let failing = flows
        .iter()
        .zip(&verdicts)
        .enumerate()
        .filter(|(_, (flow, verdict))| {
            is_pauli_only_flow(flow)
                && matches!(
                    verdict,
                    FlowVerdict::Violated(FlowViolation::InputMismatch { .. })
                )
        })
        .map(|(index, (flow, _))| (index, flow))
        .collect::<Vec<_>>();
    if failing.is_empty() {
        return Ok(verdicts);
    }
    let inputs = failing
        .iter()
        .map(|(_, flow)| flow.input().clone())
        .collect::<Vec<_>>();
    // The circuit already passed the sparse tracker and its unitary part inverts, so a failure
    // other than the caller's signal or budget is a bug.
    let images = crate::circuit_inverse::unitary_part_inverse(circuit)
        .and_then(|inverse| pull_back_paulis_with_sparse_tracker(&inverse, &inputs, options))
        .map_err(|error| {
            if stops_flow_check(&error) {
                error
            } else {
                AnalysisError::internal(
                    AnalysisOperation::FlowCheck,
                    format!(
                        "could not compute the circuit's image of a failing flow input: {error}"
                    ),
                )
            }
        })?;
    if images.len() != failing.len() {
        return Err(AnalysisError::internal(
            AnalysisOperation::FlowCheck,
            "sparse reverse tracking returned fewer images than flows",
        ));
    }
    let circuit_qubits = circuit.simulated_qubit_count();
    for ((index, flow), image) in failing.into_iter().zip(images) {
        let width = circuit_qubits
            .max(flow.input().len())
            .max(flow.output().len());
        // Beyond the tracked range the circuit is the identity, so the input is its own image.
        let actual_output = region_with_idle_tail(&image, flow.input(), width, options)?;
        let verdict = verdicts.get_mut(index).ok_or_else(|| {
            AnalysisError::internal(
                AnalysisOperation::FlowCheck,
                "flow check lost a failing flow's verdict",
            )
        })?;
        *verdict = FlowVerdict::Violated(FlowViolation::OutputMismatch {
            expected_output: unsigned_pauli(flow.output()),
            actual_output,
        });
    }
    Ok(verdicts)
}

/// Tracks each Pauli string backwards through `circuit` and returns the Pauli strings it pulls
/// back to, unsigned and only as wide as their support.
fn pull_back_paulis_with_sparse_tracker(
    circuit: &Circuit,
    paulis: &[PauliString],
    options: &UnsignedFlowCheckOptions,
) -> AnalysisResult<Vec<PauliString>> {
    let qubit_count = paulis
        .iter()
        .map(PauliString::len)
        .fold(circuit.simulated_qubit_count(), usize::max);
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
        seed_flow_pauli_output(&mut tracker, pauli, *target, &options.cancellation)?;
        tracker.check_live_budget()?;
    }
    tracker.undo_circuit(circuit)?;
    targets
        .into_iter()
        .map(|target| {
            tracker.check_cancelled()?;
            Ok(tracker
                .compact_region_for_target(target, options.memory_budget)?
                .value()
                .clone())
        })
        .collect()
}

/// Checks flows with the sparse reverse tracker alone.
///
/// Every Pauli mismatch is reported as [`FlowViolation::InputMismatch`]; only the public check
/// rewrites the mismatches of a unitary circuit as output mismatches.
pub(crate) fn sparse_flow_verdicts(
    circuit: &Circuit,
    flows: &[Flow],
    options: &UnsignedFlowCheckOptions,
) -> AnalysisResult<Vec<FlowVerdict>> {
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
    let qubit_count = circuit.simulated_qubit_count().max(flow_qubit_count);
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
    // A flow decided before tracking keeps its verdict here and is not seeded.
    let mut decided = Vec::with_capacity(flows.len());
    for (flow, &tracked_target) in flows.iter().zip(&labels) {
        let record_indices = match flow_record_indices(flow, measurement_count) {
            Ok(record_indices) => record_indices,
            Err(record) => {
                decided.push(Some(FlowVerdict::Violated(
                    FlowViolation::MeasurementRecordOutOfRange {
                        record: FlowMeasurementIndex::new(record),
                        measurement_count,
                    },
                )));
                continue;
            }
        };
        seed_flow_pauli_output(
            &mut tracker,
            flow.output(),
            tracked_target,
            &options.cancellation,
        )?;
        for record_index in record_indices {
            tracker.toggle_record_target_absolute(record_index, tracked_target)?;
        }
        for observable in flow.observables() {
            tracker.toggle_observable_effect(ObservableId::from(observable), tracked_target)?;
        }
        tracker.check_live_budget()?;
        decided.push(None);
    }
    tracker.undo_circuit(circuit)?;

    flows
        .iter()
        .zip(decided)
        .zip(labels)
        .map(|((flow, decided), tracked_target)| {
            if let Some(verdict) = decided {
                return Ok(verdict);
            }
            tracker.check_cancelled()?;
            if tracker.target_anticommuted(tracked_target) {
                return Ok(FlowVerdict::Violated(
                    FlowViolation::CollapseAnticommutation,
                ));
            }
            let region =
                tracker.compact_region_for_target(tracked_target, options.memory_budget)?;
            // Beyond the tracked range every circuit is the identity, so the output pulls back
            // to itself there.
            let actual =
                region_with_idle_tail(region.value(), flow.output(), flow.input().len(), options)?;
            if paulis_match_unsigned_polled(&actual, flow.input(), &options.cancellation)? {
                Ok(FlowVerdict::Holds)
            } else {
                Ok(FlowVerdict::Violated(FlowViolation::InputMismatch {
                    expected_input: unsigned_pauli(flow.input()),
                    actual_input: unsigned_pauli(&actual),
                }))
            }
        })
        .collect()
}

/// Builds the unsigned Pauli string a tracked region stands for, at least `width` qubits wide: the
/// region on the qubits a circuit target can name, and `idle` itself at and beyond
/// [`TRACKED_QUBIT_LIMIT`], where every circuit acts as the identity. The string widens to `idle`
/// when `idle` reaches past that limit. Its packed planes are admitted against the options'
/// budget before they are allocated, and the options' signal is polled once per copied word.
fn region_with_idle_tail(
    region: &PauliString,
    idle: &PauliString,
    width: usize,
    options: &UnsignedFlowCheckOptions,
) -> AnalysisResult<PauliString> {
    let mut width = width.max(region.len());
    if idle.len() > TRACKED_QUBIT_LIMIT {
        width = width.max(idle.len());
    }
    let mut result = PauliString::identity_with_budget(width, options.memory_budget).map_err(
        |error| match error {
            StabilizerError::MemoryBudgetExceeded { exceeded, .. } => {
                ResourceLimitError::budget_refusal(
                    AnalysisOperation::FlowCheck,
                    ResourceKind::MaterializedBytes,
                    exceeded,
                )
                .into()
            }
            error => AnalysisError::allocation_failed(
                AnalysisOperation::FlowCheck,
                format!("could not allocate a {width}-qubit flow Pauli string: {error}"),
            ),
        },
    )?;
    let mut countdown = CancellationCountdown::new();
    let tail_word = TRACKED_QUBIT_LIMIT / WORD_BITS;
    for (source, words) in [(region, 0..tail_word), (idle, tail_word..usize::MAX)] {
        visit_terms(
            source,
            words,
            &options.cancellation,
            &mut countdown,
            |index, basis| {
                let len = result.len();
                result.set(index, basis).map_err(|error| {
                    AnalysisError::internal(
                        AnalysisOperation::FlowCheck,
                        format!(
                            "flow Pauli term {index} is outside its {len}-qubit string: {error}"
                        ),
                    )
                })
            },
        )?;
    }
    Ok(result)
}

const WORD_BITS: usize = u64::BITS as usize;
const _: () = assert!(TRACKED_QUBIT_LIMIT.is_multiple_of(WORD_BITS));

/// Calls `visit` with the index and basis of each term of `pauli` in the packed words `words`,
/// in increasing order, polling `cancellation` once per word.
fn visit_terms(
    pauli: &PauliString,
    words: std::ops::Range<usize>,
    cancellation: &Cancellation,
    countdown: &mut CancellationCountdown,
    mut visit: impl FnMut(usize, PauliBasis) -> AnalysisResult<()>,
) -> AnalysisResult<()> {
    let (xs, zs) = (pauli.x_bits(), pauli.z_bits());
    let end = words.end.min(xs.len().max(zs.len()));
    for word in words.start..end {
        countdown.tick(cancellation)?;
        let (x_word, z_word) = (
            xs.get(word).copied().unwrap_or(0),
            zs.get(word).copied().unwrap_or(0),
        );
        let mut support = x_word | z_word;
        while support != 0 {
            let bit = support.trailing_zeros();
            support &= support - 1;
            let basis = PauliBasis::from_xz((x_word >> bit) & 1 == 1, (z_word >> bit) & 1 == 1);
            visit(word * WORD_BITS + bit as usize, basis)?;
        }
    }
    Ok(())
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
                && let GateArguments::Observable(observable) = instruction.gate_arguments()
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
    cancellation: &Cancellation,
) -> AnalysisResult<()> {
    // Qubits at and beyond the tracked range are idle in every circuit and are not tracked.
    let mut countdown = CancellationCountdown::new();
    visit_terms(
        output,
        0..TRACKED_QUBIT_LIMIT / WORD_BITS,
        cancellation,
        &mut countdown,
        |index, basis| {
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
            tracker.toggle_pauli_target(qubit, basis, target)
        },
    )
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
