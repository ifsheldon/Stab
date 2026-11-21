use std::collections::{BTreeMap, BTreeSet, VecDeque};

use stab_model::{
    DemDetectorId, DemInstruction, DemItem, DemTarget, DetectorErrorModel, Probability,
};

use super::{
    arena_index::ArenaIndex,
    budget::{GraphConstructionBudget, LogicalErrorSearchLimits, SearchBudget},
    error_traversal::{
        SearchGraphTargetPolicy, search_graph_nonzero_error_targets,
        visit_search_graph_errors_with_limits,
    },
    graphlike::two_detector_hyperedge_search,
    outcome::{HypergraphSearchOptions, LogicalErrorSearch, NoLogicalErrorReport, SymptomGrowth},
    traversal::{FoldedDemTraversal, shifted_targets},
    values::{DetectorIndex, ObservableMask, SearchGraphKind, folded_error_targets},
};
use crate::AnalysisOperation;
use crate::resources::LogicalErrorSearchResource;
use crate::{AnalysisError, AnalysisResult, ResourceLimitError};

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct Edge {
    detectors: BTreeSet<DemDetectorId>,
    observables: ObservableMask,
}

impl Edge {
    fn new(detectors: BTreeSet<DemDetectorId>, observables: ObservableMask) -> Self {
        Self {
            detectors,
            observables,
        }
    }

