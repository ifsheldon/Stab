use std::collections::BTreeMap;

use stab_model::{DemTarget, Probability};

use crate::{AnalysisError, AnalysisResult};

use super::AnalyzerTag;
use super::probabilities::xor_probability;

/// Symptom keys hold the one or two detectors of a known graphlike component;
/// values hold the component's full target span including observables.
type KnownGraphlikeComponents = BTreeMap<Vec<DemTarget>, Vec<DemTarget>>;
type TaggedErrorKey = (Vec<DemTarget>, Option<AnalyzerTag>);

/// Pinned Stim refuses to decompose a component with 64 or more terms
/// (error_analyzer.cc:1466-1469).
const MAX_PROBLEM_TERMS: usize = 64;

pub(super) fn decompose_tagged_error_probabilities(
    probabilities: BTreeMap<TaggedErrorKey, Probability>,
    block_remnant_edges: bool,
    ignore_failures: bool,
) -> AnalysisResult<BTreeMap<TaggedErrorKey, Probability>> {
    let known_graphlike = known_graphlike_components(
        probabilities
            .iter()
            .map(|((targets, _tag), probability)| (targets, *probability)),
    );
    let mut decomposed = BTreeMap::new();
    for ((targets, tag), probability) in probabilities {
        let targets = if probability.get() == 0.0 {
            // Pinned Stim's rewrite scan skips zero-probability classes
            // (error_analyzer.cc:1519-1521).
            targets
        } else {
            decompose_targets(
                &targets,
                &known_graphlike,
                block_remnant_edges,
                ignore_failures,
            )?
        };
        let key = (targets, tag);
        if let Some(existing) = decomposed.get_mut(&key) {
            *existing = xor_probability(*existing, probability)?;
        } else {
            decomposed.insert(key, probability);
        }
    }
    Ok(decomposed)
}

fn decompose_targets(
    targets: &[DemTarget],
    known_graphlike: &KnownGraphlikeComponents,
    block_remnant_edges: bool,
    ignore_failures: bool,
) -> AnalysisResult<Vec<DemTarget>> {
    if is_graphlike(targets) {
        return Ok(targets.to_vec());
    }

    let mut components = Vec::new();
    for component in split_components(targets) {
        if let Some(decomposition) = exact_decomposition(component, known_graphlike)? {
            components.extend(decomposition);
            continue;
        }
        if !block_remnant_edges
            && let Some(decomposition) = remnant_decomposition(component, known_graphlike)?
        {
            components.extend(decomposition);
            continue;
        }
        if ignore_failures {
            components.push(component.to_vec());
            continue;
        }
        return Err(AnalysisError::invalid_detector_error_model(
            decomposition_failure_message(component, block_remnant_edges),
        ));
    }
    Ok(join_components(&components))
}

fn known_graphlike_components<'a>(
    entries: impl Iterator<Item = (&'a Vec<DemTarget>, Probability)>,
) -> KnownGraphlikeComponents {
    let mut known = BTreeMap::new();
    for (targets, probability) in entries {
        // Pinned Stim's known-symptom map skips zero-probability and empty
        // classes (error_analyzer.cc:1495-1497).
        if probability.get() == 0.0 || targets.is_empty() {
            continue;
        }
        for component in split_components(targets) {
            let key = detector_key(component);
            if matches!(key.len(), 1 | 2) {
                known.insert(key, component.to_vec());
            }
        }
    }
    known
}

fn is_graphlike(targets: &[DemTarget]) -> bool {
    let mut detector_count = 0usize;
    for target in targets {
        match target {
            DemTarget::RelativeDetector(_) => {
                detector_count = detector_count.saturating_add(1);
                if detector_count > 2 {
                    return false;
                }
            }
            DemTarget::LogicalObservable(_) => {}
            DemTarget::Separator => detector_count = 0,
            DemTarget::Numeric(_) => return false,
        }
    }
    true
}

fn is_graphlike_component(targets: &[DemTarget]) -> bool {
    targets
        .iter()
        .filter(|target| matches!(target, DemTarget::RelativeDetector(_)))
        .take(3)
        .count()
        <= 2
}

