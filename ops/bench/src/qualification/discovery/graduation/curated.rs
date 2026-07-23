use super::{
    BIT_MATRIX_TRANSPOSE_ALLOCATING_GROUP_ID, BIT_MATRIX_TRANSPOSE_IN_PLACE_GROUP_ID,
    CLIFFORD_STRING_IDENTITY_GROUP_ID, CLIFFORD_STRING_NON_IDENTITY_GROUP_ID, apply,
};
use crate::error::BenchError;
use crate::qualification::model::{PerformanceDisposition, QualificationGroup, RowOrigin};
use crate::root::RepoRoot;

pub(in crate::qualification::discovery) fn groups(
    root: &RepoRoot,
    existing: &[QualificationGroup],
) -> Result<Vec<QualificationGroup>, BenchError> {
    let bit_matrix_source = existing
        .iter()
        .find(|group| group.id == "PERFQ-M5-SIMD-BIT-TABLE")
        .ok_or_else(|| {
            BenchError::Qualification(
                "curated transpose groups require the inherited bit-matrix workload".to_string(),
            )
        })?;
    let clifford_source = existing
        .iter()
        .find(|group| group.id == CLIFFORD_STRING_IDENTITY_GROUP_ID)
        .ok_or_else(|| {
            BenchError::Qualification(
                "curated non-identity Clifford group requires the identity workload".to_string(),
            )
        })?;
    [
        (
            bit_matrix_source,
            BIT_MATRIX_TRANSPOSE_IN_PLACE_GROUP_ID,
            "perfq-m5-bit-matrix-transpose-in-place",
        ),
        (
            bit_matrix_source,
            BIT_MATRIX_TRANSPOSE_ALLOCATING_GROUP_ID,
            "perfq-m5-bit-matrix-transpose-allocating",
        ),
        (
            clifford_source,
            CLIFFORD_STRING_NON_IDENTITY_GROUP_ID,
            "perfq-m6-clifford-string-non-identity",
        ),
    ]
    .into_iter()
    .map(|(source, id, manifest_row)| {
        let mut group = source.clone();
        group.id = id.to_string();
        group.manifest_row = manifest_row.to_string();
        group.row_origin = RowOrigin::Planned;
        group.disposition = PerformanceDisposition::Measured;
        group.public_api_items.clear();
        apply(root, &mut group)?;
        Ok(group)
    })
    .collect()
}
