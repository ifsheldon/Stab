use std::collections::{BTreeMap, BTreeSet};

use crate::{
    CircuitError, CircuitResult, Pauli, PauliBasis, Probability, QubitId, SingleQubitClifford,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum AnalyzerPauli {
    X,
    Y,
    Z,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum AnalyzerBasis {
    X,
    Y,
    Z,
}

impl AnalyzerBasis {
    pub(super) fn from_pauli(pauli: Pauli) -> Self {
        match pauli {
            Pauli::X => Self::X,
            Pauli::Y => Self::Y,
            Pauli::Z => Self::Z,
        }
    }
}

#[derive(Clone, Debug)]
pub(super) struct NoiseEffect {
    pub(super) qubit: QubitId,
    pub(super) pauli: AnalyzerPauli,
}

#[derive(Clone, Debug)]
pub(super) struct PendingError {
    pub(super) probability: Probability,
    pub(super) effects: Vec<NoiseEffect>,
    pub(super) measurements: Vec<usize>,
    pub(super) observables: Vec<u64>,
    pub(super) disjoint_group: Option<u64>,
}

#[derive(Clone, Debug, Default)]
pub(super) struct ObservableSensitivity {
    xs: BTreeMap<QubitId, BTreeSet<u64>>,
    zs: BTreeMap<QubitId, BTreeSet<u64>>,
}

#[derive(Clone, Debug)]
pub(super) struct PendingSingleQubitPauliChannel {
    pub(super) qubit: QubitId,
    pub(super) x_probability: Probability,
    pub(super) y_probability: Probability,
    pub(super) z_probability: Probability,
}

impl PendingSingleQubitPauliChannel {
    pub(super) fn apply_single_qubit_clifford(
        &mut self,
        clifford: SingleQubitClifford,
    ) -> CircuitResult<()> {
        let mut x_probability = None;
        let mut y_probability = None;
        let mut z_probability = None;
        for (basis, probability) in [
            (PauliBasis::X, self.x_probability),
            (PauliBasis::Y, self.y_probability),
            (PauliBasis::Z, self.z_probability),
        ] {
            let output_basis = apply_clifford_basis(clifford, basis)?;
            match output_basis {
                PauliBasis::I => {
                    return Err(non_identity_mapped_to_identity(clifford));
                }
                PauliBasis::X => assign_probability(&mut x_probability, probability, clifford)?,
                PauliBasis::Y => assign_probability(&mut y_probability, probability, clifford)?,
                PauliBasis::Z => assign_probability(&mut z_probability, probability, clifford)?,
            }
        }
        self.x_probability = x_probability.ok_or_else(|| missing_channel_basis(clifford, "X"))?;
        self.y_probability = y_probability.ok_or_else(|| missing_channel_basis(clifford, "Y"))?;
        self.z_probability = z_probability.ok_or_else(|| missing_channel_basis(clifford, "Z"))?;
        Ok(())
    }

    pub(super) fn flip_probability(&self, basis: AnalyzerBasis) -> CircuitResult<Probability> {
        let probability = match basis {
            AnalyzerBasis::X => self.y_probability.get() + self.z_probability.get(),
            AnalyzerBasis::Y => self.x_probability.get() + self.z_probability.get(),
            AnalyzerBasis::Z => self.x_probability.get() + self.y_probability.get(),
        };
        Probability::try_new(probability)
    }
}

impl PendingError {
    pub(super) fn apply_single_qubit_clifford(
        &mut self,
        qubit: QubitId,
        clifford: SingleQubitClifford,
    ) -> CircuitResult<()> {
        for effect in &mut self.effects {
            if effect.qubit == qubit {
                effect.pauli = apply_clifford_pauli(clifford, effect.pauli)?;
            }
        }
        Ok(())
    }

    pub(super) fn apply_controlled_pauli(
        &mut self,
        left: QubitId,
        right: QubitId,
        left_basis: AnalyzerPauli,
        right_basis: AnalyzerPauli,
    ) {
        let mut masks = self.effect_masks();
        let left_mask = masks.remove(&left).unwrap_or(0);
        let right_mask = masks.remove(&right).unwrap_or(0);
        let left_basis_mask = analyzer_pauli_mask(left_basis);
        let right_basis_mask = analyzer_pauli_mask(right_basis);
        let mut output_left = left_mask;
        let mut output_right = right_mask;

        if masks_anticommute(left_mask, left_basis_mask) {
            output_right ^= right_basis_mask;
        }
        if masks_anticommute(right_mask, right_basis_mask) {
            output_left ^= left_basis_mask;
        }

        insert_effect_mask(&mut masks, left, output_left);
        insert_effect_mask(&mut masks, right, output_right);
        self.effects = effects_from_masks(masks);
    }

    pub(super) fn toggle_effect(&mut self, effect: NoiseEffect) {
        let mut masks = self.effect_masks();
        let existing_mask = masks.remove(&effect.qubit).unwrap_or(0);
        let output_mask = existing_mask ^ pauli_mask(effect.pauli.into());
        insert_effect_mask(&mut masks, effect.qubit, output_mask);
        self.effects = effects_from_masks(masks);
    }

    fn effect_masks(&self) -> BTreeMap<QubitId, u8> {
        let mut masks = BTreeMap::new();
        for effect in &self.effects {
            let entry = masks.entry(effect.qubit).or_insert(0);
            *entry ^= pauli_mask(effect.pauli.into());
            if *entry == 0 {
                masks.remove(&effect.qubit);
            }
        }
        masks
    }

    pub(super) fn touches_qubit(&self, qubit: QubitId) -> bool {
        self.effects.iter().any(|effect| effect.qubit == qubit)
    }

    pub(super) fn remove_effects_touching(&mut self, qubit: QubitId) {
        self.effects.retain(|effect| effect.qubit != qubit);
    }

    pub(super) fn project_through_measurement(
        &mut self,
        qubit: QubitId,
        basis: AnalyzerBasis,
        measurement: usize,
        reset_after_measurement: bool,
    ) {
        let mut masks = self.effect_masks();
        let Some(mask) = masks.remove(&qubit) else {
            return;
        };
        if masks_anticommute(mask, analyzer_basis_mask(basis)) {
            self.measurements.push(measurement);
            if !reset_after_measurement {
                insert_effect_mask(
                    &mut masks,
                    qubit,
                    measurement_flip_representative_mask(basis),
                );
            }
        }
        self.effects = effects_from_masks(masks);
    }

    pub(super) fn flips_product_measurement(&self, terms: &[(QubitId, AnalyzerBasis)]) -> bool {
        let mut flips = false;
        for effect in &self.effects {
            for (qubit, basis) in terms {
                if effect.qubit == *qubit && pauli_flips_basis_measurement(effect.pauli, *basis) {
                    flips ^= true;
                }
            }
        }
        flips
    }

    pub(super) fn flips_measurement_record(&self, measurement: usize) -> bool {
        let mut flips = false;
        for recorded in &self.measurements {
            if *recorded == measurement {
                flips ^= true;
            }
        }
        flips
    }

    pub(super) fn toggle_observables(&mut self, observables: &[u64]) {
        let mut current = self.observables.iter().copied().collect::<BTreeSet<_>>();
        for observable in observables {
            if !current.insert(*observable) {
                current.remove(observable);
            }
        }
        self.observables = current.into_iter().collect();
    }
}

impl ObservableSensitivity {
    pub(super) fn toggle(&mut self, qubit: QubitId, basis: AnalyzerBasis, observable: u64) {
        let values = BTreeSet::from([observable]);
        match basis {
            AnalyzerBasis::X => toggle_all(self.xs.entry(qubit).or_default(), &values),
            AnalyzerBasis::Y => {
                toggle_all(self.xs.entry(qubit).or_default(), &values);
                toggle_all(self.zs.entry(qubit).or_default(), &values);
            }
            AnalyzerBasis::Z => toggle_all(self.zs.entry(qubit).or_default(), &values),
        }
        self.remove_empty(qubit);
    }

    pub(super) fn flipped_observables(&self, effects: &[NoiseEffect]) -> Vec<u64> {
        let mut observables = BTreeSet::new();
        for effect in effects {
            match effect.pauli {
                AnalyzerPauli::X => {
                    if let Some(zs) = self.zs.get(&effect.qubit) {
                        toggle_all(&mut observables, zs);
                    }
                }
                AnalyzerPauli::Y => {
                    if let Some(xs) = self.xs.get(&effect.qubit) {
                        toggle_all(&mut observables, xs);
                    }
                    if let Some(zs) = self.zs.get(&effect.qubit) {
                        toggle_all(&mut observables, zs);
                    }
                }
                AnalyzerPauli::Z => {
                    if let Some(xs) = self.xs.get(&effect.qubit) {
                        toggle_all(&mut observables, xs);
                    }
                }
            }
        }
        observables.into_iter().collect()
    }

    pub(super) fn apply_single_qubit_clifford(
        &mut self,
        qubit: QubitId,
        clifford: SingleQubitClifford,
    ) -> CircuitResult<()> {
        let input_xs = self.xs.remove(&qubit).unwrap_or_default();
        let input_zs = self.zs.remove(&qubit).unwrap_or_default();
        self.apply_basis_set(
            qubit,
            apply_clifford_basis(clifford, PauliBasis::X)?,
            &input_xs,
        )?;
        self.apply_basis_set(
            qubit,
            apply_clifford_basis(clifford, PauliBasis::Z)?,
            &input_zs,
        )?;
        self.remove_empty(qubit);
        Ok(())
    }

    pub(super) fn apply_controlled_pauli(
        &mut self,
        left: QubitId,
        right: QubitId,
        left_basis: AnalyzerPauli,
        right_basis: AnalyzerPauli,
    ) -> CircuitResult<()> {
        let left_xs = self.xs.remove(&left).unwrap_or_default();
        let left_zs = self.zs.remove(&left).unwrap_or_default();
        let right_xs = self.xs.remove(&right).unwrap_or_default();
        let right_zs = self.zs.remove(&right).unwrap_or_default();

        self.apply_controlled_basis_component(
            left,
            PauliBasis::X,
            &left_xs,
            left_basis,
            right,
            right_basis,
        )?;
        self.apply_controlled_basis_component(
            left,
            PauliBasis::Z,
            &left_zs,
            left_basis,
            right,
            right_basis,
        )?;
        self.apply_controlled_basis_component(
            right,
            PauliBasis::X,
            &right_xs,
            right_basis,
            left,
            left_basis,
        )?;
        self.apply_controlled_basis_component(
            right,
            PauliBasis::Z,
            &right_zs,
            right_basis,
            left,
            left_basis,
        )?;

        self.remove_empty(left);
        self.remove_empty(right);
        Ok(())
    }

    fn apply_basis_set(
        &mut self,
        qubit: QubitId,
        basis: PauliBasis,
        observables: &BTreeSet<u64>,
    ) -> CircuitResult<()> {
        match basis {
            PauliBasis::I => Err(CircuitError::invalid_detector_error_model(
                "logical observable sensitivity mapped to identity",
            )),
            PauliBasis::X => {
                toggle_all(self.xs.entry(qubit).or_default(), observables);
                Ok(())
            }
            PauliBasis::Y => {
                toggle_all(self.xs.entry(qubit).or_default(), observables);
                toggle_all(self.zs.entry(qubit).or_default(), observables);
                Ok(())
            }
            PauliBasis::Z => {
                toggle_all(self.zs.entry(qubit).or_default(), observables);
                Ok(())
            }
        }
    }

    fn apply_controlled_basis_component(
        &mut self,
        qubit: QubitId,
        basis: PauliBasis,
        observables: &BTreeSet<u64>,
        qubit_control_basis: AnalyzerPauli,
        other_qubit: QubitId,
        other_basis: AnalyzerPauli,
    ) -> CircuitResult<()> {
        self.apply_basis_set(qubit, basis, observables)?;
        if basis_anticommutes_with_pauli(basis, qubit_control_basis) {
            self.apply_basis_set(other_qubit, other_basis.into(), observables)?;
        }
        Ok(())
    }

    fn remove_empty(&mut self, qubit: QubitId) {
        if self.xs.get(&qubit).is_some_and(BTreeSet::is_empty) {
            self.xs.remove(&qubit);
        }
        if self.zs.get(&qubit).is_some_and(BTreeSet::is_empty) {
            self.zs.remove(&qubit);
        }
    }
}

const ANALYZER_X_MASK: u8 = 0b01;
const ANALYZER_Z_MASK: u8 = 0b10;

pub(super) fn pauli_mask(pauli: Pauli) -> u8 {
    match pauli {
        Pauli::X => ANALYZER_X_MASK,
        Pauli::Y => 0b11,
        Pauli::Z => ANALYZER_Z_MASK,
    }
}

pub(super) fn analyzer_pauli_from_mask(mask: u8) -> AnalyzerPauli {
    match mask {
        0b01 => AnalyzerPauli::X,
        0b10 => AnalyzerPauli::Z,
        0b11 => AnalyzerPauli::Y,
        _ => unreachable!("pauli masks are maintained by xor of X/Z bits"),
    }
}

pub(super) fn analyzer_paulis_anticommute(left: AnalyzerPauli, right: AnalyzerPauli) -> bool {
    masks_anticommute(analyzer_pauli_mask(left), analyzer_pauli_mask(right))
}

impl From<AnalyzerPauli> for Pauli {
    fn from(value: AnalyzerPauli) -> Self {
        match value {
            AnalyzerPauli::X => Self::X,
            AnalyzerPauli::Y => Self::Y,
            AnalyzerPauli::Z => Self::Z,
        }
    }
}

impl From<AnalyzerPauli> for PauliBasis {
    fn from(value: AnalyzerPauli) -> Self {
        match value {
            AnalyzerPauli::X => Self::X,
            AnalyzerPauli::Y => Self::Y,
            AnalyzerPauli::Z => Self::Z,
        }
    }
}

fn apply_clifford_pauli(
    clifford: SingleQubitClifford,
    pauli: AnalyzerPauli,
) -> CircuitResult<AnalyzerPauli> {
    match apply_clifford_basis(clifford, pauli.into())? {
        PauliBasis::I => Err(non_identity_mapped_to_identity(clifford)),
        PauliBasis::X => Ok(AnalyzerPauli::X),
        PauliBasis::Y => Ok(AnalyzerPauli::Y),
        PauliBasis::Z => Ok(AnalyzerPauli::Z),
    }
}

fn apply_clifford_basis(
    clifford: SingleQubitClifford,
    basis: PauliBasis,
) -> CircuitResult<PauliBasis> {
    clifford.apply_basis(basis).map_err(|error| {
        CircuitError::invalid_detector_error_model(format!(
            "failed to propagate Pauli basis through {}: {error}",
            clifford.canonical_name()
        ))
    })
}

fn assign_probability(
    slot: &mut Option<Probability>,
    probability: Probability,
    clifford: SingleQubitClifford,
) -> CircuitResult<()> {
    if slot.replace(probability).is_some() {
        return Err(CircuitError::invalid_detector_error_model(format!(
            "{} maps multiple PAULI_CHANNEL_1 components to the same basis",
            clifford.canonical_name()
        )));
    }
    Ok(())
}

fn missing_channel_basis(clifford: SingleQubitClifford, basis: &str) -> CircuitError {
    CircuitError::invalid_detector_error_model(format!(
        "{} did not map any PAULI_CHANNEL_1 component to {basis}",
        clifford.canonical_name()
    ))
}

fn non_identity_mapped_to_identity(clifford: SingleQubitClifford) -> CircuitError {
    CircuitError::invalid_detector_error_model(format!(
        "{} mapped a non-identity Pauli to identity",
        clifford.canonical_name()
    ))
}

fn pauli_flips_basis_measurement(pauli: AnalyzerPauli, basis: AnalyzerBasis) -> bool {
    masks_anticommute(analyzer_pauli_mask(pauli), analyzer_basis_mask(basis))
}

fn analyzer_pauli_mask(pauli: AnalyzerPauli) -> u8 {
    pauli_mask(pauli.into())
}

fn analyzer_basis_mask(basis: AnalyzerBasis) -> u8 {
    match basis {
        AnalyzerBasis::X => pauli_mask(Pauli::X),
        AnalyzerBasis::Y => pauli_mask(Pauli::Y),
        AnalyzerBasis::Z => pauli_mask(Pauli::Z),
    }
}

fn measurement_flip_representative_mask(basis: AnalyzerBasis) -> u8 {
    match basis {
        AnalyzerBasis::X => pauli_mask(Pauli::Z),
        AnalyzerBasis::Y | AnalyzerBasis::Z => pauli_mask(Pauli::X),
    }
}

fn basis_anticommutes_with_pauli(basis: PauliBasis, pauli: AnalyzerPauli) -> bool {
    match basis {
        PauliBasis::I => false,
        PauliBasis::X => analyzer_paulis_anticommute(AnalyzerPauli::X, pauli),
        PauliBasis::Y => analyzer_paulis_anticommute(AnalyzerPauli::Y, pauli),
        PauliBasis::Z => analyzer_paulis_anticommute(AnalyzerPauli::Z, pauli),
    }
}

fn masks_anticommute(left: u8, right: u8) -> bool {
    let left_x_right_z = (left & ANALYZER_X_MASK != 0) && (right & ANALYZER_Z_MASK != 0);
    let left_z_right_x = (left & ANALYZER_Z_MASK != 0) && (right & ANALYZER_X_MASK != 0);
    left_x_right_z ^ left_z_right_x
}

fn toggle_all(target: &mut BTreeSet<u64>, values: &BTreeSet<u64>) {
    for value in values {
        if !target.insert(*value) {
            target.remove(value);
        }
    }
}

fn insert_effect_mask(masks: &mut BTreeMap<QubitId, u8>, qubit: QubitId, mask: u8) {
    if mask == 0 {
        return;
    }
    masks.insert(qubit, mask);
}

fn effects_from_masks(masks: BTreeMap<QubitId, u8>) -> Vec<NoiseEffect> {
    masks
        .into_iter()
        .map(|(qubit, mask)| NoiseEffect {
            qubit,
            pauli: analyzer_pauli_from_mask(mask),
        })
        .collect()
}
