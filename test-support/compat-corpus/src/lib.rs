use std::collections::BTreeSet;

use serde::Deserialize;
use thiserror::Error;

pub const CORPUS_SCHEMA_VERSION: u32 = 1;
pub const PINNED_STIM_TAG: &str = "v1.16.0";
pub const PINNED_STIM_COMMIT: &str = "e2fc1eca7fd21684d433aa5f10f4504ea4860d07";

#[derive(Debug, Error)]
pub enum CorpusError {
    #[error("failed to parse result-format corpus: {0}")]
    Parse(#[from] serde_json::Error),

    #[error("invalid result-format corpus: {0}")]
    Invalid(String),
}

#[derive(Clone, Debug)]
pub struct CheckedCorpus {
    cases: Vec<CheckedCase>,
}

impl CheckedCorpus {
    pub fn parse(bytes: &[u8]) -> Result<Self, CorpusError> {
        validate(serde_json::from_slice(bytes)?)
    }

    #[must_use]
    pub fn cases(&self) -> &[CheckedCase] {
        &self.cases
    }

    #[must_use]
    pub fn accepted_count(&self) -> usize {
        self.cases
            .iter()
            .filter(|case| case.acceptance == Acceptance::Accepted)
            .count()
    }
}

#[derive(Clone, Debug)]
pub struct CheckedCase {
    id: String,
    format: ResultFormat,
    layout: Layout,
    replay_shots: usize,
    acceptance: Acceptance,
    input: Vec<u8>,
    canonical_01: Option<Vec<u8>>,
    canonical_records: Option<Vec<Vec<bool>>>,
}

impl CheckedCase {
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    #[must_use]
    pub const fn format(&self) -> ResultFormat {
        self.format
    }

    #[must_use]
    pub const fn layout(&self) -> Layout {
        self.layout
    }

    #[must_use]
    pub const fn replay_shots(&self) -> usize {
        self.replay_shots
    }

    #[must_use]
    pub const fn acceptance(&self) -> Acceptance {
        self.acceptance
    }

    #[must_use]
    pub fn input(&self) -> &[u8] {
        &self.input
    }

    #[must_use]
    pub fn canonical_01(&self) -> Option<&[u8]> {
        self.canonical_01.as_deref()
    }

