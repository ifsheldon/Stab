use std::collections::BTreeMap;

use crate::{CircuitResult, Pauli, Probability, QubitId};

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
    pub(super) disjoint_group: Option<u64>,
}

#[derive(Clone, Debug)]
pub(super) struct PendingSingleQubitPauliChannel {
    pub(super) qubit: QubitId,
    pub(super) x_probability: Probability,
    pub(super) y_probability: Probability,
    pub(super) z_probability: Probability,
}

impl PendingSingleQubitPauliChannel {
    pub(super) fn swap_xz(&mut self) {
        std::mem::swap(&mut self.x_probability, &mut self.z_probability);
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
    pub(super) fn apply_h(&mut self, qubit: QubitId) {
        for effect in &mut self.effects {
            if effect.qubit == qubit {
                effect.pauli = match effect.pauli {
                    AnalyzerPauli::X => AnalyzerPauli::Z,
                    AnalyzerPauli::Y => AnalyzerPauli::Y,
                    AnalyzerPauli::Z => AnalyzerPauli::X,
                };
            }
        }
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
