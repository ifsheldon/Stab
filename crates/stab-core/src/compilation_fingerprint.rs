use sha2::{Digest as _, Sha256};

use crate::{Circuit, ModelFingerprint};

const COMPILATION_REQUEST_FINGERPRINT_DOMAIN: &[u8] = b"stab:compilation-request-fingerprint\0";

/// Backend-neutral operation identified by a [`CompilationRequestFingerprint`].
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CompilationOperation {
    Sampling,
}

impl CompilationOperation {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Sampling => "sample",
        }
    }

    const fn discriminator(self) -> u8 {
        match self {
            Self::Sampling => 1,
        }
    }
}

/// Versioned SHA-256 identity of a backend-neutral compilation request.
///
/// Schema one binds the model fingerprint, compiler schema, operation, normalized compile
/// options, and effective configurable limits. The current public sampling compiler rejects sweep
/// controls, exposes no configurable compile limit, and has no backend-selection input.
///
/// This identity does not include shots, random seed, output encoding, or a selected execution
/// backend. It is not a compiled-plan identity.
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
    pub const SAMPLING_COMPILER_SCHEMA_VERSION: u16 = 1;
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
        hasher.update([model.dialect().fingerprint_discriminator()]);
        hasher.update(model.digest());

        // Sweep rejection is fixed compiler behavior in schema one, not a caller option.
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
