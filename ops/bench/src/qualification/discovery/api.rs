use super::CorrectnessApi;
use crate::qualification::model::{ApiDisposition, PerformanceDisposition};

pub(super) const BIT_MATRIX_TRANSPOSE_ALLOCATING_GROUP_ID: &str =
    "PERFQ-M5-BIT-MATRIX-TRANSPOSE-ALLOCATING";
pub(super) const BIT_MATRIX_TRANSPOSE_IN_PLACE_GROUP_ID: &str =
    "PERFQ-M5-BIT-MATRIX-TRANSPOSE-IN-PLACE";
pub(super) const PAULI_STRING_MULTIPLY_GROUP_ID: &str = "PERFQ-M6-PAULI-STRING";
pub(super) const PAULI_STRING_ITER_GROUP_ID: &str = "PERFQ-M6-PAULI-ITER";
pub(super) const CLIFFORD_STRING_NON_IDENTITY_GROUP_ID: &str =
    "PERFQ-M6-CLIFFORD-STRING-NON-IDENTITY";

pub(super) fn make_disposition(item: &CorrectnessApi) -> ApiDisposition {
    let performance_feature = item
        .performance_groups
        .first()
        .cloned()
        .unwrap_or_else(|| "PERF-RESOURCE-BOUNDARIES".to_string());
    let behavioral = is_behavioral(item);
    let supporting_performance_features = item.performance_groups.iter().skip(1).cloned().collect();
    let mut parent_group_ids = if behavioral {
        item.performance_groups
            .iter()
            .filter_map(|feature| qualification_group_id(item, feature))
            .collect()
    } else {
        Vec::new()
    };
    let has_complete_measured_parent =
        behavioral && parent_group_ids.len() == item.performance_groups.len();
    if !has_complete_measured_parent {
        parent_group_ids.clear();
    }
    ApiDisposition {
        id: item.id.clone(),
        path: item.path.clone(),
        kind: item.kind.clone(),
        performance_feature,
        supporting_performance_features,
        correctness_case_id: item.owner_case_id.clone(),
        disposition: if has_complete_measured_parent {
            PerformanceDisposition::CoveredByParent
        } else if behavioral {
            PerformanceDisposition::FutureCandidate
        } else {
            PerformanceDisposition::NotPerformanceRelevant
        },
        parent_group_ids,
        reason: if has_complete_measured_parent {
            "Behavioral operation is covered by the listed executable release workload.".to_string()
        } else if behavioral {
            "Behavioral operation remains visible as a future workload candidate without creating a speculative benchmark product."
                .to_string()
        } else {
            "Declaration-only, derived, marker, or diagnostic shape has no independent runtime workload."
                .to_string()
        },
    }
}

pub(super) fn is_behavioral(item: &CorrectnessApi) -> bool {
    matches!(item.kind.as_str(), "function" | "method")
        || item.kind == "trait-impl" && behavioral_trait_impl(&item.path)
}

fn qualification_group_id(item: &CorrectnessApi, performance_feature: &str) -> Option<String> {
    if performance_feature == "PERF-BIT-KERNELS" {
        match item.path.as_str() {
            "stab_core::BitMatrix::transpose" | "stab_core::bits::BitMatrix::transpose" => {
                return Some(BIT_MATRIX_TRANSPOSE_ALLOCATING_GROUP_ID.to_string());
            }
            "stab_core::BitMatrix::transpose_square_in_place"
            | "stab_core::bits::BitMatrix::transpose_square_in_place" => {
                return Some(BIT_MATRIX_TRANSPOSE_IN_PLACE_GROUP_ID.to_string());
            }
            _ => {}
        }
    }
    if performance_feature == "PERF-STABILIZER-ALGEBRA"
        && matches!(
            item.path.as_str(),
            "stab_core::CliffordString::right_multiply_in_place"
                | "stab_core::stabilizers::CliffordString::right_multiply_in_place"
        )
    {
        return Some(CLIFFORD_STRING_NON_IDENTITY_GROUP_ID.to_string());
    }
    if performance_feature == "PERF-STABILIZER-ALGEBRA"
        && matches!(
            item.path.as_str(),
            "stab_core::PauliString::right_multiply_in_place_returning_log_i_scalar"
                | "stab_core::stabilizers::PauliString::right_multiply_in_place_returning_log_i_scalar"
        )
    {
        return Some(PAULI_STRING_MULTIPLY_GROUP_ID.to_string());
    }
    if performance_feature == "PERF-STABILIZER-ALGEBRA"
        && matches!(
            item.path.as_str(),
            "stab_core::PauliStringIterator::new"
                | "stab_core::PauliStringIterator::iter_next"
                | "stab_core::PauliStringIterator::result"
                | "stab_core::stabilizers::PauliStringIterator::new"
                | "stab_core::stabilizers::PauliStringIterator::iter_next"
                | "stab_core::stabilizers::PauliStringIterator::result"
        )
    {
        return Some(PAULI_STRING_ITER_GROUP_ID.to_string());
    }
    None
}

fn behavioral_trait_impl(path: &str) -> bool {
    let Some((_, rest)) = path.split_once(" as ") else {
        return false;
    };
    let trait_name = rest
        .split_once(" for@")
        .or_else(|| rest.split_once(" for "))
        .map_or(rest, |(name, _)| name)
        .split('@')
        .next()
        .unwrap_or(rest);
    matches!(
        trait_name,
        "Display" | "From" | "FromStr" | "Iterator" | "TryFrom"
    )
}
