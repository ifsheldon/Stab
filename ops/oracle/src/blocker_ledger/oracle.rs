//! Validation of primary and supporting oracle evidence signatures.

use std::collections::BTreeMap;
use std::io::Read;
use std::path::Component;

use sha2::{Digest, Sha256};

use super::{
    BlockerCase, CaseStatus, ComparatorKind, EvidenceState, FixtureId, FixtureRelativeEvidencePath,
    MAX_TRACKED_PATH_BYTES, OracleEvidenceClass, OracleEvidenceSignature, OracleManifestComparator,
    OracleManifestParityMode, OracleManifestRow, OracleManifestStatus, OracleRunner, RepoRoot,
    Sha256Hex, digest_hex, open_regular_file, validate_display_text, validate_identifier,
};

pub(super) fn validate_oracle_reference(
    root: &RepoRoot,
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
            Some(row) => validate_existing_oracle(root, case, row, violations),
            None => violations.push(format!(
                "case {:?} references missing oracle row {:?}",
                case.id,
                case.oracle.value.as_str()
            )),
        }
    }
}

fn validate_existing_oracle(
    root: &RepoRoot,
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
            && (case.oracle.classification == OracleEvidenceClass::RustTestProxy
                || oracle_comparator_matches(case.comparator, row));
    let signature_matches = case.oracle.signature.as_ref().is_some_and(|signature| {
        oracle_signature_matches(signature, row)
            && oracle_evidence_binding_matches(
                root,
                case.oracle.classification,
                signature,
                row,
                violations,
            )
    });
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
    for (label, path) in [
        ("oracle stdin path", signature.stdin_path.as_ref()),
        (
            "oracle expected stdout path",
            signature.expected_stdout_path.as_ref(),
        ),
    ] {
        if let Some(path) = path {
            validate_repo_relative_evidence_path(label, path, violations);
        }
    }
    for (label, digest) in [
        ("oracle stdin SHA-256", signature.stdin_sha256.as_ref()),
        (
            "oracle expected stdout SHA-256",
            signature.expected_stdout_sha256.as_ref(),
        ),
    ] {
        if let Some(digest) = digest
            && (digest.0.len() != 64
                || !digest
                    .0
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)))
        {
            violations.push(format!(
                "{label} {:?} is not lowercase hexadecimal",
                digest.0
            ));
        }
    }
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

fn oracle_evidence_binding_matches(
    root: &RepoRoot,
    classification: OracleEvidenceClass,
    signature: &OracleEvidenceSignature,
    row: &OracleManifestRow,
    violations: &mut Vec<String>,
) -> bool {
    let bindings = (
        signature.stdin_path.as_ref(),
        signature.expected_stdout_path.as_ref(),
        signature.stdin_sha256.as_ref(),
        signature.expected_stdout_sha256.as_ref(),
    );
    if classification != OracleEvidenceClass::PinnedGolden {
        return matches!(bindings, (None, None, None, None));
    }
    let (Some(stdin_path), Some(stdout_path), Some(stdin_digest), Some(stdout_digest)) = bindings
    else {
        violations
            .push("pinned-golden oracle signature lacks path and SHA-256 bindings".to_string());
        return false;
    };
    if row.stdin_path.as_ref() != Some(stdin_path)
        || row.expected_stdout_path.as_ref() != Some(stdout_path)
    {
        violations.push(format!(
            "pinned-golden oracle paths {:?} and {:?} do not match manifest paths {:?} and {:?}",
            stdin_path.0,
            stdout_path.0,
            row.stdin_path.as_ref().map(|path| &path.0),
            row.expected_stdout_path.as_ref().map(|path| &path.0)
        ));
        return false;
    }
    evidence_file_sha256(root, "pinned-golden stdin", stdin_path, violations)
        .is_some_and(|actual| actual == *stdin_digest)
        && evidence_file_sha256(root, "pinned-golden stdout", stdout_path, violations)
            .is_some_and(|actual| actual == *stdout_digest)
}

fn validate_repo_relative_evidence_path(
    label: &str,
    path: &FixtureRelativeEvidencePath,
    violations: &mut Vec<String>,
) {
    let valid = !path.0.as_os_str().is_empty()
        && !path.0.is_absolute()
        && path
            .0
            .components()
            .all(|component| matches!(component, Component::Normal(_)));
    if !valid {
        violations.push(format!(
            "{label} {:?} is not a safe repository-relative path",
            path.0
        ));
    }
}

fn evidence_file_sha256(
    root: &RepoRoot,
    label: &str,
    path: &FixtureRelativeEvidencePath,
    violations: &mut Vec<String>,
) -> Option<Sha256Hex> {
    let mut local_violations = Vec::new();
    validate_repo_relative_evidence_path(label, path, &mut local_violations);
    if !local_violations.is_empty() {
        violations.extend(local_violations);
        return None;
    }
    let absolute = root.path.join("oracle").join("fixtures").join(&path.0);
    let mut file = match open_regular_file(&absolute) {
        Ok(file) => file,
        Err(error) => {
            violations.push(format!(
                "{label} {:?} is not stable evidence: {error}",
                path.0
            ));
            return None;
        }
    };
    let mut bytes = Vec::new();
    if let Err(source) = file
        .by_ref()
        .take(u64::try_from(MAX_TRACKED_PATH_BYTES).unwrap_or(u64::MAX) + 1)
        .read_to_end(&mut bytes)
    {
        violations.push(format!("failed to read {label} {:?}: {source}", path.0));
        return None;
    }
    if bytes.len() > MAX_TRACKED_PATH_BYTES {
        violations.push(format!(
            "{label} {:?} exceeds the {MAX_TRACKED_PATH_BYTES}-byte evidence limit",
            path.0
        ));
        return None;
    }
    Some(Sha256Hex(digest_hex(&Sha256::digest(bytes))))
}

pub(super) fn oracle_class_matches_runner(
    classification: OracleEvidenceClass,
    runner: OracleRunner,
) -> bool {
    matches!(
        (classification, runner),
        (OracleEvidenceClass::Direct, OracleRunner::StimCli)
            | (OracleEvidenceClass::PinnedGolden, OracleRunner::CoreFixture)
            | (OracleEvidenceClass::RustTestProxy, OracleRunner::CargoTest)
    )
}

pub(super) fn oracle_comparator_matches(
    comparator: ComparatorKind,
    row: &OracleManifestRow,
) -> bool {
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