/// Port of pinned Stim's `brute_force_decomposition_into_known_graphlike_errors`
/// (error_analyzer.cc:1464-1483): an exact within-problem search over the
/// component's own terms under a used-term mask, requiring the observable mask
/// to cancel, trying known pairs before known singles at every branch.
fn exact_decomposition(
    problem: &[DemTarget],
    known_graphlike: &KnownGraphlikeComponents,
) -> AnalysisResult<Option<Vec<Vec<DemTarget>>>> {
    if problem.len() >= MAX_PROBLEM_TERMS {
        return Err(AnalysisError::invalid_detector_error_model(
            "Not implemented: decomposing errors with more than 64 terms.",
        ));
    }
    let (observable_mask, used_term_mask) = observable_masks(problem)?;
    let mut matches = Vec::new();
    if brute_force_decomposition(
        0,
        used_term_mask,
        observable_mask,
        problem,
        known_graphlike,
        &mut matches,
    ) {
        // The recursion accumulates matches on unwind, so discovery order is
        // the reverse (error_analyzer.cc:1477-1480).
        matches.reverse();
        Ok(Some(matches))
    } else {
        Ok(None)
    }
}

/// Observable bitmask plus the used-term mask marking observable positions as
/// consumed, mirroring `obs_mask_of_targets` (error_analyzer.cc:1401-1415).
fn observable_masks(problem: &[DemTarget]) -> AnalysisResult<(u64, u64)> {
    let mut observable_mask = 0_u64;
    let mut used_term_mask = 0_u64;
    for (index, target) in problem.iter().enumerate() {
        match target {
            DemTarget::LogicalObservable(id) => {
                let id = id.get();
                if id >= 64 {
                    return Err(AnalysisError::invalid_detector_error_model(
                        "Not implemented: decomposing errors observable ids larger than 63.",
                    ));
                }
                observable_mask ^= 1_u64 << id;
                used_term_mask |= 1_u64 << index;
            }
            DemTarget::RelativeDetector(_) => {}
            DemTarget::Separator | DemTarget::Numeric(_) => {
                return Err(AnalysisError::invalid_detector_error_model(
                    "error decomposition components cannot contain separators or numeric targets",
                ));
            }
        }
    }
    Ok((observable_mask, used_term_mask))
}

fn observable_mask_of(targets: &[DemTarget]) -> u64 {
    let mut mask = 0_u64;
    for target in targets {
        if let DemTarget::LogicalObservable(id) = target
            && id.get() < 64
        {
            mask ^= 1_u64 << id.get();
        }
    }
    mask
}

fn brute_force_decomposition(
    mut start: usize,
    mut used_term_mask: u64,
    remaining_observable_mask: u64,
    problem: &[DemTarget],
    known_graphlike: &KnownGraphlikeComponents,
    matches: &mut Vec<Vec<DemTarget>>,
) -> bool {
    loop {
        if start >= problem.len() {
            return remaining_observable_mask == 0;
        }
        if (used_term_mask >> start) & 1 == 0 {
            break;
        }
        start += 1;
    }
    used_term_mask |= 1_u64 << start;

    let Some(first) = problem.get(start) else {
        return false;
    };
    for k in start + 1..=problem.len() {
        let key = if let Some(second) = problem.get(k) {
            if (used_term_mask >> k) & 1 == 1 {
                continue;
            }
            used_term_mask ^= 1_u64 << k;
            vec![*first, *second]
        } else {
            vec![*first]
        };
        if let Some(component) = known_graphlike.get(&key) {
            let observable_change = observable_mask_of(component);
            if brute_force_decomposition(
                start + 1,
                used_term_mask,
                remaining_observable_mask ^ observable_change,
                problem,
                known_graphlike,
                matches,
            ) {
                matches.push(component.clone());
                return true;
            }
        }
        if k < problem.len() {
            used_term_mask ^= 1_u64 << k;
        }
    }
    false
}

