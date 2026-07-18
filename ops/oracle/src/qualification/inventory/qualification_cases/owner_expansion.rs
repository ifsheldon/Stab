use super::{UpstreamOwnerSpec, UpstreamWordSizeFamilySpec, validate_text};
use crate::qualification::inventory::InventoryError;

pub(super) const MAX_OWNERS_PER_CASE: usize = 2_048;

#[derive(Clone, Copy)]
pub(super) enum OwnerEntryKind {
    Case,
    Mapping,
}

impl OwnerEntryKind {
    const fn label(self) -> &'static str {
        match self {
            Self::Case => "case",
            Self::Mapping => "mapping",
        }
    }
}

pub(super) fn expand_upstream_owners(
    entry_kind: OwnerEntryKind,
    mapping_id: &str,
    owners: &[UpstreamOwnerSpec],
    families: &[UpstreamWordSizeFamilySpec],
    non_upstream_owner_count: usize,
) -> Result<Vec<UpstreamOwnerSpec>, InventoryError> {
    let total_owner_count = validate_owner_plan(
        entry_kind,
        mapping_id,
        owners.len(),
        families,
        non_upstream_owner_count,
    )?;
    let upstream_owner_count = total_owner_count
        .checked_sub(non_upstream_owner_count)
        .ok_or_else(|| invalid_owner_count(entry_kind, mapping_id, "underflowed"))?;
    let mut expanded = Vec::with_capacity(upstream_owner_count);
    for owner in owners {
        expanded.push(UpstreamOwnerSpec {
            path: owner.path.clone(),
            symbol: owner.symbol.clone(),
            subcase: owner.subcase.clone(),
        });
    }
    for family in families {
        for word_size in &family.word_sizes {
            expanded.push(UpstreamOwnerSpec {
                path: family.path.clone(),
                symbol: format!("{}_{}", family.symbol_base, word_size),
                subcase: Some(format!("W={word_size}")),
            });
        }
    }
    Ok(expanded)
}

fn validate_owner_plan(
    entry_kind: OwnerEntryKind,
    mapping_id: &str,
    explicit_owner_count: usize,
    families: &[UpstreamWordSizeFamilySpec],
    non_upstream_owner_count: usize,
) -> Result<usize, InventoryError> {
    let mut total_owner_count = explicit_owner_count
        .checked_add(non_upstream_owner_count)
        .ok_or_else(|| invalid_owner_count(entry_kind, mapping_id, "overflowed"))?;
    enforce_owner_limit(entry_kind, mapping_id, total_owner_count)?;
    for family in families {
        validate_text("upstream word-size symbol base", &family.symbol_base)?;
        if family.word_sizes.is_empty() {
            return Err(InventoryError::InvalidQualificationCases(format!(
                "qualification {} {:?} has an empty upstream word-size family for {}:{}",
                entry_kind.label(),
                mapping_id,
                family.path,
                family.symbol_base
            )));
        }
        total_owner_count = total_owner_count
            .checked_add(family.word_sizes.len())
            .ok_or_else(|| invalid_owner_count(entry_kind, mapping_id, "overflowed"))?;
        enforce_owner_limit(entry_kind, mapping_id, total_owner_count)?;

        let mut seen_sizes = 0_u8;
        for word_size in &family.word_sizes {
            let bit = match word_size {
                64 => 1,
                128 => 2,
                256 => 4,
                _ => 0,
            };
            if bit == 0 || seen_sizes & bit != 0 {
                return Err(InventoryError::InvalidQualificationCases(format!(
                    "qualification {} {:?} has invalid or duplicate Stim word size {} for {}:{}",
                    entry_kind.label(),
                    mapping_id,
                    word_size,
                    family.path,
                    family.symbol_base
                )));
            }
            seen_sizes |= bit;
        }
    }
    Ok(total_owner_count)
}

fn enforce_owner_limit(
    entry_kind: OwnerEntryKind,
    mapping_id: &str,
    owner_count: usize,
) -> Result<(), InventoryError> {
    if owner_count > MAX_OWNERS_PER_CASE {
        return Err(InventoryError::InvalidQualificationCases(format!(
            "qualification {} {:?} has {} owners; limit is {}",
            entry_kind.label(),
            mapping_id,
            owner_count,
            MAX_OWNERS_PER_CASE
        )));
    }
    Ok(())
}

fn invalid_owner_count(
    entry_kind: OwnerEntryKind,
    mapping_id: &str,
    state: &str,
) -> InventoryError {
    InventoryError::InvalidQualificationCases(format!(
        "qualification {} {:?} owner count {state}",
        entry_kind.label(),
        mapping_id
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::qualification::model::RelativeSourcePath;

    #[test]
    fn word_size_families_expand_to_exact_parameterized_owners() {
        let path =
            RelativeSourcePath::try_new("src/stim/simulators/frame_simulator.test.cc".into())
                .expect("path");
        let families = vec![UpstreamWordSizeFamilySpec {
            path: path.clone(),
            symbol_base: "FrameSimulator.noisy_measurement_x".to_string(),
            word_sizes: vec![64, 128, 256],
        }];
        let expanded = expand_upstream_owners(
            OwnerEntryKind::Case,
            "cq2-word-size-family",
            &[],
            &families,
            0,
        )
        .expect("expand");
        assert_eq!(expanded.len(), 3);
        let first = expanded.first().expect("first expanded owner");
        let third = expanded.last().expect("last expanded owner");
        assert_eq!(first.path, path);
        assert_eq!(first.symbol, "FrameSimulator.noisy_measurement_x_64");
        assert_eq!(first.subcase.as_deref(), Some("W=64"));
        assert_eq!(third.symbol, "FrameSimulator.noisy_measurement_x_256");
        assert_eq!(third.subcase.as_deref(), Some("W=256"));

        let duplicate = vec![UpstreamWordSizeFamilySpec {
            path: first.path.clone(),
            symbol_base: "FrameSimulator.noisy_measurement_x".to_string(),
            word_sizes: vec![64, 64],
        }];
        assert!(
            expand_upstream_owners(
                OwnerEntryKind::Case,
                "cq2-word-size-family",
                &[],
                &duplicate,
                0,
            )
            .is_err()
        );
    }

    #[test]
    fn oversized_word_size_family_is_rejected_before_expansion() {
        let family = UpstreamWordSizeFamilySpec {
            path: RelativeSourcePath::try_new("src/stim/simulators/frame_simulator.test.cc".into())
                .expect("path"),
            symbol_base: "FrameSimulator.oversized".to_string(),
            word_sizes: vec![64; MAX_OWNERS_PER_CASE + 1],
        };

        assert!(matches!(
            expand_upstream_owners(
                OwnerEntryKind::Case,
                "cq2-oversized-family",
                &[],
                &[family],
                0,
            ),
            Err(InventoryError::InvalidQualificationCases(message))
                if message.contains("has 2049 owners")
        ));
    }
}
