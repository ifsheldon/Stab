use std::collections::{BTreeMap, BTreeSet};

use crate::{
    Circuit, CircuitError, CircuitInstruction, CircuitItem, CircuitResult, Gate, GateCategory,
    MeasureRecordOffset, Pauli, PauliBasis, QubitId, Target,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MissingDetectorOptions {
    pub ignore_non_deterministic_measurements: bool,
}

pub fn missing_detectors(
    circuit: &Circuit,
    options: MissingDetectorOptions,
) -> CircuitResult<Circuit> {
    let mut finder = MissingDetectorFinder {
        tracker: InvariantTracker::new(circuit.count_qubits(), options)?,
        measurement_count: 0,
        known_rows: Vec::new(),
        invariants: Vec::new(),
        logical_rows: BTreeMap::new(),
        ignored_logical_rows: BTreeSet::new(),
    };
    finder.process_circuit(circuit)?;
    finder.build_output()
}

struct MissingDetectorFinder {
    tracker: InvariantTracker,
    measurement_count: usize,
    known_rows: Vec<MeasurementRow>,
    invariants: Vec<MeasurementRow>,
    logical_rows: BTreeMap<u64, MeasurementRow>,
    ignored_logical_rows: BTreeSet<u64>,
}

impl MissingDetectorFinder {
    fn process_circuit(&mut self, circuit: &Circuit) -> CircuitResult<()> {
        for item in circuit.items() {
            match item {
                CircuitItem::Instruction(instruction) => self.process_instruction(instruction)?,
                CircuitItem::RepeatBlock(_) => {
                    return Err(CircuitError::invalid_detector_error_model(
                        "basic missing-detector analysis does not support repeat blocks",
                    ));
                }
            }
        }
        Ok(())
    }

    fn process_instruction(&mut self, instruction: &CircuitInstruction) -> CircuitResult<()> {
        match instruction.gate().canonical_name() {
            "R" => self.process_reset(instruction, PauliBasis::Z),
            "RX" => self.process_reset(instruction, PauliBasis::X),
            "RY" => self.process_reset(instruction, PauliBasis::Y),
            "M" => self.process_measurement(instruction, PauliBasis::Z, false),
            "MX" => self.process_measurement(instruction, PauliBasis::X, false),
            "MY" => self.process_measurement(instruction, PauliBasis::Y, false),
            "MR" => self.process_measurement(instruction, PauliBasis::Z, true),
            "MRX" => self.process_measurement(instruction, PauliBasis::X, true),
            "MRY" => self.process_measurement(instruction, PauliBasis::Y, true),
            "MPP" => self.process_mpp(instruction),
            "MXX" => self.process_pair_measurement(instruction, PauliBasis::X),
            "MYY" => self.process_pair_measurement(instruction, PauliBasis::Y),
            "MZZ" => self.process_pair_measurement(instruction, PauliBasis::Z),
            "DETECTOR" => self.process_detector(instruction),
            "OBSERVABLE_INCLUDE" => self.process_observable_include(instruction),
            "TICK" => Ok(()),
            _ if instruction.gate().category() == GateCategory::Noise => Ok(()),
            name => Err(CircuitError::invalid_detector_error_model(format!(
                "basic missing-detector analysis does not support gate {name}"
            ))),
        }
    }

    fn process_reset(
        &mut self,
        instruction: &CircuitInstruction,
        basis: PauliBasis,
    ) -> CircuitResult<()> {
        for (qubit, _) in instruction_qubits(instruction)? {
            self.tracker.reset(qubit, basis)?;
        }
        Ok(())
    }

    fn process_measurement(
        &mut self,
        instruction: &CircuitInstruction,
        basis: PauliBasis,
        reset_after_measurement: bool,
    ) -> CircuitResult<()> {
        for (qubit, inverted) in instruction_qubits(instruction)? {
            self.record_measurement(vec![(qubit, basis)], inverted)?;
            if reset_after_measurement {
                self.tracker.reset(qubit, basis)?;
            }
        }
        Ok(())
    }

    fn process_pair_measurement(
        &mut self,
        instruction: &CircuitInstruction,
        basis: PauliBasis,
    ) -> CircuitResult<()> {
        for group in instruction.target_groups() {
            let mut raw_terms = Vec::new();
            for target in group {
                let qubit = target.qubit_id().ok_or_else(|| {
                    CircuitError::invalid_detector_error_model(format!(
                        "{} target {target} is not a qubit",
                        instruction.gate().canonical_name()
                    ))
                })?;
                raw_terms.push((
                    qubit_index(qubit)?,
                    basis,
                    target.is_inverted_result_target(),
                ));
            }
            let (terms, inverted) = normalize_pauli_product_terms(raw_terms)?;
            self.record_measurement(terms, inverted)?;
        }
        Ok(())
    }

    fn process_mpp(&mut self, instruction: &CircuitInstruction) -> CircuitResult<()> {
        for group in instruction.target_groups() {
            let mut raw_terms = Vec::new();
            for target in group {
                if target.is_combiner() {
                    continue;
                }
                let basis = target.pauli_type().map(pauli_basis).ok_or_else(|| {
                    CircuitError::invalid_detector_error_model(format!(
                        "MPP target {target} is not a Pauli product target"
                    ))
                })?;
                let qubit = target.qubit_id().ok_or_else(|| {
                    CircuitError::invalid_detector_error_model(format!(
                        "MPP target {target} does not identify a qubit"
                    ))
                })?;
                raw_terms.push((
                    qubit_index(qubit)?,
                    basis,
                    target.is_inverted_result_target(),
                ));
            }
            let (terms, inverted) = normalize_pauli_product_terms(raw_terms)?;
            self.record_measurement(terms, inverted)?;
        }
        Ok(())
    }

    fn record_measurement(
        &mut self,
        terms: Vec<(usize, PauliBasis)>,
        inverted: bool,
    ) -> CircuitResult<()> {
        let measurement_index = self.measurement_count;
        self.measurement_count = self.measurement_count.checked_add(1).ok_or_else(|| {
            CircuitError::invalid_detector_error_model(
                "measurement count overflowed during missing-detector analysis",
            )
        })?;
        if let Some(invariant) =
            self.tracker
                .measure_pauli_product(&terms, inverted, measurement_index)?
        {
            self.invariants.push(invariant);
        }
        Ok(())
    }

    fn process_detector(&mut self, instruction: &CircuitInstruction) -> CircuitResult<()> {
        let mut row = MeasurementRow::new();
        for target in instruction.targets() {
            let offset = target.measurement_record_offset().ok_or_else(|| {
                CircuitError::invalid_detector_error_model(format!(
                    "DETECTOR target {target} is not a measurement record"
                ))
            })?;
            let index = self.absolute_record_index(offset)?;
            row.toggle(index);
        }
        self.known_rows.push(row);
        Ok(())
    }

    fn process_observable_include(
        &mut self,
        instruction: &CircuitInstruction,
    ) -> CircuitResult<()> {
        let observable = instruction
            .observable_id_argument()?
            .ok_or_else(|| {
                CircuitError::invalid_detector_error_model(
                    "OBSERVABLE_INCLUDE is missing an observable id argument",
                )
            })?
            .get();
        let mut row_delta = MeasurementRow::new();
        for target in instruction.targets() {
            if let Some(offset) = target.measurement_record_offset() {
                let index = self.absolute_record_index(offset)?;
                row_delta.toggle(index);
            } else if target.is_pauli_target() {
                self.ignored_logical_rows.insert(observable);
            } else {
                return Err(CircuitError::invalid_detector_error_model(format!(
                    "OBSERVABLE_INCLUDE target {target} is not supported by missing-detector analysis"
                )));
            }
        }
        self.logical_rows
            .entry(observable)
            .or_insert_with(MeasurementRow::new)
            .xor_assign(&row_delta);
        Ok(())
    }

    fn absolute_record_index(&self, offset: MeasureRecordOffset) -> CircuitResult<usize> {
        let current = i64::try_from(self.measurement_count).map_err(|_| {
            CircuitError::invalid_detector_error_model(
                "measurement count does not fit i64 during missing-detector analysis",
            )
        })?;
        let index = current
            .checked_add(i64::from(offset.get()))
            .ok_or_else(|| {
                CircuitError::invalid_detector_error_model(
                    "measurement record offset overflowed during missing-detector analysis",
                )
            })?;
        if index < 0 || index >= current {
            return Err(CircuitError::invalid_detector_error_model(format!(
                "measurement record target rec[{}] is outside missing-detector analysis history",
                offset.get()
            )));
        }
        usize::try_from(index).map_err(|_| {
            CircuitError::invalid_detector_error_model(
                "measurement record index does not fit usize during missing-detector analysis",
            )
        })
    }

    fn build_output(&self) -> CircuitResult<Circuit> {
        let mut rows = Vec::new();
        let mut original_known_rows = Vec::new();
        for row in &self.known_rows {
            rows.push(EliminationRow {
                row: row.clone(),
                invariant: false,
            });
            original_known_rows.push(row.clone());
        }
        for (observable, row) in &self.logical_rows {
            if self.ignored_logical_rows.contains(observable) {
                continue;
            }
            rows.push(EliminationRow {
                row: row.clone(),
                invariant: false,
            });
            original_known_rows.push(row.clone());
        }
        for row in &self.invariants {
            rows.push(EliminationRow {
                row: row.clone(),
                invariant: true,
            });
        }
        eliminate_rows(&mut rows, self.measurement_count);

        let mut result = Circuit::new();
        let total = self.measurement_count;
        for row in &mut rows {
            if !row.invariant || row.row.is_empty() {
                continue;
            }
            for known in &original_known_rows {
                if row.row.is_subset_of(known) {
                    row.row.xor_assign(known);
                }
            }
            let mut targets = Vec::new();
            for index in row.row.iter() {
                targets.push(Target::measurement_record(relative_offset(index, total)?));
            }
            result.append_instruction(CircuitInstruction::new(
                Gate::from_name("DETECTOR")?,
                Vec::new(),
                targets,
                None,
            )?);
        }
        Ok(result)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct MeasurementRow {
    bits: BTreeSet<usize>,
}

impl MeasurementRow {
    fn new() -> Self {
        Self {
            bits: BTreeSet::new(),
        }
    }

    fn singleton(index: usize) -> Self {
        let mut row = Self::new();
        row.toggle(index);
        row
    }

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
            .find(|row| {
                rows.get(*row)
                    .is_some_and(|row| row.row.contains(column) && !row.invariant)
            })
            .or_else(|| {
                (solved..rows.len())
                    .find(|row| rows.get(*row).is_some_and(|row| row.row.contains(column)))
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

#[derive(Clone, Debug, Eq, PartialEq)]
struct InvariantTracker {
    qubit_count: usize,
    generators: Vec<TrackedGenerator>,
}

impl InvariantTracker {
    fn new(qubit_count: usize, options: MissingDetectorOptions) -> CircuitResult<Self> {
        if options.ignore_non_deterministic_measurements {
            return Ok(Self {
                qubit_count,
                generators: Vec::new(),
            });
        }
        let mut generators = Vec::with_capacity(qubit_count);
        for qubit in 0..qubit_count {
            generators.push(TrackedGenerator::single(
                qubit_count,
                qubit,
                PauliBasis::Z,
                false,
                MeasurementRow::new(),
            ));
        }
        Ok(Self {
            qubit_count,
            generators,
        })
    }

    fn reset(&mut self, qubit: usize, basis: PauliBasis) -> CircuitResult<()> {
        self.require_qubit(qubit, "reset")?;
        let observable =
            TrackedGenerator::single(self.qubit_count, qubit, basis, false, MeasurementRow::new());
        self.force_eigenstate(observable);
        Ok(())
    }

    fn measure_pauli_product(
        &mut self,
        terms: &[(usize, PauliBasis)],
        inverted: bool,
        measurement_index: usize,
    ) -> CircuitResult<Option<MeasurementRow>> {
        for (qubit, _) in terms {
            self.require_qubit(*qubit, "measurement")?;
        }
        let observable =
            TrackedGenerator::product(self.qubit_count, terms, inverted, MeasurementRow::new());
        if let Some(mut dependencies) = self.deterministic_dependencies(&observable) {
            dependencies.toggle(measurement_index);
            return Ok(Some(dependencies));
        }
        let mut collapsed = observable;
        collapsed.dependencies = MeasurementRow::singleton(measurement_index);
        self.force_eigenstate(collapsed);
        Ok(None)
    }

    fn deterministic_dependencies(&self, observable: &TrackedGenerator) -> Option<MeasurementRow> {
        if let Some(generator) = self
            .generators
            .iter()
            .find(|generator| generator.same_bases_as(observable))
        {
            return Some(generator.dependencies.clone());
        }
        if self
            .generators
            .iter()
            .any(|generator| !generator.commutes_with(observable))
        {
            return None;
        }
        let solution = self.solve_span(observable)?;
        let mut dependencies = MeasurementRow::new();
        for (include, generator) in solution.into_iter().zip(&self.generators) {
            if include {
                dependencies.xor_assign(&generator.dependencies);
            }
        }
        Some(dependencies)
    }

    fn force_eigenstate(&mut self, observable: TrackedGenerator) {
        let Some(pivot_index) = self
            .generators
            .iter()
            .position(|generator| !generator.commutes_with(&observable))
        else {
            if self.solve_span(&observable).is_none() && self.generators.len() < self.qubit_count {
                self.generators.push(observable);
            }
            return;
        };
        let Some(pivot) = self.generators.get(pivot_index).cloned() else {
            return;
        };
        for (index, generator) in self.generators.iter_mut().enumerate() {
            if index != pivot_index && !generator.commutes_with(&observable) {
                generator.multiply_assign(&pivot);
            }
        }
        if let Some(slot) = self.generators.get_mut(pivot_index) {
            *slot = observable;
        }
    }

    fn solve_span(&self, observable: &TrackedGenerator) -> Option<Vec<bool>> {
        let width = self.qubit_count.checked_mul(2)?;
        let generator_count = self.generators.len();
        let mut basis = vec![None; width];
        for (generator_index, generator) in self.generators.iter().enumerate() {
            let mut row = SpanRow::from_generator(generator, generator_count, generator_index);
            reduce_span_row(&mut row, &basis);
            if let Some(pivot) = row.first_one()
                && let Some(slot) = basis.get_mut(pivot)
            {
                *slot = Some(row);
            }
        }

        let mut target = SpanRow {
            bits: observable.symplectic_bits(),
            coefficients: vec![false; generator_count],
        };
        for column in 0..width {
            if !target.bit(column) {
                continue;
            }
            let pivot = basis.get(column).and_then(Option::as_ref)?;
            target.xor_assign(pivot);
        }
        if target.bits.iter().any(|bit| *bit) {
            None
        } else {
            Some(target.coefficients)
        }
    }

    fn require_qubit(&self, qubit: usize, role: &str) -> CircuitResult<()> {
        if qubit >= self.qubit_count {
            return Err(CircuitError::invalid_detector_error_model(format!(
                "{role} qubit {qubit} is outside the missing-detector tracker"
            )));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TrackedGenerator {
    negative: bool,
    bases: Vec<PauliBasis>,
    dependencies: MeasurementRow,
}

impl TrackedGenerator {
    fn identity(qubit_count: usize, dependencies: MeasurementRow) -> Self {
        Self {
            negative: false,
            bases: vec![PauliBasis::I; qubit_count],
            dependencies,
        }
    }

    fn single(
        qubit_count: usize,
        qubit: usize,
        basis: PauliBasis,
        negative: bool,
        dependencies: MeasurementRow,
    ) -> Self {
        let mut generator = Self::identity(qubit_count, dependencies);
        generator.set_basis(qubit, basis);
        generator.negative = negative;
        generator
    }

    fn product(
        qubit_count: usize,
        terms: &[(usize, PauliBasis)],
        negative: bool,
        dependencies: MeasurementRow,
    ) -> Self {
        let mut generator = Self::identity(qubit_count, dependencies);
        generator.negative = negative;
        for (qubit, basis) in terms {
            generator.set_basis(*qubit, *basis);
        }
        generator
    }

    fn basis(&self, qubit: usize) -> PauliBasis {
        self.bases.get(qubit).copied().unwrap_or(PauliBasis::I)
    }

    fn set_basis(&mut self, qubit: usize, basis: PauliBasis) {
        if let Some(slot) = self.bases.get_mut(qubit) {
            *slot = basis;
        }
    }

    fn commutes_with(&self, rhs: &Self) -> bool {
        self.bases
            .iter()
            .copied()
            .zip(rhs.bases.iter().copied())
            .filter(|(left, right)| anticommutes(*left, *right))
            .count()
            .is_multiple_of(2)
    }

    fn same_bases_as(&self, rhs: &Self) -> bool {
        self.bases == rhs.bases
    }

    fn multiply_assign(&mut self, rhs: &Self) {
        let mut log_i = sign_log_i(self.negative).wrapping_add(sign_log_i(rhs.negative));
        let len = self.bases.len().max(rhs.bases.len());
        if self.bases.len() < len {
            self.bases.resize(len, PauliBasis::I);
        }
        for index in 0..len {
            let left = self.basis(index);
            let right = rhs.basis(index);
            log_i = log_i.wrapping_add(left.log_i_scalar_byproduct(right));
            self.set_basis(
                index,
                PauliBasis::from_xz(left.x_bit() ^ right.x_bit(), left.z_bit() ^ right.z_bit()),
            );
        }
        self.negative = (log_i & 2) != 0;
        self.dependencies.xor_assign(&rhs.dependencies);
    }

    fn symplectic_bits(&self) -> Vec<bool> {
        self.bases
            .iter()
            .map(|basis| basis.x_bit())
            .chain(self.bases.iter().map(|basis| basis.z_bit()))
            .collect()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SpanRow {
    bits: Vec<bool>,
    coefficients: Vec<bool>,
}

impl SpanRow {
    fn from_generator(generator: &TrackedGenerator, generator_count: usize, index: usize) -> Self {
        let mut coefficients = vec![false; generator_count];
        if let Some(coefficient) = coefficients.get_mut(index) {
            *coefficient = true;
        }
        Self {
            bits: generator.symplectic_bits(),
            coefficients,
        }
    }

    fn bit(&self, index: usize) -> bool {
        self.bits.get(index).copied().unwrap_or(false)
    }

    fn first_one(&self) -> Option<usize> {
        self.bits.iter().position(|bit| *bit)
    }

    fn xor_assign(&mut self, rhs: &Self) {
        for (bit, rhs_bit) in self.bits.iter_mut().zip(&rhs.bits) {
            *bit ^= *rhs_bit;
        }
        for (coefficient, rhs_coefficient) in self.coefficients.iter_mut().zip(&rhs.coefficients) {
            *coefficient ^= *rhs_coefficient;
        }
    }
}

fn reduce_span_row(row: &mut SpanRow, basis: &[Option<SpanRow>]) {
    for column in 0..row.bits.len() {
        if !row.bit(column) {
            continue;
        }
        let Some(pivot) = basis.get(column).and_then(Option::as_ref) else {
            return;
        };
        row.xor_assign(pivot);
    }
}

fn instruction_qubits(instruction: &CircuitInstruction) -> CircuitResult<Vec<(usize, bool)>> {
    instruction
        .targets()
        .iter()
        .map(|target| {
            let qubit = target.qubit_id().ok_or_else(|| {
                CircuitError::invalid_detector_error_model(format!(
                    "{} target {target} is not a qubit",
                    instruction.gate().canonical_name()
                ))
            })?;
            Ok((qubit_index(qubit)?, target.is_inverted_result_target()))
        })
        .collect()
}

fn qubit_index(qubit: QubitId) -> CircuitResult<usize> {
    usize::try_from(qubit.get()).map_err(|_| {
        CircuitError::invalid_detector_error_model(format!(
            "qubit id {} does not fit usize during missing-detector analysis",
            qubit.get()
        ))
    })
}

fn normalize_pauli_product_terms(
    raw_terms: Vec<(usize, PauliBasis, bool)>,
) -> CircuitResult<(Vec<(usize, PauliBasis)>, bool)> {
    let mut terms = Vec::new();
    let mut inverted = false;
    let mut phase = 0u8;
    for (qubit, basis, term_inverted) in raw_terms {
        multiply_term(&mut terms, qubit, basis, &mut phase);
        inverted ^= term_inverted;
    }
    match phase {
        0 => Ok((terms, inverted)),
        2 => Ok((terms, !inverted)),
        _ => Err(CircuitError::invalid_detector_error_model(
            "Pauli product is anti-Hermitian during missing-detector analysis",
        )),
    }
}

fn multiply_term(
    terms: &mut Vec<(usize, PauliBasis)>,
    qubit: usize,
    incoming: PauliBasis,
    phase: &mut u8,
) {
    let Some(index) = terms
        .iter()
        .position(|(existing_qubit, _)| *existing_qubit == qubit)
    else {
        terms.push((qubit, incoming));
        return;
    };
    let (_, existing) = terms.remove(index);
    let (product, phase_delta) = multiply_bases(existing, incoming);
    *phase = (*phase + phase_delta) % 4;
    if let Some(product) = product {
        terms.insert(index, (qubit, product));
    }
}

fn multiply_bases(left: PauliBasis, right: PauliBasis) -> (Option<PauliBasis>, u8) {
    match (left, right) {
        (PauliBasis::I, PauliBasis::I) => (None, 0),
        (PauliBasis::I, basis) | (basis, PauliBasis::I) => (Some(basis), 0),
        (PauliBasis::X, PauliBasis::X)
        | (PauliBasis::Y, PauliBasis::Y)
        | (PauliBasis::Z, PauliBasis::Z) => (None, 0),
        (PauliBasis::X, PauliBasis::Y) => (Some(PauliBasis::Z), 1),
        (PauliBasis::Y, PauliBasis::Z) => (Some(PauliBasis::X), 1),
        (PauliBasis::Z, PauliBasis::X) => (Some(PauliBasis::Y), 1),
        (PauliBasis::Y, PauliBasis::X) => (Some(PauliBasis::Z), 3),
        (PauliBasis::Z, PauliBasis::Y) => (Some(PauliBasis::X), 3),
        (PauliBasis::X, PauliBasis::Z) => (Some(PauliBasis::Y), 3),
    }
}

fn pauli_basis(pauli: Pauli) -> PauliBasis {
    match pauli {
        Pauli::X => PauliBasis::X,
        Pauli::Y => PauliBasis::Y,
        Pauli::Z => PauliBasis::Z,
    }
}

fn sign_log_i(negative: bool) -> u8 {
    if negative { 2 } else { 0 }
}

fn anticommutes(left: PauliBasis, right: PauliBasis) -> bool {
    (left.x_bit() && right.z_bit()) ^ (left.z_bit() && right.x_bit())
}

fn relative_offset(index: usize, total: usize) -> CircuitResult<MeasureRecordOffset> {
    let index = i64::try_from(index).map_err(|_| {
        CircuitError::invalid_detector_error_model(
            "measurement index does not fit i64 during missing-detector output",
        )
    })?;
    let total = i64::try_from(total).map_err(|_| {
        CircuitError::invalid_detector_error_model(
            "measurement count does not fit i64 during missing-detector output",
        )
    })?;
    let offset = index.checked_sub(total).ok_or_else(|| {
        CircuitError::invalid_detector_error_model(
            "relative detector offset overflowed during missing-detector output",
        )
    })?;
    MeasureRecordOffset::try_new(i32::try_from(offset).map_err(|_| {
        CircuitError::invalid_detector_error_model(format!(
            "relative detector offset {offset} does not fit i32"
        ))
    })?)
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        reason = "missing-detector parity tests use exact circuit text for compact diagnostics"
    )]

    use super::*;

    fn missing(text: &str, ignore_non_deterministic_measurements: bool) -> String {
        let circuit = Circuit::from_stim_str(text).unwrap();
        missing_detectors(
            &circuit,
            MissingDetectorOptions {
                ignore_non_deterministic_measurements,
            },
        )
        .unwrap()
        .to_stim_string()
    }

    #[test]
    fn missing_detectors_basic() {
        assert_eq!(missing("", false), "");
        assert_eq!(missing("R 0\nM 0\nDETECTOR rec[-1]\n", false), "");
        assert_eq!(
            missing("R 0\nM 0\nDETECTOR rec[-1]\nDETECTOR rec[-1]\n", false),
            ""
        );
        assert_eq!(missing("R 0\nM 0\n", false), "DETECTOR rec[-1]\n");
        assert_eq!(missing("M 0\n", false), "DETECTOR rec[-1]\n");
        assert_eq!(missing("M 0\n", true), "");
        assert_eq!(
            missing("R 0 1\nM 0 1\nDETECTOR rec[-1]\n", false),
            "DETECTOR rec[-2]\n"
        );
        assert_eq!(
            missing("M 0\nDETECTOR rec[-1] rec[-1]\n", false),
            "DETECTOR rec[-1]\n"
        );
        assert_eq!(missing("MX 0\n", false), "");
    }

    #[test]
    fn missing_detectors_supports_mpp_stabilizer_products() {
        // Adapted from Stim v1.16.0 src/stim/util_top/missing_detectors.test.cc.
        assert_eq!(
            missing(
                "MPP Z0*Z1 X0*X1\n\
                 TICK\n\
                 MPP Z0*Z1 X0*X1\n\
                 DETECTOR rec[-1] rec[-3]\n\
                 DETECTOR rec[-2] rec[-4]\n",
                false,
            ),
            "DETECTOR rec[-4]\n"
        );
        assert_eq!(
            missing(
                "MPP Z0*Z1 X0*X1\n\
                 TICK\n\
                 MPP Z0*Z1 X0*X1\n\
                 DETECTOR rec[-1] rec[-3]\n\
                 DETECTOR rec[-2] rec[-4]\n\
                 DETECTOR rec[-1] rec[-3] rec[-2] rec[-4]\n",
                false,
            ),
            "DETECTOR rec[-3] rec[-2] rec[-1]\n"
        );
        assert_eq!(
            missing(
                "MPP Z0*Z1 X0*X1\n\
                 TICK\n\
                 MPP Z0*Z1 X0*X1\n\
                 DETECTOR rec[-1] rec[-3]\n\
                 DETECTOR rec[-2] rec[-4]\n",
                true,
            ),
            ""
        );
    }

    #[test]
    fn missing_detectors_basic_reset_and_measurement_aliases() {
        assert_eq!(missing("RX 0\nMX 0\n", false), "DETECTOR rec[-1]\n");
        assert_eq!(missing("RY 0\nMY 0\n", false), "DETECTOR rec[-1]\n");
        assert_eq!(missing("RX 0\nMY 0\n", false), "");
        assert_eq!(missing("RX 0\nMY 0\n", true), "");
        assert_eq!(missing("MR 0\n", false), "DETECTOR rec[-1]\n");
        assert_eq!(missing("MR 0\n", true), "");
    }

    #[test]
    fn missing_detectors_reduces_multi_record_detector_rows() {
        assert_eq!(
            missing("R 0 1\nM 0 1\nDETECTOR rec[-1] rec[-2]\n", false),
            "DETECTOR rec[-2]\n"
        );
    }

    #[test]
    fn missing_detectors_supports_observable_interactions() {
        // Adapted from Stim v1.16.0 src/stim/util_top/missing_detectors.test.cc.
        assert_eq!(
            missing(
                "MPP Z0*Z1 X0*X1\n\
                 TICK\n\
                 MPP Z0*Z1 X0*X1\n\
                 OBSERVABLE_INCLUDE(0) rec[-1]\n\
                 DETECTOR rec[-2] rec[-4]\n\
                 OBSERVABLE_INCLUDE(0) rec[-3]\n",
                true,
            ),
            ""
        );
        assert_eq!(
            missing(
                "OBSERVABLE_INCLUDE(0) Z0 Z1\n\
                 MPP Z0*Z1 X0*X1\n\
                 TICK\n\
                 MPP Z0*Z1 X0*X1\n\
                 OBSERVABLE_INCLUDE(0) Z0 Z1\n\
                 OBSERVABLE_INCLUDE(0) rec[-1]\n\
                 DETECTOR rec[-2] rec[-4]\n\
                 OBSERVABLE_INCLUDE(0) rec[-3]\n",
                true,
            ),
            "DETECTOR rec[-3] rec[-1]\n"
        );
    }

    #[test]
    fn missing_detectors_rejects_unpromoted_control_flow_and_cliffords() {
        let repeat = Circuit::from_stim_str("REPEAT 2 {\n    M 0\n}\n").unwrap();
        let repeat_error = missing_detectors(
            &repeat,
            MissingDetectorOptions {
                ignore_non_deterministic_measurements: true,
            },
        )
        .unwrap_err();
        assert!(repeat_error.to_string().contains("repeat blocks"));

        let clifford = Circuit::from_stim_str("H 0\nM 0\n").unwrap();
        let clifford_error = missing_detectors(
            &clifford,
            MissingDetectorOptions {
                ignore_non_deterministic_measurements: false,
            },
        )
        .unwrap_err();
        assert!(clifford_error.to_string().contains("gate H"));
    }
}
