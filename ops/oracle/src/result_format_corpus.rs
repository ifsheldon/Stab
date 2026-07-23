use std::ffi::OsString;
use std::path::PathBuf;

use stab_compat_corpus::{Acceptance, CheckedCase, CheckedCorpus, CorpusError};
use thiserror::Error;

use crate::{
    OracleError, ProcessOutput, RepoRoot, ensure_stab_cli_binary, ensure_stim_binary,
    process::run_process,
};

const CORPUS_PATH: &str = "oracle/result-format-corpus.json";

#[derive(Debug, Error)]
pub(super) enum ResultFormatCorpusError {
    #[error("failed to read result-format corpus {path}: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("invalid result-format corpus {path}: {source}")]
    Corpus { path: PathBuf, source: CorpusError },

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

pub(super) fn run(
    root: &RepoRoot,
    check: bool,
    rebuild_stim: bool,
) -> Result<(), ResultFormatCorpusError> {
    let checked = load(root)?;
    if check {
        let stim_binary = ensure_stim_binary(root, rebuild_stim)
            .map_err(|error| ResultFormatCorpusError::Process(Box::new(error)))?;
        let stab_binary = ensure_stab_cli_binary(root)
            .map_err(|error| ResultFormatCorpusError::Process(Box::new(error)))?;
        for checked_case in checked.cases() {
            let args = convert_args(checked_case);
            let stim = run_process(&stim_binary, &args, checked_case.input(), Some(&root.path))
                .map_err(|error| ResultFormatCorpusError::Process(Box::new(error)))?;
            check_process("pinned Stim", checked_case, &stim)?;

            let stab = run_process(&stab_binary, &args, checked_case.input(), Some(&root.path))
                .map_err(|error| ResultFormatCorpusError::Process(Box::new(error)))?;
            check_process("Stab", checked_case, &stab)?;
        }
    }

    let accepted = checked.accepted_count();
    println!(
        "[stab-oracle] result-format corpus: {} cases, {accepted} accepted, {} rejected{}",
        checked.cases().len(),
        checked.cases().len().saturating_sub(accepted),
        if check { ", Stim and Stab checked" } else { "" }
    );
    Ok(())
}

fn load(root: &RepoRoot) -> Result<CheckedCorpus, ResultFormatCorpusError> {
    let path = root.path.join(CORPUS_PATH);
    let bytes = std::fs::read(&path).map_err(|source| ResultFormatCorpusError::Read {
        path: path.clone(),
        source,
    })?;
    CheckedCorpus::parse(&bytes).map_err(|source| ResultFormatCorpusError::Corpus { path, source })
}

fn convert_args(case: &CheckedCase) -> Vec<OsString> {
    let layout = case.layout();
    vec![
        OsString::from("convert"),
        OsString::from("--in_format"),
        OsString::from(case.format().name()),
        OsString::from("--out_format"),
        OsString::from("01"),
        OsString::from("--num_measurements"),
        OsString::from(layout.measurements().to_string()),
        OsString::from("--num_detectors"),
        OsString::from(layout.detectors().to_string()),
        OsString::from("--num_observables"),
        OsString::from(layout.observables().to_string()),
    ]
}

fn check_process(
    implementation: &'static str,
    checked_case: &CheckedCase,
    output: &ProcessOutput,
) -> Result<(), ResultFormatCorpusError> {
    let expected_success = checked_case.acceptance() == Acceptance::Accepted;
    if output.success() != expected_success {
        return Err(case_mismatch(
            implementation,
            checked_case,
            output,
            format!(
                "expected {:?} but exit status was {:?}",
                checked_case.acceptance(),
                output.status
            ),
        ));
    }
    if expected_success {
        let canonical = checked_case.canonical_01().ok_or_else(|| {
            ResultFormatCorpusError::Invalid(format!(
                "accepted case {} lost canonical output after validation",
                checked_case.id()
            ))
        })?;
        if output.stdout.bytes != canonical {
            return Err(case_mismatch(
                implementation,
                checked_case,
                output,
                format!(
                    "canonical 01 output differed: expected {}, got {}",
                    render_hex(canonical),
                    render_hex(&output.stdout.bytes)
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
        case_id: checked_case.id().to_string(),
        implementation,
        reason,
        stdout: output.stdout.render_for_diagnostics(),
        stderr: output.stderr.render_for_diagnostics(),
    }
}

fn render_hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;

    let mut rendered = String::with_capacity(bytes.len().saturating_mul(2));
    for byte in bytes {
        let _ = write!(rendered, "{byte:02x}");
    }
    rendered
}

#[cfg(test)]
mod tests {
    use super::load;
    use crate::RepoRoot;

    #[test]
    fn committed_result_format_corpus_is_structurally_valid() {
        let root = RepoRoot::resolve(std::path::Path::new("../..")).expect("repo root");
        let checked = load(&root).expect("load corpus");
        assert_eq!(checked.cases().len(), 62);
        assert_eq!(checked.accepted_count(), 20);
    }
}
