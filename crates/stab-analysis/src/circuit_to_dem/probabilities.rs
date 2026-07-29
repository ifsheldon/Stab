use std::collections::{BTreeMap, BTreeSet};

use stab_model::Probability;

use crate::AnalysisResult;

pub(super) fn toggle_all(target: &mut BTreeSet<u64>, values: impl Iterator<Item = u64>) {
    for value in values {
        if !target.insert(value) {
            target.remove(&value);
        }
    }
}

pub(super) fn merge_independent_probability<K: Ord>(
    probabilities: &mut BTreeMap<K, Probability>,
    targets: K,
    probability: Probability,
) -> AnalysisResult<()> {
    if let Some(existing) = probabilities.get_mut(&targets) {
        *existing = xor_probability(*existing, probability)?;
    } else {
        probabilities.insert(targets, probability);
    }
    Ok(())
}

pub(super) fn merge_disjoint_probability<K: Ord>(
    probabilities: &mut BTreeMap<K, Probability>,
    key: K,
    probability: Probability,
) -> AnalysisResult<()> {
    if let Some(existing) = probabilities.get_mut(&key) {
        *existing = Probability::try_new(existing.get() + probability.get())?;
    } else {
        probabilities.insert(key, probability);
    }
    Ok(())
}

pub(super) fn xor_probability(
    left: Probability,
    right: Probability,
) -> AnalysisResult<Probability> {
    Ok(Probability::try_new(
        left.get() + right.get() - 2.0 * left.get() * right.get(),
    )?)
}
