#![allow(
    dead_code,
    reason = "M10 hypergraph search internals are being landed in parity-tested slices before the full search algorithm consumes them"
)]

use std::collections::BTreeSet;
use std::fmt::{Display, Formatter};

use super::{
    DemDetectorId, DemInstruction, DemInstructionKind, DemItem, DemObservableId, DemTarget,
    DetectorErrorModel,
};
use crate::{CircuitError, CircuitResult, Probability};

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

#[derive(Clone, Debug, Eq, PartialEq)]
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
    edges: Vec<Edge>,
}

impl Node {
    fn new(edges: Vec<Edge>) -> Self {
        Self { edges }
    }

    fn add_edge(&mut self, edge: Edge) {
        if !self.edges.contains(&edge) {
            self.edges.push(edge);
        }
    }
}

impl Display for Node {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        for edge in &self.edges {
            writeln!(f, "    {edge}")?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Graph {
    nodes: Vec<Node>,
    num_observables: usize,
    distance_1_error_mask: ObservableMask,
}

impl Graph {
    fn new(node_count: usize, num_observables: usize) -> Self {
        Self {
            nodes: vec![Node::default(); node_count],
            num_observables,
            distance_1_error_mask: ObservableMask::new(),
        }
    }

    fn from_parts(
        nodes: Vec<Node>,
        num_observables: usize,
        distance_1_error_mask: ObservableMask,
    ) -> Self {
        Self {
            nodes,
            num_observables,
            distance_1_error_mask,
        }
    }

    fn add_edge_from_dem_targets(
        &mut self,
        targets: &[DemTarget],
        max_weight: usize,
    ) -> CircuitResult<()> {
        let (detectors, observables) = toggled_dem_targets(targets)?;
        if detectors.is_empty() {
            self.distance_1_error_mask = self
                .distance_1_error_mask
                .symmetric_difference(&observables);
            return Ok(());
        }
        if detectors.len() > max_weight {
            return Ok(());
        }

        let edge = Edge::new(detectors.clone(), observables);
        for detector in detectors {
            let index = usize::try_from(detector.get()).map_err(|_| {
                CircuitError::invalid_detector_error_model(format!(
                    "hypergraph detector D{} does not fit usize",
                    detector.get()
                ))
            })?;
            let Some(node) = self.nodes.get_mut(index) else {
                return Err(CircuitError::invalid_detector_error_model(format!(
                    "hypergraph detector D{} is outside the graph",
                    detector.get()
                )));
            };
            node.add_edge(edge.clone());
        }
        Ok(())
    }

    fn from_dem(model: &DetectorErrorModel, max_weight: usize) -> CircuitResult<Self> {
        let node_count = usize::try_from(model.count_detectors()?).map_err(|_| {
            CircuitError::invalid_detector_error_model("detector count does not fit usize")
        })?;
        let num_observables = usize::try_from(model.count_observables()?).map_err(|_| {
            CircuitError::invalid_detector_error_model("observable count does not fit usize")
        })?;
        let mut graph = Self::new(node_count, num_observables);
        graph.add_flattened_dem(model, 0, max_weight)?;
        Ok(graph)
    }

