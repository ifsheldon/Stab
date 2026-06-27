use std::collections::{BTreeMap, BTreeSet};

mod clifford;
mod declarations;
mod decompose;
mod effects;
mod error_decomp;
mod feedback;
mod folded;
mod gauge;
mod instructions;
mod measurements;
mod mpp;
mod probabilities;

use crate::{
    Circuit, CircuitError, CircuitInstruction, CircuitItem, CircuitResult, Probability, QubitId,
    RepeatBlock, SingleQubitClifford,
};

use super::{DemInstruction, DemRepeatBlock, DemTarget, DetectorErrorModel};
use declarations::Declaration;
use decompose::decompose_tagged_error_probabilities;
use effects::{
    AnalyzerBasis, AnalyzerPauli, NoiseEffect, ObservableSensitivity, PendingError,
    PendingSingleQubitPauliChannel, analyzer_pauli_from_mask, pauli_mask,
};
pub use error_decomp::{
    DisjointPauliProbabilities, IndependentPauliProbabilities, independent_to_disjoint_xyz_errors,
    try_disjoint_to_independent_xyz_errors,
};
use error_decomp::{
    depolarize1_independent_channel_probability, depolarize2_independent_channel_probability,
    pauli_channel2_components,
};
use folded::FoldedAnalyzer;
use gauge::find_gauge_errors;
use instructions::{is_measurement_instruction, is_noise_instruction, pair_measurement_basis};
use probabilities::{merge_disjoint_probability, merge_independent_probability, toggle_all};

const MAX_ANALYZER_REPEAT_UNROLL: u64 = 100_000;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ErrorAnalyzerOptions {
    pub fold_loops: bool,
    pub decompose_errors: bool,
    pub allow_gauge_detectors: bool,
    pub ignore_decomposition_failures: bool,
    pub block_decomposition_from_introducing_remnant_edges: bool,
    pub approximate_disjoint_errors_threshold: Option<Probability>,
}

pub fn circuit_to_detector_error_model(
    circuit: &Circuit,
    options: ErrorAnalyzerOptions,
) -> CircuitResult<DetectorErrorModel> {
    if options.fold_loops
        && circuit
            .items()
            .iter()
            .any(|item| matches!(item, CircuitItem::RepeatBlock(_)))
    {
        return FoldedAnalyzer::new(options).analyze(circuit);
    }
    Analyzer::new(options).analyze(circuit)
}

struct AnalyzerResult {
    dem: DetectorErrorModel,
    detector_count: u64,
}

struct Analyzer {
    options: ErrorAnalyzerOptions,
    measurement_count: usize,
    detector_count: u64,
    pending_errors: Vec<PendingError>,
    pending_pauli_channels: Vec<PendingSingleQubitPauliChannel>,
    else_correlated_error_remainder: Option<Probability>,
    next_disjoint_group_id: u64,
    completed_errors: Vec<PendingError>,
    gauge_errors: Vec<Vec<DemTarget>>,
    detector_terms_by_measurement: BTreeMap<usize, Vec<u64>>,
    observable_terms_by_measurement: BTreeMap<usize, Vec<u64>>,
    observable_sensitivity: ObservableSensitivity,
    declarations: Vec<Declaration>,
}

impl Analyzer {
    fn new(options: ErrorAnalyzerOptions) -> Self {
        Self {
            options,
            measurement_count: 0,
            detector_count: 0,
            pending_errors: Vec::new(),
            pending_pauli_channels: Vec::new(),
            else_correlated_error_remainder: None,
            next_disjoint_group_id: 0,
            completed_errors: Vec::new(),
            gauge_errors: Vec::new(),
            detector_terms_by_measurement: BTreeMap::new(),
            observable_terms_by_measurement: BTreeMap::new(),
            observable_sensitivity: ObservableSensitivity::default(),
            declarations: Vec::new(),
        }
    }

    fn analyze(self, circuit: &Circuit) -> CircuitResult<DetectorErrorModel> {
        self.analyze_with_stats(circuit).map(|result| result.dem)
    }

    fn analyze_with_stats(mut self, circuit: &Circuit) -> CircuitResult<AnalyzerResult> {
        self.visit_circuit(circuit)?;
        self.gauge_errors = find_gauge_errors(
            circuit,
            &self.detector_terms_by_measurement,
            &self.observable_terms_by_measurement,
            self.measurement_count,
            circuit.count_qubits(),
            self.options.allow_gauge_detectors,
        )?;
        let detector_count = self.detector_count;
        let dem = self.into_dem()?;
        Ok(AnalyzerResult {
            dem,
            detector_count,
        })
    }

    fn visit_circuit(&mut self, circuit: &Circuit) -> CircuitResult<()> {
        for item in circuit.items() {
            match item {
                CircuitItem::Instruction(instruction) => self.visit_instruction(instruction)?,
                CircuitItem::RepeatBlock(repeat) => {
                    self.end_else_correlated_error_block();
                    self.visit_repeat(repeat)?;
                    self.end_else_correlated_error_block();
                }
            }
        }
        Ok(())
    }

