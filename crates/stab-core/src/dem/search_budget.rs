use crate::{CircuitError, CircuitResult};

#[cfg(not(test))]
const MAX_DEM_SEARCH_STATES: usize = 1_000_000;
#[cfg(test)]
const MAX_DEM_SEARCH_STATES: usize = 64;

#[cfg(not(test))]
const MAX_DEM_SEARCH_TRANSITIONS: u64 = 20_000_000;
#[cfg(test)]
const MAX_DEM_SEARCH_TRANSITIONS: u64 = 4_096;

#[derive(Debug)]
pub(super) struct SearchBudget {
    context: &'static str,
    states: usize,
    transitions: u64,
}

impl SearchBudget {
    pub(super) fn new(context: &'static str) -> Self {
        Self {
            context,
            states: 0,
            transitions: 0,
        }
    }

    pub(super) fn admit_state(&mut self) -> CircuitResult<()> {
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
        self.states = next;
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
            state_budget.admit_state().expect("state within limit");
        }
        assert!(
            state_budget
                .admit_state()
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
}
