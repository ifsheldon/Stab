const DEFAULT_MAX_RECORD_BITS: usize = 1_000_000;
const DEFAULT_MAX_EXPANDED_INSTRUCTIONS: u64 = 1_000_000;
const DEFAULT_MAX_REPEAT_ITERATIONS: u64 = 1_000_000;
const DEFAULT_MAX_COMPILED_TERMS: u64 = 16_000_000;
const DEFAULT_MAX_COMPILED_BYTES: u64 = 256 * 1024 * 1024;

/// Admission limits for compiling detection conversion.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DetectionConversionLimits {
    pub(super) max_record_bits: usize,
    pub(super) max_expanded_instructions: u64,
    pub(super) max_repeat_iterations: u64,
    pub(super) max_compiled_terms: u64,
    pub(super) max_compiled_bytes: u64,
}

impl DetectionConversionLimits {
    pub const fn max_record_bits(self) -> usize {
        self.max_record_bits
    }

    pub const fn max_expanded_instructions(self) -> u64 {
        self.max_expanded_instructions
    }

    pub const fn max_repeat_iterations(self) -> u64 {
        self.max_repeat_iterations
    }

    pub const fn max_compiled_terms(self) -> u64 {
        self.max_compiled_terms
    }

    pub const fn max_compiled_bytes(self) -> u64 {
        self.max_compiled_bytes
    }

    #[must_use]
    pub const fn with_max_record_bits(mut self, limit: usize) -> Self {
        self.max_record_bits = limit;
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
    pub const fn with_max_compiled_terms(mut self, limit: u64) -> Self {
        self.max_compiled_terms = limit;
        self
    }

    #[must_use]
    pub const fn with_max_compiled_bytes(mut self, limit: u64) -> Self {
        self.max_compiled_bytes = limit;
        self
    }
}

impl Default for DetectionConversionLimits {
    fn default() -> Self {
        Self {
            max_record_bits: DEFAULT_MAX_RECORD_BITS,
            max_expanded_instructions: DEFAULT_MAX_EXPANDED_INSTRUCTIONS,
            max_repeat_iterations: DEFAULT_MAX_REPEAT_ITERATIONS,
            max_compiled_terms: DEFAULT_MAX_COMPILED_TERMS,
            max_compiled_bytes: DEFAULT_MAX_COMPILED_BYTES,
        }
    }
}