    fn visit_repeat(&mut self, repeat: &RepeatBlock) -> CircuitResult<()> {
        let repeat_count = repeat.repeat_count().get();
        if self.options.fold_loops {
            return Err(CircuitError::invalid_detector_error_model(
                "analyze_errors --fold_loops is not implemented for repeated circuits yet",
            ));
        }
        if repeat_count > MAX_ANALYZER_REPEAT_UNROLL {
            return Err(CircuitError::invalid_detector_error_model(format!(
                "analyze_errors currently supports repeat counts up to {MAX_ANALYZER_REPEAT_UNROLL}, got {repeat_count}"
            )));
        }
        for _ in 0..repeat_count {
            self.end_else_correlated_error_block();
            self.visit_circuit(repeat.body())?;
            self.end_else_correlated_error_block();
        }
        Ok(())
    }

    fn visit_instruction(&mut self, instruction: &CircuitInstruction) -> CircuitResult<()> {
        let gate_name = instruction.gate().canonical_name();
        if !matches!(
            gate_name,
            "E" | "CORRELATED_ERROR" | "ELSE_CORRELATED_ERROR"
        ) {
            self.end_else_correlated_error_block();
        }
        match gate_name {
            "X_ERROR" | "Y_ERROR" | "Z_ERROR" => self.record_single_pauli_error(instruction),
            "E" | "CORRELATED_ERROR" => self.record_correlated_error(instruction),
            "ELSE_CORRELATED_ERROR" => self.record_else_correlated_error(instruction),
            "I_ERROR" | "II_ERROR" => Ok(()),
            "PAULI_CHANNEL_1" => self.record_pauli_channel1(instruction),
            "PAULI_CHANNEL_2" => self.record_pauli_channel2(instruction),
            "DEPOLARIZE1" => self.record_depolarize1(instruction),
            "DEPOLARIZE2" => self.record_depolarize2(instruction),
            "M" | "MX" | "MY" | "MR" | "MRX" | "MRY" => self.record_measurements(instruction),
            "MXX" | "MYY" | "MZZ" => self.record_pair_measurements(instruction),
            "MPP" => self.record_pauli_product_measurements(instruction),
            "R" | "RX" | "RY" => self.record_resets(instruction),
            "HERALDED_ERASE" => self.record_heralded_erase(instruction),
            "HERALDED_PAULI_CHANNEL_1" => self.record_heralded_pauli_channel1(instruction),
            "CX" | "CY" | "CZ" | "XCX" | "XCY" | "XCZ" | "YCX" | "YCY" | "YCZ" => {
                self.apply_controlled_pauli(instruction)
            }
            "SWAP" => self.apply_swap(instruction),
            "ISWAP" | "ISWAP_DAG" | "CXSWAP" | "SWAPCX" | "CZSWAP" | "SQRT_XX" | "SQRT_XX_DAG"
            | "SQRT_YY" | "SQRT_YY_DAG" | "SQRT_ZZ" | "SQRT_ZZ_DAG" => {
                self.apply_two_qubit_clifford(instruction)
            }
            "MPAD" => self.record_measurement_pads(instruction),
            "DETECTOR" => self.record_detector(instruction),
            "OBSERVABLE_INCLUDE" => self.record_observable(instruction),
            "SHIFT_COORDS" => self.shift_coordinates(instruction),
            "TICK" | "QUBIT_COORDS" => Ok(()),
            name => {
                if let Ok(clifford) = SingleQubitClifford::from_gate(instruction.gate()) {
                    self.apply_single_qubit_clifford(instruction, clifford)
                } else if is_noise_instruction(name) {
                    Err(CircuitError::invalid_detector_error_model(format!(
                        "analyze_errors does not yet support {name}"
                    )))
                } else if is_measurement_instruction(name) {
                    Err(CircuitError::invalid_detector_error_model(format!(
                        "analyze_errors does not yet support measurement instruction {name}"
                    )))
                } else {
                    Ok(())
                }
            }
        }
    }

    fn record_single_pauli_error(&mut self, instruction: &CircuitInstruction) -> CircuitResult<()> {
        let Some(probability) = instruction.probability_argument()? else {
            return Ok(());
        };
        if probability.get() == 0.0 {
            return Ok(());
        }
        let pauli = match instruction.gate().canonical_name() {
            "X_ERROR" => AnalyzerPauli::X,
            "Y_ERROR" => AnalyzerPauli::Y,
            "Z_ERROR" => AnalyzerPauli::Z,
            _ => unreachable!("caller restricts gate names"),
        };
        for target in instruction.targets() {
            let Some(qubit) = target.qubit_id() else {
                return Err(CircuitError::invalid_detector_error_model(format!(
                    "{} target {target} is not a qubit",
                    instruction.gate().canonical_name()
                )));
            };
            self.push_single_qubit_pauli_error(
                probability,
                qubit,
                pauli,
                instruction.tag().map(str::to_owned),
            );
        }
        Ok(())
    }

    fn end_else_correlated_error_block(&mut self) {
        self.else_correlated_error_remainder = None;
    }

