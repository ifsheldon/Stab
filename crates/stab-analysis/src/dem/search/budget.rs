use crate::resources::LogicalErrorSearchResource;
use crate::{AnalysisError, AnalysisResult, ResourceLimitError};

const DEFAULT_MAX_REPEAT_UNROLL: u64 = 100_000;
const DEFAULT_MAX_REPEAT_ITERATIONS: u64 = 1_000_000;
const DEFAULT_MAX_ERROR_MECHANISMS: u64 = 5_000_000;
const DEFAULT_MAX_ERROR_TARGETS_PER_MECHANISM: usize = 65_536;
const DEFAULT_MAX_TOTAL_ERROR_TARGETS: usize = 20_000_000;
const DEFAULT_MAX_EFFECTIVE_DETECTOR_NODES: usize = 1_000_000;
const DEFAULT_MAX_GRAPH_EDGES: usize = 5_000_000;
const DEFAULT_MAX_STORED_GRAPH_TERMS: usize = 20_000_000;
const DEFAULT_MAX_HYPEREDGE_DEGREE: usize = 4_096;
const DEFAULT_MAX_HYPEREDGE_INCIDENCES: usize = 5_000_000;
const DEFAULT_MAX_SEARCH_STATES: usize = 1_000_000;
const DEFAULT_MAX_SEARCH_TRANSITIONS: u64 = 20_000_000;
const DEFAULT_MAX_SEARCH_STATE_TERMS: usize = 65_536;
const DEFAULT_MAX_STORED_SEARCH_STATE_TERMS: usize = 5_000_000;

/// Resource policy for graphlike and hypergraph logical-error search.
///
/// The policy owns limits on expanded traversal and retained search data. Repeat nesting remains
/// a fixed detector-error-model invariant, and SAT materialization has a separate policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LogicalErrorSearchLimits {
    max_repeat_unroll: u64,
    max_repeat_iterations: u64,
    max_expanded_error_mechanisms: u64,
    max_error_target_occurrences_per_mechanism: usize,
    max_total_error_target_occurrences: usize,
    max_effective_detector_nodes: usize,
    max_unique_graph_edges: usize,
    max_stored_graph_terms: usize,
    max_hyperedge_degree: usize,
    max_hyperedge_incidences: usize,
    max_search_states: usize,
    max_search_transitions: u64,
    max_search_state_terms: usize,
    max_stored_search_state_terms: usize,
}

impl LogicalErrorSearchLimits {
    /// Returns the largest repeat count that search may expand directly.
    pub const fn max_repeat_unroll(self) -> u64 {
        self.max_repeat_unroll
    }

    /// Returns the aggregate repeat-iteration budget across one folded traversal pass.
    pub const fn max_repeat_iterations(self) -> u64 {
        self.max_repeat_iterations
    }

    /// Returns the nonzero-error visit budget for one graph-construction pass.
    pub const fn max_expanded_error_mechanisms(self) -> u64 {
        self.max_expanded_error_mechanisms
    }

    /// Returns the target-occurrence limit for one nonzero error mechanism.
    pub const fn max_error_target_occurrences_per_mechanism(self) -> usize {
        self.max_error_target_occurrences_per_mechanism
    }

    /// Returns the aggregate target-occurrence budget for one graph-construction pass.
    pub const fn max_total_error_target_occurrences(self) -> usize {
        self.max_total_error_target_occurrences
    }

    /// Returns the largest number of detector nodes retained by the search graph.
    pub const fn max_effective_detector_nodes(self) -> usize {
        self.max_effective_detector_nodes
    }

    /// Returns the largest number of unique edges retained by the search graph.
    pub const fn max_unique_graph_edges(self) -> usize {
        self.max_unique_graph_edges
    }

    /// Returns the aggregate retained graph payload-term budget.
    pub const fn max_stored_graph_terms(self) -> usize {
        self.max_stored_graph_terms
    }

    /// Returns the largest detector degree accepted for one hyperedge.
    pub const fn max_hyperedge_degree(self) -> usize {
        self.max_hyperedge_degree
    }

    /// Returns the aggregate retained hyperedge-incidence budget.
    pub const fn max_hyperedge_incidences(self) -> usize {
        self.max_hyperedge_incidences
    }

    /// Returns the largest number of distinct states retained by a search.
    pub const fn max_search_states(self) -> usize {
        self.max_search_states
    }

