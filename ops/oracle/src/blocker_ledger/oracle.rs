//! Validation of primary and supporting oracle evidence signatures.

use std::collections::BTreeMap;

use super::{
    BlockerCase, CaseStatus, ComparatorKind, EvidenceState, FixtureId, OracleEvidenceClass,
    OracleEvidenceSignature, OracleManifestComparator, OracleManifestParityMode, OracleManifestRow,
    OracleManifestStatus, OracleRunner, validate_display_text, validate_identifier,
};

pub(super) fn validate_oracle_reference(
    case: &BlockerCase,
    oracle_rows: &BTreeMap<FixtureId, OracleManifestRow>,
    violations: &mut Vec<String>,
) {
    validate_identifier("oracle", case.oracle.value.as_str(), violations);
    let expected_state = if case.status == CaseStatus::Planned {
        EvidenceState::Planned
    } else {
        EvidenceState::Existing
    };
    if case.oracle.state != expected_state {
        violations.push(format!(
            "case {:?} status {} requires oracle state {:?}",
            case.id,
            case.status.as_str(),
            expected_state
        ));
    }
    let expected_classification = if case.oracle.state == EvidenceState::Planned {
        OracleEvidenceClass::Planned
    } else {
        case.oracle.classification
    };
    if case.oracle.classification != expected_classification
        || (case.oracle.state == EvidenceState::Existing
            && case.oracle.classification == OracleEvidenceClass::Planned)
    {
        violations.push(format!(
            "case {:?} oracle state {:?} is incompatible with classification {:?}",
            case.id, case.oracle.state, case.oracle.classification
        ));
    }
    match (case.oracle.state, &case.oracle.signature) {
        (EvidenceState::Existing, Some(signature)) => {
            validate_oracle_signature(signature, violations);
        }
        (EvidenceState::Existing, None) => violations.push(format!(
            "case {:?} existing oracle reference lacks a frozen evidence signature",
            case.id
        )),
        (EvidenceState::Planned, Some(_)) => violations.push(format!(
            "case {:?} planned oracle reference cannot claim an existing evidence signature",
            case.id
        )),
        _ => {}
    }
    if case.oracle.state == EvidenceState::Existing {
        match oracle_rows.get(&case.oracle.value) {
            Some(row) => validate_existing_oracle(case, row, violations),
            None => violations.push(format!(
                "case {:?} references missing oracle row {:?}",
                case.id,
                case.oracle.value.as_str()
            )),
        }
    }
}

fn validate_existing_oracle(
    case: &BlockerCase,
    row: &OracleManifestRow,
    violations: &mut Vec<String>,
) {
    if row.status != OracleManifestStatus::Implemented {
        violations.push(format!(
            "case {:?} oracle row {:?} is not implemented",
            case.id,
            case.oracle.value.as_str()
        ));
    }
    let classification_matches =
        oracle_class_matches_runner(case.oracle.classification, row.command.runner)
            && (case.oracle.classification != OracleEvidenceClass::Direct
                || oracle_comparator_matches(case.comparator, row));
    let signature_matches = case
        .oracle
        .signature
        .as_ref()
        .is_some_and(|signature| oracle_signature_matches(signature, row));
    if !classification_matches || !signature_matches {
        violations.push(format!(
            "case {:?} comparator {:?}, classification {:?}, or evidence signature is incompatible with oracle row {:?} ({:?}/{:?}, runner {:?}, argv {:?}, upstream {:?})",
            case.id,
            case.comparator,
            case.oracle.classification,
            case.oracle.value.as_str(),
            row.parity_mode,
            row.comparator,
            row.command.runner,
            row.command.argv,
            row.upstream_source.0
        ));
    }
}

pub(super) fn validate_oracle_signature(
    signature: &OracleEvidenceSignature,
    violations: &mut Vec<String>,
) {
    validate_display_text("oracle argv", &signature.argv, violations);
    validate_display_text(
        "oracle upstream source",
        &signature.upstream_source.0.to_string_lossy(),
        violations,
    );
}

pub(super) fn oracle_signature_matches(
    signature: &OracleEvidenceSignature,
    row: &OracleManifestRow,
) -> bool {
    signature.parity_mode == row.parity_mode
        && signature.comparator == row.comparator
        && signature.argv == row.command.argv
        && signature.upstream_source == row.upstream_source
}

pub(super) fn oracle_class_matches_runner(
    classification: OracleEvidenceClass,
    runner: OracleRunner,
) -> bool {
    matches!(
        (classification, runner),
        (OracleEvidenceClass::Direct, OracleRunner::StimCli)
            | (OracleEvidenceClass::RustTestProxy, OracleRunner::CargoTest)
    )
}

fn oracle_comparator_matches(comparator: ComparatorKind, row: &OracleManifestRow) -> bool {
    match comparator {
        ComparatorKind::Exact => {
            row.comparator == OracleManifestComparator::ExactOutput
                && matches!(
                    row.parity_mode,
                    OracleManifestParityMode::ExactOutput
                        | OracleManifestParityMode::ExactOutputAndStatistical
                )
        }
        ComparatorKind::Structural | ComparatorKind::StateEquivalence => {
            row.comparator == OracleManifestComparator::Structural
                && row.parity_mode == OracleManifestParityMode::Structural
        }
        ComparatorKind::Statistical => {
            row.comparator == OracleManifestComparator::Statistical
                && matches!(
                    row.parity_mode,
                    OracleManifestParityMode::Statistical
                        | OracleManifestParityMode::ExactOutputAndStatistical
                )
        }
        ComparatorKind::ErrorClass => matches!(
            row.comparator,
            OracleManifestComparator::ExactOutput | OracleManifestComparator::Structural
        ),
        ComparatorKind::SemanticInvariant => matches!(
            (row.parity_mode, row.comparator),
            (
                OracleManifestParityMode::Property,
                OracleManifestComparator::Property
            ) | (
                OracleManifestParityMode::Structural,
                OracleManifestComparator::Structural
            )
        ),
    }
}