    fn record_correlated_error(&mut self, instruction: &CircuitInstruction) -> CircuitResult<()> {
        let Some(probability) = instruction.probability_argument()? else {
            return Ok(());
        };
        self.record_correlated_error_with_probability(instruction, probability)?;
        self.else_correlated_error_remainder = Some(Probability::try_new(1.0 - probability.get())?);
        Ok(())
    }

    fn record_else_correlated_error(
        &mut self,
        instruction: &CircuitInstruction,
    ) -> CircuitResult<()> {
        let Some(threshold) = self.options.approximate_disjoint_errors_threshold else {
            return Err(CircuitError::invalid_detector_error_model(
                "ELSE_CORRELATED_ERROR requires approximate_disjoint_errors during error analysis",
            ));
        };
        let Some(probability) = instruction.probability_argument()? else {
            return Ok(());
        };
        let Some(remainder) = self.else_correlated_error_remainder else {
            return Err(CircuitError::invalid_detector_error_model(
                "ELSE_CORRELATED_ERROR must immediately follow CORRELATED_ERROR or ELSE_CORRELATED_ERROR",
            ));
        };
        let actual_probability = Probability::try_new(remainder.get() * probability.get())?;
        if actual_probability.get() > threshold.get() {
            return Err(CircuitError::invalid_detector_error_model(format!(
                "CORRELATED_ERROR/ELSE_CORRELATED_ERROR block has a component probability {} larger than the approximate_disjoint_errors threshold {}",
                actual_probability.get(),
                threshold.get()
            )));
        }
        self.record_correlated_error_with_probability(instruction, actual_probability)?;
        self.else_correlated_error_remainder = Some(Probability::try_new(
            remainder.get() * (1.0 - probability.get()),
        )?);
        Ok(())
    }

    fn record_correlated_error_with_probability(
        &mut self,
        instruction: &CircuitInstruction,
        probability: Probability,
    ) -> CircuitResult<()> {
        if probability.get() == 0.0 {
            return Ok(());
        }
        let mut effects_by_qubit = BTreeMap::new();
        for target in instruction.targets() {
            let Some(pauli) = target.pauli_type() else {
                return Err(CircuitError::invalid_detector_error_model(format!(
                    "E target {target} is not a Pauli target"
                )));
            };
            let Some(qubit) = target.qubit_id() else {
                return Err(CircuitError::invalid_detector_error_model(format!(
                    "E target {target} does not identify a qubit"
                )));
            };
            let entry = effects_by_qubit.entry(qubit).or_insert(0);
            *entry ^= pauli_mask(pauli);
            if *entry == 0 {
                effects_by_qubit.remove(&qubit);
            }
        }
        if effects_by_qubit.is_empty() {
            return Ok(());
        }
        let effects = effects_by_qubit
            .into_iter()
            .map(|(qubit, mask)| NoiseEffect {
                qubit,
                pauli: analyzer_pauli_from_mask(mask),
            })
            .collect();
        self.push_pending_error(
            probability,
            effects,
            Vec::new(),
            None,
            instruction.tag().map(str::to_owned),
        );
        Ok(())
    }

    fn record_pauli_channel1(&mut self, instruction: &CircuitInstruction) -> CircuitResult<()> {
        let Some(probabilities) = instruction.probability_arguments()? else {
            return Ok(());
        };
        let [x_probability, y_probability, z_probability] = probabilities.as_slice() else {
            return Err(CircuitError::invalid_detector_error_model(
                "PAULI_CHANNEL_1 expected three probabilities",
            ));
        };
        if let Some(independent) =
            try_disjoint_to_independent_xyz_errors(*x_probability, *y_probability, *z_probability)?
        {
            for target in instruction.targets() {
                let Some(qubit) = target.qubit_id() else {
                    return Err(CircuitError::invalid_detector_error_model(format!(
                        "PAULI_CHANNEL_1 target {target} is not a qubit"
                    )));
                };
                let tag = instruction.tag().map(str::to_owned);
                self.push_single_qubit_pauli_error(
                    independent.x,
                    qubit,
                    AnalyzerPauli::X,
                    tag.clone(),
                );
                self.push_single_qubit_pauli_error(
                    independent.y,
                    qubit,
                    AnalyzerPauli::Y,
                    tag.clone(),
                );
                self.push_single_qubit_pauli_error(independent.z, qubit, AnalyzerPauli::Z, tag);
            }
            return Ok(());
        }
        let Some(threshold) = self.options.approximate_disjoint_errors_threshold else {
            return Err(CircuitError::invalid_detector_error_model(
                "PAULI_CHANNEL_1 requires approximate_disjoint_errors during error analysis",
            ));
        };
        for probability in &probabilities {
            if probability.get() > threshold.get() {
                return Err(CircuitError::invalid_detector_error_model(format!(
                    "PAULI_CHANNEL_1 has a probability argument ({}) larger than the approximate_disjoint_errors threshold ({})",
                    probability.get(),
                    threshold.get()
                )));
            }
        }
        for target in instruction.targets() {
            let Some(qubit) = target.qubit_id() else {
                return Err(CircuitError::invalid_detector_error_model(format!(
                    "PAULI_CHANNEL_1 target {target} is not a qubit"
                )));
            };
            self.pending_pauli_channels
                .push(PendingSingleQubitPauliChannel {
                    qubit,
                    x_probability: *x_probability,
                    y_probability: *y_probability,
                    z_probability: *z_probability,
                    tag: instruction.tag().map(str::to_owned),
                });
        }
        Ok(())
    }

