#![allow(
    dead_code,
    reason = "M10 graphlike search internals are being landed in parity-tested slices before the full search algorithm consumes them"
)]

use std::collections::BTreeSet;
use std::fmt::{Display, Formatter};
use std::hash::{Hash, Hasher};

use super::{
    DemDetectorId, DemInstruction, DemInstructionKind, DemItem, DemObservableId, DemTarget,
    DetectorErrorModel,
};
use crate::{CircuitError, CircuitResult, Probability};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ObservableMask {
    observables: BTreeSet<DemObservableId>,
}

impl ObservableMask {
    pub(super) fn new() -> Self {
        Self {
            observables: BTreeSet::new(),
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

    fn toggle(&mut self, observable: DemObservableId) {
        if !self.observables.insert(observable) {
            self.observables.remove(&observable);
        }
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

impl Default for ObservableMask {
    fn default() -> Self {
        Self::new()
    }
}

impl Hash for ObservableMask {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.observables.hash(state);
    }
}

impl Ord for ObservableMask {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.observables.cmp(&other.observables)
    }
}

impl PartialOrd for ObservableMask {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Edge {
    detector: Option<DemDetectorId>,
    observables: ObservableMask,
}

impl Edge {
    pub(super) fn new(detector: Option<DemDetectorId>, observables: ObservableMask) -> Self {
        Self {
            detector,
            observables,
        }
    }
}

impl Display for Edge {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut text = match self.detector {
            Some(detector) => format_detector(detector),
            None => "[boundary]".to_string(),
        };
        self.observables.write_suffix(&mut text);
        f.write_str(&text)
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct Node {
    edges: Vec<Edge>,
}

impl Node {
    pub(super) fn new(edges: Vec<Edge>) -> Self {
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
pub(super) struct Graph {
    nodes: Vec<Node>,
    num_observables: usize,
    distance_1_error_mask: ObservableMask,
}

impl Graph {
    pub(super) fn new(node_count: usize, num_observables: usize) -> Self {
        Self {
            nodes: vec![Node::default(); node_count],
            num_observables,
            distance_1_error_mask: ObservableMask::new(),
        }
    }

    pub(super) fn from_parts(
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

    pub(super) fn add_outward_edge(
        &mut self,
        source: DemDetectorId,
        destination: Option<DemDetectorId>,
        observables: ObservableMask,
    ) -> CircuitResult<()> {
        let source_index = usize::try_from(source.get()).map_err(|_| {
            CircuitError::invalid_detector_error_model(format!(
                "graphlike source detector D{} does not fit usize",
                source.get()
            ))
        })?;
        let Some(node) = self.nodes.get_mut(source_index) else {
            return Err(CircuitError::invalid_detector_error_model(format!(
                "graphlike source detector D{} is outside the graph",
                source.get()
            )));
        };
        node.add_edge(Edge::new(destination, observables));
        Ok(())
    }

    pub(super) fn add_edges_from_targets_with_no_separators(
        &mut self,
        targets: &[DemTarget],
        ignore_ungraphlike_errors: bool,
    ) -> CircuitResult<()> {
        let mut detectors = Vec::new();
        let mut observables = ObservableMask::new();
        for target in targets {
            match *target {
                DemTarget::RelativeDetector(detector) => {
                    if detectors.len() == 2 {
                        if ignore_ungraphlike_errors {
                            return Ok(());
                        }
                        return Err(CircuitError::invalid_detector_error_model(
                            "The detector error model contained a non-graphlike error mechanism.\nYou can ignore such errors using `ignore_ungraphlike_errors`.\nYou can use `decompose_errors` when converting a circuit into a model to ensure no such errors are present.",
                        ));
                    }
                    detectors.push(detector);
                }
                DemTarget::LogicalObservable(observable) => observables.toggle(observable),
                DemTarget::Separator | DemTarget::Numeric(_) => {}
            }
        }

        match detectors.as_slice() {
            [detector] => self.add_outward_edge(*detector, None, observables)?,
            [left, right] => {
                self.add_outward_edge(*left, Some(*right), observables.clone())?;
                self.add_outward_edge(*right, Some(*left), observables)?;
            }
            [] if self.distance_1_error_mask.is_empty() && !observables.is_empty() => {
                self.distance_1_error_mask = observables;
            }
            [] => {}
            _ => unreachable!("detector count was limited above"),
        }
        Ok(())
    }

    pub(super) fn add_edges_from_separable_targets(
        &mut self,
        targets: &[DemTarget],
        ignore_ungraphlike_errors: bool,
    ) -> CircuitResult<()> {
        let mut start = 0;
        for (index, target) in targets.iter().enumerate() {
            if matches!(target, DemTarget::Separator) {
                if ignore_ungraphlike_errors {
                    return Ok(());
                }
                let component = targets.get(start..index).ok_or_else(|| {
                    CircuitError::invalid_detector_error_model(
                        "graphlike target component range is invalid",
                    )
                })?;
                self.add_edges_from_targets_with_no_separators(component, false)?;
                start = index + 1;
            }
        }
        let component = targets.get(start..).ok_or_else(|| {
            CircuitError::invalid_detector_error_model(
                "graphlike target component range is invalid",
            )
        })?;
        self.add_edges_from_targets_with_no_separators(component, ignore_ungraphlike_errors)
    }

    pub(super) fn from_dem(
        model: &DetectorErrorModel,
        ignore_ungraphlike_errors: bool,
    ) -> CircuitResult<Self> {
        let node_count = usize::try_from(model.count_detectors()?).map_err(|_| {
            CircuitError::invalid_detector_error_model("detector count does not fit usize")
        })?;
        let num_observables = usize::try_from(model.count_observables()?).map_err(|_| {
            CircuitError::invalid_detector_error_model("observable count does not fit usize")
        })?;
        let mut graph = Self::new(node_count, num_observables);
        graph.add_flattened_dem(model, 0, ignore_ungraphlike_errors)?;
        Ok(graph)
    }

    fn add_flattened_dem(
        &mut self,
        model: &DetectorErrorModel,
        mut detector_offset: u64,
        ignore_ungraphlike_errors: bool,
    ) -> CircuitResult<u64> {
        for item in model.items() {
            match item {
                DemItem::Instruction(instruction) => match instruction.kind() {
                    DemInstructionKind::Error => {
                        if instruction.args().first().copied().unwrap_or(0.0) != 0.0 {
                            let shifted = shifted_targets(instruction.targets(), detector_offset)?;
                            self.add_edges_from_separable_targets(
                                &shifted,
                                ignore_ungraphlike_errors,
                            )?;
                        }
                    }
                    DemInstructionKind::ShiftDetectors => {
                        detector_offset = detector_offset
                            .checked_add(instruction.detector_shift()?)
                            .ok_or_else(|| {
                                CircuitError::invalid_detector_error_model(
                                    "graphlike detector offset overflowed",
                                )
                            })?;
                    }
                    DemInstructionKind::Detector | DemInstructionKind::LogicalObservable => {}
                },
                DemItem::RepeatBlock(repeat) => {
                    for _ in 0..repeat.repeat_count().get() {
                        detector_offset = self.add_flattened_dem(
                            repeat.body(),
                            detector_offset,
                            ignore_ungraphlike_errors,
                        )?;
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

#[derive(Clone, Debug, Eq)]
pub(super) struct SearchState {
    detector_active: Option<DemDetectorId>,
    detector_held: Option<DemDetectorId>,
    observables: ObservableMask,
}

impl SearchState {
    pub(super) fn new(
        detector_active: Option<DemDetectorId>,
        detector_held: Option<DemDetectorId>,
        observables: ObservableMask,
    ) -> Self {
        Self {
            detector_active,
            detector_held,
            observables,
        }
    }

    pub(super) fn is_undetected(&self) -> bool {
        let canonical = self.canonical();
        canonical.detector_active.is_none() && canonical.detector_held.is_none()
    }

    pub(super) fn canonical(&self) -> Self {
        match (self.detector_active, self.detector_held) {
            (Some(left), Some(right)) if left == right => {
                Self::new(None, None, self.observables.clone())
            }
            (Some(left), Some(right)) if right < left => {
                Self::new(Some(right), Some(left), self.observables.clone())
            }
            (None, Some(detector)) => Self::new(Some(detector), None, self.observables.clone()),
            _ => self.clone(),
        }
    }

    pub(super) fn append_transition_as_error_instruction_to(
        &self,
        next: &Self,
        out: &mut DetectorErrorModel,
    ) -> CircuitResult<()> {
        let current = self.canonical();
        let next = next.canonical();
        let mut detector_targets = BTreeSet::new();
        toggle_detector(&mut detector_targets, current.detector_active);
        toggle_detector(&mut detector_targets, current.detector_held);
        toggle_detector(&mut detector_targets, next.detector_active);
        toggle_detector(&mut detector_targets, next.detector_held);

        let mut targets = Vec::new();
        for detector in detector_targets {
            targets.push(DemTarget::relative_detector(detector.get())?);
        }
        current
            .observables
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
        let canonical = self.canonical();
        let mut text = String::new();
        if let Some(detector) = canonical.detector_active {
            text.push_str(&format_detector(detector));
            text.push(' ');
        }
        if let Some(detector) = canonical.detector_held {
            text.push_str(&format_detector(detector));
            text.push(' ');
        }
        for observable in &canonical.observables.observables {
            text.push_str(&format_observable(*observable));
            text.push(' ');
        }
        f.write_str(&text)
    }
}

impl Hash for SearchState {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.canonical_fields().hash(state);
    }
}

impl Ord for SearchState {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.canonical_fields().cmp(&other.canonical_fields())
    }
}

impl PartialOrd for SearchState {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for SearchState {
    fn eq(&self, other: &Self) -> bool {
        self.canonical_fields() == other.canonical_fields()
    }
}

impl SearchState {
    fn canonical_fields(&self) -> (Option<DemDetectorId>, Option<DemDetectorId>, ObservableMask) {
        let canonical = self.canonical();
        (
            canonical.detector_active,
            canonical.detector_held,
            canonical.observables,
        )
    }
}

fn toggle_detector(targets: &mut BTreeSet<DemDetectorId>, detector: Option<DemDetectorId>) {
    let Some(detector) = detector else {
        return;
    };
    if !targets.insert(detector) {
        targets.remove(&detector);
    }
}

fn format_detector(detector: DemDetectorId) -> String {
    format!("D{}", detector.get())
}

fn format_observable(observable: DemObservableId) -> String {
    format!("L{}", observable.get())
}

fn shifted_targets(targets: &[DemTarget], detector_offset: u64) -> CircuitResult<Vec<DemTarget>> {
    targets
        .iter()
        .map(|target| match *target {
            DemTarget::RelativeDetector(detector) => DemTarget::relative_detector(
                detector_offset.checked_add(detector.get()).ok_or_else(|| {
                    CircuitError::invalid_detector_error_model(
                        "graphlike detector target overflowed",
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

#[cfg(test)]
mod tests {
    #![allow(
        clippy::expect_used,
        clippy::unwrap_used,
        reason = "unit tests use direct assertions for compact diagnostics"
    )]

    use std::collections::hash_map::DefaultHasher;

    use super::*;

    fn detector(value: u64) -> DemDetectorId {
        DemDetectorId::try_new(value).unwrap()
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

    fn state(active: Option<u64>, held: Option<u64>, observables: u64) -> SearchState {
        SearchState::new(
            active.map(detector),
            held.map(detector),
            obs_mask(observables),
        )
    }

    fn hash(state: SearchState) -> u64 {
        let mut hasher = DefaultHasher::new();
        state.hash(&mut hasher);
        hasher.finish()
    }

    #[test]
    fn graphlike_edge_matches_upstream() {
        let e1 = Edge::new(None, obs_mask(0));
        let e2 = Edge::new(Some(detector(1)), obs_mask(0));
        let e3 = Edge::new(None, obs_mask(1));
        let e4 = Edge::new(None, obs_mask(5));

        assert_eq!(e1.to_string(), "[boundary]");
        assert_eq!(e2.to_string(), "D1");
        assert_eq!(e3.to_string(), "[boundary] L0");
        assert_eq!(e4.to_string(), "[boundary] L0 L2");
        assert_eq!(e1, e1);
        assert_ne!(e1, e2);
        assert_eq!(e1, Edge::new(None, obs_mask(0)));
        assert_eq!(e2, e2);
        assert_eq!(e3, e3);
        assert_ne!(e1, e3);
    }

    #[test]
    fn graphlike_node_matches_upstream() {
        let n1 = Node::default();
        let n2 = Node::new(vec![Edge::new(None, obs_mask(0))]);
        let n3 = Node::new(vec![
            Edge::new(Some(detector(1)), obs_mask(5)),
            Edge::new(None, obs_mask(8)),
        ]);

        assert_eq!(n1.to_string(), "");
        assert_eq!(n2.to_string(), "    [boundary]\n");
        assert_eq!(n3.to_string(), "    D1 L0 L2\n    [boundary] L3\n");
        assert_eq!(n1, n1);
        assert_ne!(n1, n2);
        assert_eq!(n1, Node::default());
        assert_eq!(n2, n2);
        assert_eq!(n3, n3);
        assert_ne!(n1, n3);
    }

    #[test]
    fn graphlike_graph_equality_matches_upstream() {
        assert_eq!(Graph::new(1, 5), Graph::new(1, 5));
        assert_ne!(Graph::new(1, 5), Graph::new(2, 5));
        assert_ne!(Graph::new(1, 5), Graph::new(1, 4));

        let a = Graph::new(1, 5);
        let mut b = Graph::new(1, 5);
        assert_eq!(a, b);
        b.distance_1_error_mask = obs_mask(1);
        assert_ne!(a, b);
    }

    #[test]
    fn graphlike_graph_add_outward_edge_matches_upstream() {
        let mut g = Graph::new(3, 64);

        g.add_outward_edge(detector(1), Some(detector(2)), obs_mask(3))
            .unwrap();
        assert_eq!(
            g,
            Graph::from_parts(
                vec![
                    Node::default(),
                    Node::new(vec![Edge::new(Some(detector(2)), obs_mask(3))]),
                    Node::default(),
                ],
                64,
                obs_mask(0),
            )
        );

        g.add_outward_edge(detector(1), Some(detector(2)), obs_mask(3))
            .unwrap();
        assert_eq!(
            g,
            Graph::from_parts(
                vec![
                    Node::default(),
                    Node::new(vec![Edge::new(Some(detector(2)), obs_mask(3))]),
                    Node::default(),
                ],
                64,
                obs_mask(0),
            )
        );

        g.add_outward_edge(detector(1), Some(detector(2)), obs_mask(4))
            .unwrap();
        assert_eq!(
            g,
            Graph::from_parts(
                vec![
                    Node::default(),
                    Node::new(vec![
                        Edge::new(Some(detector(2)), obs_mask(3)),
                        Edge::new(Some(detector(2)), obs_mask(4)),
                    ]),
                    Node::default(),
                ],
                64,
                obs_mask(0),
            )
        );

        g.add_outward_edge(detector(2), Some(detector(1)), obs_mask(3))
            .unwrap();
        assert_eq!(
            g,
            Graph::from_parts(
                vec![
                    Node::default(),
                    Node::new(vec![
                        Edge::new(Some(detector(2)), obs_mask(3)),
                        Edge::new(Some(detector(2)), obs_mask(4)),
                    ]),
                    Node::new(vec![Edge::new(Some(detector(1)), obs_mask(3))]),
                ],
                64,
                obs_mask(0),
            )
        );

        g.add_outward_edge(detector(2), None, obs_mask(3)).unwrap();
        assert_eq!(
            g,
            Graph::from_parts(
                vec![
                    Node::default(),
                    Node::new(vec![
                        Edge::new(Some(detector(2)), obs_mask(3)),
                        Edge::new(Some(detector(2)), obs_mask(4)),
                    ]),
                    Node::new(vec![
                        Edge::new(Some(detector(1)), obs_mask(3)),
                        Edge::new(None, obs_mask(3)),
                    ]),
                ],
                64,
                obs_mask(0),
            )
        );
    }

    #[test]
    fn graphlike_graph_add_edges_from_targets_with_no_separators_matches_upstream() {
        let mut g = Graph::new(4, 64);

        g.add_edges_from_targets_with_no_separators(
            &[DemTarget::relative_detector(1).unwrap()],
            false,
        )
        .unwrap();
        assert_eq!(
            g,
            Graph::from_parts(
                vec![
                    Node::default(),
                    Node::new(vec![Edge::new(None, obs_mask(0))]),
                    Node::default(),
                    Node::default(),
                ],
                64,
                obs_mask(0),
            )
        );

        g.add_edges_from_targets_with_no_separators(
            &[
                DemTarget::relative_detector(1).unwrap(),
                DemTarget::relative_detector(3).unwrap(),
                DemTarget::logical_observable(5).unwrap(),
            ],
            false,
        )
        .unwrap();
        assert_eq!(
            g,
            Graph::from_parts(
                vec![
                    Node::default(),
                    Node::new(vec![
                        Edge::new(None, obs_mask(0)),
                        Edge::new(Some(detector(3)), obs_mask(32)),
                    ]),
                    Node::default(),
                    Node::new(vec![Edge::new(Some(detector(1)), obs_mask(32))]),
                ],
                64,
                obs_mask(0),
            )
        );

        g.add_edges_from_targets_with_no_separators(
            &[
                DemTarget::logical_observable(3).unwrap(),
                DemTarget::logical_observable(7).unwrap(),
            ],
            false,
        )
        .unwrap();
        assert_eq!(
            g,
            Graph::from_parts(
                vec![
                    Node::default(),
                    Node::new(vec![
                        Edge::new(None, obs_mask(0)),
                        Edge::new(Some(detector(3)), obs_mask(32)),
                    ]),
                    Node::default(),
                    Node::new(vec![Edge::new(Some(detector(1)), obs_mask(32))]),
                ],
                64,
                obs_mask((1 << 3) + (1 << 7)),
            )
        );

        let too_big = [
            DemTarget::relative_detector(1).unwrap(),
            DemTarget::relative_detector(2).unwrap(),
            DemTarget::relative_detector(3).unwrap(),
        ];
        let mut same_g = g.clone();
        assert!(
            same_g
                .add_edges_from_targets_with_no_separators(&too_big, false)
                .is_err()
        );
        assert_eq!(g, same_g);
        same_g
            .add_edges_from_targets_with_no_separators(&too_big, true)
            .unwrap();
        assert_eq!(g, same_g);
    }

    #[test]
    fn graphlike_graph_string_matches_upstream() {
        let graph = Graph::from_parts(
            vec![
                Node::default(),
                Node::new(vec![
                    Edge::new(None, obs_mask(0)),
                    Edge::new(Some(detector(3)), obs_mask(32)),
                ]),
                Node::default(),
                Node::new(vec![Edge::new(Some(detector(1)), obs_mask(32))]),
            ],
            64,
            obs_mask(0),
        );
        assert_eq!(
            graph.to_string(),
            "0:\n1:\n    [boundary]\n    D3 L5\n2:\n3:\n    D1 L5\n"
        );
    }

    #[test]
    fn graphlike_graph_add_edges_from_separable_targets_matches_upstream() {
        let mut g = Graph::new(4, 64);

        g.add_edges_from_separable_targets(
            &[
                DemTarget::relative_detector(1).unwrap(),
                DemTarget::separator(),
                DemTarget::relative_detector(1).unwrap(),
                DemTarget::relative_detector(2).unwrap(),
                DemTarget::logical_observable(4).unwrap(),
            ],
            false,
        )
        .unwrap();

        assert_eq!(
            g,
            Graph::from_parts(
                vec![
                    Node::default(),
                    Node::new(vec![
                        Edge::new(None, obs_mask(0)),
                        Edge::new(Some(detector(2)), obs_mask(16)),
                    ]),
                    Node::new(vec![Edge::new(Some(detector(1)), obs_mask(16))]),
                    Node::default(),
                ],
                64,
                obs_mask(0),
            )
        );
    }

    #[test]
    fn graphlike_graph_from_dem_matches_upstream() {
        let model = DetectorErrorModel::from_dem_str(
            "error(0.1) D0\n\
             repeat 3 {\n\
                 error(0.1) D0 D1\n\
                 shift_detectors 1\n\
             }\n\
             error(0.1) D0 L7\n\
             error(0.1) D2 ^ D3 D4 L2\n\
             detector D5\n",
        )
        .unwrap();

        assert_eq!(
            Graph::from_dem(&model, false).unwrap(),
            Graph::from_parts(
                vec![
                    Node::new(vec![
                        Edge::new(None, obs_mask(0)),
                        Edge::new(Some(detector(1)), obs_mask(0)),
                    ]),
                    Node::new(vec![
                        Edge::new(Some(detector(0)), obs_mask(0)),
                        Edge::new(Some(detector(2)), obs_mask(0)),
                    ]),
                    Node::new(vec![
                        Edge::new(Some(detector(1)), obs_mask(0)),
                        Edge::new(Some(detector(3)), obs_mask(0)),
                    ]),
                    Node::new(vec![
                        Edge::new(Some(detector(2)), obs_mask(0)),
                        Edge::new(None, obs_mask(128)),
                    ]),
                    Node::default(),
                    Node::new(vec![Edge::new(None, obs_mask(0))]),
                    Node::new(vec![Edge::new(Some(detector(7)), obs_mask(4))]),
                    Node::new(vec![Edge::new(Some(detector(6)), obs_mask(4))]),
                    Node::default(),
                ],
                8,
                obs_mask(0),
            )
        );
    }

    #[test]
    fn graphlike_graph_add_edges_from_separable_targets_ignore_matches_upstream() {
        let mut actual = Graph::new(10, 64);
        let mut expected = Graph::new(10, 64);
        let dem = DetectorErrorModel::from_dem_str("error(0.125) D0 ^ D4 D6\n").unwrap();
        let targets = dem
            .items()
            .first()
            .and_then(|item| match item {
                DemItem::Instruction(instruction) => Some(instruction.targets()),
                DemItem::RepeatBlock(_) => None,
            })
            .expect("DEM input parsed to an instruction");

        assert_eq!(actual, expected);
        actual
            .add_edges_from_separable_targets(targets, true)
            .unwrap();
        assert_eq!(actual, expected);
        actual
            .add_edges_from_separable_targets(targets, false)
            .unwrap();
        expected
            .add_outward_edge(detector(4), Some(detector(6)), obs_mask(0))
            .unwrap();
        expected
            .add_outward_edge(detector(6), Some(detector(4)), obs_mask(0))
            .unwrap();
        expected
            .add_outward_edge(detector(0), None, obs_mask(0))
            .unwrap();
        assert_eq!(actual, expected);
    }

    #[test]
    fn graphlike_search_state_construct_matches_upstream() {
        let empty = SearchState::new(None, None, ObservableMask::new());
        assert_eq!(empty.detector_active, None);
        assert_eq!(empty.detector_held, None);
        assert_eq!(empty.observables, obs_mask(0));

        let full = state(Some(2), Some(1), 3);
        assert_eq!(full.detector_active, Some(detector(2)));
        assert_eq!(full.detector_held, Some(detector(1)));
        assert_eq!(full.observables, obs_mask(3));
    }

    #[test]
    fn graphlike_search_state_is_undetected_matches_upstream() {
        assert!(!state(Some(1), Some(2), 3).is_undetected());
        assert!(!state(Some(1), Some(2), 2).is_undetected());
        assert!(!state(Some(1), Some(2), 0).is_undetected());
        assert!(state(Some(1), Some(1), 3).is_undetected());
        assert!(state(None, None, 32).is_undetected());
        assert!(state(None, None, 0).is_undetected());
    }

    #[test]
    fn graphlike_search_state_canonical_matches_upstream() {
        assert_eq!(
            state(Some(1), Some(2), 3).canonical(),
            state(Some(1), Some(2), 3)
        );
        assert_eq!(
            state(Some(2), Some(1), 3).canonical(),
            state(Some(1), Some(2), 3)
        );
        assert_eq!(state(Some(1), Some(1), 3).canonical(), state(None, None, 3));
        assert_eq!(state(Some(1), Some(1), 1).canonical(), state(None, None, 1));
        assert_eq!(state(Some(1), None, 1).canonical(), state(Some(1), None, 1));
    }

    #[test]
    fn graphlike_search_state_appends_transitions_as_error_instructions() {
        let mut out = DetectorErrorModel::new();

        state(Some(1), Some(2), 9)
            .append_transition_as_error_instruction_to(&state(Some(1), Some(2), 16), &mut out)
            .unwrap();
        assert_eq!(out.to_dem_string(), "error(1) L0 L3 L4\n");

        state(Some(1), Some(2), 9)
            .append_transition_as_error_instruction_to(&state(Some(3), Some(2), 9), &mut out)
            .unwrap();
        assert_eq!(out.to_dem_string(), "error(1) L0 L3 L4\nerror(1) D1 D3\n");

        state(Some(1), Some(2), 9)
            .append_transition_as_error_instruction_to(&state(Some(1), None, 9), &mut out)
            .unwrap();
        assert_eq!(
            out.to_dem_string(),
            "error(1) L0 L3 L4\nerror(1) D1 D3\nerror(1) D2\n"
        );

        state(None, None, 0)
            .append_transition_as_error_instruction_to(&state(Some(1), None, 9), &mut out)
            .unwrap();
        assert_eq!(
            out.to_dem_string(),
            "error(1) L0 L3 L4\nerror(1) D1 D3\nerror(1) D2\nerror(1) D1 L0 L3\n"
        );

        state(Some(1), Some(1), 0)
            .append_transition_as_error_instruction_to(&state(Some(2), Some(2), 4), &mut out)
            .unwrap();
        assert_eq!(
            out.to_dem_string(),
            "error(1) L0 L3 L4\nerror(1) D1 D3\nerror(1) D2\nerror(1) D1 L0 L3\nerror(1) L2\n"
        );
    }

    #[test]
    fn graphlike_search_state_canonical_equality_matches_upstream() {
        let v1 = state(Some(1), Some(2), 3);
        let v2 = state(Some(1), Some(4), 3);

        assert_eq!(v1, v1);
        assert_ne!(v1, v2);
        assert_eq!(v1, state(Some(2), Some(1), 3));
        assert_ne!(v1, state(Some(1), None, 3));
        assert_eq!(state(None, None, 0), state(Some(1), Some(1), 0));
        assert_eq!(state(Some(3), Some(3), 0), state(Some(1), Some(1), 0));
        assert_ne!(state(Some(3), Some(3), 1), state(Some(1), Some(1), 0));
        assert_eq!(state(Some(3), Some(3), 1), state(Some(1), Some(1), 1));
        assert_eq!(state(Some(2), None, 3), state(None, Some(2), 3));
    }

    #[test]
    fn graphlike_search_state_canonical_ordering_matches_upstream() {
        assert!(state(Some(1), Some(999), 999) < state(Some(101), Some(102), 103));
        assert!(state(Some(999), Some(1), 999) < state(Some(101), Some(102), 103));
        assert!(state(Some(101), Some(1), 999) < state(Some(101), Some(102), 103));
        assert!(state(Some(102), Some(1), 999) < state(Some(101), Some(102), 103));
        assert!(state(Some(101), Some(102), 3) < state(Some(101), Some(102), 103));

        assert!(!(state(Some(101), Some(102), 103) < state(Some(101), Some(102), 103)));
        assert!(!(state(Some(101), Some(104), 103) < state(Some(101), Some(102), 103)));
        assert!(!(state(Some(101), Some(102), 104) < state(Some(101), Some(102), 103)));
    }

    #[test]
    fn graphlike_search_state_string_matches_upstream() {
        assert_eq!(state(Some(1), Some(2), 3).to_string(), "D1 D2 L0 L1 ");
    }

    #[test]
    fn graphlike_search_state_hash_matches_upstream() {
        assert_eq!(
            hash(state(Some(1), Some(2), 3)),
            hash(state(Some(2), Some(1), 3))
        );
        assert_eq!(
            hash(state(Some(1), Some(2), 3)),
            hash(state(Some(1), Some(2), 3))
        );
        assert_ne!(
            hash(state(Some(1), Some(2), 3)),
            hash(state(Some(2), Some(2), 3))
        );
        assert_ne!(
            hash(state(Some(1), Some(2), 3)),
            hash(state(Some(1), Some(2), 4))
        );
    }
}