    fn term_count(&self) -> AnalysisResult<usize> {
        self.detectors
            .len()
            .checked_add(self.observables.len())
            .ok_or_else(|| {
                AnalysisError::count_overflow(
                    AnalysisOperation::LogicalErrorSearch,
                    "hypergraph edge term count overflowed",
                )
            })
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct Node {
    edge_ids: Vec<usize>,
    edge_id_index: BTreeSet<usize>,
}

impl Node {
    fn add_edge_id(&mut self, edge_id: usize) -> AnalysisResult<bool> {
        if self.edge_id_index.contains(&edge_id) {
            return Ok(false);
        }
        self.edge_ids.try_reserve(1).map_err(|_| {
            AnalysisError::allocation_failed(
                AnalysisOperation::LogicalErrorSearch,
                "hypergraph search cannot allocate another edge incidence",
            )
        })?;
        self.edge_id_index.insert(edge_id);
        self.edge_ids.push(edge_id);
        Ok(true)
    }
}

#[derive(Clone, Debug)]
struct Graph {
    nodes: Vec<Node>,
    edges: Vec<Edge>,
    edge_index: ArenaIndex,
    edge_incidences: usize,
    detector_index: DetectorIndex,
    has_declared_detectors: bool,
    num_observables: usize,
    distance_1_error_mask: ObservableMask,
    construction_budget: GraphConstructionBudget,
}

impl PartialEq for Graph {
    fn eq(&self, other: &Self) -> bool {
        self.nodes == other.nodes
            && self.edges == other.edges
            && self.edge_incidences == other.edge_incidences
            && self.detector_index == other.detector_index
            && self.has_declared_detectors == other.has_declared_detectors
            && self.num_observables == other.num_observables
            && self.distance_1_error_mask == other.distance_1_error_mask
    }
}

impl Eq for Graph {}

impl Graph {
    #[cfg(test)]
    fn new(node_count: usize, num_observables: usize) -> Self {
        let limits = LogicalErrorSearchLimits {
            max_unique_graph_edges: 64,
            max_stored_graph_terms: 2_048,
            max_hyperedge_degree: 64,
            max_hyperedge_incidences: 256,
            ..LogicalErrorSearchLimits::default()
        };
        Self::new_with_limits(node_count, num_observables, limits)
    }

    #[cfg(test)]
    fn new_with_limits(
        node_count: usize,
        num_observables: usize,
        limits: LogicalErrorSearchLimits,
    ) -> Self {
        Self {
            nodes: vec![Node::default(); node_count],
            edges: Vec::new(),
            edge_index: ArenaIndex::new(),
            edge_incidences: 0,
            detector_index: DetectorIndex::identity(),
            has_declared_detectors: node_count > 0,
            num_observables,
            distance_1_error_mask: ObservableMask::new(),
            construction_budget: GraphConstructionBudget::new("hypergraph search", limits),
        }
    }

    #[cfg(test)]
    fn try_new_sparse(
        detectors: BTreeSet<DemDetectorId>,
        num_observables: usize,
        has_declared_detectors: bool,
    ) -> AnalysisResult<Self> {
        Self::try_new_sparse_with_limits(
            detectors,
            num_observables,
            has_declared_detectors,
            LogicalErrorSearchLimits::default(),
        )
    }

    fn try_new_sparse_with_limits(
        detectors: BTreeSet<DemDetectorId>,
        num_observables: usize,
        has_declared_detectors: bool,
        limits: LogicalErrorSearchLimits,
    ) -> AnalysisResult<Self> {
        let node_count = detectors.len();
        let mut nodes = Vec::new();
        nodes.try_reserve_exact(node_count).map_err(|_| {
            AnalysisError::allocation_failed(
                AnalysisOperation::LogicalErrorSearch,
                format!("hypergraph search cannot allocate {node_count} sparse detector nodes"),
            )
        })?;
        nodes.resize(node_count, Node::default());

        Ok(Self {
            nodes,
            edges: Vec::new(),
            edge_index: ArenaIndex::new(),
            edge_incidences: 0,
            detector_index: DetectorIndex::sparse(detectors),
            has_declared_detectors,
            num_observables,
            distance_1_error_mask: ObservableMask::new(),
            construction_budget: GraphConstructionBudget::new("hypergraph search", limits),
        })
    }

    #[cfg(test)]
    fn from_parts(
        node_edges: Vec<Vec<Edge>>,
        num_observables: usize,
        distance_1_error_mask: ObservableMask,
    ) -> AnalysisResult<Self> {
        let mut graph = Self::new(node_edges.len(), num_observables);
        graph.distance_1_error_mask = distance_1_error_mask;
        for (node_index, edges) in node_edges.into_iter().enumerate() {
            for edge in edges {
                let (edge_id, inserted) = graph.intern_edge(edge, 2)?;
                if !inserted {
                    graph.construction_budget.admit_adjacency(2)?;
                }
                let node = graph.nodes.get_mut(node_index).ok_or_else(|| {
                    AnalysisError::internal(
                        AnalysisOperation::LogicalErrorSearch,
                        "hypergraph test node index is outside the graph",
                    )
                })?;
                if node.add_edge_id(edge_id)? {
                    graph.edge_incidences = graph.edge_incidences.saturating_add(1);
                }
            }
        }
        Ok(graph)
    }

    fn edge(&self, edge_id: usize) -> AnalysisResult<&Edge> {
        self.edges.get(edge_id).ok_or_else(|| {
            AnalysisError::internal(
                AnalysisOperation::LogicalErrorSearch,
                format!("hypergraph edge index {edge_id} is outside the edge arena"),
            )
        })
    }

    fn intern_edge(
        &mut self,
        edge: Edge,
        adjacency_stored_terms: usize,
    ) -> AnalysisResult<(usize, bool)> {
        if let Some(edge_id) = self.edge_index.find(&edge, &self.edges) {
            return Ok((edge_id, false));
        }
        let edge_hash = self.edge_index.hash(&edge);
        let edge_id = self.edges.len();
        let stored_index_and_adjacency_terms =
            adjacency_stored_terms.checked_add(1).ok_or_else(|| {
                AnalysisError::count_overflow(
                    AnalysisOperation::LogicalErrorSearch,
                    "hypergraph stored graph index count overflowed",
                )
            })?;
        let admission = self.construction_budget.preflight_unique_edge(
            edge.term_count()?,
            1,
            stored_index_and_adjacency_terms,
        )?;
        self.edges.try_reserve(1).map_err(|_| {
            AnalysisError::allocation_failed(
                AnalysisOperation::LogicalErrorSearch,
                "hypergraph search cannot allocate another edge",
            )
        })?;
        self.edge_index
            .try_reserve(&self.edges, "hypergraph search")?;
        self.edges.push(edge);
        self.edge_index
            .insert_reserved(edge_hash, edge_id, &self.edges);
        self.construction_budget.commit_unique_edge(admission)?;
        Ok((edge_id, true))
    }

    fn detector_for_node_index(&self, index: usize) -> AnalysisResult<DemDetectorId> {
        self.detector_index
            .detector_for_node_index(index, SearchGraphKind::Hypergraph)
    }

    fn node_index_for_detector(&self, detector: DemDetectorId) -> AnalysisResult<usize> {
        self.detector_index
            .node_index_for_detector(detector, SearchGraphKind::Hypergraph)
    }

    fn add_edge_from_dem_targets(
        &mut self,
        targets: &[DemTarget],
        max_weight: usize,
    ) -> AnalysisResult<()> {
        let cancellation = self.construction_budget.limits().cancellation.clone();
        let (detectors, observables) = folded_error_targets(targets, &cancellation)?;
        if detectors.is_empty() {
            if !observables.is_empty() {
                self.distance_1_error_mask = observables;
            }
            return Ok(());
        }
        if detectors.len() > max_weight {
            return Ok(());
        }
        let degree_limit = self.construction_budget.limits().max_hyperedge_degree;
        if detectors.len() > degree_limit {
            return Err(ResourceLimitError::logical_error_search(
                "hypergraph search",
                LogicalErrorSearchResource::HyperedgeDegree,
                detectors.len() as u64,
                degree_limit as u64,
            )
            .into());
        }

        let edge = Edge::new(detectors.clone(), observables);
        if self.edge_index.find(&edge, &self.edges).is_some() {
            return Ok(());
        }
        let projected_incidences = self
            .edge_incidences
            .checked_add(detectors.len())
            .ok_or_else(|| {
                AnalysisError::count_overflow(
                    AnalysisOperation::LogicalErrorSearch,
                    "hypergraph edge incidence count overflowed",
                )
            })?;
        let incidence_limit = self.construction_budget.limits().max_hyperedge_incidences;
        if projected_incidences > incidence_limit {
            return Err(ResourceLimitError::logical_error_search(
                "hypergraph search",
                LogicalErrorSearchResource::HyperedgeIncidences,
                projected_incidences as u64,
                incidence_limit as u64,
            )
            .into());
        }

        let adjacency_stored_terms = detectors.len().checked_mul(2).ok_or_else(|| {
            AnalysisError::count_overflow(
                AnalysisOperation::LogicalErrorSearch,
                "hypergraph stored edge incidence count overflowed",
            )
        })?;
        let (edge_id, inserted) = self.intern_edge(edge, adjacency_stored_terms)?;
        if !inserted {
            return Ok(());
        }
        let mut countdown = stab_model::advanced::CancellationCountdown::new();
        for detector in detectors {
            countdown.tick(&cancellation)?;
            let index = self.node_index_for_detector(detector)?;
            let Some(node) = self.nodes.get_mut(index) else {
                return Err(AnalysisError::internal(
                    AnalysisOperation::LogicalErrorSearch,
                    format!(
                        "hypergraph detector D{} is outside the graph",
                        detector.get()
                    ),
                ));
            };
            let inserted = node.add_edge_id(edge_id)?;
            if !inserted {
                return Err(AnalysisError::internal(
                    AnalysisOperation::LogicalErrorSearch,
                    "hypergraph search inserted a new edge into the same detector twice",
                ));
            }
        }
        self.edge_incidences = projected_incidences;
        Ok(())
    }

    #[cfg(test)]
    fn from_dem(model: &DetectorErrorModel, max_weight: usize) -> AnalysisResult<Self> {
        Self::from_dem_with_limits(model, max_weight, &LogicalErrorSearchLimits::default())
    }

    fn from_dem_with_limits(
        model: &DetectorErrorModel,
        max_weight: usize,
        limits: &LogicalErrorSearchLimits,
    ) -> AnalysisResult<Self> {
        let traversal = FoldedDemTraversal::new(model)?;
        let full_detector_count = traversal.root().summary().detector_count()?;
        let full_observable_count = traversal.root().summary().observable_count();
        let effective_detectors = search_graph_nonzero_error_targets(
            &traversal,
            "hypergraph search",
            SearchGraphTargetPolicy::Hypergraph {
                max_weight: max_weight.min(limits.max_hyperedge_degree),
            },
            limits,
        )?;
        let num_observables = usize::try_from(full_observable_count).map_err(|_| {
            AnalysisError::count_overflow(
                AnalysisOperation::LogicalErrorSearch,
                "observable count does not fit usize",
            )
        })?;
        let mut graph = Self::try_new_sparse_with_limits(
            effective_detectors,
            num_observables,
            full_detector_count > 0,
            limits.clone(),
        )?;
        visit_search_graph_errors_with_limits(
            &traversal,
            "hypergraph search",
            limits,
            |instruction, detector_offset| {
                let shifted = shifted_targets(instruction.targets(), detector_offset)?;
                graph.add_edge_from_dem_targets(&shifted, max_weight)
            },
        )?;
        Ok(graph)
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct SearchState {
    detectors: BTreeSet<DemDetectorId>,
    observables: ObservableMask,
}

impl SearchState {
    fn new(detectors: BTreeSet<DemDetectorId>, observables: ObservableMask) -> Self {
        Self {
            detectors,
            observables,
        }
    }

    fn is_undetected(&self) -> bool {
        self.detectors.is_empty()
    }

    fn after_crossing_edge(&self, edge: &Edge) -> Self {
        let mut detectors = self.detectors.clone();
        for detector in &edge.detectors {
            if !detectors.insert(*detector) {
                detectors.remove(detector);
            }
        }
        Self {
            detectors,
            observables: self.observables.symmetric_difference(&edge.observables),
        }
    }

    fn term_count(&self) -> AnalysisResult<usize> {
        self.detectors
            .len()
            .checked_add(self.observables.len())
            .ok_or_else(|| {
                AnalysisError::count_overflow(
                    AnalysisOperation::LogicalErrorSearch,
                    "hypergraph search state term count overflowed",
                )
            })
    }

    fn append_transition_as_error_instruction_to(
        &self,
        next: &Self,
        out: &mut DetectorErrorModel,
    ) -> AnalysisResult<()> {
        let mut detector_targets = self.detectors.clone();
        for detector in &next.detectors {
            if !detector_targets.insert(*detector) {
                detector_targets.remove(detector);
            }
        }

        let mut targets = Vec::new();
        for detector in detector_targets {
            targets.push(DemTarget::relative_detector(detector.get())?);
        }
        self.observables
            .symmetric_difference(&next.observables)
            .push_targets(&mut targets);
        out.push_instruction(DemInstruction::error(
            Probability::try_new(1.0)?,
            targets,
            None,
        )?);
        Ok(())
    }
}

/// Searches the hypergraph of `model` for an undetectable logical error.
///
/// This is a heuristic: the size limits and [`SymptomGrowth::Skip`] prune the search, so a
/// [`LogicalErrorSearch::NotFound`] outcome means none was found within them. When both size
/// limits are 2 a faster breadth-first search over the same edges runs instead and finds an
/// error exactly when this search would, of the same length; [`HypergraphSearchOptions`] says
/// how its results relate.
///
/// # Errors
///
/// Returns [`AnalysisError::Cancelled`] once the limits' cancellation signal is observed, while
/// the graph is built or during the search, and [`AnalysisError::ResourceLimit`] when a limit is
/// exceeded; a search stopped by a limit does not report [`LogicalErrorSearch::NotFound`].
pub fn find_undetectable_logical_error(
    model: &DetectorErrorModel,
    options: &HypergraphSearchOptions,
) -> AnalysisResult<LogicalErrorSearch> {
    // Stim reroutes this pair to the graphlike search, which drops every error with a `^`
    // separator; Stab keeps the hypergraph's folded edges and only swaps in the faster search.
    if options.max_edge_degree == 2 && options.max_detection_event_set_size == 2 {
        return two_detector_hyperedge_search(model, &options.limits);
    }
    hypergraph_search(model, options)
}

/// The hypergraph search itself, without the fast path at both maxima 2.
fn hypergraph_search(
    model: &DetectorErrorModel,
    options: &HypergraphSearchOptions,
) -> AnalysisResult<LogicalErrorSearch> {
    let &HypergraphSearchOptions {
        max_detection_event_set_size,
        max_edge_degree,
        symptom_growth,
        ref limits,
    } = options;
    let graph = Graph::from_dem_with_limits(model, max_edge_degree, limits)?;
    let empty = SearchState::new(BTreeSet::new(), ObservableMask::new());
    if !graph.distance_1_error_mask.is_empty() {
        let mut out = DetectorErrorModel::new();
        SearchState::new(BTreeSet::new(), graph.distance_1_error_mask)
            .append_transition_as_error_instruction_to(&empty, &mut out)?;
        return Ok(LogicalErrorSearch::Found(out));
    }

    let mut queue = VecDeque::new();
    let mut back_map = BTreeMap::new();
    let mut budget = SearchBudget::new("hypergraph search", limits);
    budget.admit_state(0, 0, false)?;
    back_map.insert(empty.clone(), empty.clone());

    for (node_index, node) in graph.nodes.iter().enumerate() {
        budget.poll_cancellation()?;
        let source = graph.detector_for_node_index(node_index)?;
        for edge_id in &node.edge_ids {
            let edge = graph.edge(*edge_id)?;
            budget.record_transition()?;
            budget.poll_cancellation_steps(
                edge.detectors.len().saturating_add(edge.observables.len()),
            )?;
            if edge.observables.is_empty() || edge.detectors.iter().next() != Some(&source) {
                continue;
            }
            if edge.detectors.len() > max_detection_event_set_size {
                continue;
            }
            let start_terms = edge
                .detectors
                .len()
                .checked_add(edge.observables.len())
                .ok_or_else(|| {
                    AnalysisError::count_overflow(
                        AnalysisOperation::LogicalErrorSearch,
                        "hypergraph initial search state term count overflowed",
                    )
                })?;
            budget.preflight_state_terms(start_terms)?;
            let start = SearchState::new(edge.detectors.clone(), edge.observables.clone());
            if !back_map.contains_key(&start) {
                budget.admit_state(start_terms, 0, true)?;
                if back_map.insert(start.clone(), empty.clone()).is_some() {
                    return Err(AnalysisError::internal(
                        AnalysisOperation::LogicalErrorSearch,
                        "hypergraph initial search state was inserted twice",
                    ));
                }
                queue.push_back(start);
            }
        }
    }

    while let Some(current) = queue.pop_front() {
        budget.poll_cancellation()?;
        let Some(active) = current.detectors.iter().next().copied() else {
            return Err(AnalysisError::internal(
                AnalysisOperation::LogicalErrorSearch,
                "hypergraph search reached a state without an active detector",
            ));
        };
        let active_index = graph.node_index_for_detector(active)?;
        let Some(node) = graph.nodes.get(active_index) else {
            return Err(AnalysisError::internal(
                AnalysisOperation::LogicalErrorSearch,
                "hypergraph active detector is outside the graph",
            ));
        };
        let current_terms = current.term_count()?;
        for edge_id in &node.edge_ids {
            let edge = graph.edge(*edge_id)?;
            budget.record_transition()?;
            budget.poll_cancellation_steps(
                current_terms
                    .saturating_add(edge.detectors.len())
                    .saturating_add(edge.observables.len()),
            )?;
            let next_detector_terms = current
                .detectors
                .symmetric_difference(&edge.detectors)
                .count();
            if next_detector_terms > max_detection_event_set_size {
                continue;
            }
            if symptom_growth == SymptomGrowth::Skip
                && next_detector_terms > current.detectors.len()
            {
                continue;
            }
            let next_terms = next_detector_terms
                .checked_add(
                    current
                        .observables
                        .symmetric_difference_len(&edge.observables),
                )
                .ok_or_else(|| {
                    AnalysisError::count_overflow(
                        AnalysisOperation::LogicalErrorSearch,
                        "hypergraph next search state term count overflowed",
                    )
                })?;
            budget.preflight_state_terms(next_terms)?;
            let next = current.after_crossing_edge(edge);
            if back_map.contains_key(&next) {
                continue;
            }
            let undetected = next.is_undetected();
            budget.admit_state(next_terms, current_terms, !undetected)?;
            if back_map.insert(next.clone(), current.clone()).is_some() {
                return Err(AnalysisError::internal(
                    AnalysisOperation::LogicalErrorSearch,
                    "hypergraph search state was inserted twice",
                ));
            }
            if undetected {
                if next.observables.is_empty() {
                    return Err(AnalysisError::internal(
                        AnalysisOperation::LogicalErrorSearch,
                        "hypergraph search reached an empty logical state unexpectedly",
                    ));
                }
                return backtrack_path(&back_map, &next, &mut budget)
                    .map(LogicalErrorSearch::Found);
            }
            queue.push_back(next);
        }
    }

    Ok(LogicalErrorSearch::NotFound(
        NoLogicalErrorReport::hypergraph(
            graph.num_observables == 0,
            !graph.has_declared_detectors,
            model.count_errors()? == 0,
        ),
    ))
}

fn backtrack_path(
    back_map: &BTreeMap<SearchState, SearchState>,
    final_state: &SearchState,
    budget: &mut SearchBudget,
) -> AnalysisResult<DetectorErrorModel> {
    let mut out = DetectorErrorModel::new();
    let mut current = final_state.clone();
    loop {
        budget.poll_cancellation()?;
        let Some(previous) = back_map.get(&current) else {
            return Err(AnalysisError::internal(
                AnalysisOperation::LogicalErrorSearch,
                "hypergraph search backtracking reached an unknown state",
            ));
        };
        current.append_transition_as_error_instruction_to(previous, &mut out)?;
        if previous.is_undetected() {
            break;
        }
        current = previous.clone();
    }
    sorted_error_model_with_cancelled_pairs(out)
}

fn sorted_error_model_with_cancelled_pairs(
    model: DetectorErrorModel,
) -> AnalysisResult<DetectorErrorModel> {
    let mut instructions = Vec::new();
    for item in model.items() {
        let DemItem::Instruction(instruction) = item else {
            return Err(AnalysisError::internal(
                AnalysisOperation::LogicalErrorSearch,
                "hypergraph search produced a repeat block unexpectedly",
            ));
        };
        instructions.push(instruction.clone());
    }
    instructions.sort_by(|left, right| left.targets().cmp(right.targets()));

    let mut kept: Vec<DemInstruction> = Vec::new();
    for instruction in instructions {
        if kept
            .last()
            .is_some_and(|previous| previous.targets() == instruction.targets())
        {
            kept.pop();
        } else {
            kept.push(instruction);
        }
    }

    let mut sorted = DetectorErrorModel::new();
    for instruction in kept {
        sorted.push_instruction(instruction);
    }
    Ok(sorted)
}

#[cfg(test)]
mod limit_policy_tests {
    #![allow(
        clippy::expect_used,
        reason = "focused resource-policy tests use fixed valid DEM fixtures"
    )]

    use super::*;

    #[test]
    fn hyperedge_degree_and_incidence_rejections_leave_graph_unchanged() {
        let two_detector_edge = [
            DemTarget::relative_detector(0).expect("D0"),
            DemTarget::relative_detector(1).expect("D1"),
        ];
        let degree_exact = LogicalErrorSearchLimits {
            max_hyperedge_degree: 2,
            ..LogicalErrorSearchLimits::default()
        };
        let mut exact_graph = Graph::new_with_limits(2, 0, degree_exact.clone());
        exact_graph
            .add_edge_from_dem_targets(&two_detector_edge, usize::MAX)
            .expect("the exact two-detector degree maximum is accepted");
        assert_eq!(exact_graph.edge_incidences, 2);

        let mut degree_rejected = Graph::new_with_limits(
            2,
            0,
            LogicalErrorSearchLimits {
                max_hyperedge_degree: 1,
                ..degree_exact
            },
        );
        let error = degree_rejected
            .add_edge_from_dem_targets(&two_detector_edge, usize::MAX)
            .expect_err("the first detector above the degree policy should fail");
        assert!(
            error
                .to_string()
                .contains("edges with at most 1 detectors, got 2")
        );
        assert!(degree_rejected.edges.is_empty());
        assert!(
            degree_rejected
                .nodes
                .iter()
                .all(|node| node.edge_ids.is_empty())
        );
        assert_eq!(degree_rejected.edge_incidences, 0);

        let limits = LogicalErrorSearchLimits {
            max_hyperedge_degree: 2,
            max_hyperedge_incidences: 2,
            max_unique_graph_edges: 2,
            ..LogicalErrorSearchLimits::default()
        };
        let mut graph = Graph::new_with_limits(2, 2, limits);
        graph
            .add_edge_from_dem_targets(
                &[
                    DemTarget::relative_detector(0).expect("D0"),
                    DemTarget::relative_detector(1).expect("D1"),
                    DemTarget::logical_observable(0).expect("L0"),
                ],
                usize::MAX,
            )
            .expect("first edge reaches the exact incidence boundary");
        let before_edges = graph.edges.clone();
        let before_nodes = graph.nodes.clone();

        let error = graph
            .add_edge_from_dem_targets(
                &[
                    DemTarget::relative_detector(0).expect("D0"),
                    DemTarget::logical_observable(1).expect("L1"),
                ],
                usize::MAX,
            )
            .expect_err("next incidence exceeds the policy");
        assert!(
            error
                .to_string()
                .contains("at most 2 edge incidences, got at least 3")
        );
        assert_eq!(graph.edges, before_edges);
        assert_eq!(graph.nodes, before_nodes);
        assert_eq!(graph.edge_incidences, 2);
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod two_detector_fast_path_tests;