    fn add_flattened_dem(
        &mut self,
        model: &DetectorErrorModel,
        mut detector_offset: u64,
        max_weight: usize,
    ) -> CircuitResult<u64> {
        for item in model.items() {
            match item {
                DemItem::Instruction(instruction) => match instruction.kind() {
                    DemInstructionKind::Error => {
                        if instruction.args().first().copied().unwrap_or(0.0) != 0.0 {
                            let shifted = shifted_targets(instruction.targets(), detector_offset)?;
                            self.add_edge_from_dem_targets(&shifted, max_weight)?;
                        }
                    }
                    DemInstructionKind::ShiftDetectors => {
                        detector_offset = detector_offset
                            .checked_add(instruction.detector_shift()?)
                            .ok_or_else(|| {
                                CircuitError::invalid_detector_error_model(
                                    "hypergraph detector offset overflowed",
                                )
                            })?;
                    }
                    DemInstructionKind::Detector | DemInstructionKind::LogicalObservable => {}
                },
                DemItem::RepeatBlock(repeat) => {
                    for _ in 0..repeat.repeat_count().get() {
                        detector_offset =
                            self.add_flattened_dem(repeat.body(), detector_offset, max_weight)?;
                    }
                }
            }
        }
        Ok(detector_offset)
    }
}

impl Display for Graph {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        for (index, node) in self.nodes.iter().enumerate() {
            writeln!(f, "{index}:")?;
            write!(f, "{node}")?;
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

fn shifted_targets(targets: &[DemTarget], detector_offset: u64) -> CircuitResult<Vec<DemTarget>> {
    targets
        .iter()
        .map(|target| match *target {
            DemTarget::RelativeDetector(detector) => DemTarget::relative_detector(
                detector_offset.checked_add(detector.get()).ok_or_else(|| {
                    CircuitError::invalid_detector_error_model(
                        "hypergraph detector target overflowed",
                    )
                })?,
            ),
            DemTarget::LogicalObservable(observable) => {
                DemTarget::logical_observable(observable.get())
            }
            DemTarget::Separator => Ok(DemTarget::separator()),
            DemTarget::Numeric(value) => Ok(DemTarget::numeric(value)),
        })
        .collect()
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
    fn hyper_node_matches_upstream() {
        let n1 = Node::default();
        let n2 = Node::new(vec![edge(&[2], 0)]);
        let n3 = Node::new(vec![edge(&[1, 3], 5), edge(&[3], 8)]);

        assert_eq!(n1.to_string(), "");
        assert_eq!(n2.to_string(), "    [boundary] D2\n");
        assert_eq!(n3.to_string(), "    D1 D3 L0 L2\n    [boundary] D3 L3\n");
        assert_eq!(n1, n1);
        assert_ne!(n1, n2);
        assert_eq!(n1, Node::default());
        assert_eq!(n2, n2);
        assert_eq!(n3, n3);
        assert_ne!(n1, n3);
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
            graph,
            Graph::from_parts(
                vec![
                    Node::default(),
                    Node::new(vec![edge(&[1], 8)]),
                    Node::default(),
                ],
                64,
                obs_mask(0),
            )
        );

        graph
            .add_edge_from_dem_targets(&first_targets("error(0.01) D0 D1 D2 L0\n"), usize::MAX)
            .unwrap();
        assert_eq!(
            graph,
            Graph::from_parts(
                vec![
                    Node::new(vec![edge(&[0, 1, 2], 1)]),
                    Node::new(vec![edge(&[1], 8), edge(&[0, 1, 2], 1)]),
                    Node::new(vec![edge(&[0, 1, 2], 1)]),
                ],
                64,
                obs_mask(0),
            )
        );
    }

    #[test]
    fn hyper_graph_display_matches_upstream() {
        let graph = Graph::from_parts(
            vec![
                Node::default(),
                Node::new(vec![edge(&[1], 0), edge(&[1, 3], 32)]),
                Node::default(),
                Node::new(vec![edge(&[1, 3], 32)]),
            ],
            64,
            obs_mask(0),
        );

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
            Graph::from_parts(
                vec![
                    Node::new(vec![edge(&[0], 0), edge(&[0, 1], 0)]),
                    Node::new(vec![edge(&[0, 1], 0), edge(&[1, 2], 0)]),
                    Node::new(vec![edge(&[1, 2], 0), edge(&[2, 3], 0)]),
                    Node::new(vec![edge(&[2, 3], 0), edge(&[3], 128)]),
                    Node::default(),
                    Node::new(vec![edge(&[5, 6, 7], 4)]),
                    Node::new(vec![edge(&[5, 6, 7], 4)]),
                    Node::new(vec![edge(&[5, 6, 7], 4)]),
                    Node::default(),
                ],
                8,
                obs_mask(0),
            )
        );

        assert_eq!(
            Graph::from_dem(&dem, 2).unwrap(),
            Graph::from_parts(
                vec![
                    Node::new(vec![edge(&[0], 0), edge(&[0, 1], 0)]),
                    Node::new(vec![edge(&[0, 1], 0), edge(&[1, 2], 0)]),
                    Node::new(vec![edge(&[1, 2], 0), edge(&[2, 3], 0)]),
                    Node::new(vec![edge(&[2, 3], 0), edge(&[3], 128)]),
                    Node::default(),
                    Node::default(),
                    Node::default(),
                    Node::default(),
                    Node::default(),
                ],
                8,
                obs_mask(0),
            )
        );

        assert_eq!(
            Graph::from_dem(&dem, 1).unwrap(),
            Graph::from_parts(
                vec![
                    Node::new(vec![edge(&[0], 0)]),
                    Node::default(),
                    Node::default(),
                    Node::new(vec![edge(&[3], 128)]),
                    Node::default(),
                    Node::default(),
                    Node::default(),
                    Node::default(),
                    Node::default(),
                ],
                8,
                obs_mask(0),
            )
        );
    }
}
