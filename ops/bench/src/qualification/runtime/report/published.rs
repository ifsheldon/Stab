use std::path::{Path, PathBuf};

use super::{
    QualificationReport, ReportArgs, ReportError, preflight_artifact, render_markdown,
    validate_report,
};
use crate::qualification::runtime::run::sha256_hex;
use crate::root::RepoRoot;

pub(in crate::qualification::runtime) const MAX_PUBLISHED_REPORT_BYTES: usize = 4 << 20;
pub(in crate::qualification::runtime) const MAX_PUBLISHED_PREFLIGHT_BYTES: usize = 1 << 20;

pub(in crate::qualification::runtime) fn run(
    root: &RepoRoot,
    expected_performance_inventory_sha256: &str,
    expected_correctness_inventory_sha256: &str,
    args: ReportArgs,
) -> Result<PathBuf, ReportError> {
    let (report, report_json) = load_bound_report(
        root,
        &args.input,
        expected_performance_inventory_sha256,
        expected_correctness_inventory_sha256,
    )?;
    let preflight_json = render_preflight(&report, &report_json)?;
    let markdown = render_markdown(&report, &sha256_hex(&report_json))?;

    let output = super::super::artifact::QualificationOutput::begin(root, &args.input)?;
    output.require_current_artifact("report.json", &report_json)?;
    output.write("report.json", &report_json)?;
    output.write("preflight.json", &preflight_json)?;
    output.write("report.md", markdown.as_bytes())?;
    let relative = output.relative().to_path_buf();
    output.commit()?;
    Ok(relative)
}

pub(in crate::qualification::runtime) fn load_validated_published_report(
    root: &RepoRoot,
    input: &Path,
    expected_performance_inventory_sha256: &str,
    expected_correctness_inventory_sha256: &str,
) -> Result<QualificationReport, ReportError> {
    Ok(load_validated_published_evidence(
        root,
        input,
        expected_performance_inventory_sha256,
        expected_correctness_inventory_sha256,
    )?
    .report)
}

pub(in crate::qualification::runtime) struct PublishedReportEvidence {
    pub(in crate::qualification::runtime) report: QualificationReport,
    pub(in crate::qualification::runtime) report_sha256: String,
    pub(in crate::qualification::runtime) preflight_sha256: String,
}

pub(in crate::qualification::runtime) fn load_validated_published_evidence(
    root: &RepoRoot,
    input: &Path,
    expected_performance_inventory_sha256: &str,
    expected_correctness_inventory_sha256: &str,
) -> Result<PublishedReportEvidence, ReportError> {
    let (report, report_json) = load_bound_report(
        root,
        input,
        expected_performance_inventory_sha256,
        expected_correctness_inventory_sha256,
    )?;
    let expected_preflight = render_preflight(&report, &report_json)?;
    let actual_preflight = super::super::artifact::read_artifact_bounded(
        root,
        input,
        "preflight.json",
        MAX_PUBLISHED_PREFLIGHT_BYTES,
    )?;
    if actual_preflight != expected_preflight {
        return Err(ReportError::PreflightBinding);
    }
    Ok(PublishedReportEvidence {
        report,
        report_sha256: sha256_hex(&report_json),
        preflight_sha256: sha256_hex(&actual_preflight),
    })
}

fn load_bound_report(
    root: &RepoRoot,
    input: &Path,
    expected_performance_inventory_sha256: &str,
    expected_correctness_inventory_sha256: &str,
) -> Result<(QualificationReport, Vec<u8>), ReportError> {
    let report_json = super::super::artifact::read_artifact_bounded(
        root,
        input,
        "report.json",
        MAX_PUBLISHED_REPORT_BYTES,
    )?;
    if report_json.is_empty() || !report_json.ends_with(b"\n") {
        return Err(ReportError::ReportBoundary);
    }
    let report: QualificationReport =
        serde_json::from_slice(&report_json).map_err(ReportError::Json)?;
    let mut canonical = serde_json::to_vec_pretty(&report).map_err(ReportError::Json)?;
    canonical.push(b'\n');
    if canonical != report_json {
        let offset = canonical
            .iter()
            .zip(&report_json)
            .position(|(expected, actual)| expected != actual)
            .unwrap_or_else(|| canonical.len().min(report_json.len()));
        return Err(ReportError::NonCanonicalReport {
            offset,
            actual: report_json.get(offset).copied(),
            expected: canonical.get(offset).copied(),
            actual_length: report_json.len(),
            expected_length: canonical.len(),
        });
    }
    validate_report(
        root,
        &report,
        expected_performance_inventory_sha256,
        expected_correctness_inventory_sha256,
    )?;
    if Path::new(&report.command.output) != input {
        return Err(ReportError::OutputBinding);
    }
    Ok((report, report_json))
}

fn render_preflight(
    report: &QualificationReport,
    report_json: &[u8],
) -> Result<Vec<u8>, ReportError> {
    let preflight = preflight_artifact(report, report_json)?;
    let mut bytes = serde_json::to_vec_pretty(&preflight).map_err(ReportError::Json)?;
    bytes.push(b'\n');
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    #[test]
    fn source_owned_float_evidence_round_trips_canonically() {
        let value = 0.001_487_307_957_989_267_3_f64;
        let encoded = serde_json::to_vec(&value).expect("serialize evidence float");
        assert_eq!(encoded, b"0.0014873079579892673");

        let decoded: f64 = serde_json::from_slice(&encoded).expect("parse evidence float");
        assert_eq!(decoded.to_bits(), value.to_bits());
        assert_eq!(
            serde_json::to_vec(&decoded).expect("reserialize evidence float"),
            encoded
        );
    }
}