    fn push_single_qubit_pauli_error(
        &mut self,
        probability: Probability,
        qubit: QubitId,
        pauli: AnalyzerPauli,
        tag: Option<String>,
    ) {
        if probability.get() == 0.0 {
            return;
        }
        self.push_pending_error(
            probability,
            vec![NoiseEffect { qubit, pauli }],
            Vec::new(),
            None,
            tag,
        );
    }

    fn record_pauli_channel2(&mut self, instruction: &CircuitInstruction) -> CircuitResult<()> {
        let Some(threshold) = self.options.approximate_disjoint_errors_threshold else {
            return Err(CircuitError::invalid_detector_error_model(
                "PAULI_CHANNEL_2 requires approximate_disjoint_errors during error analysis",
            ));
        };
        let Some(probabilities) = instruction.probability_arguments()? else {
            return Ok(());
        };
        let probabilities: [Probability; 15] =
            probabilities.try_into().map_err(|probabilities: Vec<_>| {
                CircuitError::invalid_detector_error_model(format!(
                    "PAULI_CHANNEL_2 expected 15 probabilities, got {}",
                    probabilities.len()
                ))
            })?;
        for probability in &probabilities {
            if probability.get() > threshold.get() {
                return Err(CircuitError::invalid_detector_error_model(format!(
                    "PAULI_CHANNEL_2 has a probability argument ({}) larger than the approximate_disjoint_errors threshold ({})",
                    probability.get(),
                    threshold.get()
                )));
            }
        }
        for group in instruction.target_groups() {
            let [left, right] = group else {
                return Err(CircuitError::invalid_detector_error_model(
                    "PAULI_CHANNEL_2 expected paired qubit targets",
                ));
            };
            let Some(left_qubit) = left.qubit_id() else {
                return Err(CircuitError::invalid_detector_error_model(format!(
                    "PAULI_CHANNEL_2 target {left} is not a qubit"
                )));
            };
            let Some(right_qubit) = right.qubit_id() else {
                return Err(CircuitError::invalid_detector_error_model(format!(
                    "PAULI_CHANNEL_2 target {right} is not a qubit"
                )));
            };
            let group_id = self.allocate_disjoint_group_id()?;
            for (probability, left_pauli, right_pauli) in pauli_channel2_components(probabilities) {
                if probability.get() == 0.0 {
                    continue;
                }
                self.push_two_qubit_pauli_error(
                    probability,
                    (left_qubit, left_pauli),
                    (right_qubit, right_pauli),
                    Some(group_id),
                    instruction.tag().map(str::to_owned),
                );
            }
        }
        Ok(())
    }

    fn allocate_disjoint_group_id(&mut self) -> CircuitResult<u64> {
        let group_id = self.next_disjoint_group_id;
        self.next_disjoint_group_id =
            self.next_disjoint_group_id.checked_add(1).ok_or_else(|| {
                CircuitError::invalid_detector_error_model("disjoint error group count overflowed")
            })?;
        Ok(group_id)
    }

    fn record_depolarize2(&mut self, instruction: &CircuitInstruction) -> CircuitResult<()> {
        let Some(probability) = instruction.probability_argument()? else {
            return Ok(());
        };
        if probability.get() == 0.0 {
            return Ok(());
        }
        let channel_probability = depolarize2_independent_channel_probability(probability)?;
        for group in instruction.target_groups() {
            let [left, right] = group else {
                return Err(CircuitError::invalid_detector_error_model(
                    "DEPOLARIZE2 expected paired qubit targets",
                ));
            };
            let Some(left_qubit) = left.qubit_id() else {
                return Err(CircuitError::invalid_detector_error_model(format!(
                    "DEPOLARIZE2 target {left} is not a qubit"
                )));
            };
            let Some(right_qubit) = right.qubit_id() else {
                return Err(CircuitError::invalid_detector_error_model(format!(
                    "DEPOLARIZE2 target {right} is not a qubit"
                )));
            };
            let paulis = [AnalyzerPauli::X, AnalyzerPauli::Y, AnalyzerPauli::Z];
            for left_pauli in [None, Some(paulis[0]), Some(paulis[1]), Some(paulis[2])] {
                for right_pauli in [None, Some(paulis[0]), Some(paulis[1]), Some(paulis[2])] {
                    if left_pauli.is_none() && right_pauli.is_none() {
                        continue;
                    }
                    self.push_two_qubit_pauli_error(
                        channel_probability,
                        (left_qubit, left_pauli),
                        (right_qubit, right_pauli),
                        None,
                        instruction.tag().map(str::to_owned),
                    );
                }
            }
        }
        Ok(())
    }

