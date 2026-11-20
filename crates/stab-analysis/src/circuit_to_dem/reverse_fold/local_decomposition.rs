use stab_model::{DemDetectorId, DemTarget, Probability};

use crate::AnalysisOperation;
use crate::{AnalysisError, AnalysisResult};

/// Pinned Stim assigns each detector of a composite error one bit of a 16-bit local mask.
const MAX_LOCAL_DETECTORS: usize = 16;

/// Composite channels combine at most four basis errors, so at most sixteen combinations.
const MAX_COMBINATIONS: usize = 1 << super::MAX_COMBINATION_BASES;
const _: () = assert!(MAX_COMBINATIONS <= u16::BITS as usize);

/// A set of combination indexes, iterated in ascending order.
#[derive(Clone, Copy, Debug, Default)]
struct CombinationSet(u16);

impl CombinationSet {
    fn insert(&mut self, index: usize) {
        self.0 |= 1 << index;
    }

    fn iter(self) -> impl Iterator<Item = usize> {
        (0..MAX_COMBINATIONS).filter(move |&index| (self.0 >> index) & 1 == 1)
    }
}

/// Rewrites composite-error combinations as graphlike components where the basis errors allow.
///
/// `joined` is a reused buffer; each rewritten combination swaps storage with it.
pub(super) fn locally_decompose_combinations<B: AsRef<[DemTarget]>>(
    basis_errors: &[B],
    combinations: &mut [Vec<DemTarget>],
    joined: &mut Vec<DemTarget>,
) -> AnalysisResult<()> {
    // First-seen order assigns the mask bits, like the ordered insertion it ports.
    let mut involved_detectors = [None::<DemDetectorId>; MAX_LOCAL_DETECTORS];
    let mut involved_count = 0;
    for basis in basis_errors {
        for target in basis.as_ref() {
            if let Some(detector) = target.detector()
                && !involved_detectors
                    .iter()
                    .take(involved_count)
                    .any(|seen| *seen == Some(detector))
            {
                let slot = involved_detectors.get_mut(involved_count).ok_or_else(|| {
                    AnalysisError::unsupported(
                        AnalysisOperation::CircuitToDetectorErrorModel,
                        None,
                        "an error case in a composite error exceeded 16 detector symptoms",
                    )
                })?;
                *slot = Some(detector);
                involved_count += 1;
            }
        }
    }

    if combinations.len() > MAX_COMBINATIONS {
        return Err(AnalysisError::unsupported(
            AnalysisOperation::CircuitToDetectorErrorModel,
            None,
            format!(
                "composite error has {} combinations, more than the {MAX_COMBINATIONS} supported",
                combinations.len()
            ),
        ));
    }
    let mut detector_masks = [0_u64; MAX_COMBINATIONS];
    for (slot, targets) in detector_masks.iter_mut().zip(combinations.iter()).skip(1) {
        let mut mask = 0_u64;
        for target in targets {
            if let Some(detector) = target.detector() {
                let bit = involved_detectors
                    .iter()
                    .take(involved_count)
                    .position(|seen| *seen == Some(detector))
                    .ok_or_else(|| {
                        AnalysisError::internal(
                            AnalysisOperation::CircuitToDetectorErrorModel,
                            "composite error detector has no local mask",
                        )
                    })?;
                mask ^= 1_u64 << bit;
            }
        }
        *slot = mask;
    }

    let detector_counts = detector_masks.map(u64::count_ones);
    let mut solved = [false; MAX_COMBINATIONS];
    let mut single_detector_union = 0_u64;
    for ((detector_count, detector_mask), solved_slot) in detector_counts
        .iter()
        .zip(&detector_masks)
        .zip(&mut solved)
        .take(combinations.len())
        .skip(1)
    {
        if *detector_count == 1 {
            single_detector_union |= *detector_mask;
            *solved_slot = true;
        }
    }
    let mut irreducible_pairs = CombinationSet::default();
    for (index, ((detector_count, detector_mask), solved_slot)) in detector_counts
        .iter()
        .zip(&detector_masks)
        .zip(&mut solved)
        .enumerate()
        .take(combinations.len())
        .skip(1)
    {
        if *detector_count == 2 && *detector_mask & !single_detector_union != 0 {
            irreducible_pairs.insert(index);
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
        joined.clear();
        let mut remnants = if goal & !single_detector_union == 0 {
            goal
        } else {
            let mut contained_pair = None;
            for pair in irreducible_pairs.iter() {
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
                push_component(
                    joined,
                    indexed(combinations, pair, "irreducible composite error component")?,
                );
                goal & !pair_mask
            } else if let Some((left, right)) = find_two_disjoint_pairs(
                goal,
                single_detector_union,
                irreducible_pairs,
                &detector_masks,
            )? {
                let left_component = indexed(combinations, left, "left composite error pair")?;
                let right_component = indexed(combinations, right, "right composite error pair")?;
                if left_component <= right_component {
                    push_component(joined, left_component);
                    push_component(joined, right_component);
                } else {
                    push_component(joined, right_component);
                    push_component(joined, left_component);
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
                return Err(AnalysisError::internal(
                    AnalysisOperation::CircuitToDetectorErrorModel,
                    "composite error local decomposition left an unsolved detector",
                ));
            };
            remnants &= !detector_mask;
            push_component(
                joined,
                indexed(combinations, single, "single composite error component")?,
            );
        }
        // Components are solved singles and pairs, which are never rewritten, so reading them
        // before replacing the goal matches joining copies taken up front.
        std::mem::swap(
            indexed_mut(
                combinations,
                goal_index,
                "decomposed composite error component",
            )?,
            joined,
        );
    }
    Ok(())
}

fn find_two_disjoint_pairs(
    goal: u64,
    single_detector_union: u64,
    pairs: CombinationSet,
    masks: &[u64],
) -> AnalysisResult<Option<(usize, usize)>> {
    for left in pairs.iter() {
        for right in pairs.iter().filter(|&right| right > left) {
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

/// Appends one component, separated from any previous component.
fn push_component(joined: &mut Vec<DemTarget>, component: &[DemTarget]) {
    if !joined.is_empty() {
        joined.push(DemTarget::separator());
    }
    joined.extend_from_slice(component);
}

pub(super) fn merge_indistinguishable_disjoint_probabilities(
    targets: &[Vec<DemTarget>],
    probabilities: &mut [Probability],
) -> AnalysisResult<()> {
    if targets.len() != probabilities.len() {
        return Err(AnalysisError::internal(
            AnalysisOperation::CircuitToDetectorErrorModel,
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
                let source_probability =
                    *indexed(probabilities, source, "disjoint source probability")?;
                if destination != 0 {
                    let destination_probability = *indexed(
                        probabilities,
                        destination,
                        "disjoint destination probability",
                    )?;
                    // Re-associating validated disjoint terms can overshoot 1.0 by a few ULPs.
                    let sum = destination_probability.get() + source_probability.get();
                    let clamped = if sum > 1.0 && sum - 1.0 <= 16.0 * f64::EPSILON {
                        1.0
                    } else {
                        sum
                    };
                    *indexed_mut(
                        probabilities,
                        destination,
                        "disjoint destination probability",
                    )? = Probability::try_new(clamped)?;
                }
                *indexed_mut(probabilities, source, "disjoint source probability")? =
                    Probability::try_new(0.0)?;
            }
        }
    }
    Ok(())
}

fn indexed<'a, T>(values: &'a [T], index: usize, context: &str) -> AnalysisResult<&'a T> {
    values.get(index).ok_or_else(|| {
        AnalysisError::internal(
            AnalysisOperation::CircuitToDetectorErrorModel,
            format!("folded analyzer {context} index {index} is out of range"),
        )
    })
}

fn indexed_mut<'a, T>(
    values: &'a mut [T],
    index: usize,
    context: &str,
) -> AnalysisResult<&'a mut T> {
    values.get_mut(index).ok_or_else(|| {
        AnalysisError::internal(
            AnalysisOperation::CircuitToDetectorErrorModel,
            format!("folded analyzer {context} index {index} is out of range"),
        )
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

    fn detector_basis_errors(count: u64) -> AnalysisResult<Vec<Vec<DemTarget>>> {
        (0..count)
            .map(|detector| Ok(vec![DemTarget::relative_detector(detector)?]))
            .collect()
    }

    #[test]
    fn local_decomposition_accepts_sixteen_detector_symptoms() -> AnalysisResult<()> {
        let basis_errors = detector_basis_errors(16)?;
        locally_decompose_combinations(&basis_errors, &mut [Vec::new()], &mut Vec::new())
    }

    #[test]
    fn local_decomposition_rejects_seventeen_detector_symptoms() -> AnalysisResult<()> {
        let basis_errors = detector_basis_errors(17)?;
        let error =
            locally_decompose_combinations(&basis_errors, &mut [Vec::new()], &mut Vec::new())
                .expect_err("seventeen detector symptoms must exceed the local mask contract");
        assert!(error.to_string().contains("exceeded 16 detector symptoms"));
        Ok(())
    }
}
