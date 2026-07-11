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

fn sparse_graph(detectors: &[u64], node_edges: Vec<Vec<Edge>>, num_observables: usize) -> Graph {
    let mut graph = Graph::try_new_sparse(detector_set(detectors), num_observables, true).unwrap();
    assert_eq!(graph.nodes.len(), node_edges.len());
    for (node_index, edges) in node_edges.into_iter().enumerate() {
        for edge in edges {
            let (edge_id, inserted) = graph.intern_edge(edge, 2).unwrap();
            if !inserted {
                graph.construction_budget.admit_adjacency(2).unwrap();
            }
            if graph.nodes[node_index].add_edge_id(edge_id).unwrap() {
                graph.edge_incidences += 1;
            }
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
    for observable in 0..64 {
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
