use std::collections::{BTreeMap, BTreeSet};

use crate::{CircuitError, CircuitResult, DemTarget, Probability};

use super::probabilities::xor_probability;

type KnownGraphlikeComponents = BTreeMap<Vec<DemTarget>, Vec<DemTarget>>;

pub(super) fn decompose_error_probabilities(
    probabilities: BTreeMap<Vec<DemTarget>, Probability>,
    block_remnant_edges: bool,
    ignore_failures: bool,
) -> CircuitResult<BTreeMap<Vec<DemTarget>, Probability>> {
    let known_graphlike = known_graphlike_components(probabilities.keys());
    let mut decomposed = BTreeMap::new();
    for (targets, probability) in probabilities {
        let targets = decompose_targets(
            &targets,
            &known_graphlike,
            block_remnant_edges,
            ignore_failures,
        )?;
        if let Some(existing) = decomposed.get_mut(&targets) {
            *existing = xor_probability(*existing, probability)?;
        } else {
            decomposed.insert(targets, probability);
        }
    }
    Ok(decomposed)
}

fn decompose_targets(
    targets: &[DemTarget],
    known_graphlike: &KnownGraphlikeComponents,
    block_remnant_edges: bool,
    ignore_failures: bool,
) -> CircuitResult<Vec<DemTarget>> {
    if is_graphlike(targets) {
        return Ok(targets.to_vec());
    }

    let mut components = Vec::new();
    for component in split_components(targets) {
        if is_graphlike_component(component) {
            components.push(component.to_vec());
            continue;
        }
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
        return Err(CircuitError::invalid_detector_error_model(
            decomposition_failure_message(component, block_remnant_edges),
        ));
    }
    Ok(join_components(&components))
}

fn known_graphlike_components<'a>(
    targets: impl Iterator<Item = &'a Vec<DemTarget>>,
) -> KnownGraphlikeComponents {
    let mut known = BTreeMap::new();
    for targets in targets {
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

fn exact_decomposition(
    problem: &[DemTarget],
    known_graphlike: &KnownGraphlikeComponents,
) -> CircuitResult<Option<Vec<Vec<DemTarget>>>> {
    let mut remaining = BTreeSet::new();
    toggle_targets(&mut remaining, problem)?;
    let candidates = known_graphlike.values().cloned().collect::<Vec<_>>();
    let mut output = Vec::new();
    let mut visited = BTreeSet::new();
    if decompose_remaining(&mut remaining, &candidates, &mut output, &mut visited)? {
        Ok(Some(output))
    } else {
        Ok(None)
    }
}

fn decompose_remaining(
    remaining: &mut BTreeSet<DemTarget>,
    candidates: &[Vec<DemTarget>],
    output: &mut Vec<Vec<DemTarget>>,
    visited: &mut BTreeSet<Vec<DemTarget>>,
) -> CircuitResult<bool> {
    if remaining.is_empty() {
        return Ok(true);
    }
    if !visited.insert(remaining.iter().copied().collect()) {
        return Ok(false);
    }
    let Some(pivot) = remaining
        .iter()
        .copied()
        .find(|target| matches!(target, DemTarget::RelativeDetector(_)))
    else {
        return Ok(false);
    };

    for candidate in candidates {
        if !candidate.contains(&pivot) {
            continue;
        }
        toggle_targets(remaining, candidate)?;
        output.push(candidate.clone());
        if decompose_remaining(remaining, candidates, output, visited)? {
            return Ok(true);
        }
        output.pop();
        toggle_targets(remaining, candidate)?;
    }
    Ok(false)
}

fn remnant_decomposition(
    problem: &[DemTarget],
    known_graphlike: &KnownGraphlikeComponents,
) -> CircuitResult<Option<Vec<Vec<DemTarget>>>> {
    for known in known_graphlike.values() {
        let remnant = symmetric_difference(problem, known)?;
        if remnant.is_empty() {
            continue;
        }
        if is_graphlike_component(&remnant) {
            return Ok(Some(vec![known.clone(), remnant]));
        }
    }
    Ok(None)
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
