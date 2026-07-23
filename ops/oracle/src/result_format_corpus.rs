use std::collections::BTreeSet;
use std::ffi::OsString;
use std::path::PathBuf;

use serde::Deserialize;
use thiserror::Error;

use crate::{
    OracleError, ProcessOutput, RepoRoot, STIM_COMMIT, STIM_TAG, ensure_stab_cli_binary,
    ensure_stim_binary, process::run_process,
};

const CORPUS_PATH: &str = "oracle/result-format-corpus.json";
const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Error)]
pub(super) enum ResultFormatCorpusError {
    #[error("failed to read result-format corpus {path}: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("failed to parse result-format corpus {path}: {source}")]
    Parse {
        path: PathBuf,
        source: serde_json::Error,
    },

    #[error("invalid result-format corpus: {0}")]
    Invalid(String),

    #[error("result-format corpus process failed: {0}")]
    Process(Box<OracleError>),

    #[error(
        "result-format corpus case {case_id} disagreed for {implementation}: {reason}\nstdout:\n{stdout}\nstderr:\n{stderr}"
    )]
    CaseMismatch {
        case_id: String,
        implementation: &'static str,
        reason: String,
        stdout: String,
        stderr: String,
    },
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Corpus {
    schema_version: u32,
    stim_tag: String,
    stim_commit: String,
    cases: Vec<CorpusCase>,
}

#[derive(Clone, Debug, Deserialize)]
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

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
enum ResultFormat {
    #[serde(rename = "01")]
    ZeroOne,
    #[serde(rename = "hits")]
    Hits,
    #[serde(rename = "dets")]
    Dets,
}