/// Port of pinned Stim's `decompose_and_append_component_to_tail`
/// (error_analyzer.cc:1335-1399): greedily peel known pairs, then known
/// singles, and accept when at most two detectors remain unmatched, emitting
/// whatever the XOR tracker still holds as the remnant component.
fn remnant_decomposition(
    problem: &[DemTarget],
    known_graphlike: &KnownGraphlikeComponents,
) -> AnalysisResult<Option<Vec<Vec<DemTarget>>>> {
    // Pinned Stim's greedy path passes components with at most two detectors
    // through unchanged before any matching (error_analyzer.cc:1348-1352).
    if is_graphlike_component(problem) {
        return Ok(Some(vec![problem.to_vec()]));
    }
    let mut done = problem
        .iter()
        .map(|target| !matches!(target, DemTarget::RelativeDetector(_)))
        .collect::<Vec<_>>();
    let mut sparse = SparseXor::default();
    sparse.toggle_all(problem)?;
    let mut components = Vec::new();

    for k in 0..problem.len() {
        if done.get(k).copied().unwrap_or(true) {
            continue;
        }
        for k2 in k + 1..problem.len() {
            if done.get(k2).copied().unwrap_or(true) {
                continue;
            }
            let (Some(first), Some(second)) = (problem.get(k), problem.get(k2)) else {
                continue;
            };
            if let Some(component) = known_graphlike.get(&vec![*first, *second]) {
                set_done(&mut done, k);
                set_done(&mut done, k2);
                components.push(component.clone());
                sparse.toggle_all(component)?;
                break;
            }
        }
    }

    let mut missed = 0_usize;
    for k in 0..problem.len() {
        if !done.get(k).copied().unwrap_or(true)
            && let Some(first) = problem.get(k)
            && let Some(component) = known_graphlike.get(&vec![*first])
        {
            set_done(&mut done, k);
            components.push(component.clone());
            sparse.toggle_all(component)?;
        }
        missed += usize::from(!done.get(k).copied().unwrap_or(true));
    }

    if missed <= 2 {
        let remnant = sparse.into_targets();
        if !remnant.is_empty() {
            components.push(remnant);
        }
        Ok(Some(components))
    } else {
        Ok(None)
    }
}

fn set_done(done: &mut [bool], index: usize) {
    if let Some(flag) = done.get_mut(index) {
        *flag = true;
    }
}

#[derive(Default)]
struct SparseXor {
    targets: std::collections::BTreeSet<DemTarget>,
}

impl SparseXor {
    fn toggle_all(&mut self, values: &[DemTarget]) -> AnalysisResult<()> {
        for value in values {
            match value {
                DemTarget::RelativeDetector(_) | DemTarget::LogicalObservable(_) => {
                    if !self.targets.insert(*value) {
                        self.targets.remove(value);
                    }
                }
                DemTarget::Separator => {}
                DemTarget::Numeric(_) => {
                    return Err(AnalysisError::invalid_detector_error_model(
                        "error decomposition cannot use numeric DEM targets",
                    ));
                }
            }
        }
        Ok(())
    }

    fn into_targets(self) -> Vec<DemTarget> {
        self.targets.into_iter().collect()
    }
}

fn split_components(targets: &[DemTarget]) -> impl Iterator<Item = &[DemTarget]> {
    targets.split(|target| matches!(target, DemTarget::Separator))
}

fn detector_key(targets: &[DemTarget]) -> Vec<DemTarget> {
    targets
        .iter()
        .copied()
        .filter(|target| matches!(target, DemTarget::RelativeDetector(_)))
        .collect()
}

fn join_components(components: &[Vec<DemTarget>]) -> Vec<DemTarget> {
    let total_len = components
        .iter()
        .map(Vec::len)
        .sum::<usize>()
        .saturating_add(components.len().saturating_sub(1));
    let mut joined = Vec::with_capacity(total_len);
    for component in components {
        if !joined.is_empty() {
            joined.push(DemTarget::separator());
        }
        joined.extend_from_slice(component);
    }
    joined
}

fn decomposition_failure_message(component: &[DemTarget], block_remnant_edges: bool) -> String {
    let mut message = format!(
        "Failed to decompose errors into graphlike components with at most two symptoms.\nThe error component that failed to decompose is '{}'.",
        format_component(component)
    );
    if block_remnant_edges {
        message.push_str(
            "\n\nNote: `block_decomposition_from_introducing_remnant_edges` is ON.\nTurning it off may prevent this error.",
        );
    }
    message
}

fn format_component(targets: &[DemTarget]) -> String {
    targets
        .iter()
        .filter(|target| !matches!(target, DemTarget::Separator))
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}
