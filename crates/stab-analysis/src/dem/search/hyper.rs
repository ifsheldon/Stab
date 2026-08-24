use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt::{Display, Formatter};

use stab_model::{
    DemDetectorId, DemInstruction, DemItem, DemObservableId, DemTarget, DetectorErrorModel,
    Probability,
};

use super::{
    arena_index::ArenaIndex,
    budget::{GraphConstructionBudget, LogicalErrorSearchLimits, SearchBudget},
    error_traversal::{
        SearchGraphTargetPolicy, search_graph_nonzero_error_targets,
        visit_search_graph_errors_with_limits,
    },
    traversal::{FoldedDemTraversal, shifted_targets},
    values::{DetectorIndex, ObservableMask, SearchGraphKind},
};
use crate::resources::LogicalErrorSearchResource;
use crate::{AnalysisError, AnalysisResult, ResourceLimitError};

#[cfg(test)]
const MAX_HYPERGRAPH_EDGE_DEGREE: usize = 64;

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
                AnalysisError::invalid_detector_error_model("hypergraph edge term count overflowed")
            })
    }
}

impl Display for Edge {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut text = String::new();
        match self.detectors.len() {
            0 => text.push_str("[silent]"),
            1 => {
                text.push_str("[boundary] ");
                let detector = self.detectors.iter().next().ok_or(std::fmt::Error)?;
                text.push_str(&format_detector(*detector));
            }
            _ => {
                for (index, detector) in self.detectors.iter().enumerate() {
                    if index > 0 {
                        text.push(' ');
                    }
                    text.push_str(&format_detector(*detector));
                }
            }
        }
        for observable in self.observables.iter() {
            text.push(' ');
            text.push_str(&format_observable(observable));
        }
        f.write_str(&text)
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
            AnalysisError::invalid_detector_error_model(
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
        let limits = LogicalErrorSearchLimits::default()
            .with_max_unique_graph_edges(64)
            .with_max_stored_graph_terms(2_048)
            .with_max_hyperedge_degree(64)
            .with_max_hyperedge_incidences(256);
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
            AnalysisError::invalid_detector_error_model(format!(
                "hypergraph search cannot allocate {node_count} sparse detector nodes"
            ))
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
                    AnalysisError::invalid_detector_error_model(
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
            AnalysisError::invalid_detector_error_model(format!(
                "hypergraph edge index {edge_id} is outside the edge arena"
            ))
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
                AnalysisError::invalid_detector_error_model(
                    "hypergraph stored graph index count overflowed",
                )
            })?;
        let admission = self.construction_budget.preflight_unique_edge(
            edge.term_count()?,
            1,
            stored_index_and_adjacency_terms,
        )?;
        self.edges.try_reserve(1).map_err(|_| {
            AnalysisError::invalid_detector_error_model(
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
        let (detectors, observables) = toggled_dem_targets(targets)?;
        if detectors.is_empty() {
            if !observables.is_empty() {
                self.distance_1_error_mask = observables;
            }
            return Ok(());
        }
        if detectors.len() > max_weight {
            return Ok(());
        }
        let degree_limit = self.construction_budget.limits().max_hyperedge_degree();
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
                AnalysisError::invalid_detector_error_model(
                    "hypergraph edge incidence count overflowed",
                )
            })?;
        let incidence_limit = self.construction_budget.limits().max_hyperedge_incidences();
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
            AnalysisError::invalid_detector_error_model(
                "hypergraph stored edge incidence count overflowed",
            )
        })?;
        let (edge_id, inserted) = self.intern_edge(edge, adjacency_stored_terms)?;
        if !inserted {
            return Ok(());
        }
        for detector in detectors {
            let index = self.node_index_for_detector(detector)?;
            let Some(node) = self.nodes.get_mut(index) else {
                return Err(AnalysisError::invalid_detector_error_model(format!(
                    "hypergraph detector D{} is outside the graph",
                    detector.get()
                )));
            };
            let inserted = node.add_edge_id(edge_id)?;
            if !inserted {
                return Err(AnalysisError::invalid_detector_error_model(
                    "hypergraph search inserted a new edge into the same detector twice",
                ));
            }
        }
        self.edge_incidences = projected_incidences;
        Ok(())
    }

    #[cfg(test)]
    fn from_dem(model: &DetectorErrorModel, max_weight: usize) -> AnalysisResult<Self> {
        Self::from_dem_with_limits(model, max_weight, LogicalErrorSearchLimits::default())
    }

    fn from_dem_with_limits(
        model: &DetectorErrorModel,
        max_weight: usize,
        limits: LogicalErrorSearchLimits,
    ) -> AnalysisResult<Self> {
        let traversal = FoldedDemTraversal::new(model)?;
        let full_detector_count = traversal.root().summary().detector_count()?;
        let full_observable_count = traversal.root().summary().observable_count();
        let effective_detectors = search_graph_nonzero_error_targets(
            &traversal,
            "hypergraph search",
            SearchGraphTargetPolicy::Hypergraph {
                max_weight: max_weight.min(limits.max_hyperedge_degree()),
            },
            limits,
        )?;
        let num_observables = usize::try_from(full_observable_count).map_err(|_| {
            AnalysisError::invalid_detector_error_model("observable count does not fit usize")
        })?;
        let mut graph = Self::try_new_sparse_with_limits(
            effective_detectors,
            num_observables,
            full_detector_count > 0,
            limits,
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

impl Display for Graph {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        for (index, node) in self.nodes.iter().enumerate() {
            writeln!(f, "{index}:")?;
            for edge_id in &node.edge_ids {
                let edge = self.edges.get(*edge_id).ok_or(std::fmt::Error)?;
                writeln!(f, "    {edge}")?;
            }
        }
        Ok(())
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
                AnalysisError::invalid_detector_error_model(
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
            .push_targets(&mut targets)?;
        out.push_instruction(DemInstruction::error(
            Probability::try_new(1.0)?,
            targets,
            None,
        )?);
        Ok(())
    }
}

impl Display for SearchState {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut text = String::new();
        for detector in &self.detectors {
            text.push_str(&format_detector(*detector));
            text.push(' ');
        }
        for observable in self.observables.iter() {
            text.push_str(&format_observable(observable));
            text.push(' ');
        }
        f.write_str(&text)
    }
}

pub fn find_undetectable_logical_error(
    model: &DetectorErrorModel,
    dont_explore_detection_event_sets_with_size_above: usize,
    dont_explore_edges_with_degree_above: usize,
    dont_explore_edges_increasing_symptom_degree: bool,
) -> AnalysisResult<DetectorErrorModel> {
    find_undetectable_logical_error_with_limits(
        model,
        dont_explore_detection_event_sets_with_size_above,
        dont_explore_edges_with_degree_above,
        dont_explore_edges_increasing_symptom_degree,
        LogicalErrorSearchLimits::default(),
    )
}

pub fn find_undetectable_logical_error_with_limits(
    model: &DetectorErrorModel,
    dont_explore_detection_event_sets_with_size_above: usize,
    dont_explore_edges_with_degree_above: usize,
    dont_explore_edges_increasing_symptom_degree: bool,
    limits: LogicalErrorSearchLimits,
) -> AnalysisResult<DetectorErrorModel> {
    if dont_explore_edges_with_degree_above == 2
        && dont_explore_detection_event_sets_with_size_above == 2
    {
        return super::graphlike::shortest_graphlike_undetectable_logical_error_with_limits(
            model, true, limits,
        );
    }

    let graph = Graph::from_dem_with_limits(model, dont_explore_edges_with_degree_above, limits)?;
    let empty = SearchState::new(BTreeSet::new(), ObservableMask::new());
    if !graph.distance_1_error_mask.is_empty() {
        let mut out = DetectorErrorModel::new();
        SearchState::new(BTreeSet::new(), graph.distance_1_error_mask)
            .append_transition_as_error_instruction_to(&empty, &mut out)?;
        return Ok(out);
    }

    let mut queue = VecDeque::new();
    let mut back_map = BTreeMap::new();
    let mut budget = SearchBudget::new("hypergraph search", limits);
    budget.admit_state(0, 0, false)?;
    back_map.insert(empty.clone(), empty.clone());

    for (node_index, node) in graph.nodes.iter().enumerate() {
        let source = graph.detector_for_node_index(node_index)?;
        for edge_id in &node.edge_ids {
            let edge = graph.edge(*edge_id)?;
            budget.record_transition()?;
            if edge.observables.is_empty() || edge.detectors.iter().next() != Some(&source) {
                continue;
            }
            if edge.detectors.len() > dont_explore_detection_event_sets_with_size_above {
                continue;
            }
            let start_terms = edge
                .detectors
                .len()
                .checked_add(edge.observables.len())
                .ok_or_else(|| {
                    AnalysisError::invalid_detector_error_model(
                        "hypergraph initial search state term count overflowed",
                    )
                })?;
            budget.preflight_state_terms(start_terms)?;
            let start = SearchState::new(edge.detectors.clone(), edge.observables.clone());
            if !back_map.contains_key(&start) {
                budget.admit_state(start_terms, 0, true)?;
                if back_map.insert(start.clone(), empty.clone()).is_some() {
                    return Err(AnalysisError::invalid_detector_error_model(
                        "hypergraph initial search state was inserted twice",
                    ));
                }
                queue.push_back(start);
            }
        }
    }

    while let Some(current) = queue.pop_front() {
        let Some(active) = current.detectors.iter().next().copied() else {
            return Err(AnalysisError::invalid_detector_error_model(
                "hypergraph search reached a state without an active detector",
            ));
        };
        let active_index = graph.node_index_for_detector(active)?;
        let Some(node) = graph.nodes.get(active_index) else {
            return Err(AnalysisError::invalid_detector_error_model(
                "hypergraph active detector is outside the graph",
            ));
        };
        let current_terms = current.term_count()?;
        for edge_id in &node.edge_ids {
            let edge = graph.edge(*edge_id)?;
            budget.record_transition()?;
            let next_detector_terms = current
                .detectors
                .symmetric_difference(&edge.detectors)
                .count();
            if next_detector_terms > dont_explore_detection_event_sets_with_size_above {
                continue;
            }
            if dont_explore_edges_increasing_symptom_degree
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
                    AnalysisError::invalid_detector_error_model(
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
                return Err(AnalysisError::invalid_detector_error_model(
                    "hypergraph search state was inserted twice",
                ));
            }
            if undetected {
                if next.observables.is_empty() {
                    return Err(AnalysisError::invalid_detector_error_model(
                        "hypergraph search reached an empty logical state unexpectedly",
                    ));
                }
                return backtrack_path(&back_map, &next);
            }
            queue.push_back(next);
        }
    }

    Err(AnalysisError::invalid_detector_error_model(
        no_hypergraph_logical_error_message(model, &graph)?,
    ))
}

fn backtrack_path(
    back_map: &BTreeMap<SearchState, SearchState>,
    final_state: &SearchState,
) -> AnalysisResult<DetectorErrorModel> {
    let mut out = DetectorErrorModel::new();
    let mut current = final_state.clone();
    loop {
        let Some(previous) = back_map.get(&current) else {
            return Err(AnalysisError::invalid_detector_error_model(
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
            return Err(AnalysisError::invalid_detector_error_model(
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

fn no_hypergraph_logical_error_message(
    model: &DetectorErrorModel,
    graph: &Graph,
) -> AnalysisResult<String> {
    let mut message = String::from("Failed to find any logical errors.");
    if graph.num_observables == 0 {
        message.push_str(
            "\n    WARNING: NO OBSERVABLES. The circuit or detector error model didn't define any observables, making it vacuously impossible to find a logical error.",
        );
    }
    if !graph.has_declared_detectors {
        message.push_str(
            "\n    WARNING: NO DETECTORS. The circuit or detector error model didn't define any detectors.",
        );
    }
    if model.count_errors()? == 0 {
        message.push_str(
            "\n    WARNING: NO ERRORS. The circuit or detector error model didn't include any errors, making it vacuously impossible to find a logical error.",
        );
    }
    Ok(message)
}

fn toggled_dem_targets(
    targets: &[DemTarget],
) -> AnalysisResult<(BTreeSet<DemDetectorId>, ObservableMask)> {
    let mut detectors = BTreeSet::new();
    let mut observables = ObservableMask::new();
    for target in targets {
        match *target {
            DemTarget::RelativeDetector(detector) => {
                if !detectors.insert(detector) {
                    detectors.remove(&detector);
                }
            }
            DemTarget::LogicalObservable(observable) => observables.toggle(observable),
            DemTarget::Separator => {}
            DemTarget::Numeric(_) => {
                return Err(AnalysisError::invalid_detector_error_model(
                    "hypergraph error targets cannot include numeric targets",
                ));
            }
        }
    }
    Ok((detectors, observables))
}

fn format_detector(detector: DemDetectorId) -> String {
    format!("D{}", detector.get())
}

fn format_observable(observable: DemObservableId) -> String {
    format!("L{}", observable.get())
}

#[cfg(test)]
mod limit_policy_tests {
    #![allow(
        clippy::expect_used,
        reason = "focused resource-policy tests use fixed valid DEM fixtures"
    )]

    use super::*;

    #[test]
    fn default_policy_preserves_hypergraph_search_result() {
        let model = DetectorErrorModel::from_dem_str("error(0.1) D0\nerror(0.1) D0 L0\n")
            .expect("valid search model");
        let legacy =
            find_undetectable_logical_error(&model, 3, 3, false).expect("default search succeeds");
        let explicit = find_undetectable_logical_error_with_limits(
            &model,
            3,
            3,
            false,
            LogicalErrorSearchLimits::default(),
        )
        .expect("explicit default search succeeds");
        assert_eq!(legacy, explicit);
    }

    #[test]
    fn effective_detector_limit_is_exact_and_independent() {
        let model = DetectorErrorModel::from_dem_str("error(0.1) D0 L0\nerror(0.1) D1 L1\n")
            .expect("valid graph model");
        let exact = LogicalErrorSearchLimits::default()
            .with_max_effective_detector_nodes(2)
            .with_max_search_states(1);
        let graph = Graph::from_dem_with_limits(&model, usize::MAX, exact)
            .expect("two effective detectors fit");
        assert_eq!(graph.nodes.len(), 2);

        let error = Graph::from_dem_with_limits(
            &model,
            usize::MAX,
            exact.with_max_effective_detector_nodes(1),
        )
        .expect_err("third-party graph allocation must not start beyond the node limit");
        assert!(
            error
                .to_string()
                .contains("at most 1 effective detector nodes, got 2")
        );
    }

    #[test]
    fn hyperedge_degree_and_incidence_rejections_leave_graph_unchanged() {
        let two_detector_edge = [
            DemTarget::relative_detector(0).expect("D0"),
            DemTarget::relative_detector(1).expect("D1"),
        ];
        let degree_exact = LogicalErrorSearchLimits::default().with_max_hyperedge_degree(2);
        let mut exact_graph = Graph::new_with_limits(2, 0, degree_exact);
        exact_graph
            .add_edge_from_dem_targets(&two_detector_edge, usize::MAX)
            .expect("the exact two-detector degree maximum is accepted");
        assert_eq!(exact_graph.edge_incidences, 2);

        let mut degree_rejected =
            Graph::new_with_limits(2, 0, degree_exact.with_max_hyperedge_degree(1));
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

        let limits = LogicalErrorSearchLimits::default()
            .with_max_hyperedge_degree(2)
            .with_max_hyperedge_incidences(2)
            .with_max_unique_graph_edges(2);
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

    #[test]
    fn default_hyperedge_degree_boundary_is_executed_exactly() {
        let default_degree = LogicalErrorSearchLimits::default().max_hyperedge_degree();
        let exact_targets = (0..default_degree)
            .map(|detector| {
                DemTarget::relative_detector(
                    u64::try_from(detector).expect("default degree fits a detector identifier"),
                )
                .expect("default degree uses valid detector identifiers")
            })
            .collect::<Vec<_>>();
        let mut exact_graph =
            Graph::new_with_limits(default_degree, 0, LogicalErrorSearchLimits::default());
        exact_graph
            .add_edge_from_dem_targets(&exact_targets, usize::MAX)
            .expect("the production hyperedge-degree maximum is accepted");
        assert_eq!(exact_graph.edge_incidences, default_degree);

        let first_excess = default_degree
            .checked_add(1)
            .expect("the production hyperedge-degree maximum is finite");
        let excessive_targets = (0..first_excess)
            .map(|detector| {
                DemTarget::relative_detector(
                    u64::try_from(detector).expect("first excess fits a detector identifier"),
                )
                .expect("first excess uses valid detector identifiers")
            })
            .collect::<Vec<_>>();
        let mut rejected_graph =
            Graph::new_with_limits(first_excess, 0, LogicalErrorSearchLimits::default());
        let error = rejected_graph
            .add_edge_from_dem_targets(&excessive_targets, usize::MAX)
            .expect_err("the first degree above the production limit is rejected");
        assert!(
            error.to_string().contains(&format!(
                "edges with at most {default_degree} detectors, got {first_excess}"
            )),
            "unexpected error: {error}"
        );
        assert!(rejected_graph.edges.is_empty());
        assert_eq!(rejected_graph.edge_incidences, 0);
        assert!(
            rejected_graph
                .nodes
                .iter()
                .all(|node| node.edge_ids.is_empty())
        );
    }

    #[test]
    fn hypergraph_search_state_boundary_is_inclusive() {
        let model = DetectorErrorModel::from_dem_str(
            "error(0.1) D0 L0\nerror(0.1) D0 L1\nerror(0.1) D0 L2\n",
        )
        .expect("valid search model");
        let exact = LogicalErrorSearchLimits::default().with_max_search_states(5);
        find_undetectable_logical_error_with_limits(&model, 3, 3, false, exact)
            .expect("the first derived logical state reaches the exact boundary");

        let error = find_undetectable_logical_error_with_limits(
            &model,
            3,
            3,
            false,
            exact.with_max_search_states(4),
        )
        .expect_err("first state past the limit");
        assert!(
            error
                .to_string()
                .contains("at most 4 search states, got at least 5")
        );
    }
}

#[cfg(test)]
mod resource_tests;

#[cfg(test)]
mod tests;
