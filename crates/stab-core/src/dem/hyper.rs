#![allow(
    dead_code,
    reason = "M10 hypergraph search internals are being landed in parity-tested slices before the full search algorithm consumes them"
)]

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt::{Display, Formatter};

use super::{
    DemDetectorId, DemInstruction, DemItem, DemObservableId, DemTarget, DetectorErrorModel,
    error_traversal::{
        SearchGraphTargetPolicy, search_graph_nonzero_error_targets, visit_search_graph_errors,
    },
    search_budget::{GraphConstructionBudget, SearchBudget},
    traversal::{FoldedDemTraversal, shifted_targets},
};
use crate::{CircuitError, CircuitResult, Probability};

const MAX_FULL_DEM_SEARCH_GRAPH_NODES: usize = 1_000_000;
#[cfg(not(test))]
const MAX_HYPERGRAPH_EDGE_DEGREE: usize = 4_096;
#[cfg(test)]
const MAX_HYPERGRAPH_EDGE_DEGREE: usize = 64;
#[cfg(not(test))]
const MAX_HYPERGRAPH_EDGE_INCIDENCES: usize = 5_000_000;
#[cfg(test)]
const MAX_HYPERGRAPH_EDGE_INCIDENCES: usize = 256;

#[derive(Clone, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
struct ObservableMask {
    observables: BTreeSet<DemObservableId>,
}

impl ObservableMask {
    fn new() -> Self {
        Self {
            observables: BTreeSet::new(),
        }
    }

    fn toggle(&mut self, observable: DemObservableId) {
        if !self.observables.insert(observable) {
            self.observables.remove(&observable);
        }
    }

    fn symmetric_difference(&self, other: &Self) -> Self {
        let mut observables = self.observables.clone();
        for observable in &other.observables {
            if !observables.insert(*observable) {
                observables.remove(observable);
            }
        }
        Self { observables }
    }

    fn symmetric_difference_len(&self, other: &Self) -> usize {
        self.observables
            .symmetric_difference(&other.observables)
            .count()
    }

    fn len(&self) -> usize {
        self.observables.len()
    }

    fn is_empty(&self) -> bool {
        self.observables.is_empty()
    }

    fn write_suffix(&self, out: &mut String) {
        for observable in &self.observables {
            out.push(' ');
            out.push_str(&format_observable(*observable));
        }
    }

