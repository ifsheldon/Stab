use std::collections::{BTreeMap, BTreeSet};

use crate::{CircuitError, CircuitResult, Probability};

use super::xor_probability;
use crate::DemTarget;

pub(super) fn decompose_error_probabilities(
    probabilities: BTreeMap<Vec<DemTarget>, Probability>,
) -> CircuitResult<BTreeMap<Vec<DemTarget>, Probability>> {
    let known_graphlike = probabilities
        .keys()
        .filter(|targets| is_graphlike(targets))
        .cloned()
        .collect::<Vec<_>>();
    let mut decomposed = BTreeMap::new();
    for (targets, probability) in probabilities {
        let targets = decompose_targets(targets, &known_graphlike)?;
        if let Some(existing) = decomposed.get_mut(&targets) {
            *existing = xor_probability(*existing, probability)?;
        } else {
            decomposed.insert(targets, probability);
        }
    }
    Ok(decomposed)
}

fn decompose_targets(
    targets: Vec<DemTarget>,
    known_graphlike: &[Vec<DemTarget>],
) -> CircuitResult<Vec<DemTarget>> {
    if is_graphlike(&targets) {
        return Ok(targets);
    }
    for known in known_graphlike {
        let remnant = symmetric_difference(&targets, known)?;
        if remnant.is_empty() {
            continue;
        }
        if is_graphlike_component(&remnant) {
            return Ok(join_components(known, &remnant));
        }
    }
    Err(CircuitError::invalid_detector_error_model(format!(
        "Failed to decompose errors into graphlike components with at most two symptoms.\nThe error component that failed to decompose is '{}'.",
        format_component(&targets)
    )))
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

fn symmetric_difference(left: &[DemTarget], right: &[DemTarget]) -> CircuitResult<Vec<DemTarget>> {
    let mut targets = BTreeSet::new();
    toggle_targets(&mut targets, left)?;
    toggle_targets(&mut targets, right)?;
    Ok(targets.into_iter().collect())
}

fn toggle_targets(targets: &mut BTreeSet<DemTarget>, values: &[DemTarget]) -> CircuitResult<()> {
    for value in values {
        match value {
            DemTarget::RelativeDetector(_) | DemTarget::LogicalObservable(_) => {
                if !targets.insert(*value) {
                    targets.remove(value);
                }
            }
            DemTarget::Separator => {}
            DemTarget::Numeric(_) => {
                return Err(CircuitError::invalid_detector_error_model(
                    "error decomposition cannot use numeric DEM targets",
                ));
            }
        }
    }
    Ok(())
}

fn join_components(first: &[DemTarget], second: &[DemTarget]) -> Vec<DemTarget> {
    let mut joined = Vec::with_capacity(first.len() + 1 + second.len());
    joined.extend_from_slice(first);
    joined.push(DemTarget::separator());
    joined.extend_from_slice(second);
    joined
}

fn format_component(targets: &[DemTarget]) -> String {
    targets
        .iter()
        .filter(|target| !matches!(target, DemTarget::Separator))
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}
