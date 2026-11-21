use stab_model::Cancellation;

/// Options of [`crate::shortest_error_sat_problem`] and [`crate::likeliest_error_sat_problem`]:
/// admission limits for DEM SAT traversal, CNF materialization, and WCNF serialization.
///
/// These limits belong to the SAT-generation operation. They do not change DEM parsing,
/// validation, or compact-model semantics.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SatMaterializationLimits {
    /// Largest number of instructions the folded traversal may visit.
    pub max_expanded_instructions: u64,
    /// Largest number of repeat iterations the folded traversal may visit.
    pub max_repeat_iterations: u64,
    /// Largest number of flattened error mechanisms.
    pub max_error_mechanisms: usize,
    /// Largest number of detector and observable target occurrences across those mechanisms.
    pub max_target_occurrences: usize,
    /// Largest number of SAT variables.
    pub max_variables: usize,
    /// Largest number of clauses.
    pub max_clauses: usize,
    /// Largest number of literals across all clauses.
    pub max_clause_literals: usize,
    /// Largest serialized WCNF size in bytes.
    pub max_output_bytes: usize,
    /// Signal polled once per traversed instruction and repeat iteration, flattened error
    /// mechanism, emitted clause, and serialized clause.
    pub cancellation: Cancellation,
}

impl SatMaterializationLimits {
    pub const DEFAULT_MAX_EXPANDED_INSTRUCTIONS: u64 = 1_000_000;
    pub const DEFAULT_MAX_REPEAT_ITERATIONS: u64 = 1_000_000;
    pub const DEFAULT_MAX_ERROR_MECHANISMS: usize = 250_000;
    pub const DEFAULT_MAX_TARGET_OCCURRENCES: usize = 500_000;
    pub const DEFAULT_MAX_VARIABLES: usize = 500_000;
    pub const DEFAULT_MAX_CLAUSES: usize = 500_000;
    pub const DEFAULT_MAX_CLAUSE_LITERALS: usize = 1_500_000;
    pub const DEFAULT_MAX_OUTPUT_BYTES: usize = 128 * 1024 * 1024;
}

impl Default for SatMaterializationLimits {
    fn default() -> Self {
        Self {
            max_expanded_instructions: Self::DEFAULT_MAX_EXPANDED_INSTRUCTIONS,
            max_repeat_iterations: Self::DEFAULT_MAX_REPEAT_ITERATIONS,
            max_error_mechanisms: Self::DEFAULT_MAX_ERROR_MECHANISMS,
            max_target_occurrences: Self::DEFAULT_MAX_TARGET_OCCURRENCES,
            max_variables: Self::DEFAULT_MAX_VARIABLES,
            max_clauses: Self::DEFAULT_MAX_CLAUSES,
            max_clause_literals: Self::DEFAULT_MAX_CLAUSE_LITERALS,
            max_output_bytes: Self::DEFAULT_MAX_OUTPUT_BYTES,
            cancellation: Cancellation::never(),
        }
    }
}