    fn push_targets(&self, targets: &mut Vec<DemTarget>) -> CircuitResult<()> {
        for observable in &self.observables {
            targets.push(DemTarget::logical_observable(observable.get())?);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
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

    fn term_count(&self) -> CircuitResult<usize> {
        self.detectors
            .len()
            .checked_add(self.observables.len())
            .ok_or_else(|| {
                CircuitError::invalid_detector_error_model("hypergraph edge term count overflowed")
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
        self.observables.write_suffix(&mut text);
        f.write_str(&text)
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct Node {
    edge_ids: Vec<usize>,
    edge_id_index: BTreeSet<usize>,
}

impl Node {
    fn add_edge_id(&mut self, edge_id: usize) -> CircuitResult<bool> {
        if self.edge_id_index.contains(&edge_id) {
            return Ok(false);
        }
        self.edge_ids.try_reserve(1).map_err(|_| {
            CircuitError::invalid_detector_error_model(
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
    edge_index: BTreeMap<Edge, usize>,
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
            && self.edge_index == other.edge_index
            && self.edge_incidences == other.edge_incidences
            && self.detector_index == other.detector_index
            && self.has_declared_detectors == other.has_declared_detectors
            && self.num_observables == other.num_observables
            && self.distance_1_error_mask == other.distance_1_error_mask
    }
}

impl Eq for Graph {}

#[derive(Clone, Debug, Eq, PartialEq)]
enum DetectorIndex {
    Identity,
    Sparse {
        node_to_detector: Vec<DemDetectorId>,
        detector_to_node: BTreeMap<DemDetectorId, usize>,
    },
}

impl Graph {
    fn new(node_count: usize, num_observables: usize) -> Self {
        Self {
            nodes: vec![Node::default(); node_count],
            edges: Vec::new(),
            edge_index: BTreeMap::new(),
            edge_incidences: 0,
            detector_index: DetectorIndex::Identity,
            has_declared_detectors: node_count > 0,
            num_observables,
            distance_1_error_mask: ObservableMask::new(),
            construction_budget: GraphConstructionBudget::new("hypergraph search"),
        }
    }

    fn try_new(node_count: usize, num_observables: usize) -> CircuitResult<Self> {
        let mut nodes = Vec::new();
        nodes.try_reserve_exact(node_count).map_err(|_| {
            CircuitError::invalid_detector_error_model(format!(
                "hypergraph search cannot allocate {node_count} detector nodes"
            ))
        })?;
        nodes.resize(node_count, Node::default());
        Ok(Self {
            nodes,
            edges: Vec::new(),
            edge_index: BTreeMap::new(),
            edge_incidences: 0,
            detector_index: DetectorIndex::Identity,
            has_declared_detectors: node_count > 0,
            num_observables,
            distance_1_error_mask: ObservableMask::new(),
            construction_budget: GraphConstructionBudget::new("hypergraph search"),
        })
    }

    fn try_new_sparse(
        detectors: BTreeSet<DemDetectorId>,
        num_observables: usize,
        has_declared_detectors: bool,
    ) -> CircuitResult<Self> {
        let node_count = detectors.len();
        let mut nodes = Vec::new();
        nodes.try_reserve_exact(node_count).map_err(|_| {
            CircuitError::invalid_detector_error_model(format!(
                "hypergraph search cannot allocate {node_count} sparse detector nodes"
            ))
        })?;
        nodes.resize(node_count, Node::default());

        let node_to_detector: Vec<_> = detectors.into_iter().collect();
        let detector_to_node = node_to_detector
            .iter()
            .copied()
            .enumerate()
            .map(|(index, detector)| (detector, index))
            .collect();
        Ok(Self {
            nodes,
            edges: Vec::new(),
            edge_index: BTreeMap::new(),
            edge_incidences: 0,
            detector_index: DetectorIndex::Sparse {
                node_to_detector,
                detector_to_node,
            },
            has_declared_detectors,
            num_observables,
            distance_1_error_mask: ObservableMask::new(),
            construction_budget: GraphConstructionBudget::new("hypergraph search"),
        })
    }

    fn from_parts(
        node_edges: Vec<Vec<Edge>>,
        num_observables: usize,
        distance_1_error_mask: ObservableMask,
    ) -> CircuitResult<Self> {
        let mut graph = Self::new(node_edges.len(), num_observables);
        graph.distance_1_error_mask = distance_1_error_mask;
        for (node_index, edges) in node_edges.into_iter().enumerate() {
            for edge in edges {
                let (edge_id, inserted) = graph.intern_edge(edge, 2)?;
                if !inserted {
                    graph.construction_budget.admit_adjacency(2)?;
                }
                let node = graph.nodes.get_mut(node_index).ok_or_else(|| {
                    CircuitError::invalid_detector_error_model(
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

    fn edge(&self, edge_id: usize) -> CircuitResult<&Edge> {
        self.edges.get(edge_id).ok_or_else(|| {
            CircuitError::invalid_detector_error_model(format!(
                "hypergraph edge index {edge_id} is outside the edge arena"
            ))
        })
    }

    fn intern_edge(
        &mut self,
        edge: Edge,
        adjacency_stored_terms: usize,
    ) -> CircuitResult<(usize, bool)> {
        if let Some(edge_id) = self.edge_index.get(&edge).copied() {
            return Ok((edge_id, false));
        }
        let edge_id = self.edges.len();
        self.edges.try_reserve(1).map_err(|_| {
            CircuitError::invalid_detector_error_model(
                "hypergraph search cannot allocate another edge",
            )
        })?;
        self.construction_budget.admit_unique_edge(
            edge.term_count()?,
            2,
            adjacency_stored_terms,
        )?;
        self.edges.push(edge.clone());
        self.edge_index.insert(edge, edge_id);
        Ok((edge_id, true))
    }

    fn detector_for_node_index(&self, index: usize) -> CircuitResult<DemDetectorId> {
        match &self.detector_index {
            DetectorIndex::Identity => {
                let index = u64::try_from(index).map_err(|_| {
                    CircuitError::invalid_detector_error_model(
                        "hypergraph node index does not fit detector id",
                    )
                })?;
                DemDetectorId::try_new(index)
            }
            DetectorIndex::Sparse {
                node_to_detector, ..
            } => node_to_detector.get(index).copied().ok_or_else(|| {
                CircuitError::invalid_detector_error_model(format!(
                    "hypergraph sparse node index {index} is outside the graph"
                ))
            }),
        }
    }

    fn node_index_for_detector(&self, detector: DemDetectorId) -> CircuitResult<usize> {
        match &self.detector_index {
            DetectorIndex::Identity => usize::try_from(detector.get()).map_err(|_| {
                CircuitError::invalid_detector_error_model(format!(
                    "hypergraph detector D{} does not fit usize",
                    detector.get()
                ))
            }),
            DetectorIndex::Sparse {
                detector_to_node, ..
            } => detector_to_node.get(&detector).copied().ok_or_else(|| {
                CircuitError::invalid_detector_error_model(format!(
                    "hypergraph detector D{} is outside the sparse graph",
                    detector.get()
                ))
            }),
        }
    }

    fn add_edge_from_dem_targets(
        &mut self,
        targets: &[DemTarget],
        max_weight: usize,
    ) -> CircuitResult<()> {
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
        if detectors.len() > MAX_HYPERGRAPH_EDGE_DEGREE {
            return Err(CircuitError::invalid_detector_error_model(format!(
                "hypergraph search currently supports edges with at most {MAX_HYPERGRAPH_EDGE_DEGREE} detectors, got {}",
                detectors.len()
            )));
        }

        let edge = Edge::new(detectors.clone(), observables);
        if self.edge_index.contains_key(&edge) {
            return Ok(());
        }
        let projected_incidences = self
            .edge_incidences
            .checked_add(detectors.len())
            .ok_or_else(|| {
                CircuitError::invalid_detector_error_model(
                    "hypergraph edge incidence count overflowed",
                )
            })?;
        if projected_incidences > MAX_HYPERGRAPH_EDGE_INCIDENCES {
            return Err(CircuitError::invalid_detector_error_model(format!(
                "hypergraph search currently supports at most {MAX_HYPERGRAPH_EDGE_INCIDENCES} edge incidences, got at least {projected_incidences}"
            )));
        }

        let adjacency_stored_terms = detectors.len().checked_mul(2).ok_or_else(|| {
            CircuitError::invalid_detector_error_model(
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
                return Err(CircuitError::invalid_detector_error_model(format!(
                    "hypergraph detector D{} is outside the graph",
                    detector.get()
                )));
            };
            let inserted = node.add_edge_id(edge_id)?;
            if !inserted {
                return Err(CircuitError::invalid_detector_error_model(
                    "hypergraph search inserted a new edge into the same detector twice",
                ));
            }
        }
        self.edge_incidences = projected_incidences;
        Ok(())
    }

    fn from_dem(model: &DetectorErrorModel, max_weight: usize) -> CircuitResult<Self> {
        let traversal = FoldedDemTraversal::new(model)?;
        let full_detector_count = traversal.root().summary().detector_count()?;
        let full_observable_count = traversal.root().summary().observable_count();
        let effective_detectors = search_graph_nonzero_error_targets(
            &traversal,
            "hypergraph search",
            SearchGraphTargetPolicy::Hypergraph {
                max_weight: max_weight.min(MAX_HYPERGRAPH_EDGE_DEGREE),
            },
            MAX_FULL_DEM_SEARCH_GRAPH_NODES,
        )?;
        let effective_detector_count = effective_detectors.len();
        if effective_detector_count > MAX_FULL_DEM_SEARCH_GRAPH_NODES {
            return Err(CircuitError::invalid_detector_error_model(format!(
                "hypergraph search currently supports at most {MAX_FULL_DEM_SEARCH_GRAPH_NODES} effective detector nodes, got {effective_detector_count}"
            )));
        }
        let num_observables = usize::try_from(full_observable_count).map_err(|_| {
            CircuitError::invalid_detector_error_model("observable count does not fit usize")
        })?;
        let mut graph = Self::try_new_sparse(
            effective_detectors,
            num_observables,
            full_detector_count > 0,
        )?;
        visit_search_graph_errors(
            &traversal,
            "hypergraph search",
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

    fn term_count(&self) -> CircuitResult<usize> {
        self.detectors
            .len()
            .checked_add(self.observables.len())
            .ok_or_else(|| {
                CircuitError::invalid_detector_error_model(
                    "hypergraph search state term count overflowed",
                )
            })
    }

    fn append_transition_as_error_instruction_to(
        &self,
        next: &Self,
        out: &mut DetectorErrorModel,
    ) -> CircuitResult<()> {
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
        for observable in &self.observables.observables {
            text.push_str(&format_observable(*observable));
            text.push(' ');
        }
        f.write_str(&text)
    }
}

pub(super) fn find_undetectable_logical_error(
    model: &DetectorErrorModel,
    dont_explore_detection_event_sets_with_size_above: usize,
    dont_explore_edges_with_degree_above: usize,
    dont_explore_edges_increasing_symptom_degree: bool,
) -> CircuitResult<DetectorErrorModel> {
    if dont_explore_edges_with_degree_above == 2
        && dont_explore_detection_event_sets_with_size_above == 2
    {
        return super::shortest_graphlike_undetectable_logical_error(model, true);
    }

    let graph = Graph::from_dem(model, dont_explore_edges_with_degree_above)?;
    let empty = SearchState::new(BTreeSet::new(), ObservableMask::new());
    if !graph.distance_1_error_mask.is_empty() {
        let mut out = DetectorErrorModel::new();
        SearchState::new(BTreeSet::new(), graph.distance_1_error_mask)
            .append_transition_as_error_instruction_to(&empty, &mut out)?;
        return Ok(out);
    }

    let mut queue = VecDeque::new();
    let mut back_map = BTreeMap::new();
    let mut budget = SearchBudget::new("hypergraph search");
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
                    CircuitError::invalid_detector_error_model(
                        "hypergraph initial search state term count overflowed",
                    )
                })?;
            budget.preflight_state_terms(start_terms)?;
            let start = SearchState::new(edge.detectors.clone(), edge.observables.clone());
            if !back_map.contains_key(&start) {
                budget.admit_state(start_terms, 0, true)?;
                if back_map.insert(start.clone(), empty.clone()).is_some() {
                    return Err(CircuitError::invalid_detector_error_model(
                        "hypergraph initial search state was inserted twice",
                    ));
                }
                queue.push_back(start);
            }
        }
    }

    while let Some(current) = queue.pop_front() {
        let Some(active) = current.detectors.iter().next().copied() else {
            return Err(CircuitError::invalid_detector_error_model(
                "hypergraph search reached a state without an active detector",
            ));
        };
        let active_index = graph.node_index_for_detector(active)?;
        let Some(node) = graph.nodes.get(active_index) else {
            return Err(CircuitError::invalid_detector_error_model(
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
                    CircuitError::invalid_detector_error_model(
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
                return Err(CircuitError::invalid_detector_error_model(
                    "hypergraph search state was inserted twice",
                ));
            }
            if undetected {
                if next.observables.is_empty() {
                    return Err(CircuitError::invalid_detector_error_model(
                        "hypergraph search reached an empty logical state unexpectedly",
                    ));
                }
                return backtrack_path(&back_map, &next);
            }
            queue.push_back(next);
        }
    }

    Err(CircuitError::invalid_detector_error_model(
        no_hypergraph_logical_error_message(model, &graph)?,
    ))
}

fn backtrack_path(
    back_map: &BTreeMap<SearchState, SearchState>,
    final_state: &SearchState,
) -> CircuitResult<DetectorErrorModel> {
    let mut out = DetectorErrorModel::new();
    let mut current = final_state.clone();
    loop {
        let Some(previous) = back_map.get(&current) else {
            return Err(CircuitError::invalid_detector_error_model(
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
) -> CircuitResult<DetectorErrorModel> {
    let mut instructions = Vec::new();
    for item in model.items() {
        let DemItem::Instruction(instruction) = item else {
            return Err(CircuitError::invalid_detector_error_model(
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
) -> CircuitResult<String> {
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
) -> CircuitResult<(BTreeSet<DemDetectorId>, ObservableMask)> {
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
                return Err(CircuitError::invalid_detector_error_model(
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
mod resource_tests;

#[cfg(test)]
mod tests;
