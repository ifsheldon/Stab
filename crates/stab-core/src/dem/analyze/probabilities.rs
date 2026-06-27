use std::collections::{BTreeMap, BTreeSet};

use crate::{CircuitResult, DemTarget, Probability};

pub(super) fn toggle_all(target: &mut BTreeSet<u64>, values: impl Iterator<Item = u64>) {
    for value in values {
        if !target.insert(value) {
            target.remove(&value);
        }
    }
}

pub(super) fn merge_independent_probability(
    probabilities: &mut BTreeMap<Vec<DemTarget>, Probability>,
    targets: Vec<DemTarget>,
    probability: Probability,
) -> CircuitResult<()> {
    if let Some(existing) = probabilities.get_mut(&targets) {
        *existing = xor_probability(*existing, probability)?;
    } else {
        probabilities.insert(targets, probability);
    }
    Ok(())
}

pub(super) fn merge_disjoint_probability(
    probabilities: &mut BTreeMap<(u64, Vec<DemTarget>), Probability>,
    key: (u64, Vec<DemTarget>),
    probability: Probability,
) -> CircuitResult<()> {
    if let Some(existing) = probabilities.get_mut(&key) {
        *existing = Probability::try_new(existing.get() + probability.get())?;
    } else {
        probabilities.insert(key, probability);
    }
    Ok(())
}

pub(super) fn xor_probability(left: Probability, right: Probability) -> CircuitResult<Probability> {
    Probability::try_new(left.get() + right.get() - 2.0 * left.get() * right.get())
}
