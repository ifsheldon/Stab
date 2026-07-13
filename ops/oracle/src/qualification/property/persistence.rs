use std::path::{Path, PathBuf};

use thiserror::Error;

use super::{
    FailureReason, MAX_FAILURE_REASON_BYTES, MAX_PERSISTENCE_BYTES, MinimizedPropertyFailure,
    PropertyCase, PropertyCaseIndex, PropertySeed,
};

const MAGIC: &str = "STAB-CQ1-PROPERTY-1";
const PAYLOAD_MARKER: &[u8] = b"\npayload-follows\n\n";

#[derive(Debug, Error)]
pub(crate) enum PersistenceError {
    #[error("property persistence exceeds the {maximum}-byte limit: {actual} bytes")]
    TooLarge { actual: usize, maximum: usize },

    #[error("property persistence metadata is malformed: {0}")]
    Malformed(&'static str),

    #[error("property persistence field {field:?} is invalid: {value:?}")]
    InvalidField { field: &'static str, value: String },

    #[error("property persistence reason is not valid UTF-8")]
    InvalidReasonUtf8,

    #[error("property persistence generated seed disagrees with seed and case index")]
    GeneratedSeedMismatch,

    #[error("failed to read property persistence {path}: {reason}")]
    Read { path: PathBuf, reason: Box<str> },

    #[error("failed to write property persistence {path}: {reason}")]
    Write { path: PathBuf, reason: Box<str> },
}

pub(crate) fn parse_persistence(
    bytes: &[u8],
) -> Result<MinimizedPropertyFailure, PersistenceError> {
    if bytes.len() > MAX_PERSISTENCE_BYTES {
        return Err(PersistenceError::TooLarge {
            actual: bytes.len(),
            maximum: MAX_PERSISTENCE_BYTES,
        });
    }
    let marker = bytes
        .windows(PAYLOAD_MARKER.len())
        .position(|window| window == PAYLOAD_MARKER)
        .ok_or(PersistenceError::Malformed("missing payload marker"))?;
    let header = std::str::from_utf8(
        bytes
            .get(..marker)
            .ok_or(PersistenceError::Malformed("header boundary is invalid"))?,
    )
    .map_err(|_| PersistenceError::Malformed("metadata is not valid UTF-8"))?;
    let payload_start = marker
        .checked_add(PAYLOAD_MARKER.len())
        .ok_or(PersistenceError::Malformed("payload boundary overflow"))?;
    let payload = bytes
        .get(payload_start..)
        .ok_or(PersistenceError::Malformed("payload boundary is invalid"))?;
    let mut lines = header.lines();
    if lines.next() != Some(MAGIC) {
        return Err(PersistenceError::Malformed("wrong magic"));
    }
    let seed = parse_hex_u64(field(&mut lines, "seed")?, "seed")?;
    let case_index = parse_u32(field(&mut lines, "case-index")?, "case-index")?;
    let generated_seed = parse_hex_u64(field(&mut lines, "generated-seed")?, "generated-seed")?;
    let original_length = parse_usize(field(&mut lines, "original-bytes")?, "original-bytes")?;
    let minimized_length = parse_usize(field(&mut lines, "minimized-bytes")?, "minimized-bytes")?;
    let reason_truncated = parse_bool(field(&mut lines, "reason-truncated")?, "reason-truncated")?;
    let reason = decode_reason(field(&mut lines, "reason-hex")?)?;
    if lines.next().is_some() {
        return Err(PersistenceError::Malformed("unexpected metadata field"));
    }
    if minimized_length != payload.len() {
        return Err(PersistenceError::Malformed(
            "minimized byte count disagrees with payload",
        ));
    }
    if original_length < minimized_length {
        return Err(PersistenceError::Malformed(
            "original byte count is smaller than minimized payload",
        ));
    }
    let case = PropertyCase::new(PropertySeed::new(seed), PropertyCaseIndex(case_index));
    if case.generated_seed().get() != generated_seed {
        return Err(PersistenceError::GeneratedSeedMismatch);
    }
    Ok(MinimizedPropertyFailure {
        case,
        original_length,
        reason: FailureReason {
            text: reason.into_boxed_str(),
            truncated: reason_truncated,
        },
        minimized_input: payload.to_vec(),
    })
}

pub(crate) fn read_persistence(path: &Path) -> Result<MinimizedPropertyFailure, PersistenceError> {
    let bytes = crate::safe_file::read_regular_file_bounded(path, MAX_PERSISTENCE_BYTES).map_err(
        |source| PersistenceError::Read {
            path: path.to_path_buf(),
            reason: source.to_string().into_boxed_str(),
        },
    )?;
    parse_persistence(&bytes)
}

pub(crate) fn write_persistence(
    path: &Path,
    failure: &MinimizedPropertyFailure,
) -> Result<(), PersistenceError> {
    crate::safe_file::atomic_write_regular_file(path, failure.render_persistence().as_bytes())
        .map_err(|source| PersistenceError::Write {
            path: path.to_path_buf(),
            reason: source.to_string().into_boxed_str(),
        })
}

fn field<'a>(
    lines: &mut impl Iterator<Item = &'a str>,
    name: &'static str,
) -> Result<&'a str, PersistenceError> {
    lines
        .next()
        .and_then(|line| line.strip_prefix(name))
        .and_then(|value| value.strip_prefix('='))
        .ok_or(PersistenceError::Malformed(
            "missing or reordered metadata field",
        ))
}

