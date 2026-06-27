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

    pub(super) fn apply_cx(&mut self, control: QubitId, target: QubitId) {
        let mut masks = self.effect_masks();
        let control_mask = masks.remove(&control).unwrap_or(0);
        let target_mask = masks.remove(&target).unwrap_or(0);
        let mut output_control = 0;
        let mut output_target = 0;

        if control_mask & ANALYZER_X_MASK != 0 {
            output_control ^= ANALYZER_X_MASK;
            output_target ^= ANALYZER_X_MASK;
        }
        if control_mask & ANALYZER_Z_MASK != 0 {
            output_control ^= ANALYZER_Z_MASK;
        }
        if target_mask & ANALYZER_X_MASK != 0 {
            output_target ^= ANALYZER_X_MASK;
        }
        if target_mask & ANALYZER_Z_MASK != 0 {
            output_control ^= ANALYZER_Z_MASK;
            output_target ^= ANALYZER_Z_MASK;
        }

        insert_effect_mask(&mut masks, control, output_control);
        insert_effect_mask(&mut masks, target, output_target);
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

    pub(super) fn flips_measurement(&self, qubit: QubitId, basis: AnalyzerBasis) -> bool {
        self.effects.iter().any(|effect| {
            effect.qubit == qubit && pauli_flips_basis_measurement(effect.pauli, basis)
        })
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

    pub(super) fn apply_cx(&mut self, control: QubitId, target: QubitId) {
        let control_xs = self.xs.remove(&control).unwrap_or_default();
        let control_zs = self.zs.remove(&control).unwrap_or_default();
        let target_xs = self.xs.remove(&target).unwrap_or_default();
        let target_zs = self.zs.remove(&target).unwrap_or_default();

        toggle_all(self.xs.entry(control).or_default(), &control_xs);
        toggle_all(self.xs.entry(target).or_default(), &control_xs);
        toggle_all(self.zs.entry(control).or_default(), &control_zs);
        toggle_all(self.xs.entry(target).or_default(), &target_xs);
        toggle_all(self.zs.entry(control).or_default(), &target_zs);
        toggle_all(self.zs.entry(target).or_default(), &target_zs);

        self.remove_empty(control);
        self.remove_empty(target);
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
    matches!(
        (pauli, basis),
        (AnalyzerPauli::X, AnalyzerBasis::Y | AnalyzerBasis::Z)
            | (AnalyzerPauli::Y, AnalyzerBasis::X | AnalyzerBasis::Z)
            | (AnalyzerPauli::Z, AnalyzerBasis::X | AnalyzerBasis::Y)
    )
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
