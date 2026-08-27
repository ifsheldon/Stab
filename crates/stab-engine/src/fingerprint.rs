use sha2::{Digest as _, Sha256};
use stab_model::{Circuit, ModelFingerprint};

const COMPILATION_REQUEST_FINGERPRINT_DOMAIN: &[u8] = b"stab:compilation-request-fingerprint\0";

/// Backend-neutral operation identified by a [`CompilationRequestFingerprint`].
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CompilationOperation {
    Sampling,
    MeasurementToDetection,
    DetectionSampling,
    DemSampling,
}

impl CompilationOperation {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Sampling => "sample",
            Self::MeasurementToDetection => "m2d",
            Self::DetectionSampling => "detect",
            Self::DemSampling => "sample_dem",
        }
    }

    const fn discriminator(self) -> u8 {
        match self {
            Self::Sampling => 1,
            Self::MeasurementToDetection => 2,
            Self::DetectionSampling => 3,
            Self::DemSampling => 4,
        }
    }
}

/// Versioned SHA-256 identity of a backend-neutral compilation request.
///
/// Schema one binds the model fingerprint, compiler schema, operation, normalized compile
/// options, and effective configurable limits. The current public sampling compiler treats omitted
/// sweep data as all false and exposes no configurable compile limit. The executable backend is deliberately
/// excluded because this identity describes the lowering request instead of the compiled plan.
///
/// This identity does not include shots, random seed, output encoding, or the execution backend. It
/// is not a compiled-plan identity.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct CompilationRequestFingerprint {
    schema_version: u16,
    compiler_schema_version: u16,
    operation: CompilationOperation,
    model_fingerprint: ModelFingerprint,
    digest: [u8; 32],
}

impl CompilationRequestFingerprint {
    pub const SCHEMA_VERSION: u16 = 1;
    pub const SAMPLING_COMPILER_SCHEMA_VERSION: u16 = 3;
    pub const ALGORITHM: &'static str = "sha256";

    pub fn for_sampling(circuit: &Circuit) -> Self {
        let operation = CompilationOperation::Sampling;
        let model = circuit.fingerprint();
        let mut hasher = Sha256::new();
        hasher.update(COMPILATION_REQUEST_FINGERPRINT_DOMAIN);
        hasher.update(Self::SCHEMA_VERSION.to_be_bytes());
        hasher.update([operation.discriminator()]);
        hasher.update(Self::SAMPLING_COMPILER_SCHEMA_VERSION.to_be_bytes());
        hasher.update(model.schema_version().to_be_bytes());
        hasher.update([
            stab_model::advanced::model_dialect_fingerprint_discriminator(model.dialect()),
        ]);
        hasher.update(model.digest());

        // Classical-control lowering is fixed compiler behavior, not a caller option.
        encode_len(&mut hasher, 0);

        // The current compiler has representability checks but no configurable compile limits.
        encode_len(&mut hasher, 0);

        Self {
            schema_version: Self::SCHEMA_VERSION,
            compiler_schema_version: Self::SAMPLING_COMPILER_SCHEMA_VERSION,
            operation,
            model_fingerprint: model,
            digest: hasher.finalize().into(),
        }
    }

    pub const fn schema_version(self) -> u16 {
        self.schema_version
    }

    pub const fn compiler_schema_version(self) -> u16 {
        self.compiler_schema_version
    }

    pub const fn operation(self) -> CompilationOperation {
        self.operation
    }

    pub const fn model_fingerprint(self) -> ModelFingerprint {
        self.model_fingerprint
    }

    pub const fn digest(self) -> [u8; 32] {
        self.digest
    }

    pub fn digest_hex(self) -> String {
        hex::encode(self.digest)
    }
}

fn encode_len(hasher: &mut Sha256, value: usize) {
    let bytes = value.to_be_bytes();
    let mut encoded = [0; size_of::<u128>()];
    for (output, input) in encoded.iter_mut().rev().zip(bytes.iter().rev()) {
        *output = *input;
    }
    hasher.update(encoded);
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::expect_used,
        reason = "fingerprint contract tests use direct fixture setup assertions"
    )]

    use super::*;

    const RICH_CIRCUIT: &str = "X_ERROR[π](0.12345641) 0\n\
M !1\n\
CX sweep[7] 2\n\
MPP !X0*Y1*!Z2\n\
DETECTOR[coord](-0, 1.25) rec[-0] rec[-1]\n\
REPEAT[loop] 3 {\n\
    H 3\n\
}\n";

    #[test]
    fn sampling_compilation_fingerprint_normalizes_source_spelling() {
        let canonical = Circuit::from_stim_str("R 0\nREPEAT 3 {\n    CNOT 0 1\n    M 0 1\n}\n")
            .expect("canonical request circuit");
        let alternate = Circuit::from_stim_str("r 0\r\nrepeat 3 {\r\ncx 0 1\r\nm 0 1\r\n}\r\n")
            .expect("alternate request circuit");
        let changed = Circuit::from_stim_str("R 0\nREPEAT 4 {\nCX 0 1\nM 0 1\n}\n")
            .expect("changed request circuit");

        let fingerprint = CompilationRequestFingerprint::for_sampling(&canonical);
        assert_eq!(
            fingerprint,
            CompilationRequestFingerprint::for_sampling(&alternate)
        );
        assert_ne!(
            fingerprint,
            CompilationRequestFingerprint::for_sampling(&changed)
        );
        assert_eq!(fingerprint.schema_version(), 1);
        assert_eq!(fingerprint.compiler_schema_version(), 3);
        assert_eq!(fingerprint.operation(), CompilationOperation::Sampling);
        assert_eq!(fingerprint.operation().as_str(), "sample");
        assert_eq!(fingerprint.model_fingerprint(), canonical.fingerprint());
        assert_eq!(fingerprint.digest().len(), 32);
        assert_eq!(fingerprint.digest_hex().len(), 64);

        let frozen = CompilationRequestFingerprint::for_sampling(
            &Circuit::from_stim_str(RICH_CIRCUIT).expect("frozen request circuit"),
        );
        assert_eq!(
            frozen.digest_hex(),
            "156cd4ed97e9f1da74a8d13d7e39d39731a90844da22316c614c379b8e0cce3d"
        );
    }

    #[test]
    fn sampling_compilation_fingerprint_does_not_allocate() {
        let circuit = Circuit::from_stim_str("R 0\nREPEAT 3 {\n    CNOT 0 1\n    M 0 1\n}\n")
            .expect("request circuit");

        let allocations = allocation_counter::measure(|| {
            std::hint::black_box(CompilationRequestFingerprint::for_sampling(&circuit));
        });
        assert_eq!(allocations.count_total, 0, "{allocations:?}");
        assert_eq!(allocations.bytes_total, 0, "{allocations:?}");
    }
}
