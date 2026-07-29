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

    #[cfg(test)]
    pub(super) fn admit_unique_edge(
        &mut self,
        edge_terms: usize,
        edge_stored_copies: usize,
        adjacency_stored_terms: usize,
    ) -> AnalysisResult<()> {
        let admission =
            self.preflight_unique_edge(edge_terms, edge_stored_copies, adjacency_stored_terms)?;
        self.commit_unique_edge(admission)
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

    use super::*;

    #[test]
    fn logical_error_search_defaults_are_production_identical() {
        let limits = LogicalErrorSearchLimits::default();
        assert_eq!(limits.max_repeat_unroll(), 100_000);
        assert_eq!(limits.max_repeat_iterations(), 1_000_000);
        assert_eq!(limits.max_expanded_error_mechanisms(), 5_000_000);
        assert_eq!(limits.max_error_target_occurrences_per_mechanism(), 65_536);
        assert_eq!(limits.max_total_error_target_occurrences(), 20_000_000);
        assert_eq!(limits.max_effective_detector_nodes(), 1_000_000);
        assert_eq!(limits.max_unique_graph_edges(), 5_000_000);
        assert_eq!(limits.max_stored_graph_terms(), 20_000_000);
        assert_eq!(limits.max_hyperedge_degree(), 4_096);
        assert_eq!(limits.max_hyperedge_incidences(), 5_000_000);
        assert_eq!(limits.max_search_states(), 1_000_000);
        assert_eq!(limits.max_search_transitions(), 20_000_000);
        assert_eq!(limits.max_search_state_terms(), 65_536);
        assert_eq!(limits.max_stored_search_state_terms(), 5_000_000);
    }

    #[test]
    fn logical_error_search_builders_change_independent_dimensions() {
        let defaults = LogicalErrorSearchLimits::default();
        let changed = defaults.with_max_search_states(7);
        assert_eq!(changed.max_search_states(), 7);
        assert_eq!(
            changed.max_search_transitions(),
            defaults.max_search_transitions()
        );
        assert_eq!(
            changed.max_expanded_error_mechanisms(),
            defaults.max_expanded_error_mechanisms()
        );
        assert_eq!(
            changed.max_stored_graph_terms(),
            defaults.max_stored_graph_terms()
        );
    }

    #[test]
    fn search_budget_enforces_state_and_transition_limits() {
        let state_limits = LogicalErrorSearchLimits::default().with_max_search_states(64);
        let mut state_budget = SearchBudget::new("test", state_limits);
        for _ in 0..state_limits.max_search_states() {
            state_budget
                .admit_state(0, 0, false)
                .expect("state within limit");
        }
        assert!(
            state_budget
                .admit_state(0, 0, false)
                .expect_err("state beyond limit")
                .to_string()
                .contains("at most 64 search states")
        );

        let transition_limits =
            LogicalErrorSearchLimits::default().with_max_search_transitions(4_096);
        let mut transition_budget = SearchBudget::new("test", transition_limits);
        for _ in 0..transition_limits.max_search_transitions() {
            transition_budget
                .record_transition()
                .expect("transition within limit");
        }
        assert!(
            transition_budget
                .record_transition()
                .expect_err("transition beyond limit")
                .to_string()
                .contains("at most 4096 search transitions")
        );
    }

    #[test]
    fn search_budget_enforces_per_state_and_aggregate_payload_limits() {
        let limits = LogicalErrorSearchLimits::default()
            .with_max_search_state_terms(64)
            .with_max_stored_search_state_terms(256);
        let mut budget = SearchBudget::new("test", limits);
        assert!(
            budget
                .preflight_state_terms(limits.max_search_state_terms())
                .is_ok()
        );
        assert!(
            budget
                .preflight_state_terms(limits.max_search_state_terms() + 1)
                .expect_err("state payload beyond limit")
                .to_string()
                .contains("at most 64 detector and observable terms per search state")
        );

        budget
            .admit_state(
                limits.max_search_state_terms(),
                limits.max_search_state_terms(),
                true,
            )
            .expect("three bounded payload copies fit the aggregate limit");
        budget
            .admit_state(limits.max_search_state_terms(), 0, false)
            .expect("aggregate boundary is inclusive");
        assert!(
            budget
                .admit_state(1, 0, false)
                .expect_err("aggregate state payload beyond limit")
                .to_string()
                .contains("at most 256 stored detector and observable search-state terms")
        );
    }

    #[test]
    fn graph_construction_budget_enforces_edge_and_payload_limits() {
        let edge_limits = LogicalErrorSearchLimits::default().with_max_unique_graph_edges(64);
        let mut edge_budget = GraphConstructionBudget::new("test graph", edge_limits);
        for _ in 0..edge_limits.max_unique_graph_edges() {
            edge_budget
                .admit_unique_edge(1, 1, 0)
                .expect("edge within limit");
        }
        assert!(
            edge_budget
                .admit_unique_edge(1, 1, 0)
                .expect_err("edge beyond limit")
                .to_string()
                .contains("at most 64 unique graph edges")
        );

        let payload_limits = LogicalErrorSearchLimits::default().with_max_stored_graph_terms(2_048);
        let mut payload_budget = GraphConstructionBudget::new("test graph", payload_limits);
        payload_budget
            .admit_unique_edge(payload_limits.max_stored_graph_terms() / 2, 2, 0)
            .expect("payload boundary is inclusive");
        assert!(
            payload_budget
                .admit_adjacency(1)
                .expect_err("payload beyond limit")
                .to_string()
                .contains("at most 2048 stored graph payload terms")
        );
    }

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
