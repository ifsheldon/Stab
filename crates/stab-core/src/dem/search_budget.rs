use crate::{CircuitError, CircuitResult};

#[cfg(not(test))]
const MAX_DEM_SEARCH_STATES: usize = 1_000_000;
#[cfg(test)]
const MAX_DEM_SEARCH_STATES: usize = 64;

#[cfg(not(test))]
const MAX_DEM_SEARCH_TRANSITIONS: u64 = 20_000_000;
#[cfg(test)]
const MAX_DEM_SEARCH_TRANSITIONS: u64 = 4_096;

#[cfg(not(test))]
const MAX_DEM_SEARCH_STATE_TERMS: usize = 65_536;
#[cfg(test)]
const MAX_DEM_SEARCH_STATE_TERMS: usize = 64;

#[cfg(not(test))]
const MAX_DEM_SEARCH_STORED_STATE_TERMS: usize = 5_000_000;
#[cfg(test)]
const MAX_DEM_SEARCH_STORED_STATE_TERMS: usize = 256;

#[cfg(not(test))]
const MAX_DEM_SEARCH_GRAPH_EDGES: usize = 1_000_000;
#[cfg(test)]
const MAX_DEM_SEARCH_GRAPH_EDGES: usize = 64;

#[cfg(not(test))]
const MAX_DEM_SEARCH_STORED_GRAPH_TERMS: usize = 20_000_000;
#[cfg(test)]
const MAX_DEM_SEARCH_STORED_GRAPH_TERMS: usize = 2_048;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct GraphConstructionBudget {
    context: &'static str,
    edges: usize,
    stored_terms: usize,
}

impl GraphConstructionBudget {
    pub(super) fn new(context: &'static str) -> Self {
        Self {
            context,
            edges: 0,
            stored_terms: 0,
        }
    }

    pub(super) fn admit_unique_edge(
        &mut self,
        edge_terms: usize,
        edge_stored_copies: usize,
        adjacency_stored_terms: usize,
    ) -> CircuitResult<()> {
        let edges = self.edges.checked_add(1).ok_or_else(|| {
            CircuitError::invalid_detector_error_model(format!(
                "{} graph edge count overflowed",
                self.context
            ))
        })?;
        if edges > MAX_DEM_SEARCH_GRAPH_EDGES {
            return Err(CircuitError::invalid_detector_error_model(format!(
                "{} currently supports at most {MAX_DEM_SEARCH_GRAPH_EDGES} unique graph edges, got at least {edges}",
                self.context
            )));
        }
        let added_terms = edge_terms
            .checked_mul(edge_stored_copies)
            .and_then(|terms| terms.checked_add(adjacency_stored_terms))
            .ok_or_else(|| {
                CircuitError::invalid_detector_error_model(format!(
                    "{} stored graph term count overflowed",
                    self.context
                ))
            })?;
        let stored_terms = self.checked_stored_terms(added_terms)?;
        self.edges = edges;
        self.stored_terms = stored_terms;
        Ok(())
    }

    pub(super) fn admit_adjacency(&mut self, stored_copies: usize) -> CircuitResult<()> {
        self.stored_terms = self.checked_stored_terms(stored_copies)?;
        Ok(())
    }

    fn checked_stored_terms(&self, added_terms: usize) -> CircuitResult<usize> {
        let stored_terms = self.stored_terms.checked_add(added_terms).ok_or_else(|| {
            CircuitError::invalid_detector_error_model(format!(
                "{} stored graph term count overflowed",
                self.context
            ))
        })?;
        if stored_terms > MAX_DEM_SEARCH_STORED_GRAPH_TERMS {
            return Err(CircuitError::invalid_detector_error_model(format!(
                "{} currently supports at most {MAX_DEM_SEARCH_STORED_GRAPH_TERMS} stored detector and observable graph terms, got at least {stored_terms}",
                self.context
            )));
        }
        Ok(stored_terms)
    }
}

#[derive(Debug)]
pub(super) struct SearchBudget {
    context: &'static str,
    states: usize,
    transitions: u64,
    stored_state_terms: usize,
}

impl SearchBudget {
    pub(super) fn new(context: &'static str) -> Self {
        Self {
            context,
            states: 0,
            transitions: 0,
            stored_state_terms: 0,
        }
    }

