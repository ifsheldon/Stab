#![allow(
    clippy::expect_used,
    reason = "resource tests use direct assertions for fixed admission boundaries"
)]

use std::collections::BTreeSet;

use super::{DemDetectorId, DemObservableId, Graph, ObservableMask};

#[test]
fn graphlike_construction_bounds_unique_edges_and_persistent_payload() {
    let detector = DemDetectorId::try_new(0).expect("D0");
    let mut edge_limited = Graph::new(1, 128);
    for observable in 0..64 {
        edge_limited
            .add_outward_edge(detector, None, observable_mask([observable]))
            .expect("edge within test limit");
    }
    let error = edge_limited
        .add_outward_edge(detector, None, observable_mask([64]))
        .expect_err("unique edge cap");
    assert!(error.to_string().contains("at most 64 unique graph edges"));
    assert_eq!(edge_limited.nodes.first().expect("D0 node").edges.len(), 64);

    let mut payload_limited = Graph::new(1, 2_048);
    payload_limited
        .add_outward_edge(detector, None, observable_mask(0..2_047))
        .expect("payload boundary is inclusive");
    let error = payload_limited
        .add_outward_edge(detector, None, observable_mask([2_047]))
        .expect_err("persistent graph payload cap");
    assert!(
        error
            .to_string()
            .contains("at most 2048 stored graph payload terms")
    );
    assert_eq!(
        payload_limited.nodes.first().expect("D0 node").edges.len(),
        1
    );
}

fn observable_mask(values: impl IntoIterator<Item = u64>) -> ObservableMask {
    ObservableMask {
        observables: values
            .into_iter()
            .map(|value| DemObservableId::try_new(value).expect("observable id"))
            .collect::<BTreeSet<_>>(),
    }
}