    fn push_two_qubit_pauli_error(
        &mut self,
        probability: Probability,
        left: (QubitId, Option<AnalyzerPauli>),
        right: (QubitId, Option<AnalyzerPauli>),
        disjoint_group: Option<u64>,
        tag: Option<String>,
    ) {
        let mut effects = Vec::new();
        let (left_qubit, left_pauli) = left;
        if let Some(pauli) = left_pauli {
            effects.push(NoiseEffect {
                qubit: left_qubit,
                pauli,
            });
        }
        let (right_qubit, right_pauli) = right;
        if let Some(pauli) = right_pauli {
            effects.push(NoiseEffect {
                qubit: right_qubit,
                pauli,
            });
        }
        self.push_pending_error(probability, effects, Vec::new(), disjoint_group, tag);
    }

    fn push_pending_error(
        &mut self,
        probability: Probability,
        effects: Vec<NoiseEffect>,
        measurements: Vec<usize>,
        disjoint_group: Option<u64>,
        tag: Option<String>,
    ) {
        let observables = self.observable_sensitivity.flipped_observables(&effects);
        self.pending_errors.push(PendingError {
            probability,
            effects,
            measurements,
            observables,
            disjoint_group,
            tag,
        });
    }

    fn record_depolarize1(&mut self, instruction: &CircuitInstruction) -> CircuitResult<()> {
        let Some(probability) = instruction.probability_argument()? else {
            return Ok(());
        };
        if probability.get() == 0.0 {
            return Ok(());
        }
        let channel_probability = depolarize1_independent_channel_probability(probability)?;
        for target in instruction.targets() {
            let Some(qubit) = target.qubit_id() else {
                return Err(CircuitError::invalid_detector_error_model(format!(
                    "DEPOLARIZE1 target {target} is not a qubit"
                )));
            };
            let tag = instruction.tag().map(str::to_owned);
            self.push_single_qubit_pauli_error(
                channel_probability,
                qubit,
                AnalyzerPauli::X,
                tag.clone(),
            );
            self.push_single_qubit_pauli_error(
                channel_probability,
                qubit,
                AnalyzerPauli::Y,
                tag.clone(),
            );
            self.push_single_qubit_pauli_error(channel_probability, qubit, AnalyzerPauli::Z, tag);
        }
        Ok(())
    }

    fn record_heralded_pauli_channel1(
        &mut self,
        instruction: &CircuitInstruction,
    ) -> CircuitResult<()> {
        let Some(probabilities) = instruction.probability_arguments()? else {
            return Ok(());
        };
        let [i_probability, x_probability, y_probability, z_probability] = probabilities.as_slice()
        else {
            return Err(CircuitError::invalid_detector_error_model(
                "HERALDED_PAULI_CHANNEL_1 expected four probabilities",
            ));
        };
        let non_zero_count = probabilities
            .iter()
            .filter(|probability| probability.get() > 0.0)
            .count();
        let threshold = if non_zero_count > 1 {
            let Some(threshold) = self.options.approximate_disjoint_errors_threshold else {
                return Err(CircuitError::invalid_detector_error_model(
                    "HERALDED_PAULI_CHANNEL_1 with multiple non-zero components requires approximate_disjoint_errors during error analysis",
                ));
            };
            for probability in &probabilities {
                if probability.get() > threshold.get() {
                    return Err(CircuitError::invalid_detector_error_model(format!(
                        "HERALDED_PAULI_CHANNEL_1 has a probability argument ({}) larger than the approximate_disjoint_errors threshold ({})",
                        probability.get(),
                        threshold.get()
                    )));
                }
            }
            Some(threshold)
        } else {
            None
        };

        self.record_heralded_pauli_components(
            instruction,
            [
                (*i_probability, None),
                (*x_probability, Some(AnalyzerPauli::X)),
                (*y_probability, Some(AnalyzerPauli::Y)),
                (*z_probability, Some(AnalyzerPauli::Z)),
            ],
            threshold.is_some(),
        )
    }

    fn record_heralded_erase(&mut self, instruction: &CircuitInstruction) -> CircuitResult<()> {
        let Some(probability) = instruction.probability_argument()? else {
            return Ok(());
        };
        let use_disjoint_group = if probability.get() > 0.0 {
            let Some(threshold) = self.options.approximate_disjoint_errors_threshold else {
                return Err(CircuitError::invalid_detector_error_model(
                    "HERALDED_ERASE requires approximate_disjoint_errors during error analysis",
                ));
            };
            if probability.get() > threshold.get() {
                return Err(CircuitError::invalid_detector_error_model(format!(
                    "HERALDED_ERASE has a probability argument ({}) larger than the approximate_disjoint_errors threshold ({})",
                    probability.get(),
                    threshold.get()
                )));
            }
            true
        } else {
            false
        };
        let component_probability = Probability::try_new(probability.get() / 4.0)?;
        self.record_heralded_pauli_components(
            instruction,
            [
                (component_probability, None),
                (component_probability, Some(AnalyzerPauli::X)),
                (component_probability, Some(AnalyzerPauli::Y)),
                (component_probability, Some(AnalyzerPauli::Z)),
            ],
            use_disjoint_group,
        )
    }