    #[must_use]
    pub fn canonical_records(&self) -> Option<&[Vec<bool>]> {
        self.canonical_records.as_deref()
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
pub enum ResultFormat {
    #[serde(rename = "01")]
    ZeroOne,
    #[serde(rename = "hits")]
    Hits,
    #[serde(rename = "dets")]
    Dets,
}

impl ResultFormat {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::ZeroOne => "01",
            Self::Hits => "hits",
            Self::Dets => "dets",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Layout {
    measurements: usize,
    detectors: usize,
    observables: usize,
}

impl Layout {
    #[must_use]
    pub const fn measurements(self) -> usize {
        self.measurements
    }

    #[must_use]
    pub const fn detectors(self) -> usize {
        self.detectors
    }

    #[must_use]
    pub const fn observables(self) -> usize {
        self.observables
    }

    pub fn total_bits(self) -> Result<usize, CorpusError> {
        self.measurements
            .checked_add(self.detectors)
            .and_then(|value| value.checked_add(self.observables))
            .ok_or_else(|| {
                CorpusError::Invalid(
                    "a result-format layout total width overflowed usize".to_string(),
                )
            })
    }

    #[must_use]
    pub const fn is_measurement_only(self) -> bool {
        self.detectors == 0 && self.observables == 0
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Acceptance {
    Accepted,
    Rejected,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Corpus {
    schema_version: u32,
    stim_tag: String,
    stim_commit: String,
    cases: Vec<CorpusCase>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CorpusCase {
    id: String,
    format: ResultFormat,
    layout: Layout,
    input_hex: String,
    replay_shots: usize,
    acceptance: Acceptance,
    #[serde(default)]
    canonical_01_hex: Option<String>,
}

fn validate(corpus: Corpus) -> Result<CheckedCorpus, CorpusError> {
    if corpus.schema_version != CORPUS_SCHEMA_VERSION {
        return Err(CorpusError::Invalid(format!(
            "schema version {} does not match {CORPUS_SCHEMA_VERSION}",
            corpus.schema_version
        )));
    }
    if corpus.stim_tag != PINNED_STIM_TAG || corpus.stim_commit != PINNED_STIM_COMMIT {
        return Err(CorpusError::Invalid(format!(
            "Stim identity {}@{} does not match {PINNED_STIM_TAG}@{PINNED_STIM_COMMIT}",
            corpus.stim_tag, corpus.stim_commit
        )));
    }
    if corpus.cases.is_empty() {
        return Err(CorpusError::Invalid("corpus contains no cases".to_string()));
    }

    let mut ids = BTreeSet::new();
    let mut checked = Vec::with_capacity(corpus.cases.len());
    for case in corpus.cases {
        validate_case_id(&case.id)?;
        if !ids.insert(case.id.clone()) {
            return Err(CorpusError::Invalid(format!(
                "duplicate case id {}",
                case.id
            )));
        }
        if case.replay_shots == 0 {
            return Err(CorpusError::Invalid(format!(
                "case {} has zero replay_shots",
                case.id
            )));
        }
        let width = case.layout.total_bits()?;
        if width == 0 {
            return Err(CorpusError::Invalid(format!(
                "case {} has a zero-width layout that the convert CLI cannot express",
                case.id
            )));
        }
        let input = decode_hex(&case.id, "input_hex", &case.input_hex)?;
        let canonical_01 = case
            .canonical_01_hex
            .as_deref()
            .map(|value| decode_hex(&case.id, "canonical_01_hex", value))
            .transpose()?;
        match (case.acceptance, canonical_01.as_ref()) {
            (Acceptance::Accepted, None) => {
                return Err(CorpusError::Invalid(format!(
                    "accepted case {} is missing canonical_01_hex",
                    case.id
                )));
            }
            (Acceptance::Rejected, Some(_)) => {
                return Err(CorpusError::Invalid(format!(
                    "rejected case {} must not have canonical_01_hex",
                    case.id
                )));
            }
            _ => {}
        }
        let canonical_records = canonical_01
            .as_deref()
            .map(|bytes| decode_canonical_01(&case.id, bytes, width, case.replay_shots))
            .transpose()?;
        checked.push(CheckedCase {
            id: case.id,
            format: case.format,
            layout: case.layout,
            replay_shots: case.replay_shots,
            acceptance: case.acceptance,
            input,
            canonical_01,
            canonical_records,
        });
    }
    Ok(CheckedCorpus { cases: checked })
}

fn validate_case_id(id: &str) -> Result<(), CorpusError> {
    if id.is_empty()
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        return Err(CorpusError::Invalid(format!(
            "case id {id:?} is not lowercase kebab-case"
        )));
    }
    Ok(())
}

fn decode_hex(case_id: &str, field: &'static str, value: &str) -> Result<Vec<u8>, CorpusError> {
    if value
        .bytes()
        .any(|byte| !byte.is_ascii_digit() && !(b'a'..=b'f').contains(&byte))
    {
        return Err(CorpusError::Invalid(format!(
            "case {case_id} field {field} is not canonical lowercase hexadecimal"
        )));
    }
    hex::decode(value).map_err(|error| {
        CorpusError::Invalid(format!(
            "case {case_id} field {field} is invalid hexadecimal: {error}"
        ))
    })
}

fn decode_canonical_01(
    case_id: &str,
    bytes: &[u8],
    width: usize,
    expected_records: usize,
) -> Result<Vec<Vec<bool>>, CorpusError> {
    let record_width = width.checked_add(1).ok_or_else(|| {
        CorpusError::Invalid(format!(
            "case {case_id} canonical 01 record width overflowed"
        ))
    })?;
    if !bytes.len().is_multiple_of(record_width) {
        return Err(CorpusError::Invalid(format!(
            "case {case_id} canonical 01 bytes do not contain complete LF-terminated records"
        )));
    }

    let mut records = Vec::with_capacity(bytes.len() / record_width);
    for chunk in bytes.chunks_exact(record_width) {
        let Some((terminator, bits)) = chunk.split_last() else {
            return Err(CorpusError::Invalid(format!(
                "case {case_id} canonical 01 record is empty"
            )));
        };
        if *terminator != b'\n' {
            return Err(CorpusError::Invalid(format!(
                "case {case_id} canonical 01 record is not LF-terminated"
            )));
        }
        let mut record = Vec::with_capacity(width);
        for byte in bits {
            match *byte {
                b'0' => record.push(false),
                b'1' => record.push(true),
                other => {
                    return Err(CorpusError::Invalid(format!(
                        "case {case_id} canonical 01 record contains invalid byte {other:#04x}"
                    )));
                }
            }
        }
        records.push(record);
    }
    if records.len() != expected_records {
        return Err(CorpusError::Invalid(format!(
            "case {case_id} canonical output has {} records but replay_shots is {expected_records}",
            records.len()
        )));
    }
    Ok(records)
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "corpus validation tests mutate compact serde_json fixtures and require precise failure setup"
)]
mod tests {
    use serde_json::{Value, json};

    use super::{CheckedCorpus, CorpusError, PINNED_STIM_COMMIT, PINNED_STIM_TAG};

    fn valid_corpus() -> Value {
        json!({
            "schema_version": 1,
            "stim_tag": PINNED_STIM_TAG,
            "stim_commit": PINNED_STIM_COMMIT,
            "cases": [{
                "id": "accepted-01",
                "format": "01",
                "layout": {
                    "measurements": 2,
                    "detectors": 0,
                    "observables": 0
                },
                "input_hex": "31300a",
                "replay_shots": 1,
                "acceptance": "accepted",
                "canonical_01_hex": "31300a"
            }]
        })
    }

    fn parse(value: &Value) -> Result<CheckedCorpus, CorpusError> {
        CheckedCorpus::parse(&serde_json::to_vec(value).expect("serialize corpus"))
    }

    #[test]
    fn valid_corpus_decodes_independent_canonical_records() {
        let corpus = parse(&valid_corpus()).expect("valid corpus");
        let case = &corpus.cases()[0];
        assert_eq!(
            case.canonical_records(),
            Some([vec![true, false]].as_slice())
        );
        assert_eq!(corpus.accepted_count(), 1);
    }

    #[test]
    fn unknown_fields_are_rejected() {
        let mut value = valid_corpus();
        value["unknown"] = json!(true);
        assert!(matches!(parse(&value), Err(CorpusError::Parse(_))));
    }

    #[test]
    fn duplicate_ids_are_rejected() {
        let mut value = valid_corpus();
        let duplicate = value["cases"][0].clone();
        value["cases"]
            .as_array_mut()
            .expect("case array")
            .push(duplicate);
        let error = parse(&value).expect_err("duplicate must fail").to_string();
        assert!(error.contains("duplicate case id"));
    }

    #[test]
    fn noncanonical_and_invalid_hex_are_rejected() {
        for input in ["A0", "0g", "0"] {
            let mut value = valid_corpus();
            value["cases"][0]["input_hex"] = json!(input);
            assert!(parse(&value).is_err(), "{input}");
        }
    }

    #[test]
    fn layout_width_overflow_is_rejected() {
        let mut value = valid_corpus();
        value["cases"][0]["layout"]["measurements"] = json!(usize::MAX);
        value["cases"][0]["layout"]["detectors"] = json!(1);
        let error = parse(&value).expect_err("overflow must fail").to_string();
        assert!(error.contains("total width overflowed"));
    }

    #[test]
    fn pinned_stim_identity_is_required() {
        for field in ["stim_tag", "stim_commit"] {
            let mut value = valid_corpus();
            value[field] = json!("wrong");
            let error = parse(&value).expect_err("identity must fail").to_string();
            assert!(error.contains("Stim identity"));
        }
    }

    #[test]
    fn acceptance_and_canonical_output_must_agree() {
        let mut accepted_missing = valid_corpus();
        accepted_missing["cases"][0]
            .as_object_mut()
            .expect("case")
            .remove("canonical_01_hex");
        assert!(parse(&accepted_missing).is_err());

        let mut rejected_with_output = valid_corpus();
        rejected_with_output["cases"][0]["acceptance"] = json!("rejected");
        assert!(parse(&rejected_with_output).is_err());
    }

    #[test]
    fn canonical_width_bytes_and_record_count_are_checked() {
        for canonical in ["310a", "31300d", "31320a", "31300a31300a"] {
            let mut value = valid_corpus();
            value["cases"][0]["canonical_01_hex"] = json!(hex::encode(canonical));
            assert!(parse(&value).is_err(), "{canonical:?}");
        }
    }
}
