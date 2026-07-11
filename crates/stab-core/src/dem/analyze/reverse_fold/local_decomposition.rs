use std::collections::{BTreeMap, BTreeSet};

use crate::{CircuitError, CircuitResult, DemTarget, Probability};

pub(super) fn locally_decompose_combinations(
    basis_errors: &[BTreeSet<DemTarget>],
    combinations: &mut [Vec<DemTarget>],
) -> CircuitResult<()> {
    let mut involved_detectors = BTreeMap::new();
    for basis in basis_errors {
        for target in basis {
            if let DemTarget::RelativeDetector(detector) = target {
                let next = involved_detectors.len();
                if !involved_detectors.contains_key(detector) {
                    if next >= 16 {
                        return Err(CircuitError::invalid_detector_error_model(
                            "an error case in a composite error exceeded 16 detector symptoms",
                        ));
                    }
                    involved_detectors.insert(*detector, next);
                }
            }
        }
    }

    let mut detector_masks = vec![0_u64; combinations.len()];
    for (slot, targets) in detector_masks.iter_mut().zip(combinations.iter()).skip(1) {
        let mut mask = 0_u64;
        for target in targets {
            if let DemTarget::RelativeDetector(detector) = target {
                let bit = involved_detectors.get(detector).copied().ok_or_else(|| {
                    CircuitError::invalid_detector_error_model(
                        "composite error detector has no local mask",
                    )
                })?;
                mask ^= 1_u64 << bit;
            }
        }
        *slot = mask;
    }

    let detector_counts = detector_masks
        .iter()
        .map(|mask| mask.count_ones())
        .collect::<Vec<_>>();
    let mut solved = vec![false; combinations.len()];
    let mut single_detector_union = 0_u64;
    for ((detector_count, detector_mask), solved_slot) in detector_counts
        .iter()
        .zip(&detector_masks)
        .zip(&mut solved)
        .skip(1)
    {
        if *detector_count == 1 {
            single_detector_union |= *detector_mask;
            *solved_slot = true;
        }
    }
    let mut irreducible_pairs = Vec::new();
    for (index, ((detector_count, detector_mask), solved_slot)) in detector_counts
        .iter()
        .zip(&detector_masks)
        .zip(&mut solved)
        .enumerate()
        .skip(1)
    {
        if *detector_count == 2 && *detector_mask & !single_detector_union != 0 {
            irreducible_pairs.push(index);
            *solved_slot = true;
        }
    }

    for goal_index in 1..combinations.len() {
        let detector_count = *indexed(
            &detector_counts,
            goal_index,
            "composite error detector count",
        )?;
        let is_solved = *indexed(&solved, goal_index, "composite error solved state")?;
        if detector_count == 0 || is_solved {
            continue;
        }
        let goal = *indexed(&detector_masks, goal_index, "composite error detector mask")?;
        let mut components = Vec::new();
        let mut remnants = if goal & !single_detector_union == 0 {
            goal
        } else {
            let mut contained_pair = None;
            for &pair in &irreducible_pairs {
                let pair_mask = *indexed(
                    &detector_masks,
                    pair,
                    "irreducible composite error pair mask",
                )?;
                if goal & pair_mask == pair_mask && goal & !(single_detector_union | pair_mask) == 0
                {
                    contained_pair = Some((pair, pair_mask));
                    break;
                }
            }
            if let Some((pair, pair_mask)) = contained_pair {
                components.push(
                    indexed(combinations, pair, "irreducible composite error component")?.clone(),
                );
                goal & !pair_mask
            } else if let Some((left, right)) = find_two_disjoint_pairs(
                goal,
                single_detector_union,
                &irreducible_pairs,
                &detector_masks,
            )? {
                let left_component =
                    indexed(combinations, left, "left composite error pair")?.clone();
                let right_component =
                    indexed(combinations, right, "right composite error pair")?.clone();
                if left_component <= right_component {
                    components.push(left_component);
                    components.push(right_component);
                } else {
                    components.push(right_component);
                    components.push(left_component);
                }
                let left_mask = *indexed(&detector_masks, left, "left composite error pair mask")?;
                let right_mask =
                    *indexed(&detector_masks, right, "right composite error pair mask")?;
                goal & !(left_mask | right_mask)
            } else {
                continue;
            }
        };

        while remnants != 0 {
            let mut single_match = None;
            for index in 1..combinations.len() {
                let detector_count = *indexed(
                    &detector_counts,
                    index,
                    "single composite error detector count",
                )?;
                let detector_mask = *indexed(
                    &detector_masks,
                    index,
                    "single composite error detector mask",
                )?;
                if detector_count == 1 && detector_mask & !remnants == 0 {
                    single_match = Some((index, detector_mask));
                    break;
                }
            }
            let Some((single, detector_mask)) = single_match else {
                return Err(CircuitError::invalid_detector_error_model(
                    "composite error local decomposition left an unsolved detector",
                ));
            };
            remnants &= !detector_mask;
            components
                .push(indexed(combinations, single, "single composite error component")?.clone());
        }
        *indexed_mut(
            combinations,
            goal_index,
            "decomposed composite error component",
        )? = join_error_components(&components);
    }
    Ok(())
}