    /// Returns the largest number of graph transitions a search may examine.
    pub const fn max_search_transitions(self) -> u64 {
        self.max_search_transitions
    }

    /// Returns the detector-and-observable term limit for one search state.
    pub const fn max_search_state_terms(self) -> usize {
        self.max_search_state_terms
    }

    /// Returns the aggregate retained search-state payload-term budget.
    pub const fn max_stored_search_state_terms(self) -> usize {
        self.max_stored_search_state_terms
    }

    /// Sets the largest repeat count that search may expand directly.
    #[must_use]
    pub const fn with_max_repeat_unroll(mut self, limit: u64) -> Self {
        self.max_repeat_unroll = limit;
        self
    }

    /// Sets the aggregate repeat-iteration budget across one folded traversal pass.
    #[must_use]
    pub const fn with_max_repeat_iterations(mut self, limit: u64) -> Self {
        self.max_repeat_iterations = limit;
        self
    }

    /// Sets the nonzero-error visit budget for one graph-construction pass.
    #[must_use]
    pub const fn with_max_expanded_error_mechanisms(mut self, limit: u64) -> Self {
        self.max_expanded_error_mechanisms = limit;
        self
    }

    /// Sets the target-occurrence limit for one nonzero error mechanism.
    #[must_use]
    pub const fn with_max_error_target_occurrences_per_mechanism(mut self, limit: usize) -> Self {
        self.max_error_target_occurrences_per_mechanism = limit;
        self
    }

    /// Sets the aggregate target-occurrence budget for one graph-construction pass.
    #[must_use]
    pub const fn with_max_total_error_target_occurrences(mut self, limit: usize) -> Self {
        self.max_total_error_target_occurrences = limit;
        self
    }

    /// Sets the largest number of detector nodes retained by the search graph.
    #[must_use]
    pub const fn with_max_effective_detector_nodes(mut self, limit: usize) -> Self {
        self.max_effective_detector_nodes = limit;
        self
    }

    /// Sets the largest number of unique edges retained by the search graph.
    #[must_use]
    pub const fn with_max_unique_graph_edges(mut self, limit: usize) -> Self {
        self.max_unique_graph_edges = limit;
        self
    }

    /// Sets the aggregate retained graph payload-term budget.
    #[must_use]
    pub const fn with_max_stored_graph_terms(mut self, limit: usize) -> Self {
        self.max_stored_graph_terms = limit;
        self
    }

    /// Sets the largest detector degree accepted for one hyperedge.
    #[must_use]
    pub const fn with_max_hyperedge_degree(mut self, limit: usize) -> Self {
        self.max_hyperedge_degree = limit;
        self
    }

    /// Sets the aggregate retained hyperedge-incidence budget.
    #[must_use]
    pub const fn with_max_hyperedge_incidences(mut self, limit: usize) -> Self {
        self.max_hyperedge_incidences = limit;
        self
    }

    /// Sets the largest number of distinct states retained by a search.
    #[must_use]
    pub const fn with_max_search_states(mut self, limit: usize) -> Self {
        self.max_search_states = limit;
        self
    }

    /// Sets the largest number of graph transitions a search may examine.
    #[must_use]
    pub const fn with_max_search_transitions(mut self, limit: u64) -> Self {
        self.max_search_transitions = limit;
        self
    }

    /// Sets the detector-and-observable term limit for one search state.
    #[must_use]
    pub const fn with_max_search_state_terms(mut self, limit: usize) -> Self {
        self.max_search_state_terms = limit;
        self
    }

    /// Sets the aggregate retained search-state payload-term budget.
    #[must_use]
    pub const fn with_max_stored_search_state_terms(mut self, limit: usize) -> Self {
        self.max_stored_search_state_terms = limit;
        self
    }
}