fn parse_hex_u64(value: &str, field: &'static str) -> Result<u64, PersistenceError> {
    if value.len() != 16 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(invalid_field(field, value));
    }
    u64::from_str_radix(value, 16).map_err(|_| invalid_field(field, value))
}

fn parse_u32(value: &str, field: &'static str) -> Result<u32, PersistenceError> {
    value.parse().map_err(|_| invalid_field(field, value))
}

fn parse_usize(value: &str, field: &'static str) -> Result<usize, PersistenceError> {
    value.parse().map_err(|_| invalid_field(field, value))
}

fn parse_bool(value: &str, field: &'static str) -> Result<bool, PersistenceError> {
    match value {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(invalid_field(field, value)),
    }
}

fn decode_reason(value: &str) -> Result<String, PersistenceError> {
    if value.len() > 2 * MAX_FAILURE_REASON_BYTES
        || !value.len().is_multiple_of(2)
        || !value.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(invalid_field("reason-hex", value));
    }
    let mut bytes = Vec::with_capacity(value.len() / 2);
    for pair in value.as_bytes().chunks_exact(2) {
        let text = std::str::from_utf8(pair).map_err(|_| invalid_field("reason-hex", value))?;
        bytes.push(u8::from_str_radix(text, 16).map_err(|_| invalid_field("reason-hex", value))?);
    }
    String::from_utf8(bytes).map_err(|_| PersistenceError::InvalidReasonUtf8)
}

fn invalid_field(field: &'static str, value: &str) -> PersistenceError {
    PersistenceError::InvalidField {
        field,
        value: value.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::qualification::property::{PropertyPlan, PropertyRunError};

    fn minimized_failure() -> MinimizedPropertyFailure {
        let plan = PropertyPlan::try_new(PropertySeed::new(7), 1, 16, None)
            .expect("valid persistence plan");
        match plan.run(
            |_| vec![1, 7, 8, 2],
            |bytes| {
                if bytes.windows(2).any(|window| window == [7, 8]) {
                    Err("contains marker")
                } else {
                    Ok(())
                }
            },
        ) {
            Err(PropertyRunError::Failure(failure)) => failure,
            other => panic!("expected minimized failure, got {other:?}"),
        }
    }

    #[test]
    fn persistence_round_trip_writes_loads_and_preserves_replay_payload() {
        let failure = minimized_failure();
        let directory = tempfile::tempdir().expect("temporary persistence directory");
        let path = directory.path().join("regression.case");

        write_persistence(&path, &failure).expect("write property persistence");
        let parsed = read_persistence(&path).expect("load property persistence");

        assert_eq!(parsed, failure);
        assert_eq!(parsed.minimized_input(), [7, 8]);
    }

    #[test]
    fn persistence_rejects_truncation_length_mismatch_and_oversize() {
        let rendered = minimized_failure().render_persistence().into_vec();
        let truncated = rendered
            .get(..rendered.len().saturating_sub(1))
            .expect("truncated persistence boundary");
        assert!(parse_persistence(truncated).is_err());

        let mut wrong_length = rendered.clone();
        let marker = b"minimized-bytes=2";
        let position = wrong_length
            .windows(marker.len())
            .position(|window| window == marker)
            .expect("minimized length field");
        let digit = position + marker.len() - 1;
        *wrong_length.get_mut(digit).expect("minimized length digit") = b'3';
        assert!(parse_persistence(&wrong_length).is_err());

        assert!(matches!(
            parse_persistence(&vec![0; MAX_PERSISTENCE_BYTES + 1]),
            Err(PersistenceError::TooLarge { .. })
        ));
    }
}
