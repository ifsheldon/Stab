use std::path::{Path, PathBuf};

use super::{
    QualificationReport, ReportArgs, ReportError, preflight_artifact, render_markdown,
    validate_report,
};
use crate::qualification::runtime::run::sha256_hex;
use crate::root::RepoRoot;

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
    let markdown = render_markdown(&report, &sha256_hex(&report_json));

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
    let (report, report_json) = load_bound_report(
        root,
        input,
        expected_performance_inventory_sha256,
        expected_correctness_inventory_sha256,
    )?;
    let expected_preflight = render_preflight(&report, &report_json)?;
    let actual_preflight = super::super::artifact::read_artifact(root, input, "preflight.json")?;
    if actual_preflight != expected_preflight {
        return Err(ReportError::PreflightBinding);
    }
    Ok(report)
}

fn load_bound_report(
    root: &RepoRoot,
    input: &Path,
    expected_performance_inventory_sha256: &str,
    expected_correctness_inventory_sha256: &str,
) -> Result<(QualificationReport, Vec<u8>), ReportError> {
    let report_json = super::super::artifact::read_artifact(root, input, "report.json")?;
    if report_json.is_empty() || !report_json.ends_with(b"\n") {
        return Err(ReportError::ReportBoundary);
    }
    let report: QualificationReport =
        serde_json::from_slice(&report_json).map_err(ReportError::Json)?;
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
