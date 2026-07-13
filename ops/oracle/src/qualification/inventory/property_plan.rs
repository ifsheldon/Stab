use std::path::PathBuf;

use sha2::{Digest as _, Sha256};

use super::{InventoryError, MAX_SOURCE_BYTES};
use crate::RepoRoot;
use crate::qualification::model::{
    CaseId, Comparator, EvidenceState, EvidenceStatus, PropertyExecutionMode,
    PropertyExecutionPlan, PropertyPersistencePolicy, PropertyPlanRef, PropertyPlanSource,
    RelativeSourcePath, SemanticDigest,
};

pub(super) fn planned_reference(
    comparator: Comparator,
    case_id: &CaseId,
) -> Option<PropertyPlanRef> {
    (comparator == Comparator::Property).then(|| PropertyPlanRef {
        state: EvidenceState::Planned,
        source: PropertyPlanSource::QualificationCase,
        id: case_id.to_string(),
        plan: None,
    })
}

pub(super) fn oracle_reference(
    root: &RepoRoot,
    comparator: Comparator,
    status: EvidenceStatus,
    source_id: &str,
    case_id: &CaseId,
) -> Result<Option<PropertyPlanRef>, InventoryError> {
    if comparator != Comparator::Property {
        return Ok(None);
    }
    if status == EvidenceStatus::Planned {
        return Ok(planned_reference(comparator, case_id));
    }
    let (corpus_path, generator_domain) = match source_id {
        "coverage-mem-bit-ref" => (
            "crates/stab-core/tests/bits.rs",
            "bit-vector tail and storage boundary corpus",
        ),
        "coverage-mem-simd-util" => (
            "crates/stab-core/tests/bits.rs",
            "scalar bit-utility boundary corpus",
        ),
        "coverage-mem-simd-word" => (
            "crates/stab-core/tests/bits.rs",
            "SIMD word scalar-reference corpus",
        ),
        "coverage-util-bot-twiddle" => (
            "crates/stab-core/tests/bits.rs",
            "twiddle helper boundary and upstream example corpus",
        ),
        "coverage-stabilizers-pauli-string-ref" => (
            "crates/stab-core/tests/stabilizers.rs",
            "Pauli reference weight and active-term corpus",
        ),
        _ => {
            return Err(InventoryError::MissingOraclePropertyPlan {
                id: source_id.to_string(),
            });
        }
    };
    let path = RelativeSourcePath::try_new(PathBuf::from(corpus_path))
        .map_err(|_| InventoryError::InvalidSourcePath(corpus_path.to_string()))?;
    let bytes =
        crate::safe_file::read_regular_file_bounded(&root.path.join(corpus_path), MAX_SOURCE_BYTES)
            .map_err(|source| InventoryError::Read {
                path: root.path.join(corpus_path),
                reason: source.to_string().into_boxed_str(),
            })?;
    Ok(Some(PropertyPlanRef {
        state: EvidenceState::Existing,
        source: PropertyPlanSource::OracleFixture,
        id: source_id.to_string(),
        plan: Some(PropertyExecutionPlan {
            generator_domain: generator_domain.to_string(),
            maximum_generated_bytes: 0,
            seeds: Vec::new(),
            case_count: 1,
            corpus_path: Some(path),
            corpus_sha256: Some(SemanticDigest::from_bytes(Sha256::digest(bytes).into())),
            persistence_policy: PropertyPersistencePolicy::ExistingFocusedRegression,
            execution_mode: PropertyExecutionMode::CargoSubprocess,
        }),
    }))
}