fn find_two_disjoint_pairs(
    goal: u64,
    single_detector_union: u64,
    pairs: &[usize],
    masks: &[u64],
) -> CircuitResult<Option<(usize, usize)>> {
    for (position, &left) in pairs.iter().enumerate() {
        for &right in pairs.iter().skip(position + 1) {
            let left_mask = *indexed(masks, left, "left irreducible detector mask")?;
            let right_mask = *indexed(masks, right, "right irreducible detector mask")?;
            if left_mask & right_mask == 0
                && goal & !(single_detector_union | left_mask | right_mask) == 0
            {
                return Ok(Some((left, right)));
            }
        }
    }
    Ok(None)
}

fn join_error_components(components: &[Vec<DemTarget>]) -> Vec<DemTarget> {
    let mut joined = Vec::new();
    for (index, component) in components.iter().enumerate() {
        if index > 0 {
            joined.push(DemTarget::separator());
        }
        joined.extend(component.iter().copied());
    }
    joined
}

pub(super) fn merge_indistinguishable_disjoint_probabilities(
    targets: &[Vec<DemTarget>],
    probabilities: &mut [Probability],
) -> CircuitResult<()> {
    if targets.len() != probabilities.len() {
        return Err(CircuitError::invalid_detector_error_model(
            "disjoint probability and target table lengths differ",
        ));
    }
    for mask in 1..targets.len() {
        if !indexed(targets, mask, "disjoint target mask")?.is_empty() {
            continue;
        }
        for destination in 0..targets.len() {
            let source = destination ^ mask;
            if source > destination {
                let destination_probability = *indexed(
                    probabilities,
                    destination,
                    "disjoint destination probability",
                )?;
                let source_probability =
                    *indexed(probabilities, source, "disjoint source probability")?;
                *indexed_mut(
                    probabilities,
                    destination,
                    "disjoint destination probability",
                )? =
                    Probability::try_new(destination_probability.get() + source_probability.get())?;
                *indexed_mut(probabilities, source, "disjoint source probability")? =
                    Probability::try_new(0.0)?;
            }
        }
    }
    Ok(())
}

fn indexed<'a, T>(values: &'a [T], index: usize, context: &str) -> CircuitResult<&'a T> {
    values.get(index).ok_or_else(|| {
        CircuitError::invalid_detector_error_model(format!(
            "folded analyzer {context} index {index} is out of range"
        ))
    })
}

fn indexed_mut<'a, T>(
    values: &'a mut [T],
    index: usize,
    context: &str,
) -> CircuitResult<&'a mut T> {
    values.get_mut(index).ok_or_else(|| {
        CircuitError::invalid_detector_error_model(format!(
            "folded analyzer {context} index {index} is out of range"
        ))
    })
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::expect_used,
        clippy::panic_in_result_fn,
        reason = "unit tests use direct assertions for compact boundary diagnostics"
    )]

    use super::*;

    fn detector_basis_errors(count: u64) -> CircuitResult<Vec<BTreeSet<DemTarget>>> {
        (0..count)
            .map(|detector| Ok(BTreeSet::from([DemTarget::relative_detector(detector)?])))
            .collect()
    }

    #[test]
    fn local_decomposition_accepts_sixteen_detector_symptoms() -> CircuitResult<()> {
        let basis_errors = detector_basis_errors(16)?;
        locally_decompose_combinations(&basis_errors, &mut [Vec::new()])
    }

    #[test]
    fn local_decomposition_rejects_seventeen_detector_symptoms() -> CircuitResult<()> {
        let basis_errors = detector_basis_errors(17)?;
        let error = locally_decompose_combinations(&basis_errors, &mut [Vec::new()])
            .expect_err("seventeen detector symptoms must exceed the local mask contract");
        assert!(error.to_string().contains("exceeded 16 detector symptoms"));
        Ok(())
    }
}
