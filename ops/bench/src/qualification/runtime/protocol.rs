use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde::{Deserialize, Deserializer, Serialize};
use thiserror::Error;

pub(super) const PROTOCOL_SCHEMA_VERSION: u32 = 2;
const MAX_PROTOCOL_BYTES: usize = 1 << 20;
const MAX_PROTOCOL_LINE_BYTES: usize = 16 << 10;
const MAX_PROTOCOL_ROWS: usize = 64;
const MAX_PROTOCOL_ID_BYTES: usize = 128;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Implementation {
    Stim,
    Stab,
}

impl fmt::Display for Implementation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Stim => "stim",
            Self::Stab => "stab",
        })
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum EvidenceMode {
    Timing,
    Memory,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub(crate) struct ProtocolId(Box<str>);

impl ProtocolId {
    pub(super) fn try_new(value: impl Into<String>) -> Result<Self, ProtocolError> {
        let value = value.into();
        if value.is_empty()
            || value.len() > MAX_PROTOCOL_ID_BYTES
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"-_.:/".contains(&byte))
        {
            return Err(ProtocolError::InvalidId(value));
        }
        Ok(Self(value.into_boxed_str()))
    }
}

impl fmt::Display for ProtocolId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for ProtocolId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::try_new(value).map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub(crate) struct Sha256Digest(Box<str>);

impl Sha256Digest {
    pub(super) fn try_new(value: impl Into<String>) -> Result<Self, ProtocolError> {
        let value = value.into();
        if value.len() != 64
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(ProtocolError::InvalidSha256(value));
        }
        Ok(Self(value.into_boxed_str()))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub(crate) struct SemanticDigest(Box<str>);

impl SemanticDigest {
    pub(super) fn try_new(value: impl Into<String>) -> Result<Self, ProtocolError> {
        let value = value.into();
        if value.len() != 64
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(ProtocolError::InvalidSemanticDigest(value));
        }
        Ok(Self(value.into_boxed_str()))
    }
}

impl<'de> Deserialize<'de> for SemanticDigest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::try_new(value).map_err(serde::de::Error::custom)
    }
}

impl<'de> Deserialize<'de> for Sha256Digest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::try_new(value).map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub(crate) struct GitCommit(Box<str>);