    fn record_heralded_pauli_components(
        &mut self,
        instruction: &CircuitInstruction,
        components: [(Probability, Option<AnalyzerPauli>); 4],
        use_disjoint_group: bool,
    ) -> CircuitResult<()> {
        for target in instruction.targets() {
            let Some(qubit) = target.qubit_id() else {
                return Err(CircuitError::invalid_detector_error_model(format!(
                    "{} target {target} is not a qubit",
                    instruction.gate().canonical_name()
                )));
            };
            let measurement_index = self.measurement_count;
            self.measurement_count = self.measurement_count.checked_add(1).ok_or_else(|| {
                CircuitError::invalid_detector_error_model("measurement count overflowed")
            })?;
            let disjoint_group = if use_disjoint_group {
                Some(self.allocate_disjoint_group_id()?)
            } else {
                None
            };
            for (probability, pauli) in components {
                if probability.get() == 0.0 {
                    continue;
                }
                let effects = pauli
                    .map(|pauli| NoiseEffect { qubit, pauli })
                    .into_iter()
                    .collect();
                self.push_pending_error(
                    probability,
                    effects,
                    vec![measurement_index],
                    disjoint_group,
                    instruction.tag().map(str::to_owned),
                );
            }
        }
        Ok(())
    }

    fn record_pair_measurements(&mut self, instruction: &CircuitInstruction) -> CircuitResult<()> {
        let basis =
            pair_measurement_basis(instruction.gate().canonical_name()).ok_or_else(|| {
                CircuitError::invalid_detector_error_model("unknown pair measurement basis")
            })?;
        for group in instruction.target_groups() {
            let [left, right] = group else {
                return Err(CircuitError::invalid_detector_error_model(format!(
                    "{} expected paired qubit targets",
                    instruction.gate().canonical_name()
                )));
            };
            let Some(left) = left.qubit_id() else {
                return Err(CircuitError::invalid_detector_error_model(format!(
                    "{} target {left} is not a qubit",
                    instruction.gate().canonical_name()
                )));
            };
            let Some(right) = right.qubit_id() else {
                return Err(CircuitError::invalid_detector_error_model(format!(
                    "{} target {right} is not a qubit",
                    instruction.gate().canonical_name()
                )));
            };
            self.reject_pending_single_qubit_channels_through_product_measurement(
                instruction,
                &[left, right],
            )?;
            let measurement_index = self.measurement_count;
            self.measurement_count = self.measurement_count.checked_add(1).ok_or_else(|| {
                CircuitError::invalid_detector_error_model("measurement count overflowed")
            })?;
            let terms = [(left, basis), (right, basis)];
            for pending in &mut self.pending_errors {
                if pending.flips_product_measurement(&terms) {
                    pending.measurements.push(measurement_index);
                }
            }
            if let Some(probability) = instruction.probability_argument()?
                && probability.get() > 0.0
            {
                self.completed_errors.push(PendingError {
                    probability,
                    effects: Vec::new(),
                    measurements: vec![measurement_index],
                    observables: Vec::new(),
                    disjoint_group: None,
                    tag: instruction.tag().map(str::to_owned),
                });
            }
        }
        Ok(())
    }

    fn reject_pending_single_qubit_channels_through_product_measurement(
        &self,
        instruction: &CircuitInstruction,
        qubits: &[QubitId],
    ) -> CircuitResult<()> {
        if self
            .pending_pauli_channels
            .iter()
            .any(|pending| qubits.contains(&pending.qubit))
        {
            return Err(CircuitError::invalid_detector_error_model(format!(
                "analyze_errors does not yet support propagating pending single-qubit Pauli channels through {}",
                instruction.gate().canonical_name()
            )));
        }
        Ok(())
    }

    fn record_measurement_pads(&mut self, instruction: &CircuitInstruction) -> CircuitResult<()> {
        self.measurement_count = self
            .measurement_count
            .checked_add(instruction.target_groups().len())
            .ok_or_else(|| {
                CircuitError::invalid_detector_error_model("measurement count overflowed")
            })?;
        Ok(())
    }

    fn record_resets(&mut self, instruction: &CircuitInstruction) -> CircuitResult<()> {
        for target in instruction.targets() {
            let Some(qubit) = target.qubit_id() else {
                return Err(CircuitError::invalid_detector_error_model(format!(
                    "{} target {target} is not a qubit",
                    instruction.gate().canonical_name()
                )));
            };
            self.cut_pending_errors_at_qubit(qubit);
            self.pending_pauli_channels
                .retain(|pending| pending.qubit != qubit);
        }
        Ok(())
    }

    fn apply_swap(&mut self, instruction: &CircuitInstruction) -> CircuitResult<()> {
        let gate_name = instruction.gate().canonical_name();
        for group in instruction.target_groups() {
            let [left, right] = group else {
                return Err(CircuitError::invalid_detector_error_model(format!(
                    "{gate_name} expected paired qubit targets"
                )));
            };
            let left = left.qubit_id().ok_or_else(|| {
                CircuitError::invalid_detector_error_model(format!(
                    "{gate_name} target {left} is not a qubit"
                ))
            })?;
            let right = right.qubit_id().ok_or_else(|| {
                CircuitError::invalid_detector_error_model(format!(
                    "{gate_name} target {right} is not a qubit"
                ))
            })?;
            for pending in &mut self.pending_errors {
                pending.apply_swap(left, right);
            }
            for pending in &mut self.pending_pauli_channels {
                pending.apply_swap(left, right);
            }
            self.observable_sensitivity.apply_swap(left, right);
        }
        Ok(())
    }