impl ResultFormat {
    const fn name(self) -> &'static str {
        match self {
            Self::ZeroOne => "01",
            Self::Hits => "hits",
            Self::Dets => "dets",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Layout {
    measurements: usize,
    detectors: usize,
    observables: usize,
}

impl Layout {
    fn total_bits(self) -> Result<usize, ResultFormatCorpusError> {
        self.measurements
            .checked_add(self.detectors)
            .and_then(|value| value.checked_add(self.observables))
            .ok_or_else(|| {
                ResultFormatCorpusError::Invalid(
                    "a result-format layout total width overflowed usize".to_string(),
                )
            })
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
enum Acceptance {
    Accepted,
    Rejected,
}

#[derive(Clone, Debug)]
struct CheckedCase {
    case: CorpusCase,
    input: Vec<u8>,
    canonical_01: Option<Vec<u8>>,
}

pub(super) fn run(
    root: &RepoRoot,
    check: bool,
    rebuild_stim: bool,
) -> Result<(), ResultFormatCorpusError> {
    let corpus = load(root)?;
    let checked = validate(corpus)?;
    if check {
        let stim_binary = ensure_stim_binary(root, rebuild_stim)
            .map_err(|error| ResultFormatCorpusError::Process(Box::new(error)))?;
        let stab_binary = ensure_stab_cli_binary(root)
            .map_err(|error| ResultFormatCorpusError::Process(Box::new(error)))?;
        for checked_case in &checked {
            let args = convert_args(&checked_case.case);
            let stim = run_process(&stim_binary, &args, &checked_case.input, Some(&root.path))
                .map_err(|error| ResultFormatCorpusError::Process(Box::new(error)))?;
            check_process("pinned Stim", checked_case, &stim)?;

            let stab = run_process(&stab_binary, &args, &checked_case.input, Some(&root.path))
                .map_err(|error| ResultFormatCorpusError::Process(Box::new(error)))?;
            check_process("Stab", checked_case, &stab)?;
        }
    }

    let accepted = checked
        .iter()
        .filter(|case| case.case.acceptance == Acceptance::Accepted)
        .count();
    println!(
        "[stab-oracle] result-format corpus: {} cases, {accepted} accepted, {} rejected{}",
        checked.len(),
        checked.len().saturating_sub(accepted),
        if check { ", Stim and Stab checked" } else { "" }
    );
    Ok(())
}

fn load(root: &RepoRoot) -> Result<Corpus, ResultFormatCorpusError> {
    let path = root.path.join(CORPUS_PATH);
    let bytes = std::fs::read(&path).map_err(|source| ResultFormatCorpusError::Read {
        path: path.clone(),
        source,
    })?;
    serde_json::from_slice(&bytes).map_err(|source| ResultFormatCorpusError::Parse { path, source })
}

fn validate(corpus: Corpus) -> Result<Vec<CheckedCase>, ResultFormatCorpusError> {
    if corpus.schema_version != SCHEMA_VERSION {
        return Err(ResultFormatCorpusError::Invalid(format!(
            "schema version {} does not match {SCHEMA_VERSION}",
            corpus.schema_version
        )));
    }
    if corpus.stim_tag != STIM_TAG || corpus.stim_commit != STIM_COMMIT {
        return Err(ResultFormatCorpusError::Invalid(format!(
            "Stim identity {}@{} does not match {STIM_TAG}@{STIM_COMMIT}",
            corpus.stim_tag, corpus.stim_commit
        )));
    }
    if corpus.cases.is_empty() {
        return Err(ResultFormatCorpusError::Invalid(
            "corpus contains no cases".to_string(),
        ));
    }

    let mut ids = BTreeSet::new();
    let mut checked = Vec::with_capacity(corpus.cases.len());
    for case in corpus.cases {
        if case.id.is_empty()
            || !case
                .id
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        {
            return Err(ResultFormatCorpusError::Invalid(format!(
                "case id {:?} is not lowercase kebab-case",
                case.id
            )));
        }
        if !ids.insert(case.id.clone()) {
            return Err(ResultFormatCorpusError::Invalid(format!(
                "duplicate case id {}",
                case.id
            )));
        }
        if case.replay_shots == 0 {
            return Err(ResultFormatCorpusError::Invalid(format!(
                "case {} has zero replay_shots",
                case.id
            )));
        }
        if case.layout.total_bits()? == 0 {
            return Err(ResultFormatCorpusError::Invalid(format!(
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
                return Err(ResultFormatCorpusError::Invalid(format!(
                    "accepted case {} is missing canonical_01_hex",
                    case.id
                )));
            }
            (Acceptance::Rejected, Some(_)) => {
                return Err(ResultFormatCorpusError::Invalid(format!(
                    "rejected case {} must not have canonical_01_hex",
                    case.id
                )));
            }
            _ => {}
        }
        if let Some(canonical) = canonical_01.as_ref() {
            let records = stab_core::result_formats::read_records(
                canonical,
                stab_core::SampleFormat::ZeroOne,
                case.layout.total_bits()?,
            )
            .map_err(|error| {
                ResultFormatCorpusError::Invalid(format!(
                    "case {} has invalid canonical 01 data: {error}",
                    case.id
                ))
            })?;
            if records.len() != case.replay_shots {
                return Err(ResultFormatCorpusError::Invalid(format!(
                    "case {} canonical output has {} records but replay_shots is {}",
                    case.id,
                    records.len(),
                    case.replay_shots
                )));
            }
        }
        checked.push(CheckedCase {
            case,
            input,
            canonical_01,
        });
    }
    Ok(checked)
}

fn decode_hex(
    case_id: &str,
    field: &'static str,
    value: &str,
) -> Result<Vec<u8>, ResultFormatCorpusError> {
    if value
        .bytes()
        .any(|byte| !byte.is_ascii_digit() && !(b'a'..=b'f').contains(&byte))
    {
        return Err(ResultFormatCorpusError::Invalid(format!(
            "case {case_id} field {field} is not canonical lowercase hexadecimal"
        )));
    }
    hex::decode(value).map_err(|error| {
        ResultFormatCorpusError::Invalid(format!(
            "case {case_id} field {field} is invalid hexadecimal: {error}"
        ))
    })
}

fn convert_args(case: &CorpusCase) -> Vec<OsString> {
    vec![
        OsString::from("convert"),
        OsString::from("--in_format"),
        OsString::from(case.format.name()),
        OsString::from("--out_format"),
        OsString::from("01"),
        OsString::from("--num_measurements"),
        OsString::from(case.layout.measurements.to_string()),
        OsString::from("--num_detectors"),
        OsString::from(case.layout.detectors.to_string()),
        OsString::from("--num_observables"),
        OsString::from(case.layout.observables.to_string()),
    ]
}

fn check_process(
    implementation: &'static str,
    checked_case: &CheckedCase,
    output: &ProcessOutput,
) -> Result<(), ResultFormatCorpusError> {
    let expected_success = checked_case.case.acceptance == Acceptance::Accepted;
    if output.success() != expected_success {
        return Err(case_mismatch(
            implementation,
            checked_case,
            output,
            format!(
                "expected {:?} but exit status was {:?}",
                checked_case.case.acceptance, output.status
            ),
        ));
    }
    if expected_success {
        let canonical = checked_case.canonical_01.as_deref().ok_or_else(|| {
            ResultFormatCorpusError::Invalid(format!(
                "accepted case {} lost canonical output after validation",
                checked_case.case.id
            ))
        })?;
        if output.stdout.bytes != canonical {
            return Err(case_mismatch(
                implementation,
                checked_case,
                output,
                format!(
                    "canonical 01 output differed: expected {}, got {}",
                    hex::encode(canonical),
                    hex::encode(&output.stdout.bytes)
                ),
            ));
        }
        if output.stderr.has_non_whitespace() {
            return Err(case_mismatch(
                implementation,
                checked_case,
                output,
                "accepted case emitted stderr".to_string(),
            ));
        }
    } else if !output.stderr.has_non_whitespace() {
        return Err(case_mismatch(
            implementation,
            checked_case,
            output,
            "rejected case did not emit a diagnostic".to_string(),
        ));
    }
    Ok(())
}

fn case_mismatch(
    implementation: &'static str,
    checked_case: &CheckedCase,
    output: &ProcessOutput,
    reason: String,
) -> ResultFormatCorpusError {
    ResultFormatCorpusError::CaseMismatch {
        case_id: checked_case.case.id.clone(),
        implementation,
        reason,
        stdout: output.stdout.render_for_diagnostics(),
        stderr: output.stderr.render_for_diagnostics(),
    }
}

#[cfg(test)]
mod tests {
    use super::{load, validate};
    use crate::RepoRoot;

    #[test]
    fn committed_result_format_corpus_is_structurally_valid() {
        let root = RepoRoot::resolve(std::path::Path::new("../..")).expect("repo root");
        let corpus = load(&root).expect("load corpus");
        let checked = validate(corpus).expect("validate corpus");
        assert!(checked.len() > 50);
    }
}
