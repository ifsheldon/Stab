/// Admission limits for DEM SAT traversal, CNF materialization, and WCNF serialization.
///
/// These limits belong to the SAT-generation operation. They do not change DEM parsing,
/// validation, or compact-model semantics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SatMaterializationLimits {
    max_repeat_unroll: u64,
    max_expanded_instructions: u64,
    max_repeat_iterations: u64,
    max_error_mechanisms: usize,
    max_target_occurrences: usize,
    max_variables: usize,
    max_clauses: usize,
    max_clause_literals: usize,
    max_output_bytes: usize,
}

impl SatMaterializationLimits {
    pub const DEFAULT_MAX_REPEAT_UNROLL: u64 = 100_000;
    pub const DEFAULT_MAX_EXPANDED_INSTRUCTIONS: u64 = 1_000_000;
    pub const DEFAULT_MAX_REPEAT_ITERATIONS: u64 = 1_000_000;
    pub const DEFAULT_MAX_ERROR_MECHANISMS: usize = 250_000;
    pub const DEFAULT_MAX_TARGET_OCCURRENCES: usize = 500_000;
    pub const DEFAULT_MAX_VARIABLES: usize = 500_000;
    pub const DEFAULT_MAX_CLAUSES: usize = 500_000;
    pub const DEFAULT_MAX_CLAUSE_LITERALS: usize = 1_500_000;
    pub const DEFAULT_MAX_OUTPUT_BYTES: usize = 128 * 1024 * 1024;

    pub const fn max_repeat_unroll(self) -> u64 {
        self.max_repeat_unroll
    }

    pub const fn max_expanded_instructions(self) -> u64 {
        self.max_expanded_instructions
    }

    pub const fn max_repeat_iterations(self) -> u64 {
        self.max_repeat_iterations
    }

    pub const fn max_error_mechanisms(self) -> usize {
        self.max_error_mechanisms
    }

    pub const fn max_target_occurrences(self) -> usize {
        self.max_target_occurrences
    }

    pub const fn max_variables(self) -> usize {
        self.max_variables
    }

    pub const fn max_clauses(self) -> usize {
        self.max_clauses
    }

    pub const fn max_clause_literals(self) -> usize {
        self.max_clause_literals
    }

    pub const fn max_output_bytes(self) -> usize {
        self.max_output_bytes
    }

    #[must_use]
    pub const fn with_max_repeat_unroll(mut self, limit: u64) -> Self {
        self.max_repeat_unroll = limit;
        self
    }

    #[must_use]
    pub const fn with_max_expanded_instructions(mut self, limit: u64) -> Self {
        self.max_expanded_instructions = limit;
        self
    }

    #[must_use]
    pub const fn with_max_repeat_iterations(mut self, limit: u64) -> Self {
        self.max_repeat_iterations = limit;
        self
    }

    #[must_use]
    pub const fn with_max_error_mechanisms(mut self, limit: usize) -> Self {
        self.max_error_mechanisms = limit;
        self
    }

    #[must_use]
    pub const fn with_max_target_occurrences(mut self, limit: usize) -> Self {
        self.max_target_occurrences = limit;
        self
    }

    #[must_use]
    pub const fn with_max_variables(mut self, limit: usize) -> Self {
        self.max_variables = limit;
        self
    }

    #[must_use]
    pub const fn with_max_clauses(mut self, limit: usize) -> Self {
        self.max_clauses = limit;
        self
    }

    #[must_use]
    pub const fn with_max_clause_literals(mut self, limit: usize) -> Self {
        self.max_clause_literals = limit;
        self
    }

    #[must_use]
    pub const fn with_max_output_bytes(mut self, limit: usize) -> Self {
        self.max_output_bytes = limit;
        self
    }
}

impl Default for SatMaterializationLimits {
    fn default() -> Self {
        Self {
            max_repeat_unroll: Self::DEFAULT_MAX_REPEAT_UNROLL,
            max_expanded_instructions: Self::DEFAULT_MAX_EXPANDED_INSTRUCTIONS,
            max_repeat_iterations: Self::DEFAULT_MAX_REPEAT_ITERATIONS,
            max_error_mechanisms: Self::DEFAULT_MAX_ERROR_MECHANISMS,
            max_target_occurrences: Self::DEFAULT_MAX_TARGET_OCCURRENCES,
            max_variables: Self::DEFAULT_MAX_VARIABLES,
            max_clauses: Self::DEFAULT_MAX_CLAUSES,
            max_clause_literals: Self::DEFAULT_MAX_CLAUSE_LITERALS,
            max_output_bytes: Self::DEFAULT_MAX_OUTPUT_BYTES,
        }
    }
}
