use std::collections::BTreeMap;

use stab_model::Probability;

use crate::AnalysisResult;

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
    // Pinned Stim evaluates `old * (1 - p) + (1 - old) * p`
    // (error_analyzer.cc:1088), and the frozen Linux AArch64 Release build
    // contracts it to `fma(old, 1 - p, (1 - old) * p)` (one `fmadd` in
    // `add_error`; verified by disassembly). The fused form rounds once where
    // the plain form rounds twice, so byte-exact `.dem` parity requires
    // replicating the contraction, not just the written expression.
    Ok(Probability::try_new(left.get().mul_add(
        1.0 - right.get(),
        (1.0 - left.get()) * right.get(),
    ))?)
}
