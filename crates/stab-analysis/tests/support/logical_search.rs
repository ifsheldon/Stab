#![allow(
    dead_code,
    reason = "each test crate uses a subset of the shared logical-search options"
)]

use stab_analysis::{
    GraphlikeSearchOptions, HypergraphSearchOptions, LogicalErrorSearchLimits, SymptomGrowth,
    UngraphlikeErrors,
};

/// Graphlike search that fails on an ungraphlike error component.
pub(crate) fn rejecting_ungraphlike() -> GraphlikeSearchOptions {
    GraphlikeSearchOptions {
        ungraphlike_errors: UngraphlikeErrors::Reject,
        ..GraphlikeSearchOptions::default()
    }
}

/// Hypergraph search with no size pruning and default limits.
pub(crate) fn unbounded_hypergraph() -> HypergraphSearchOptions {
    HypergraphSearchOptions {
        max_detection_event_set_size: usize::MAX,
        max_edge_degree: usize::MAX,
        symptom_growth: SymptomGrowth::Explore,
        limits: LogicalErrorSearchLimits::default(),
    }
}
