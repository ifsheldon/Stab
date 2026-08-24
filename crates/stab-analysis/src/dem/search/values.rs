use std::collections::{BTreeMap, BTreeSet};

use stab_model::{DemDetectorId, DemObservableId, DemTarget};

use crate::{AnalysisError, AnalysisResult};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum SearchGraphKind {
    Graphlike,
    Hypergraph,
}

impl SearchGraphKind {
    const fn name(self) -> &'static str {
        match self {
            Self::Graphlike => "graphlike",
            Self::Hypergraph => "hypergraph",
        }
    }
}

#[derive(Clone, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(super) struct ObservableMask {
    pub(super) observables: BTreeSet<DemObservableId>,
}

impl ObservableMask {
    pub(super) fn new() -> Self {
        Self::default()
    }

    pub(super) fn toggle(&mut self, observable: DemObservableId) {
        if !self.observables.insert(observable) {
            self.observables.remove(&observable);
        }
    }

    pub(super) fn symmetric_difference(&self, other: &Self) -> Self {
        let mut observables = self.observables.clone();
        for observable in &other.observables {
            if !observables.insert(*observable) {
                observables.remove(observable);
            }
        }
        Self { observables }
    }

    pub(super) fn symmetric_difference_len(&self, other: &Self) -> usize {
        self.observables
            .symmetric_difference(&other.observables)
            .count()
    }

    pub(super) fn len(&self) -> usize {
        self.observables.len()
    }

    pub(super) fn is_empty(&self) -> bool {
        self.observables.is_empty()
    }

    pub(super) fn iter(&self) -> impl Iterator<Item = DemObservableId> + '_ {
        self.observables.iter().copied()
    }

    pub(super) fn push_targets(&self, targets: &mut Vec<DemTarget>) -> AnalysisResult<()> {
        for observable in &self.observables {
            targets.push(DemTarget::logical_observable(observable.get())?);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum DetectorIndex {
    #[cfg(test)]
    Identity,
    Sparse {
        node_to_detector: Vec<DemDetectorId>,
        detector_to_node: BTreeMap<DemDetectorId, usize>,
    },
}

impl DetectorIndex {
    #[cfg(test)]
    pub(super) const fn identity() -> Self {
        Self::Identity
    }

    pub(super) fn sparse(detectors: BTreeSet<DemDetectorId>) -> Self {
        let node_to_detector: Vec<_> = detectors.into_iter().collect();
        let detector_to_node = node_to_detector
            .iter()
            .copied()
            .enumerate()
            .map(|(index, detector)| (detector, index))
            .collect();
        Self::Sparse {
            node_to_detector,
            detector_to_node,
        }
    }

    pub(super) fn detector_for_node_index(
        &self,
        index: usize,
        kind: SearchGraphKind,
    ) -> AnalysisResult<DemDetectorId> {
        match self {
            #[cfg(test)]
            Self::Identity => {
                let index = u64::try_from(index).map_err(|_| {
                    AnalysisError::invalid_detector_error_model(format!(
                        "{} node index does not fit detector id",
                        kind.name()
                    ))
                })?;
                DemDetectorId::try_new(index).map_err(Into::into)
            }
            Self::Sparse {
                node_to_detector, ..
            } => node_to_detector.get(index).copied().ok_or_else(|| {
                AnalysisError::invalid_detector_error_model(format!(
                    "{} sparse node index {index} is outside the graph",
                    kind.name()
                ))
            }),
        }
    }

    pub(super) fn node_index_for_detector(
        &self,
        detector: DemDetectorId,
        kind: SearchGraphKind,
    ) -> AnalysisResult<usize> {
        match self {
            #[cfg(test)]
            Self::Identity => usize::try_from(detector.get()).map_err(|_| {
                AnalysisError::invalid_detector_error_model(format!(
                    "{} detector D{} does not fit usize",
                    kind.name(),
                    detector.get()
                ))
            }),
            Self::Sparse {
                detector_to_node, ..
            } => detector_to_node.get(&detector).copied().ok_or_else(|| {
                AnalysisError::invalid_detector_error_model(format!(
                    "{} detector D{} is outside the sparse graph",
                    kind.name(),
                    detector.get()
                ))
            }),
        }
    }
}