    fn apply_single_qubit_clifford(
        &mut self,
        instruction: &CircuitInstruction,
        clifford: SingleQubitClifford,
    ) -> CircuitResult<()> {
        let gate_name = instruction.gate().canonical_name();
        for target in instruction.targets() {
            let Some(qubit) = target.qubit_id() else {
                return Err(CircuitError::invalid_detector_error_model(format!(
                    "{gate_name} target {target} is not a qubit"
                )));
            };
            for pending in &mut self.pending_errors {
                pending.apply_single_qubit_clifford(qubit, clifford)?;
            }
            for pending in &mut self.pending_pauli_channels {
                if pending.qubit == qubit {
                    pending.apply_single_qubit_clifford(clifford)?;
                }
            }
            self.observable_sensitivity
                .apply_single_qubit_clifford(qubit, clifford)?;
        }
        Ok(())
    }

    fn apply_quantum_controlled_pauli(
        &mut self,
        left: QubitId,
        right: QubitId,
        left_basis: AnalyzerPauli,
        right_basis: AnalyzerPauli,
    ) -> CircuitResult<()> {
        self.expand_pending_single_qubit_channels_touching(left, right)?;
        for pending in &mut self.pending_errors {
            pending.apply_controlled_pauli(left, right, left_basis, right_basis);
        }
        self.observable_sensitivity
            .apply_controlled_pauli(left, right, left_basis, right_basis)
    }

    fn cut_pending_errors_at_qubit(&mut self, qubit: QubitId) {
        let mut still_pending = Vec::new();
        for mut pending in self.pending_errors.drain(..) {
            if pending.touches_qubit(qubit) {
                pending.remove_effects_touching(qubit);
                if pending.effects.is_empty() {
                    self.completed_errors.push(pending);
                } else {
                    still_pending.push(pending);
                }
            } else {
                still_pending.push(pending);
            }
        }
        self.pending_errors = still_pending;
    }

    fn record_detector(&mut self, instruction: &CircuitInstruction) -> CircuitResult<()> {
        let detector_id = self.detector_count;
        self.detector_count = self.detector_count.checked_add(1).ok_or_else(|| {
            CircuitError::invalid_detector_error_model("detector count overflowed")
        })?;
        for target in instruction.targets() {
            let Some(offset) = target.measurement_record_offset() else {
                return Err(CircuitError::invalid_detector_error_model(format!(
                    "DETECTOR target {target} is not a measurement record"
                )));
            };
            let measurement = self.measurement_index_from_offset(offset.get())?;
            self.detector_terms_by_measurement
                .entry(measurement)
                .or_default()
                .push(detector_id);
        }
        self.declarations.push(Declaration::Detector {
            detector_id,
            coordinates: instruction.args().to_vec(),
            tag: instruction.tag().map(str::to_owned),
        });
        Ok(())
    }

    fn record_observable(&mut self, instruction: &CircuitInstruction) -> CircuitResult<()> {
        let observable = instruction.observable_id_argument()?.ok_or_else(|| {
            CircuitError::invalid_detector_error_model("OBSERVABLE_INCLUDE missing observable id")
        })?;
        let mut has_pauli_target = false;
        let mut has_measurement_record_target = false;
        let has_targets = !instruction.targets().is_empty();
        for target in instruction.targets() {
            if let Some(offset) = target.measurement_record_offset() {
                has_measurement_record_target = true;
                let measurement = self.measurement_index_from_offset(offset.get())?;
                self.observable_terms_by_measurement
                    .entry(measurement)
                    .or_default()
                    .push(observable.get());
            } else if let Some(pauli) = target.pauli_type() {
                has_pauli_target = true;
                let qubit = target.qubit_id().ok_or_else(|| {
                    CircuitError::invalid_detector_error_model(format!(
                        "OBSERVABLE_INCLUDE target {target} does not identify a qubit"
                    ))
                })?;
                self.observable_sensitivity.toggle(
                    qubit,
                    AnalyzerBasis::from_pauli(pauli),
                    observable.get(),
                );
            } else {
                return Err(CircuitError::invalid_detector_error_model(format!(
                    "analyze_errors does not yet support OBSERVABLE_INCLUDE target {target}"
                )));
            }
        }
        if has_pauli_target
            || has_measurement_record_target
            || instruction.tag().is_some()
            || !has_targets
        {
            self.declarations.push(Declaration::Observable {
                observable: observable.get(),
                tag: instruction.tag().map(str::to_owned),
            });
        }
        Ok(())
    }