    pub(super) fn preflight_state_terms(&self, state_terms: usize) -> CircuitResult<()> {
        if state_terms > MAX_DEM_SEARCH_STATE_TERMS {
            return Err(CircuitError::invalid_detector_error_model(format!(
                "{} currently supports at most {MAX_DEM_SEARCH_STATE_TERMS} detector and observable terms per search state, got {state_terms}",
                self.context
            )));
        }
        Ok(())
    }

    pub(super) fn admit_state(
        &mut self,
        state_terms: usize,
        predecessor_terms: usize,
        queued: bool,
    ) -> CircuitResult<()> {
        self.preflight_state_terms(state_terms)?;
        self.preflight_state_terms(predecessor_terms)?;
        let next = self.states.checked_add(1).ok_or_else(|| {
            CircuitError::invalid_detector_error_model(format!(
                "{} search state count overflowed",
                self.context
            ))
        })?;
        if next > MAX_DEM_SEARCH_STATES {
            return Err(CircuitError::invalid_detector_error_model(format!(
                "{} currently supports at most {MAX_DEM_SEARCH_STATES} search states, got at least {next}",
                self.context
            )));
        }
        let state_copies = if queued { 2 } else { 1 };
        let added_state_terms = state_terms
            .checked_mul(state_copies)
            .and_then(|terms| terms.checked_add(predecessor_terms))
            .ok_or_else(|| {
                CircuitError::invalid_detector_error_model(format!(
                    "{} stored search-state term count overflowed",
                    self.context
                ))
            })?;
        let stored_state_terms = self
            .stored_state_terms
            .checked_add(added_state_terms)
            .ok_or_else(|| {
                CircuitError::invalid_detector_error_model(format!(
                    "{} stored search-state term count overflowed",
                    self.context
                ))
            })?;
        if stored_state_terms > MAX_DEM_SEARCH_STORED_STATE_TERMS {
            return Err(CircuitError::invalid_detector_error_model(format!(
                "{} currently supports at most {MAX_DEM_SEARCH_STORED_STATE_TERMS} stored detector and observable search-state terms, got at least {stored_state_terms}",
                self.context
            )));
        }
        self.states = next;
        self.stored_state_terms = stored_state_terms;
        Ok(())
    }

    pub(super) fn record_transition(&mut self) -> CircuitResult<()> {
        let next = self.transitions.checked_add(1).ok_or_else(|| {
            CircuitError::invalid_detector_error_model(format!(
                "{} search transition count overflowed",
                self.context
            ))
        })?;
        if next > MAX_DEM_SEARCH_TRANSITIONS {
            return Err(CircuitError::invalid_detector_error_model(format!(
                "{} currently supports at most {MAX_DEM_SEARCH_TRANSITIONS} search transitions, got at least {next}",
                self.context
            )));
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
    fn search_budget_enforces_state_and_transition_limits() {
        let mut state_budget = SearchBudget::new("test");
        for _ in 0..MAX_DEM_SEARCH_STATES {
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

        let mut transition_budget = SearchBudget::new("test");
        for _ in 0..MAX_DEM_SEARCH_TRANSITIONS {
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
        let mut budget = SearchBudget::new("test");
        assert!(
            budget
                .preflight_state_terms(MAX_DEM_SEARCH_STATE_TERMS)
                .is_ok()
        );
        assert!(
            budget
                .preflight_state_terms(MAX_DEM_SEARCH_STATE_TERMS + 1)
                .expect_err("state payload beyond limit")
                .to_string()
                .contains("at most 64 detector and observable terms per search state")
        );

        budget
            .admit_state(MAX_DEM_SEARCH_STATE_TERMS, MAX_DEM_SEARCH_STATE_TERMS, true)
            .expect("three bounded payload copies fit the aggregate limit");
        budget
            .admit_state(MAX_DEM_SEARCH_STATE_TERMS, 0, false)
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
        let mut edge_budget = GraphConstructionBudget::new("test graph");
        for _ in 0..MAX_DEM_SEARCH_GRAPH_EDGES {
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

        let mut payload_budget = GraphConstructionBudget::new("test graph");
        payload_budget
            .admit_unique_edge(MAX_DEM_SEARCH_STORED_GRAPH_TERMS / 2, 2, 0)
            .expect("payload boundary is inclusive");
        assert!(
            payload_budget
                .admit_adjacency(1)
                .expect_err("payload beyond limit")
                .to_string()
                .contains("at most 2048 stored detector and observable graph terms")
        );
    }
}
