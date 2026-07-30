use stab_model::ModelDialect;

use crate::CompilationOperation;

/// Source-owned registration metadata for one public compiler family.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct CompilationDescriptor {
    operation: CompilationOperation,
    input_dialect: ModelDialect,
    compiler_schema_version: u16,
    request_fingerprint_schema_version: Option<u16>,
    configurable_limits: bool,
    backend_selection: bool,
}

impl CompilationDescriptor {
    pub(crate) const fn new(
        operation: CompilationOperation,
        input_dialect: ModelDialect,
        compiler_schema_version: u16,
        request_fingerprint_schema_version: Option<u16>,
        configurable_limits: bool,
        backend_selection: bool,
    ) -> Self {
        Self {
            operation,
            input_dialect,
            compiler_schema_version,
            request_fingerprint_schema_version,
            configurable_limits,
            backend_selection,
        }
    }

    pub const fn operation(self) -> CompilationOperation {
        self.operation
    }

    pub const fn input_dialect(self) -> ModelDialect {
        self.input_dialect
    }

    pub const fn compiler_schema_version(self) -> u16 {
        self.compiler_schema_version
    }

    /// The public request-fingerprint schema, or `None` when this compiler has no such identity.
    pub const fn request_fingerprint_schema_version(self) -> Option<u16> {
        self.request_fingerprint_schema_version
    }

    pub const fn has_configurable_limits(self) -> bool {
        self.configurable_limits
    }

    pub const fn supports_backend_selection(self) -> bool {
        self.backend_selection
    }
}

/// Compatibility name for the sampling descriptor type.
pub type SamplingCompilationDescriptor = CompilationDescriptor;
