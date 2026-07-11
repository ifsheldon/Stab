#![allow(
    dead_code,
    reason = "M10 hypergraph search internals are being landed in parity-tested slices before the full search algorithm consumes them"
)]

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt::{Display, Formatter};

use super::{
    DemDetectorId, DemInstruction, DemInstructionKind, DemItem, DemObservableId, DemTarget,
    DetectorErrorModel,
    error_traversal::{
        SearchGraphTargetPolicy, search_graph_nonzero_error_targets, visit_search_graph_errors,
    },
    search_budget::SearchBudget,
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
}

impl Node {
    fn add_edge_id(&mut self, edge_id: usize) -> CircuitResult<()> {
        if !self.edge_ids.contains(&edge_id) {
            self.edge_ids.try_reserve(1).map_err(|_| {
                CircuitError::invalid_detector_error_model(
                    "hypergraph search cannot allocate another edge incidence",
                )
            })?;
            self.edge_ids.push(edge_id);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Graph {
    nodes: Vec<Node>,
    edges: Vec<Edge>,
    edge_index: BTreeMap<Edge, usize>,
    edge_incidences: usize,
    detector_index: DetectorIndex,
    has_declared_detectors: bool,
    num_observables: usize,
    distance_1_error_mask: ObservableMask,
}

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
                let edge_id = graph.intern_edge(edge)?;
                let node = graph.nodes.get_mut(node_index).ok_or_else(|| {
                    CircuitError::invalid_detector_error_model(
                        "hypergraph test node index is outside the graph",
                    )
                })?;
                node.add_edge_id(edge_id)?;
                graph.edge_incidences = graph.edge_incidences.saturating_add(1);
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

    fn intern_edge(&mut self, edge: Edge) -> CircuitResult<usize> {
        if let Some(edge_id) = self.edge_index.get(&edge).copied() {
            return Ok(edge_id);
        }
        let edge_id = self.edges.len();
        self.edges.try_reserve(1).map_err(|_| {
            CircuitError::invalid_detector_error_model(
                "hypergraph search cannot allocate another edge",
            )
        })?;
        self.edges.push(edge.clone());
        self.edge_index.insert(edge, edge_id);
        Ok(edge_id)
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

        let edge_id = self.intern_edge(edge)?;
        for detector in detectors {
            let index = self.node_index_for_detector(detector)?;
            let Some(node) = self.nodes.get_mut(index) else {
                return Err(CircuitError::invalid_detector_error_model(format!(
                    "hypergraph detector D{} is outside the graph",
                    detector.get()
                )));
            };
            node.add_edge_id(edge_id)?;
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
    budget.admit_state()?;
    back_map.insert(empty.clone(), empty.clone());

    for (node_index, node) in graph.nodes.iter().enumerate() {
        let source = graph.detector_for_node_index(node_index)?;
        for edge_id in &node.edge_ids {
            let edge = graph.edge(*edge_id)?;
            budget.record_transition()?;
            if edge.observables.is_empty() || edge.detectors.iter().next() != Some(&source) {
                continue;
            }
            let start = SearchState::new(edge.detectors.clone(), edge.observables.clone());
            if let std::collections::btree_map::Entry::Vacant(entry) = back_map.entry(start.clone())
            {
                budget.admit_state()?;
                entry.insert(empty.clone());
                if start.detectors.len() <= dont_explore_detection_event_sets_with_size_above {
                    queue.push_back(start);
                }
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
        for edge_id in &node.edge_ids {
            let edge = graph.edge(*edge_id)?;
            budget.record_transition()?;
            let next = current.after_crossing_edge(edge);
            if next.detectors.len() > dont_explore_detection_event_sets_with_size_above {
                continue;
            }
            if dont_explore_edges_increasing_symptom_degree
                && next.detectors.len() > current.detectors.len()
            {
                continue;
            }
            let std::collections::btree_map::Entry::Vacant(entry) = back_map.entry(next.clone())
            else {
                continue;
            };
            budget.admit_state()?;
            entry.insert(current.clone());
            if next.is_undetected() {
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
    if count_nonzero_error_instructions(model)? == 0 {
        message.push_str(
            "\n    WARNING: NO ERRORS. The circuit or detector error model didn't include any errors, making it vacuously impossible to find a logical error.",
        );
    }
    Ok(message)
}

fn count_nonzero_error_instructions(model: &DetectorErrorModel) -> CircuitResult<usize> {
    let mut total = 0usize;
    for item in model.items() {
        let count = match item {
            DemItem::Instruction(instruction) => usize::from(
                instruction.kind() == DemInstructionKind::Error
                    && instruction.args().first().copied().unwrap_or(0.0) != 0.0,
            ),
            DemItem::RepeatBlock(repeat) => {
                let repeat_count = usize::try_from(repeat.repeat_count().get()).map_err(|_| {
                    CircuitError::invalid_detector_error_model(
                        "repeat count does not fit usize while counting hypergraph errors",
                    )
                })?;
                repeat_count
                    .checked_mul(count_nonzero_error_instructions(repeat.body())?)
                    .ok_or_else(|| {
                        CircuitError::invalid_detector_error_model(
                            "hypergraph error count overflowed",
                        )
                    })?
            }
        };
        total = total.checked_add(count).ok_or_else(|| {
            CircuitError::invalid_detector_error_model("hypergraph error count overflowed")
        })?;
    }
    Ok(total)
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
mod tests {
    #![allow(
        clippy::expect_used,
        clippy::indexing_slicing,
        clippy::unwrap_used,
        reason = "unit tests use direct assertions for compact diagnostics"
    )]

    use super::*;

    fn detector(value: u64) -> DemDetectorId {
        DemDetectorId::try_new(value).unwrap()
    }

    fn detector_set(values: &[u64]) -> BTreeSet<DemDetectorId> {
        values.iter().copied().map(detector).collect()
    }

    fn obs_mask(bits: u64) -> ObservableMask {
        let mut observables = BTreeSet::new();
        for index in 0..64 {
            if bits & (1 << index) != 0 {
                observables.insert(DemObservableId::try_new(index).unwrap());
            }
        }
        ObservableMask { observables }
    }

    fn edge(detectors: &[u64], observables: u64) -> Edge {
        Edge::new(detector_set(detectors), obs_mask(observables))
    }

    fn sparse_graph(
        detectors: &[u64],
        node_edges: Vec<Vec<Edge>>,
        num_observables: usize,
    ) -> Graph {
        let mut graph =
            Graph::try_new_sparse(detector_set(detectors), num_observables, true).unwrap();
        assert_eq!(graph.nodes.len(), node_edges.len());
        for (node_index, edges) in node_edges.into_iter().enumerate() {
            for edge in edges {
                let edge_id = graph.intern_edge(edge).unwrap();
                graph.nodes[node_index].add_edge_id(edge_id).unwrap();
                graph.edge_incidences += 1;
            }
        }
        graph
    }

    fn state(detectors: &[u64], observables: u64) -> SearchState {
        SearchState::new(detector_set(detectors), obs_mask(observables))
    }

    fn first_targets(dem: &str) -> Vec<DemTarget> {
        let model = DetectorErrorModel::from_dem_str(dem).unwrap();
        let instruction = model
            .items()
            .first()
            .and_then(|item| match item {
                DemItem::Instruction(instruction) => Some(instruction),
                DemItem::RepeatBlock(_) => None,
            })
            .unwrap();
        instruction.targets().to_vec()
    }

    fn find(
        dem: &str,
        dont_explore_detection_event_sets_with_size_above: usize,
        dont_explore_edges_with_degree_above: usize,
        dont_explore_edges_increasing_symptom_degree: bool,
    ) -> CircuitResult<String> {
        let model = DetectorErrorModel::from_dem_str(dem)?;
        find_undetectable_logical_error(
            &model,
            dont_explore_detection_event_sets_with_size_above,
            dont_explore_edges_with_degree_above,
            dont_explore_edges_increasing_symptom_degree,
        )
        .map(|error| error.to_dem_string())
    }

    #[test]
    fn hyper_edge_matches_upstream() {
        let e1 = edge(&[], 0);
        let e2 = edge(&[1], 0);
        let e3 = edge(&[], 1);
        let e4 = edge(&[1, 2], 5);

        assert_eq!(e1.to_string(), "[silent]");
        assert_eq!(e2.to_string(), "[boundary] D1");
        assert_eq!(e3.to_string(), "[silent] L0");
        assert_eq!(e4.to_string(), "D1 D2 L0 L2");
        assert_eq!(e1, e1);
        assert_ne!(e1, e2);
        assert_eq!(e1, edge(&[], 0));
        assert_eq!(e2, e2);
        assert_eq!(e3, e3);
        assert_ne!(e1, e3);
    }

    #[test]
    fn hyper_node_adjacency_reuses_edge_arena() {
        let shared = edge(&[1, 3], 5);
        let graph = Graph::from_parts(
            vec![
                vec![],
                vec![shared.clone()],
                vec![],
                vec![shared, edge(&[3], 8)],
            ],
            64,
            obs_mask(0),
        )
        .unwrap();

        assert_eq!(graph.edges.len(), 2);
        assert_eq!(graph.nodes[1].edge_ids, vec![0]);
        assert_eq!(graph.nodes[3].edge_ids, vec![0, 1]);
    }

    #[test]
    fn hyper_search_state_appends_transition_as_error_instruction_matches_upstream() {
        let mut out = DetectorErrorModel::new();

        state(&[1, 2], 9)
            .append_transition_as_error_instruction_to(&state(&[1, 2], 16), &mut out)
            .unwrap();
        assert_eq!(out.to_dem_string(), "error(1) L0 L3 L4\n");

        state(&[], 9)
            .append_transition_as_error_instruction_to(&state(&[1, 2, 4], 16), &mut out)
            .unwrap();
        assert_eq!(
            out.to_dem_string(),
            "error(1) L0 L3 L4\nerror(1) D1 D2 D4 L0 L3 L4\n"
        );

        state(&[1, 2], 9)
            .append_transition_as_error_instruction_to(&state(&[2, 3], 9), &mut out)
            .unwrap();
        assert_eq!(
            out.to_dem_string(),
            "error(1) L0 L3 L4\nerror(1) D1 D2 D4 L0 L3 L4\nerror(1) D1 D3\n"
        );
    }

    #[test]
    fn hyper_search_state_equality_ordering_and_display_match_upstream() {
        assert_eq!(state(&[1, 2], 3), state(&[1, 2], 3));
        assert_ne!(state(&[1, 2], 3), state(&[1, 4], 3));
        assert_ne!(state(&[1, 2], 3), state(&[1], 3));
        assert_ne!(state(&[1, 2], 3), state(&[1, 2], 4));

        assert!(state(&[1], 999) < state(&[1, 2], 999));
        assert!(state(&[1, 999], 999) < state(&[101, 102], 103));
        assert!(state(&[1, 101], 999) < state(&[101, 102], 103));
        assert!(state(&[1, 102], 999) < state(&[101, 102], 103));
        assert!(state(&[101, 102], 3) < state(&[101, 102], 103));
        assert!(state(&[101, 102], 103) >= state(&[101, 102], 103));
        assert!(state(&[101, 104], 103) >= state(&[101, 102], 103));
        assert!(state(&[101, 102], 104) >= state(&[101, 102], 103));

        assert_eq!(state(&[1, 2], 3).to_string(), "D1 D2 L0 L1 ");
    }

    #[test]
    fn hyper_graph_equality_matches_upstream() {
        assert_eq!(Graph::new(1, 64), Graph::new(1, 64));
        assert_ne!(Graph::new(1, 64), Graph::new(2, 64));
        assert_ne!(Graph::new(1, 64), Graph::new(1, 32));

        let a = Graph::new(1, 64);
        let mut b = Graph::new(1, 64);
        assert_eq!(a, b);
        b.distance_1_error_mask = obs_mask(1);
        assert_ne!(a, b);
    }

    #[test]
    fn hyper_graph_add_edge_from_dem_targets_matches_upstream() {
        let mut graph = Graph::new(3, 64);
        graph
            .add_edge_from_dem_targets(&first_targets("error(0.01) D0 D1 L3 ^ D0\n"), usize::MAX)
            .unwrap();
        assert_eq!(
            graph.to_string(),
            Graph::from_parts(vec![vec![], vec![edge(&[1], 8)], vec![]], 64, obs_mask(0),)
                .unwrap()
                .to_string()
        );

        graph
            .add_edge_from_dem_targets(&first_targets("error(0.01) D0 D1 D2 L0\n"), usize::MAX)
            .unwrap();
        assert_eq!(
            graph.to_string(),
            Graph::from_parts(
                vec![
                    vec![edge(&[0, 1, 2], 1)],
                    vec![edge(&[1], 8), edge(&[0, 1, 2], 1)],
                    vec![edge(&[0, 1, 2], 1)],
                ],
                64,
                obs_mask(0),
            )
            .unwrap()
            .to_string()
        );
        assert_eq!(graph.edges.len(), 2);
        assert_eq!(graph.edge_incidences, 4);
    }

    #[test]
    fn hyper_graph_rejects_excessive_edge_degree_before_adjacency_allocation() {
        let mut graph = Graph::new(MAX_HYPERGRAPH_EDGE_DEGREE + 1, 0);
        let targets = (0..=MAX_HYPERGRAPH_EDGE_DEGREE)
            .map(|detector| DemTarget::relative_detector(detector as u64).unwrap())
            .collect::<Vec<_>>();

        let error = graph
            .add_edge_from_dem_targets(&targets, usize::MAX)
            .expect_err("hard edge-degree cap");
        assert!(
            error
                .to_string()
                .contains("edges with at most 64 detectors")
        );
        assert!(graph.edges.is_empty());
        assert_eq!(graph.edge_incidences, 0);
    }

    #[test]
    fn hyper_graph_rejects_excessive_edge_incidences_before_allocation() {
        let mut graph = Graph::new(MAX_HYPERGRAPH_EDGE_DEGREE, 5);
        let detector_targets = (0..MAX_HYPERGRAPH_EDGE_DEGREE)
            .map(|detector| DemTarget::relative_detector(detector as u64).unwrap())
            .collect::<Vec<_>>();
        for observable in 0..4 {
            let mut targets = detector_targets.clone();
            targets.push(DemTarget::logical_observable(observable).unwrap());
            graph
                .add_edge_from_dem_targets(&targets, usize::MAX)
                .unwrap();
        }

        let mut rejected = detector_targets;
        rejected.push(DemTarget::logical_observable(4).unwrap());
        let error = graph
            .add_edge_from_dem_targets(&rejected, usize::MAX)
            .expect_err("hard edge-incidence cap");
        assert!(error.to_string().contains("at most 256 edge incidences"));
        assert_eq!(graph.edges.len(), 4);
        assert_eq!(graph.edge_incidences, 256);
    }

    #[test]
    fn hyper_graph_display_matches_upstream() {
        let graph = Graph::from_parts(
            vec![
                vec![],
                vec![edge(&[1], 0), edge(&[1, 3], 32)],
                vec![],
                vec![edge(&[1, 3], 32)],
            ],
            64,
            obs_mask(0),
        )
        .unwrap();

        assert_eq!(
            graph.to_string(),
            "0:\n1:\n    [boundary] D1\n    D1 D3 L5\n2:\n3:\n    D1 D3 L5\n"
        );
    }

    #[test]
    fn hyper_graph_from_dem_matches_upstream() {
        let dem = DetectorErrorModel::from_dem_str(
            "error(0.1) D0\nrepeat 3 {\n    error(0.1) D0 D1\n    shift_detectors 1\n}\nerror(0.1) D0 L7\nerror(0.1) D2 ^ D3 D4 L2\ndetector D5\n",
        )
        .unwrap();

        assert_eq!(
            Graph::from_dem(&dem, usize::MAX).unwrap(),
            sparse_graph(
                &[0, 1, 2, 3, 5, 6, 7],
                vec![
                    vec![edge(&[0], 0), edge(&[0, 1], 0)],
                    vec![edge(&[0, 1], 0), edge(&[1, 2], 0)],
                    vec![edge(&[1, 2], 0), edge(&[2, 3], 0)],
                    vec![edge(&[2, 3], 0), edge(&[3], 128)],
                    vec![edge(&[5, 6, 7], 4)],
                    vec![edge(&[5, 6, 7], 4)],
                    vec![edge(&[5, 6, 7], 4)],
                ],
                8,
            )
        );

        assert_eq!(
            Graph::from_dem(&dem, 2).unwrap(),
            sparse_graph(
                &[0, 1, 2, 3],
                vec![
                    vec![edge(&[0], 0), edge(&[0, 1], 0)],
                    vec![edge(&[0, 1], 0), edge(&[1, 2], 0)],
                    vec![edge(&[1, 2], 0), edge(&[2, 3], 0)],
                    vec![edge(&[2, 3], 0), edge(&[3], 128)],
                ],
                8,
            )
        );

        assert_eq!(
            Graph::from_dem(&dem, 1).unwrap(),
            sparse_graph(&[0, 3], vec![vec![edge(&[0], 0)], vec![edge(&[3], 128)]], 8,)
        );
    }

    #[test]
    fn hyper_algo_no_error_matches_upstream() {
        assert!(find("", usize::MAX, usize::MAX, false).is_err());
        assert!(find("error(0.1) D0 L0\n", usize::MAX, usize::MAX, false).is_err());
        assert!(
            find(
                "error(0.1) D0\nerror(0.1) D0 D1\nerror(0.1) D1\n",
                usize::MAX,
                usize::MAX,
                false
            )
            .is_err()
        );
    }

    #[test]
    fn hyper_algo_rejects_excessive_search_states() {
        let mut text = String::new();
        for observable in 0..=64 {
            text.push_str(&format!("error(0.1) D0 L{observable}\n"));
        }
        let error = find(&text, 3, 3, false).expect_err("search state cap");
        assert!(error.to_string().contains("at most 64 search states"));
    }

    #[test]
    fn hyper_algo_distance_1_matches_upstream() {
        assert_eq!(
            find("error(0.1) L0\n", usize::MAX, usize::MAX, false).unwrap(),
            "error(1) L0\n"
        );
    }

    #[test]
    fn hyper_algo_distance_2_matches_upstream() {
        assert_eq!(
            find(
                "error(0.1) D0\nerror(0.1) D0 L0\n",
                usize::MAX,
                usize::MAX,
                false
            )
            .unwrap(),
            "error(1) D0\nerror(1) D0 L0\n"
        );

        assert_eq!(
            find(
                "error(0.1) D0 L0\nerror(0.1) D0 L1\n",
                usize::MAX,
                usize::MAX,
                false
            )
            .unwrap(),
            "error(1) D0 L0\nerror(1) D0 L1\n"
        );

        assert_eq!(
            find(
                "error(0.1) D0 D1 L0\nerror(0.1) D0 D1 L1\n",
                usize::MAX,
                usize::MAX,
                false
            )
            .unwrap(),
            "error(1) D0 D1 L0\nerror(1) D0 D1 L1\n"
        );

        assert_eq!(
            find(
                "error(0.1) D0 D1 L1\nerror(0.1) D0 D1 L0\n",
                usize::MAX,
                usize::MAX,
                false
            )
            .unwrap(),
            "error(1) D0 D1 L0\nerror(1) D0 D1 L1\n"
        );
    }

    #[test]
    fn hyper_algo_distance_3_matches_upstream() {
        assert_eq!(
            find(
                "error(0.1) D0\nerror(0.1) D0 D1 L0\nerror(0.1) D1\n",
                usize::MAX,
                usize::MAX,
                false
            )
            .unwrap(),
            "error(1) D0\nerror(1) D0 D1 L0\nerror(1) D1\n"
        );

        assert_eq!(
            find(
                "error(0.1) D1\nerror(0.1) D1 D0 L0\nerror(0.1) D0\n",
                usize::MAX,
                usize::MAX,
                false
            )
            .unwrap(),
            "error(1) D0\nerror(1) D0 D1 L0\nerror(1) D1\n"
        );
    }

    #[test]
    fn hyper_algo_hyper_error_matches_upstream() {
        assert_eq!(
            find(
                "\
error(0.1) D0 D1
error(0.1) D0 D1 D2 D3
error(0.1) D2 D3 D4 D5 L0
error(0.1) D4 D5 D6 D7
error(0.1) D6 D7 D8 D9
error(0.1) D8
error(0.1) D9
",
                4,
                4,
                true
            )
            .unwrap(),
            "\
error(1) D0 D1
error(1) D0 D1 D2 D3
error(1) D2 D3 D4 D5 L0
error(1) D4 D5 D6 D7
error(1) D6 D7 D8 D9
error(1) D8
error(1) D9
"
        );
    }
}
