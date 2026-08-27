use crate::{
    CompilationOperation, Gate, ModelDialect, ParseLimits, RecordFormat,
    result_formats::{CodecCapability, codec_capabilities},
};

/// One compiler registration exposed through [`CapabilitySet`].
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct CompilationCapability {
    operation: CompilationOperation,
    input_dialect: ModelDialect,
    compiler_schema_version: u16,
    request_fingerprint_schema_version: Option<u16>,
    configurable_limits: bool,
}

impl CompilationCapability {
    pub(crate) const fn new(
        operation: CompilationOperation,
        input_dialect: ModelDialect,
        compiler_schema_version: u16,
        request_fingerprint_schema_version: Option<u16>,
        configurable_limits: bool,
    ) -> Self {
        Self {
            operation,
            input_dialect,
            compiler_schema_version,
            request_fingerprint_schema_version,
            configurable_limits,
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

    pub const fn request_fingerprint_schema_version(self) -> Option<u16> {
        self.request_fingerprint_schema_version
    }

    /// Whether this compiler currently exposes a configurable resource budget.
    pub const fn has_configurable_limits(self) -> bool {
        self.configurable_limits
    }
}

/// Runtime view of Stab's source-owned product descriptors.
///
/// The set is assembled from the closed gate table, codec registry, and compiler registrations.
/// It deliberately does not read qualification inventories or a separately maintained status
/// manifest.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct CapabilitySet {
    private: (),
}

impl CapabilitySet {
    pub const SCHEMA_VERSION: u16 = 1;
    pub const STIM_COMPATIBILITY_VERSION: &'static str = "1.16.0";

    pub const fn current() -> Self {
        Self { private: () }
    }

    pub fn dialects(self) -> impl ExactSizeIterator<Item = ModelDialect> {
        ModelDialect::all()
    }

    pub fn gates(self) -> impl ExactSizeIterator<Item = Gate> {
        Gate::all()
    }

    pub fn record_formats(self) -> impl ExactSizeIterator<Item = RecordFormat> {
        codec_capabilities().iter().map(|codec| codec.format())
    }

    pub fn codecs(self) -> impl ExactSizeIterator<Item = CodecCapability> {
        codec_capabilities().iter().copied()
    }

    pub fn compilation_operations(self) -> impl ExactSizeIterator<Item = CompilationCapability> {
        stab_engine::COMPILATION_DESCRIPTORS
            .iter()
            .copied()
            .map(|descriptor| {
                CompilationCapability::new(
                    descriptor.operation(),
                    descriptor.input_dialect(),
                    descriptor.compiler_schema_version(),
                    descriptor.request_fingerprint_schema_version(),
                    descriptor.has_configurable_limits(),
                )
            })
    }

    pub const fn default_parse_limits(self, _dialect: ModelDialect) -> ParseLimits {
        ParseLimits::new(
            ParseLimits::DEFAULT_SOURCE_LINES,
            ParseLimits::DEFAULT_REPEAT_NESTING,
        )
    }
}