    fn shift_coordinates(&mut self, instruction: &CircuitInstruction) -> CircuitResult<()> {
        self.declarations.push(Declaration::Shift {
            coordinates: instruction.args().to_vec(),
            detector_shift: 0,
            tag: instruction.tag().map(str::to_owned),
        });
        Ok(())
    }

    fn measurement_index_from_offset(&self, offset: i32) -> CircuitResult<usize> {
        let measurement_count = i64::try_from(self.measurement_count).map_err(|_| {
            CircuitError::invalid_detector_error_model("measurement count does not fit i64")
        })?;
        let index = measurement_count
            .checked_add(i64::from(offset))
            .ok_or_else(|| {
                CircuitError::invalid_detector_error_model("measurement offset overflowed")
            })?;
        if index < 0 || index >= measurement_count {
            return Err(CircuitError::invalid_detector_error_model(format!(
                "measurement record offset rec[{offset}] is out of range"
            )));
        }
        usize::try_from(index).map_err(|_| {
            CircuitError::invalid_detector_error_model("measurement index does not fit usize")
        })
    }

    fn into_dem(self) -> CircuitResult<DetectorErrorModel> {
        let mut dem = DetectorErrorModel::new();
        let mut merged_error_probabilities = BTreeMap::new();
        let mut disjoint_error_probabilities = BTreeMap::new();
        let mut touched_detectors = BTreeSet::new();
        let mut touched_logical_observables = BTreeSet::new();
        for pending in self
            .completed_errors
            .into_iter()
            .chain(self.pending_errors)
            .filter(|pending| !pending.measurements.is_empty() || !pending.observables.is_empty())
        {
            let mut detectors = BTreeSet::new();
            let mut observables = BTreeSet::new();
            toggle_all(&mut observables, pending.observables.into_iter());
            for measurement in pending.measurements {
                toggle_all(
                    &mut detectors,
                    self.detector_terms_by_measurement
                        .get(&measurement)
                        .into_iter()
                        .flatten()
                        .copied(),
                );
                toggle_all(
                    &mut observables,
                    self.observable_terms_by_measurement
                        .get(&measurement)
                        .into_iter()
                        .flatten()
                        .copied(),
                );
            }
            if detectors.is_empty() && observables.is_empty() {
                continue;
            }
            let mut targets = Vec::with_capacity(detectors.len() + observables.len());
            for detector in detectors {
                targets.push(DemTarget::relative_detector(detector)?);
            }
            for observable in observables {
                targets.push(DemTarget::logical_observable(observable)?);
            }
            if let Some(group_id) = pending.disjoint_group {
                merge_disjoint_probability(
                    &mut disjoint_error_probabilities,
                    (group_id, targets, pending.tag),
                    pending.probability,
                )?;
            } else {
                merge_independent_probability(
                    &mut merged_error_probabilities,
                    (targets, pending.tag),
                    pending.probability,
                )?;
            }
        }

        for ((_group_id, targets, tag), probability) in disjoint_error_probabilities {
            merge_independent_probability(
                &mut merged_error_probabilities,
                (targets, tag),
                probability,
            )?;
        }
        for targets in self.gauge_errors {
            merge_independent_probability(
                &mut merged_error_probabilities,
                (targets, None),
                Probability::try_new(0.5)?,
            )?;
        }

        if self.options.decompose_errors {
            merged_error_probabilities = decompose_tagged_error_probabilities(
                merged_error_probabilities,
                self.options
                    .block_decomposition_from_introducing_remnant_edges,
                self.options.ignore_decomposition_failures,
            )?;
        }

        for ((targets, tag), probability) in merged_error_probabilities {
            if probability.get() == 0.0 {
                continue;
            }
            touched_detectors.extend(targets.iter().filter_map(|target| match target {
                DemTarget::RelativeDetector(id) => Some(id.get()),
                _ => None,
            }));
            let touched_observables = targets.iter().filter_map(|target| match target {
                DemTarget::LogicalObservable(id) => Some(id.get()),
                _ => None,
            });
            for observable in touched_observables {
                touched_logical_observables.insert(observable);
            }
            dem.push_instruction(DemInstruction::error(probability, targets, tag)?);
        }

        for declaration in self.declarations {
            match declaration {
                Declaration::Detector {
                    detector_id,
                    coordinates,
                    tag,
                } => {
                    if coordinates.is_empty()
                        && tag.is_none()
                        && touched_detectors.contains(&detector_id)
                    {
                        continue;
                    }
                    dem.push_instruction(DemInstruction::detector(
                        coordinates,
                        DemTarget::relative_detector(detector_id)?,
                        tag,
                    )?);
                }
                Declaration::Observable { observable, tag } => {
                    if tag.is_some() || !touched_logical_observables.contains(&observable) {
                        dem.push_instruction(DemInstruction::logical_observable(
                            DemTarget::logical_observable(observable)?,
                            tag,
                        )?);
                    }
                }
                Declaration::Shift {
                    coordinates,
                    detector_shift,
                    tag,
                } => {
                    dem.push_instruction(DemInstruction::shift_detectors(
                        coordinates,
                        detector_shift,
                        tag,
                    )?);
                }
            }
        }
        Ok(dem)
    }
}
