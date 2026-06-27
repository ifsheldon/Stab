use std::collections::{BTreeMap, BTreeSet};

use crate::{
    Circuit, CircuitError, CircuitInstruction, CircuitItem, CircuitResult, Probability, QubitId,
    RepeatBlock,
};

use super::{DemInstruction, DemRepeatBlock, DemTarget, DetectorErrorModel};

const MAX_ANALYZER_REPEAT_UNROLL: u64 = 100_000;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ErrorAnalyzerOptions {
    pub fold_loops: bool,
    pub decompose_errors: bool,
    pub allow_gauge_detectors: bool,
    pub approximate_disjoint_errors: bool,
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

struct FoldedAnalyzer {
    options: ErrorAnalyzerOptions,
}

impl FoldedAnalyzer {
    fn new(options: ErrorAnalyzerOptions) -> Self {
        Self { options }
    }

    fn analyze(&self, circuit: &Circuit) -> CircuitResult<DetectorErrorModel> {
        let mut dem = DetectorErrorModel::new();
        for item in circuit.items() {
            match item {
                CircuitItem::Instruction(_) => {
                    return Err(CircuitError::invalid_detector_error_model(
                        "analyze_errors --fold_loops currently supports top-level repeat blocks only",
                    ));
                }
                CircuitItem::RepeatBlock(repeat) => {
                    dem.push_repeat_block(self.analyze_repeat(repeat)?);
                }
            }
        }
        Ok(dem)
    }

    fn analyze_repeat(&self, repeat: &RepeatBlock) -> CircuitResult<DemRepeatBlock> {
        let mut body_options = self.options;
        body_options.fold_loops = false;
        let mut result = Analyzer::new(body_options).analyze_with_stats(repeat.body())?;
        if result.detector_count > 0 {
            result.dem.push_instruction(DemInstruction::shift_detectors(
                Vec::new(),
                result.detector_count,
                None,
            )?);
        }
        Ok(DemRepeatBlock::new(
            repeat.repeat_count(),
            result.dem,
            repeat.tag().map(ToOwned::to_owned),
        ))
    }
}

struct AnalyzerResult {
    dem: DetectorErrorModel,
    detector_count: u64,
}

struct Analyzer {
    options: ErrorAnalyzerOptions,
    measurement_count: usize,
    detector_count: u64,
    coord_offset: Vec<f64>,
    pending_errors: Vec<PendingError>,
    completed_errors: Vec<PendingError>,
    detector_terms_by_measurement: BTreeMap<usize, Vec<u64>>,
    observable_terms_by_measurement: BTreeMap<usize, Vec<u64>>,
    detector_declarations: Vec<DetectorDeclaration>,
}

impl Analyzer {
    fn new(options: ErrorAnalyzerOptions) -> Self {
        Self {
            options,
            measurement_count: 0,
            detector_count: 0,
            coord_offset: Vec::new(),
            pending_errors: Vec::new(),
            completed_errors: Vec::new(),
            detector_terms_by_measurement: BTreeMap::new(),
            observable_terms_by_measurement: BTreeMap::new(),
            detector_declarations: Vec::new(),
        }
    }

    fn analyze(self, circuit: &Circuit) -> CircuitResult<DetectorErrorModel> {
        self.analyze_with_stats(circuit).map(|result| result.dem)
    }

    fn analyze_with_stats(mut self, circuit: &Circuit) -> CircuitResult<AnalyzerResult> {
        self.visit_circuit(circuit)?;
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
                CircuitItem::RepeatBlock(repeat) => self.visit_repeat(repeat)?,
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
            self.visit_circuit(repeat.body())?;
        }
        Ok(())
    }

    fn visit_instruction(&mut self, instruction: &CircuitInstruction) -> CircuitResult<()> {
        match instruction.gate().canonical_name() {
            "X_ERROR" | "Y_ERROR" | "Z_ERROR" => self.record_single_pauli_error(instruction),
            "M" | "MX" | "MY" | "MR" | "MRX" | "MRY" => self.record_measurements(instruction),
            "MPAD" => self.record_measurement_pads(instruction),
            "DETECTOR" => self.record_detector(instruction),
            "OBSERVABLE_INCLUDE" => self.record_observable(instruction),
            "SHIFT_COORDS" => self.shift_coordinates(instruction),
            "TICK" | "QUBIT_COORDS" | "R" | "RX" | "RY" => Ok(()),
            name if is_noise_instruction(name) => Err(CircuitError::invalid_detector_error_model(
                format!("analyze_errors does not yet support {name}"),
            )),
            name if is_measurement_instruction(name) => {
                Err(CircuitError::invalid_detector_error_model(format!(
                    "analyze_errors does not yet support measurement instruction {name}"
                )))
            }
            _ => Ok(()),
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
            self.pending_errors.push(PendingError {
                probability,
                effects: vec![NoiseEffect { qubit, pauli }],
                measurements: Vec::new(),
            });
        }
        Ok(())
    }

    fn record_measurements(&mut self, instruction: &CircuitInstruction) -> CircuitResult<()> {
        let basis = measurement_basis(instruction.gate().canonical_name()).ok_or_else(|| {
            CircuitError::invalid_detector_error_model("unknown measurement basis")
        })?;
        for group in instruction.target_groups() {
            let Some(target) = group.first() else {
                continue;
            };
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
            for pending in &mut self.pending_errors {
                if pending.flips_measurement(qubit, basis) {
                    pending.measurements.push(measurement_index);
                }
            }
            let mut still_pending = Vec::new();
            for pending in self.pending_errors.drain(..) {
                if pending.touches_qubit(qubit) {
                    self.completed_errors.push(pending);
                } else {
                    still_pending.push(pending);
                }
            }
            self.pending_errors = still_pending;
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

    fn record_detector(&mut self, instruction: &CircuitInstruction) -> CircuitResult<()> {
        let detector_id = self.detector_count;
        self.detector_count = self.detector_count.checked_add(1).ok_or_else(|| {
            CircuitError::invalid_detector_error_model("detector count overflowed")
        })?;
        let coordinates = shifted_coordinates(&self.coord_offset, instruction.args());
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
        self.detector_declarations.push(DetectorDeclaration {
            detector_id,
            coordinates,
        });
        Ok(())
    }

    fn record_observable(&mut self, instruction: &CircuitInstruction) -> CircuitResult<()> {
        let observable = instruction.observable_id_argument()?.ok_or_else(|| {
            CircuitError::invalid_detector_error_model("OBSERVABLE_INCLUDE missing observable id")
        })?;
        for target in instruction.targets() {
            if let Some(offset) = target.measurement_record_offset() {
                let measurement = self.measurement_index_from_offset(offset.get())?;
                self.observable_terms_by_measurement
                    .entry(measurement)
                    .or_default()
                    .push(observable.get());
            } else {
                return Err(CircuitError::invalid_detector_error_model(format!(
                    "analyze_errors does not yet support OBSERVABLE_INCLUDE target {target}"
                )));
            }
        }
        Ok(())
    }

    fn shift_coordinates(&mut self, instruction: &CircuitInstruction) -> CircuitResult<()> {
        for (index, value) in instruction.args().iter().copied().enumerate() {
            if index == self.coord_offset.len() {
                self.coord_offset.push(value);
            } else if let Some(offset) = self.coord_offset.get_mut(index) {
                *offset += value;
            }
        }
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
        let mut touched_detectors = BTreeSet::new();
        for pending in self
            .completed_errors
            .into_iter()
            .chain(self.pending_errors)
            .filter(|pending| !pending.measurements.is_empty())
        {
            let mut detectors = BTreeSet::new();
            let mut observables = BTreeSet::new();
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
            touched_detectors.extend(detectors.iter().copied());
            let mut targets = Vec::with_capacity(detectors.len() + observables.len());
            for detector in detectors {
                targets.push(DemTarget::relative_detector(detector)?);
            }
            for observable in observables {
                targets.push(DemTarget::logical_observable(observable)?);
            }
            dem.push_instruction(DemInstruction::error(pending.probability, targets, None)?);
        }

        for declaration in self.detector_declarations {
            if declaration.coordinates.is_empty()
                && touched_detectors.contains(&declaration.detector_id)
            {
                continue;
            }
            dem.push_instruction(DemInstruction::detector(
                declaration.coordinates,
                DemTarget::relative_detector(declaration.detector_id)?,
                None,
            )?);
        }
        Ok(dem)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AnalyzerPauli {
    X,
    Y,
    Z,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AnalyzerBasis {
    X,
    Y,
    Z,
}

#[derive(Clone, Debug)]
struct NoiseEffect {
    qubit: QubitId,
    pauli: AnalyzerPauli,
}

#[derive(Clone, Debug)]
struct PendingError {
    probability: Probability,
    effects: Vec<NoiseEffect>,
    measurements: Vec<usize>,
}

impl PendingError {
    fn touches_qubit(&self, qubit: QubitId) -> bool {
        self.effects.iter().any(|effect| effect.qubit == qubit)
    }

    fn flips_measurement(&self, qubit: QubitId, basis: AnalyzerBasis) -> bool {
        self.effects.iter().any(|effect| {
            effect.qubit == qubit
                && matches!(
                    (effect.pauli, basis),
                    (AnalyzerPauli::X, AnalyzerBasis::Y | AnalyzerBasis::Z)
                        | (AnalyzerPauli::Y, AnalyzerBasis::X | AnalyzerBasis::Z)
                        | (AnalyzerPauli::Z, AnalyzerBasis::X | AnalyzerBasis::Y)
                )
        })
    }
}

#[derive(Clone, Debug)]
struct DetectorDeclaration {
    detector_id: u64,
    coordinates: Vec<f64>,
}

fn shifted_coordinates(offset: &[f64], local: &[f64]) -> Vec<f64> {
    local
        .iter()
        .copied()
        .enumerate()
        .map(|(index, value)| offset.get(index).copied().unwrap_or(0.0) + value)
        .collect()
}

fn measurement_basis(name: &str) -> Option<AnalyzerBasis> {
    match name {
        "M" | "MR" => Some(AnalyzerBasis::Z),
        "MX" | "MRX" => Some(AnalyzerBasis::X),
        "MY" | "MRY" => Some(AnalyzerBasis::Y),
        _ => None,
    }
}

fn is_measurement_instruction(name: &str) -> bool {
    matches!(
        name,
        "MXX" | "MYY" | "MZZ" | "MPP" | "HERALDED_PAULI_CHANNEL_1"
    )
}

fn is_noise_instruction(name: &str) -> bool {
    matches!(
        name,
        "DEPOLARIZE1"
            | "DEPOLARIZE2"
            | "I_ERROR"
            | "II_ERROR"
            | "PAULI_CHANNEL_1"
            | "PAULI_CHANNEL_2"
            | "ELSE_CORRELATED_ERROR"
            | "E"
    )
}

fn toggle_all(target: &mut BTreeSet<u64>, values: impl Iterator<Item = u64>) {
    for value in values {
        if !target.insert(value) {
            target.remove(&value);
        }
    }
}