impl GitCommit {
    pub(super) fn try_new(value: impl Into<String>) -> Result<Self, ProtocolError> {
        let value = value.into();
        if value.len() != 40 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(ProtocolError::InvalidGitCommit(value));
        }
        Ok(Self(value.to_ascii_lowercase().into_boxed_str()))
    }

    pub(super) fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for GitCommit {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::try_new(value).map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct WorkerMeasurement {
    pub(super) schema_version: u32,
    pub(super) implementation: Implementation,
    pub(super) evidence_mode: EvidenceMode,
    pub(super) workload_id: ProtocolId,
    pub(super) measurement_id: ProtocolId,
    pub(super) iteration_count: u64,
    pub(super) elapsed_seconds: f64,
    pub(super) work_count: u64,
    pub(super) output_digest: SemanticDigest,
    pub(super) setup_rss_bytes: Option<u64>,
    pub(super) peak_rss_bytes: Option<u64>,
    pub(super) affinity_cpu: Option<u32>,
    pub(super) stim_commit: GitCommit,
    pub(super) source_digest: Sha256Digest,
    pub(super) build_fingerprint: Sha256Digest,
}

impl WorkerMeasurement {
    pub(super) fn validate_values(&self) -> Result<(), ProtocolError> {
        if self.schema_version != PROTOCOL_SCHEMA_VERSION {
            return Err(ProtocolError::SchemaVersion {
                actual: self.schema_version,
                expected: PROTOCOL_SCHEMA_VERSION,
            });
        }
        if self.iteration_count == 0 {
            return Err(ProtocolError::ZeroIterations {
                measurement: self.measurement_id.clone(),
            });
        }
        if !self.elapsed_seconds.is_finite() || self.elapsed_seconds <= 0.0 {
            return Err(ProtocolError::InvalidElapsed {
                measurement: self.measurement_id.clone(),
                value: self.elapsed_seconds,
            });
        }
        if self.work_count == 0 {
            return Err(ProtocolError::ZeroWork {
                measurement: self.measurement_id.clone(),
            });
        }
        match (self.setup_rss_bytes, self.peak_rss_bytes) {
            (Some(setup), Some(peak)) if peak < setup => {
                return Err(ProtocolError::MemoryOrdering {
                    measurement: self.measurement_id.clone(),
                    setup,
                    peak,
                });
            }
            (Some(_), None) | (None, Some(_)) => {
                return Err(ProtocolError::IncompleteMemory {
                    measurement: self.measurement_id.clone(),
                });
            }
            _ => {}
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub(crate) struct ProtocolExpectation {
    pub(super) implementation: Implementation,
    pub(super) evidence_mode: EvidenceMode,
    pub(super) workload_id: ProtocolId,
    pub(super) measurement_ids: BTreeSet<ProtocolId>,
    pub(super) iteration_count: u64,
    pub(super) affinity_cpu: Option<u32>,
    pub(super) stim_commit: GitCommit,
    pub(super) source_digest: Sha256Digest,
    pub(super) build_fingerprint: Sha256Digest,
}

impl ProtocolExpectation {
    pub(super) fn validate(&self, rows: &[WorkerMeasurement]) -> Result<(), ProtocolError> {
        let mut observed = BTreeSet::new();
        for row in rows {
            if row.implementation != self.implementation
                || row.evidence_mode != self.evidence_mode
                || row.workload_id != self.workload_id
                || row.iteration_count != self.iteration_count
                || row.affinity_cpu != self.affinity_cpu
                || row.stim_commit != self.stim_commit
                || row.source_digest != self.source_digest
                || row.build_fingerprint != self.build_fingerprint
            {
                return Err(ProtocolError::ExpectationMismatch {
                    measurement: row.measurement_id.clone(),
                });
            }
            if !observed.insert(row.measurement_id.clone()) {
                return Err(ProtocolError::DuplicateMeasurement(
                    row.measurement_id.clone(),
                ));
            }
        }
        if observed != self.measurement_ids {
            return Err(ProtocolError::MeasurementSet {
                expected: self.measurement_ids.clone(),
                actual: observed,
            });
        }
        Ok(())
    }
}

pub(crate) fn parse_worker_json_lines(
    bytes: &[u8],
) -> Result<Vec<WorkerMeasurement>, ProtocolError> {
    if bytes.len() > MAX_PROTOCOL_BYTES {
        return Err(ProtocolError::OutputTooLarge {
            actual: bytes.len(),
            maximum: MAX_PROTOCOL_BYTES,
        });
    }
    if bytes.is_empty() || !bytes.ends_with(b"\n") {
        return Err(ProtocolError::MissingTerminatingNewline);
    }
    let text = std::str::from_utf8(bytes).map_err(ProtocolError::Utf8)?;
    let mut rows = Vec::new();
    for line in text.lines() {
        if line.is_empty() {
            return Err(ProtocolError::EmptyLine);
        }
        if line.len() > MAX_PROTOCOL_LINE_BYTES {
            return Err(ProtocolError::LineTooLarge {
                actual: line.len(),
                maximum: MAX_PROTOCOL_LINE_BYTES,
            });
        }
        if rows.len() == MAX_PROTOCOL_ROWS {
            return Err(ProtocolError::TooManyRows {
                maximum: MAX_PROTOCOL_ROWS,
            });
        }
        let row: WorkerMeasurement = serde_json::from_str(line).map_err(ProtocolError::Json)?;
        row.validate_values()?;
        rows.push(row);
    }
    if rows.is_empty() {
        return Err(ProtocolError::MissingRows);
    }
    let mut ids = BTreeMap::new();
    for row in &rows {
        if ids
            .insert(row.measurement_id.clone(), row.workload_id.clone())
            .is_some()
        {
            return Err(ProtocolError::DuplicateMeasurement(
                row.measurement_id.clone(),
            ));
        }
    }
    Ok(rows)
}

#[derive(Debug, Error)]
pub(crate) enum ProtocolError {
    #[error("invalid qualification protocol id {0:?}")]
    InvalidId(String),
    #[error("invalid lowercase SHA-256 digest {0:?}")]
    InvalidSha256(String),
    #[error("invalid 256-bit semantic digest {0:?}")]
    InvalidSemanticDigest(String),
    #[error("invalid 40-character Git commit {0:?}")]
    InvalidGitCommit(String),
    #[error("qualification worker output is {actual} bytes, exceeding {maximum}")]
    OutputTooLarge { actual: usize, maximum: usize },
    #[error("qualification worker output must end with exactly delimited JSON Lines")]
    MissingTerminatingNewline,
    #[error("qualification worker output is not UTF-8: {0}")]
    Utf8(std::str::Utf8Error),
    #[error("qualification worker output contains an empty JSON line")]
    EmptyLine,
    #[error("qualification worker JSON line is {actual} bytes, exceeding {maximum}")]
    LineTooLarge { actual: usize, maximum: usize },
    #[error("qualification worker output exceeds {maximum} rows")]
    TooManyRows { maximum: usize },
    #[error("qualification worker output contains no rows")]
    MissingRows,
    #[error("qualification worker JSON is invalid: {0}")]
    Json(serde_json::Error),
    #[error("qualification protocol schema is {actual}, expected {expected}")]
    SchemaVersion { actual: u32, expected: u32 },
    #[error("measurement {measurement} has zero iterations")]
    ZeroIterations { measurement: ProtocolId },
    #[error("measurement {measurement} has invalid elapsed seconds {value}")]
    InvalidElapsed { measurement: ProtocolId, value: f64 },
    #[error("measurement {measurement} has zero semantic work")]
    ZeroWork { measurement: ProtocolId },
    #[error("measurement {measurement} reports peak RSS {peak} below setup RSS {setup}")]
    MemoryOrdering {
        measurement: ProtocolId,
        setup: u64,
        peak: u64,
    },
    #[error("measurement {measurement} reports only one of setup or peak RSS")]
    IncompleteMemory { measurement: ProtocolId },
    #[error("qualification worker repeats measurement {0}")]
    DuplicateMeasurement(ProtocolId),
    #[error("measurement {measurement} does not match the worker invocation receipt")]
    ExpectationMismatch { measurement: ProtocolId },
    #[error("qualification worker measurement set differs: expected {expected:?}, got {actual:?}")]
    MeasurementSet {
        expected: BTreeSet<ProtocolId>,
        actual: BTreeSet<ProtocolId>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_line() -> String {
        serde_json::json!({
            "schema_version": PROTOCOL_SCHEMA_VERSION,
            "implementation": "stab",
            "evidence_mode": "timing",
            "workload_id": "protocol-smoke",
            "measurement_id": "main",
            "iteration_count": 4,
            "elapsed_seconds": 0.25,
            "work_count": 64,
            "output_digest": "a".repeat(64),
            "setup_rss_bytes": 1024,
            "peak_rss_bytes": 2048,
            "affinity_cpu": null,
            "stim_commit": "e2fc1eca7fd21684d433aa5f10f4504ea4860d07",
            "source_digest": "b".repeat(64),
            "build_fingerprint": "c".repeat(64),
        })
        .to_string()
    }

    #[test]
    fn parses_one_bounded_protocol_row() {
        let bytes = format!("{}\n", valid_line());
        let rows = parse_worker_json_lines(bytes.as_bytes()).expect("valid protocol row");
        assert_eq!(rows.len(), 1);
        assert_eq!(
            rows.first()
                .map(|row| row.measurement_id.to_string())
                .as_deref(),
            Some("main")
        );
    }

    #[test]
    fn rejects_unknown_fields_and_noncanonical_boundaries() {
        let mut unknown: serde_json::Value =
            serde_json::from_str(&valid_line()).expect("valid fixture JSON");
        unknown
            .as_object_mut()
            .expect("fixture is an object")
            .insert("extra".to_string(), serde_json::json!(1));
        let unknown = unknown.to_string();
        assert!(parse_worker_json_lines(format!("{unknown}\n").as_bytes()).is_err());
        assert!(parse_worker_json_lines(valid_line().as_bytes()).is_err());
        assert!(parse_worker_json_lines(format!("{}\n\n", valid_line()).as_bytes()).is_err());
    }

    #[test]
    fn rejects_zero_work_and_inconsistent_memory_order() {
        let zero_work = valid_line().replace("\"work_count\":64", "\"work_count\":0");
        assert!(parse_worker_json_lines(format!("{zero_work}\n").as_bytes()).is_err());
        let memory = valid_line().replace("\"peak_rss_bytes\":2048", "\"peak_rss_bytes\":512");
        assert!(parse_worker_json_lines(format!("{memory}\n").as_bytes()).is_err());
    }

    #[test]
    fn expectation_rejects_stale_fingerprint_and_missing_measurement() {
        let bytes = format!("{}\n", valid_line());
        let rows = parse_worker_json_lines(bytes.as_bytes()).expect("valid protocol row");
        let expectation = ProtocolExpectation {
            implementation: Implementation::Stab,
            evidence_mode: EvidenceMode::Timing,
            workload_id: ProtocolId::try_new("protocol-smoke").expect("workload id"),
            measurement_ids: [ProtocolId::try_new("main").expect("measurement id")]
                .into_iter()
                .collect(),
            iteration_count: 4,
            affinity_cpu: None,
            stim_commit: GitCommit::try_new("e2fc1eca7fd21684d433aa5f10f4504ea4860d07")
                .expect("commit"),
            source_digest: Sha256Digest::try_new("b".repeat(64)).expect("source digest"),
            build_fingerprint: Sha256Digest::try_new("d".repeat(64))
                .expect("different fingerprint"),
        };
        assert!(expectation.validate(&rows).is_err());
    }

    #[test]
    fn rejects_oversized_nonfinite_and_excess_worker_rows() {
        let oversized = vec![b'x'; MAX_PROTOCOL_BYTES + 1];
        assert!(matches!(
            parse_worker_json_lines(&oversized),
            Err(ProtocolError::OutputTooLarge { .. })
        ));

        let nonfinite = valid_line().replace("0.25", "1e999");
        assert!(parse_worker_json_lines(format!("{nonfinite}\n").as_bytes()).is_err());

        let mut lines = String::new();
        for index in 0..=MAX_PROTOCOL_ROWS {
            let line = valid_line().replace("\"main\"", &format!("\"main-{index}\""));
            lines.push_str(&line);
            lines.push('\n');
        }
        assert!(matches!(
            parse_worker_json_lines(lines.as_bytes()),
            Err(ProtocolError::TooManyRows { .. })
        ));
    }

    #[test]
    fn expectation_rejects_affinity_and_extra_measurement_drift() {
        let rows = parse_worker_json_lines(format!("{}\n", valid_line()).as_bytes())
            .expect("valid protocol row");
        let expectation = ProtocolExpectation {
            implementation: Implementation::Stab,
            evidence_mode: EvidenceMode::Timing,
            workload_id: ProtocolId::try_new("protocol-smoke").expect("workload id"),
            measurement_ids: [ProtocolId::try_new("main").expect("measurement id")]
                .into_iter()
                .collect(),
            iteration_count: 4,
            affinity_cpu: Some(0),
            stim_commit: GitCommit::try_new("e2fc1eca7fd21684d433aa5f10f4504ea4860d07")
                .expect("commit"),
            source_digest: Sha256Digest::try_new("b".repeat(64)).expect("source digest"),
            build_fingerprint: Sha256Digest::try_new("c".repeat(64)).expect("build fingerprint"),
        };
        assert!(expectation.validate(&rows).is_err());

        let extra = valid_line().replace("\"main\"", "\"extra\"");
        let mut two_rows = format!("{}\n", valid_line());
        two_rows.push_str(&extra);
        two_rows.push('\n');
        let rows = parse_worker_json_lines(two_rows.as_bytes()).expect("two unique rows");
        let mut expectation = expectation;
        expectation.affinity_cpu = None;
        assert!(expectation.validate(&rows).is_err());
    }
}