impl Default for LogicalErrorSearchLimits {
    fn default() -> Self {
        Self {
            max_repeat_unroll: DEFAULT_MAX_REPEAT_UNROLL,
            max_repeat_iterations: DEFAULT_MAX_REPEAT_ITERATIONS,
            max_expanded_error_mechanisms: DEFAULT_MAX_ERROR_MECHANISMS,
            max_error_target_occurrences_per_mechanism: DEFAULT_MAX_ERROR_TARGETS_PER_MECHANISM,
            max_total_error_target_occurrences: DEFAULT_MAX_TOTAL_ERROR_TARGETS,
            max_effective_detector_nodes: DEFAULT_MAX_EFFECTIVE_DETECTOR_NODES,
            max_unique_graph_edges: DEFAULT_MAX_GRAPH_EDGES,
            max_stored_graph_terms: DEFAULT_MAX_STORED_GRAPH_TERMS,
            max_hyperedge_degree: DEFAULT_MAX_HYPEREDGE_DEGREE,
            max_hyperedge_incidences: DEFAULT_MAX_HYPEREDGE_INCIDENCES,
            max_search_states: DEFAULT_MAX_SEARCH_STATES,
            max_search_transitions: DEFAULT_MAX_SEARCH_TRANSITIONS,
            max_search_state_terms: DEFAULT_MAX_SEARCH_STATE_TERMS,
            max_stored_search_state_terms: DEFAULT_MAX_STORED_SEARCH_STATE_TERMS,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct GraphConstructionBudget {
    context: &'static str,
    limits: LogicalErrorSearchLimits,
    edges: usize,
    stored_terms: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct GraphEdgeAdmission {
    prior_edges: usize,
    prior_stored_terms: usize,
    edges: usize,
    stored_terms: usize,
}

impl GraphConstructionBudget {
    pub(super) fn new(context: &'static str, limits: LogicalErrorSearchLimits) -> Self {
        Self {
            context,
            limits,
            edges: 0,
            stored_terms: 0,
        }
    }

    pub(super) const fn limits(&self) -> LogicalErrorSearchLimits {
        self.limits
    }

    pub(super) fn preflight_unique_edge(
        &self,
        edge_terms: usize,
        edge_stored_copies: usize,
        adjacency_stored_terms: usize,
    ) -> AnalysisResult<GraphEdgeAdmission> {
        let edges = self.edges.checked_add(1).ok_or_else(|| {
            AnalysisError::invalid_detector_error_model(format!(
                "{} graph edge count overflowed",
                self.context
            ))
        })?;
        let limit = self.limits.max_unique_graph_edges();
        if edges > limit {
            return Err(ResourceLimitError::logical_error_search(
                self.context,
                LogicalErrorSearchResource::UniqueGraphEdges,
                edges as u64,
                limit as u64,
            )
            .into());
        }
        let added_terms = edge_terms
            .checked_mul(edge_stored_copies)
            .and_then(|terms| terms.checked_add(adjacency_stored_terms))
            .ok_or_else(|| {
                AnalysisError::invalid_detector_error_model(format!(
                    "{} stored graph term count overflowed",
                    self.context
                ))
            })?;
        let stored_terms = self.checked_stored_terms(added_terms)?;
        Ok(GraphEdgeAdmission {
            prior_edges: self.edges,
            prior_stored_terms: self.stored_terms,
            edges,
            stored_terms,
        })
    }

    pub(super) fn commit_unique_edge(
        &mut self,
        admission: GraphEdgeAdmission,
    ) -> AnalysisResult<()> {
        if self.edges != admission.prior_edges || self.stored_terms != admission.prior_stored_terms
        {
            return Err(AnalysisError::invalid_detector_error_model(format!(
                "{} graph construction admission became stale before commit",
                self.context
            )));
        }
        self.edges = admission.edges;
        self.stored_terms = admission.stored_terms;
        Ok(())
    }

    #[cfg(test)]
    pub(super) fn admit_adjacency(&mut self, stored_copies: usize) -> AnalysisResult<()> {
        self.stored_terms = self.checked_stored_terms(stored_copies)?;
        Ok(())
    }

    fn checked_stored_terms(&self, added_terms: usize) -> AnalysisResult<usize> {
        let stored_terms = self.stored_terms.checked_add(added_terms).ok_or_else(|| {
            AnalysisError::invalid_detector_error_model(format!(
                "{} stored graph term count overflowed",
                self.context
            ))
        })?;
        let limit = self.limits.max_stored_graph_terms();
        if stored_terms > limit {
            return Err(ResourceLimitError::logical_error_search(
                self.context,
                LogicalErrorSearchResource::StoredGraphTerms,
                stored_terms as u64,
                limit as u64,
            )
            .into());
        }
        Ok(stored_terms)
    }
}

#[derive(Debug)]
pub(super) struct SearchBudget {
    context: &'static str,
    limits: LogicalErrorSearchLimits,
    states: usize,
    transitions: u64,
    stored_state_terms: usize,
}

impl SearchBudget {
    pub(super) fn new(context: &'static str, limits: LogicalErrorSearchLimits) -> Self {
        Self {
            context,
            limits,
            states: 0,
            transitions: 0,
            stored_state_terms: 0,
        }
    }

    pub(super) fn preflight_state_terms(&self, state_terms: usize) -> AnalysisResult<()> {
        let limit = self.limits.max_search_state_terms();
        if state_terms > limit {
            return Err(ResourceLimitError::logical_error_search(
                self.context,
                LogicalErrorSearchResource::SearchStateTerms,
                state_terms as u64,
                limit as u64,
            )
            .into());
        }
        Ok(())
    }

    pub(super) fn admit_state(
        &mut self,
        state_terms: usize,
        predecessor_terms: usize,
        queued: bool,
    ) -> AnalysisResult<()> {
        self.preflight_state_terms(state_terms)?;
        self.preflight_state_terms(predecessor_terms)?;
        let next = self.states.checked_add(1).ok_or_else(|| {
            AnalysisError::invalid_detector_error_model(format!(
                "{} search state count overflowed",
                self.context
            ))
        })?;
        let state_limit = self.limits.max_search_states();
        if next > state_limit {
            return Err(ResourceLimitError::logical_error_search(
                self.context,
                LogicalErrorSearchResource::SearchStates,
                next as u64,
                state_limit as u64,
            )
            .into());
        }
        let state_copies = if queued { 2 } else { 1 };
        let added_state_terms = state_terms
            .checked_mul(state_copies)
            .and_then(|terms| terms.checked_add(predecessor_terms))
            .ok_or_else(|| {
                AnalysisError::invalid_detector_error_model(format!(
                    "{} stored search-state term count overflowed",
                    self.context
                ))
            })?;
        let stored_state_terms = self
            .stored_state_terms
            .checked_add(added_state_terms)
            .ok_or_else(|| {
                AnalysisError::invalid_detector_error_model(format!(
                    "{} stored search-state term count overflowed",
                    self.context
                ))
            })?;
        let stored_limit = self.limits.max_stored_search_state_terms();
        if stored_state_terms > stored_limit {
            return Err(ResourceLimitError::logical_error_search(
                self.context,
                LogicalErrorSearchResource::StoredSearchStateTerms,
                stored_state_terms as u64,
                stored_limit as u64,
            )
            .into());
        }
        self.states = next;
        self.stored_state_terms = stored_state_terms;
        Ok(())
    }

    pub(super) fn record_transition(&mut self) -> AnalysisResult<()> {
        let next = self.transitions.checked_add(1).ok_or_else(|| {
            AnalysisError::invalid_detector_error_model(format!(
                "{} search transition count overflowed",
                self.context
            ))
        })?;
        let limit = self.limits.max_search_transitions();
        if next > limit {
            return Err(ResourceLimitError::logical_error_search(
                self.context,
                LogicalErrorSearchResource::SearchTransitions,
                next,
                limit,
            )
            .into());
        }
        self.transitions = next;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::expect_used,
        reason = "unit tests use direct assertions for compact boundary diagnostics"
    )]

    use crate::ResourceKind;

    use super::*;

    #[test]
    fn graph_construction_preflight_does_not_consume_budget_until_commit() {
        let mut budget =
            GraphConstructionBudget::new("test graph", LogicalErrorSearchLimits::default());
        let admission = budget
            .preflight_unique_edge(3, 1, 2)
            .expect("edge fits the construction budget");
        assert_eq!(budget.edges, 0);
        assert_eq!(budget.stored_terms, 0);

        budget
            .commit_unique_edge(admission)
            .expect("fresh admission commits");
        assert_eq!(budget.edges, 1);
        assert_eq!(budget.stored_terms, 5);
    }

    #[test]
    fn graph_construction_rejects_a_stale_admission() {
        let mut budget =
            GraphConstructionBudget::new("test graph", LogicalErrorSearchLimits::default());
        let first = budget
            .preflight_unique_edge(1, 1, 0)
            .expect("first edge fits");
        let stale = budget
            .preflight_unique_edge(1, 1, 0)
            .expect("concurrent preflight also fits");
        budget
            .commit_unique_edge(first)
            .expect("first admission commits");

        let error = budget
            .commit_unique_edge(stale)
            .expect_err("stale admission must not overwrite the committed counters");
        assert!(error.to_string().contains("admission became stale"));
        assert_eq!(budget.edges, 1);
        assert_eq!(budget.stored_terms, 1);
    }

    #[test]
    fn graph_and_search_budgets_accept_exact_limits_and_reject_first_excess() {
        let limits = LogicalErrorSearchLimits::default()
            .with_max_unique_graph_edges(1)
            .with_max_stored_graph_terms(3)
            .with_max_search_states(1)
            .with_max_search_transitions(1)
            .with_max_search_state_terms(2)
            .with_max_stored_search_state_terms(5);

        let mut graph = GraphConstructionBudget::new("test graph", limits);
        let exact_edge = graph
            .preflight_unique_edge(2, 1, 1)
            .expect("exact edge and stored-term limits are inclusive");
        graph
            .commit_unique_edge(exact_edge)
            .expect("exact graph admission commits");
        assert_resource(
            graph
                .preflight_unique_edge(0, 0, 0)
                .expect_err("first excess edge is rejected"),
            ResourceKind::UniqueGraphEdges,
            2,
            1,
        );
        assert_resource(
            graph
                .admit_adjacency(1)
                .expect_err("first excess stored graph term is rejected"),
            ResourceKind::StoredGraphTerms,
            4,
            3,
        );

        let mut search = SearchBudget::new("test search", limits);
        search
            .preflight_state_terms(2)
            .expect("exact per-state term limit is inclusive");
        assert_resource(
            search
                .preflight_state_terms(3)
                .expect_err("first excess per-state term is rejected"),
            ResourceKind::SearchStateTerms,
            3,
            2,
        );
        search
            .admit_state(2, 1, true)
            .expect("exact state and retained-term limits are inclusive");
        assert_resource(
            search
                .admit_state(0, 0, false)
                .expect_err("first excess state is rejected"),
            ResourceKind::SearchStates,
            2,
            1,
        );
        let mut retained_terms = SearchBudget::new("test search", limits.with_max_search_states(2));
        retained_terms
            .admit_state(2, 1, true)
            .expect("exact retained-term limit is inclusive");
        assert_resource(
            retained_terms
                .admit_state(1, 0, false)
                .expect_err("first excess retained term is rejected"),
            ResourceKind::StoredSearchStateTerms,
            6,
            5,
        );
        search
            .record_transition()
            .expect("exact transition limit is inclusive");
        assert_resource(
            search
                .record_transition()
                .expect_err("first excess transition is rejected"),
            ResourceKind::SearchTransitions,
            2,
            1,
        );
    }

    fn assert_resource(error: AnalysisError, expected: ResourceKind, actual: u64, limit: u64) {
        let resource = error
            .resource_limit_error()
            .expect("resource rejection retains typed context");
        assert_eq!(resource.resource(), expected);
        assert_eq!(resource.actual(), actual);
        assert_eq!(resource.limit(), limit);
    }

    #[test]
    fn budget_overflow_rejects_without_committing_partial_state() {
        let mut graph =
            GraphConstructionBudget::new("test graph", LogicalErrorSearchLimits::default());
        graph.edges = usize::MAX;
        let error = graph
            .preflight_unique_edge(0, 0, 0)
            .expect_err("edge count overflow");
        assert!(error.to_string().contains("edge count overflowed"));
        assert_eq!(graph.edges, usize::MAX);
        assert_eq!(graph.stored_terms, 0);

        graph.edges = 0;
        graph.stored_terms = usize::MAX;
        let error = graph
            .preflight_unique_edge(1, 1, 0)
            .expect_err("stored graph term overflow");
        assert!(
            error
                .to_string()
                .contains("stored graph term count overflowed")
        );
        assert_eq!(graph.edges, 0);
        assert_eq!(graph.stored_terms, usize::MAX);

        let mut search = SearchBudget::new("test search", LogicalErrorSearchLimits::default());
        search.states = usize::MAX;
        let error = search
            .admit_state(0, 0, false)
            .expect_err("search state overflow");
        assert!(error.to_string().contains("state count overflowed"));
        assert_eq!(search.states, usize::MAX);
        assert_eq!(search.stored_state_terms, 0);

        search.states = 0;
        search.transitions = u64::MAX;
        let error = search
            .record_transition()
            .expect_err("search transition overflow");
        assert!(error.to_string().contains("transition count overflowed"));
        assert_eq!(search.transitions, u64::MAX);
    }
}
