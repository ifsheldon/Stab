use std::path::{Component, Path};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::artifacts::{read_bounded, verify_binding};
use super::{ArtifactBinding, focused_error};
use crate::error::BenchError;
use crate::manifest::is_safe_benchmark_id;
use crate::qualification::{GitCommit, Sha256Digest};
use crate::report::CompareReport;
use crate::root::RepoRoot;

mod producer;

pub(crate) use producer::{A6ProfileReceiptArgs, produce};

pub(super) const PROFILE_RECEIPT_SCHEMA_VERSION: u32 = 1;
pub(super) const MAX_PROFILE_RECEIPT_BYTES: u64 = 64 << 10;
pub(super) const MAX_PROFILE_DATA_BYTES: u64 = 64 << 20;

const MAX_PROBE_STDERR_BYTES: usize = 4 << 10;
const MIN_PERF_DATA_BYTES: u64 = 104;
const MIN_PERF_DATA_BYTES_USIZE: usize = 104;
const PERF_DATA_MAGIC: &[u8; 8] = b"PERFILE2";
const PERF_DATA_REVERSED_MAGIC: &[u8; 8] = b"2ELIFREP";
const RECEIPT_FILE_NAME: &str = "profile-receipt.json";
const PERF_DATA_FILE_NAME: &str = "perf.data";

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ProfileReceipt {
    schema_version: u32,
    profiler: ProfileKind,
    identity: ProfileReceiptIdentity,
    probe: ProfileProbeDiagnostics,
    outcome: ProfileOutcome,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum ProfileKind {
    LinuxPerfRecord,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ProfileReceiptIdentity {
    row_id: String,
    focused_report: ArtifactBinding,
    source_revision: String,
    stab_executable_sha256: String,
    host_fingerprint: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ProfileProbeDiagnostics {
    exit_code: i32,
    perf_event_paranoid: Option<i32>,
    stderr_hex: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "status", rename_all = "kebab-case", deny_unknown_fields)]
pub(super) enum ProfileOutcome {
    Captured { data: ProfileDataBinding },
    Unavailable { reason: ProfileUnavailableReason },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum ProfileUnavailableReason {
    KernelPolicyDenied,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ProfileDataBinding {
    path: String,
    sha256: String,
    bytes: u64,
}

impl ProfileReceiptIdentity {
    pub(super) fn from_focused_report(
        focused_report_binding: &ArtifactBinding,
        focused_report: &CompareReport,
    ) -> Result<Self, BenchError> {
        validate_artifact_binding(
            "focused report",
            focused_report_binding,
            Some("compare.json"),
        )?;
        let [row] = focused_report.rows.as_slice() else {
            return Err(profile_error(format!(
                "focused report must contain exactly one row, found {}",
                focused_report.rows.len()
            )));
        };
        if focused_report.stab.local_modifications {
            return Err(profile_error(
                "focused report must bind a clean Stab revision",
            ));
        }
        let identity = Self {
            row_id: row.id.clone(),
            focused_report: focused_report_binding.clone(),
            source_revision: focused_report.stab.commit.clone(),
            stab_executable_sha256: focused_report.stab.executable_sha256.clone(),
            host_fingerprint: focused_report.machine.host_fingerprint.clone(),
        };
        identity.validate()?;
        Ok(identity)
    }

    fn validate(&self) -> Result<(), BenchError> {
        if !is_safe_benchmark_id(&self.row_id) {
            return Err(profile_error(format!(
                "row id {:?} is not a safe benchmark id",
                self.row_id
            )));
        }
        validate_artifact_binding("focused report", &self.focused_report, Some("compare.json"))?;
        if !GitCommit::is_canonical_str(&self.source_revision) {
            return Err(profile_error(
                "source revision must be a lowercase 40-byte Git object id",
            ));
        }
        validate_sha256("Stab executable SHA-256", &self.stab_executable_sha256)?;
        validate_sha256("host fingerprint", &self.host_fingerprint)
    }
}

impl ProfileProbeDiagnostics {
    pub(super) fn from_stderr(
        exit_code: i32,
        perf_event_paranoid: Option<i32>,
        stderr: &[u8],
    ) -> Result<Self, BenchError> {
        if stderr.len() > MAX_PROBE_STDERR_BYTES {
            return Err(profile_error(format!(
                "profile probe stderr contains {} bytes, maximum is {MAX_PROBE_STDERR_BYTES}",
                stderr.len()
            )));
        }
        let diagnostics = Self {
            exit_code,
            perf_event_paranoid,
            stderr_hex: hex::encode(stderr),
        };
        diagnostics.validate()?;
        Ok(diagnostics)
    }

    pub(super) fn stderr(&self) -> Result<Vec<u8>, BenchError> {
        decode_probe_stderr(&self.stderr_hex)
    }

    fn validate(&self) -> Result<(), BenchError> {
        if !(0..=255).contains(&self.exit_code) {
            return Err(profile_error(format!(
                "profile probe exit code {} is outside 0..=255",
                self.exit_code
            )));
        }
        if self
            .perf_event_paranoid
            .is_some_and(|value| !(-1..=1024).contains(&value))
        {
            return Err(profile_error(
                "perf_event_paranoid is outside the supported -1..=1024 range",
            ));
        }
        drop(self.stderr()?);
        Ok(())
    }
}

impl ProfileDataBinding {
    pub(super) fn bind(root: &RepoRoot, path: &Path) -> Result<Self, BenchError> {
        let path_text = path.to_str().ok_or_else(|| {
            profile_error(format!(
                "profile data path {} is not valid UTF-8",
                path.display()
            ))
        })?;
        validate_target_benchmark_path("profile data", path_text, Some(PERF_DATA_FILE_NAME))?;
        let bytes = read_bounded(&root.resolve_relative(path), MAX_PROFILE_DATA_BYTES)?;
        validate_perf_data(path_text, &bytes)?;
        let byte_count = u64::try_from(bytes.len())
            .map_err(|_| profile_error("profile data length does not fit u64"))?;
        Ok(Self {
            path: path_text.to_string(),
            sha256: hex::encode(Sha256::digest(&bytes)),
            bytes: byte_count,
        })
    }

    fn validate_shape(&self) -> Result<(), BenchError> {
        validate_target_benchmark_path("profile data", &self.path, Some(PERF_DATA_FILE_NAME))?;
        validate_sha256("profile data SHA-256", &self.sha256)?;
        if !(MIN_PERF_DATA_BYTES..=MAX_PROFILE_DATA_BYTES).contains(&self.bytes) {
            return Err(profile_error(format!(
                "profile data size {} is outside {MIN_PERF_DATA_BYTES}..={MAX_PROFILE_DATA_BYTES}",
                self.bytes
            )));
        }
        Ok(())
    }
}

impl ProfileReceipt {
    pub(super) fn captured(
        identity: ProfileReceiptIdentity,
        probe: ProfileProbeDiagnostics,
        data: ProfileDataBinding,
    ) -> Result<Self, BenchError> {
        let receipt = Self {
            schema_version: PROFILE_RECEIPT_SCHEMA_VERSION,
            profiler: ProfileKind::LinuxPerfRecord,
            identity,
            probe,
            outcome: ProfileOutcome::Captured { data },
        };
        receipt.validate_shape()?;
        Ok(receipt)
    }

    pub(super) fn unavailable(
        identity: ProfileReceiptIdentity,
        probe: ProfileProbeDiagnostics,
    ) -> Result<Self, BenchError> {
        let receipt = Self {
            schema_version: PROFILE_RECEIPT_SCHEMA_VERSION,
            profiler: ProfileKind::LinuxPerfRecord,
            identity,
            probe,
            outcome: ProfileOutcome::Unavailable {
                reason: ProfileUnavailableReason::KernelPolicyDenied,
            },
        };
        receipt.validate_shape()?;
        Ok(receipt)
    }

    pub(super) fn outcome(&self) -> &ProfileOutcome {
        &self.outcome
    }

    pub(super) fn to_pretty_json(&self) -> Result<Vec<u8>, BenchError> {
        self.validate_shape()?;
        let mut bytes = serde_json::to_vec_pretty(self)?;
        bytes.push(b'\n');
        if u64::try_from(bytes.len()).map_or(true, |len| len > MAX_PROFILE_RECEIPT_BYTES) {
            return Err(profile_error(format!(
                "serialized profile receipt exceeds {MAX_PROFILE_RECEIPT_BYTES} bytes"
            )));
        }
        Ok(bytes)
    }

    fn validate_shape(&self) -> Result<(), BenchError> {
        if self.schema_version != PROFILE_RECEIPT_SCHEMA_VERSION {
            return Err(profile_error(format!(
                "profile receipt schema_version={} expected {PROFILE_RECEIPT_SCHEMA_VERSION}",
                self.schema_version
            )));
        }
        if self.profiler != ProfileKind::LinuxPerfRecord {
            return Err(profile_error(
                "profile receipt must use the Linux perf-record contract",
            ));
        }
        self.identity.validate()?;
        self.probe.validate()?;
        match &self.outcome {
            ProfileOutcome::Captured { data } => {
                if self.probe.exit_code != 0 {
                    return Err(profile_error(
                        "captured profile requires a successful probe",
                    ));
                }
                data.validate_shape()
            }
            ProfileOutcome::Unavailable {
                reason: ProfileUnavailableReason::KernelPolicyDenied,
            } => {
                if self.probe.exit_code == 0 {
                    return Err(profile_error(
                        "kernel-policy-denied requires a failed probe",
                    ));
                }
                if !self
                    .probe
                    .perf_event_paranoid
                    .is_some_and(|value| value >= 2)
                {
                    return Err(profile_error(
                        "kernel-policy-denied requires perf_event_paranoid >= 2",
                    ));
                }
                if self.probe.stderr()?.is_empty() {
                    return Err(profile_error(
                        "kernel-policy-denied requires nonempty probe diagnostics",
                    ));
                }
                if !is_kernel_policy_denial(&self.probe.stderr()?) {
                    return Err(profile_error(
                        "kernel-policy-denied diagnostics do not identify a permission or perf-event policy denial",
                    ));
                }
                Ok(())
            }
        }
    }
}

pub(super) fn read_and_validate(
    root: &RepoRoot,
    receipt_binding: &ArtifactBinding,
    focused_report_binding: &ArtifactBinding,
    focused_report: &CompareReport,
    forbidden_bindings: &[ArtifactBinding],
) -> Result<ProfileReceipt, BenchError> {
    validate_artifact_binding("profile receipt", receipt_binding, Some(RECEIPT_FILE_NAME))?;
    let bytes = verify_binding(root, receipt_binding, MAX_PROFILE_RECEIPT_BYTES)?;
    let receipt: ProfileReceipt = serde_json::from_slice(&bytes).map_err(|error| {
        profile_error(format!("failed to parse {}: {error}", receipt_binding.path))
    })?;
    receipt.validate_against(
        root,
        receipt_binding,
        focused_report_binding,
        focused_report,
        forbidden_bindings,
    )?;
    Ok(receipt)
}

pub(super) fn validate_against(
    receipt: &ProfileReceipt,
    root: &RepoRoot,
    receipt_binding: &ArtifactBinding,
    focused_report_binding: &ArtifactBinding,
    focused_report: &CompareReport,
    forbidden_bindings: &[ArtifactBinding],
) -> Result<(), BenchError> {
    receipt.validate_against(
        root,
        receipt_binding,
        focused_report_binding,
        focused_report,
        forbidden_bindings,
    )
}

impl ProfileReceipt {
    fn validate_against(
        &self,
        root: &RepoRoot,
        receipt_binding: &ArtifactBinding,
        focused_report_binding: &ArtifactBinding,
        focused_report: &CompareReport,
        forbidden_bindings: &[ArtifactBinding],
    ) -> Result<(), BenchError> {
        self.validate_shape()?;
        validate_artifact_binding("profile receipt", receipt_binding, Some(RECEIPT_FILE_NAME))?;
        validate_artifact_binding(
            "focused report",
            focused_report_binding,
            Some("compare.json"),
        )?;
        require_distinct_artifact(
            "profile receipt",
            receipt_binding,
            "focused report",
            focused_report_binding,
        )?;
        require_exact_binding(
            "profile receipt focused report",
            &self.identity.focused_report,
            focused_report_binding,
        )?;

        let expected_identity =
            ProfileReceiptIdentity::from_focused_report(focused_report_binding, focused_report)?;
        require_identity(&self.identity, &expected_identity)?;
        for forbidden in forbidden_bindings {
            require_distinct_artifact(
                "profile receipt",
                receipt_binding,
                "forbidden artifact",
                forbidden,
            )?;
        }

        if let ProfileOutcome::Captured { data } = &self.outcome {
            require_sibling_profile_data(receipt_binding, data)?;
            require_distinct_data("profile data", data, "profile receipt", receipt_binding)?;
            require_distinct_data(
                "profile data",
                data,
                "focused report",
                focused_report_binding,
            )?;
            for forbidden in forbidden_bindings {
                require_distinct_data("profile data", data, "forbidden artifact", forbidden)?;
            }
            verify_profile_data(root, data)?;
        }
        Ok(())
    }
}

fn verify_profile_data(root: &RepoRoot, binding: &ProfileDataBinding) -> Result<(), BenchError> {
    binding.validate_shape()?;
    let bytes = read_bounded(
        &root.resolve_relative(Path::new(&binding.path)),
        MAX_PROFILE_DATA_BYTES,
    )?;
    let actual_bytes = u64::try_from(bytes.len())
        .map_err(|_| profile_error("profile data length does not fit u64"))?;
    if actual_bytes != binding.bytes {
        return Err(profile_error(format!(
            "profile data {} has {actual_bytes} bytes, expected {}",
            binding.path, binding.bytes
        )));
    }
    let actual_sha256 = hex::encode(Sha256::digest(&bytes));
    if actual_sha256 != binding.sha256 {
        return Err(profile_error(format!(
            "profile data {} SHA-256 is {actual_sha256}, expected {}",
            binding.path, binding.sha256
        )));
    }
    validate_perf_data(&binding.path, &bytes)
}

fn validate_perf_data(path: &str, bytes: &[u8]) -> Result<(), BenchError> {
    if bytes.len() < MIN_PERF_DATA_BYTES_USIZE {
        return Err(profile_error(format!(
            "profile data {path} is shorter than the {MIN_PERF_DATA_BYTES}-byte PERF header"
        )));
    }
    if !bytes.starts_with(PERF_DATA_MAGIC) && !bytes.starts_with(PERF_DATA_REVERSED_MAGIC) {
        return Err(profile_error(format!(
            "profile data {path} does not start with PERF data magic"
        )));
    }
    Ok(())
}

fn require_identity(
    actual: &ProfileReceiptIdentity,
    expected: &ProfileReceiptIdentity,
) -> Result<(), BenchError> {
    require_exact_binding(
        "profile receipt focused report",
        &actual.focused_report,
        &expected.focused_report,
    )?;
    if actual.row_id != expected.row_id
        || actual.source_revision != expected.source_revision
        || actual.stab_executable_sha256 != expected.stab_executable_sha256
        || actual.host_fingerprint != expected.host_fingerprint
    {
        return Err(profile_error(
            "profile receipt identity does not match its focused report",
        ));
    }
    Ok(())
}

fn require_exact_binding(
    label: &str,
    actual: &ArtifactBinding,
    expected: &ArtifactBinding,
) -> Result<(), BenchError> {
    if actual.path != expected.path || actual.sha256 != expected.sha256 {
        return Err(profile_error(format!(
            "{label} does not match the supplied artifact binding"
        )));
    }
    Ok(())
}

fn require_sibling_profile_data(
    receipt: &ArtifactBinding,
    data: &ProfileDataBinding,
) -> Result<(), BenchError> {
    if Path::new(&receipt.path).parent() != Path::new(&data.path).parent() {
        return Err(profile_error(
            "profile data must be a sibling of its profile receipt",
        ));
    }
    Ok(())
}

fn require_distinct_artifact(
    left_label: &str,
    left: &ArtifactBinding,
    right_label: &str,
    right: &ArtifactBinding,
) -> Result<(), BenchError> {
    if left.path == right.path || left.sha256 == right.sha256 {
        return Err(profile_error(format!("{left_label} reuses {right_label}")));
    }
    Ok(())
}

fn require_distinct_data(
    data_label: &str,
    data: &ProfileDataBinding,
    other_label: &str,
    other: &ArtifactBinding,
) -> Result<(), BenchError> {
    if data.path == other.path || data.sha256 == other.sha256 {
        return Err(profile_error(format!("{data_label} reuses {other_label}")));
    }
    Ok(())
}

fn validate_artifact_binding(
    label: &str,
    binding: &ArtifactBinding,
    expected_file_name: Option<&str>,
) -> Result<(), BenchError> {
    validate_target_benchmark_path(label, &binding.path, expected_file_name)?;
    validate_sha256(&format!("{label} SHA-256"), &binding.sha256)
}

fn validate_target_benchmark_path(
    label: &str,
    value: &str,
    expected_file_name: Option<&str>,
) -> Result<(), BenchError> {
    let path = Path::new(value);
    if value.is_empty()
        || path.is_absolute()
        || !path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
    {
        return Err(profile_error(format!(
            "{label} path {value:?} is not a safe repository-relative path"
        )));
    }
    let mut components = path.components();
    if components.next() != Some(Component::Normal("target".as_ref()))
        || components.next() != Some(Component::Normal("benchmarks".as_ref()))
        || components.next().is_none()
    {
        return Err(profile_error(format!(
            "{label} path {value:?} must be under target/benchmarks"
        )));
    }
    if let Some(expected) = expected_file_name
        && path.file_name() != Some(expected.as_ref())
    {
        return Err(profile_error(format!(
            "{label} path {value:?} must end in {expected:?}"
        )));
    }
    Ok(())
}

fn validate_sha256(label: &str, value: &str) -> Result<(), BenchError> {
    if !Sha256Digest::is_valid_str(value) {
        return Err(profile_error(format!(
            "{label} must be a lowercase 64-byte SHA-256"
        )));
    }
    Ok(())
}

fn decode_probe_stderr(value: &str) -> Result<Vec<u8>, BenchError> {
    if value.len() > MAX_PROBE_STDERR_BYTES * 2 {
        return Err(profile_error(format!(
            "profile probe stderr exceeds {MAX_PROBE_STDERR_BYTES} bytes"
        )));
    }
    let bytes = hex::decode(value)
        .map_err(|error| profile_error(format!("profile probe stderr is invalid hex: {error}")))?;
    if bytes.len() > MAX_PROBE_STDERR_BYTES {
        return Err(profile_error(format!(
            "profile probe stderr exceeds {MAX_PROBE_STDERR_BYTES} bytes"
        )));
    }
    Ok(bytes)
}

fn is_kernel_policy_denial(stderr: &[u8]) -> bool {
    let message = String::from_utf8_lossy(stderr).to_ascii_lowercase();
    [
        "no permission to enable",
        "permission error",
        "permission denied",
        "operation not permitted",
        "perf_event_paranoid",
        "access to performance monitoring and observability operations is limited",
    ]
    .iter()
    .any(|needle| message.contains(needle))
}

fn profile_error(message: impl Into<String>) -> BenchError {
    focused_error(format!("profile receipt: {}", message.into()))
}

#[cfg(test)]
mod tests {
    use std::fs::OpenOptions;
    use std::io::{Seek as _, SeekFrom, Write as _};

    use tempfile::TempDir;

    use super::*;
    use crate::comparability::ComparabilityClass;
    use crate::manifest::{Milestone, Runner, ThresholdClass};
    use crate::report::{CompareRowResult, Measurement, MeasurementObservation};

    const REVISION: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const EXECUTABLE_SHA256: &str =
        "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd";
    const HOST_FINGERPRINT: &str =
        "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";

    struct Fixture {
        _directory: TempDir,
        root: RepoRoot,
        report: CompareReport,
        report_binding: ArtifactBinding,
        receipt_path: String,
        data_path: String,
    }

    impl Fixture {
        fn new() -> Self {
            let directory = tempfile::tempdir().expect("temporary repository");
            let output = directory
                .path()
                .join("target/benchmarks/a6-profile-row-aaaaaaaa");
            std::fs::create_dir_all(&output).expect("profile output directory");
            let root = RepoRoot::resolve(directory.path()).expect("repository root");
            let report = focused_report();
            let report_path = "target/benchmarks/a6-focused-row-aaaaaaaa/compare.json";
            let absolute_report = directory.path().join(report_path);
            std::fs::create_dir_all(absolute_report.parent().expect("focused report parent"))
                .expect("focused report directory");
            let report_bytes = serde_json::to_vec_pretty(&report).expect("focused report JSON");
            std::fs::write(&absolute_report, &report_bytes).expect("focused report");
            let report_binding = binding(report_path, &report_bytes);
            let data_path = "target/benchmarks/a6-profile-row-aaaaaaaa/perf.data".to_string();
            std::fs::write(directory.path().join(&data_path), perf_data()).expect("PERF data");
            Self {
                _directory: directory,
                root,
                report,
                report_binding,
                receipt_path: "target/benchmarks/a6-profile-row-aaaaaaaa/profile-receipt.json"
                    .to_string(),
                data_path,
            }
        }

        fn identity(&self) -> ProfileReceiptIdentity {
            ProfileReceiptIdentity::from_focused_report(&self.report_binding, &self.report)
                .expect("focused identity")
        }

        fn data_binding(&self) -> ProfileDataBinding {
            ProfileDataBinding::bind(&self.root, Path::new(&self.data_path))
                .expect("profile data binding")
        }

        fn write_receipt(&self, receipt: &ProfileReceipt) -> ArtifactBinding {
            let bytes = receipt.to_pretty_json().expect("receipt JSON");
            std::fs::write(
                self.root.resolve_relative(Path::new(&self.receipt_path)),
                &bytes,
            )
            .expect("profile receipt");
            binding(&self.receipt_path, &bytes)
        }
    }

    #[test]
    fn captured_receipt_round_trips_and_validates_exact_identity() {
        let fixture = Fixture::new();
        let probe =
            ProfileProbeDiagnostics::from_stderr(0, Some(4), b"").expect("successful probe");
        let receipt = ProfileReceipt::captured(fixture.identity(), probe, fixture.data_binding())
            .expect("captured receipt");
        let binding = fixture.write_receipt(&receipt);

        let parsed = read_and_validate(
            &fixture.root,
            &binding,
            &fixture.report_binding,
            &fixture.report,
            &[],
        )
        .expect("valid receipt");
        assert!(matches!(parsed.outcome(), ProfileOutcome::Captured { .. }));
    }

    #[test]
    fn unavailable_receipt_is_typed_and_requires_bounded_denial_evidence() {
        let fixture = Fixture::new();
        let probe = ProfileProbeDiagnostics::from_stderr(
            255,
            Some(4),
            b"perf_event_open: Operation not permitted",
        )
        .expect("failed probe");
        let receipt =
            ProfileReceipt::unavailable(fixture.identity(), probe).expect("unavailable receipt");
        let binding = fixture.write_receipt(&receipt);

        let parsed = read_and_validate(
            &fixture.root,
            &binding,
            &fixture.report_binding,
            &fixture.report,
            &[],
        )
        .expect("valid unavailable receipt");
        assert!(matches!(
            parsed.outcome(),
            ProfileOutcome::Unavailable {
                reason: ProfileUnavailableReason::KernelPolicyDenied
            }
        ));

        let successful_probe =
            ProfileProbeDiagnostics::from_stderr(0, Some(4), b"").expect("successful probe");
        let error = ProfileReceipt::unavailable(fixture.identity(), successful_probe)
            .expect_err("successful probe cannot prove unavailability");
        assert!(error.to_string().contains("requires a failed probe"));

        let weak_policy_probe =
            ProfileProbeDiagnostics::from_stderr(1, Some(1), b"unrelated profiler failure")
                .expect("bounded probe");
        let error = ProfileReceipt::unavailable(fixture.identity(), weak_policy_probe)
            .expect_err("weak policy does not prove denial");
        assert!(error.to_string().contains("perf_event_paranoid >= 2"));

        let unrelated_probe =
            ProfileProbeDiagnostics::from_stderr(1, Some(4), b"unknown event cycles")
                .expect("bounded probe");
        let error = ProfileReceipt::unavailable(fixture.identity(), unrelated_probe)
            .expect_err("an unrelated perf error cannot prove policy denial");
        assert!(
            error
                .to_string()
                .contains("do not identify a permission or perf-event policy denial")
        );
    }

    #[test]
    fn producer_publishes_one_immutable_receipt_after_full_validation() {
        let fixture = Fixture::new();
        let probe =
            ProfileProbeDiagnostics::from_stderr(0, Some(4), b"").expect("successful probe");
        let receipt = ProfileReceipt::captured(fixture.identity(), probe, fixture.data_binding())
            .expect("captured receipt");
        let output = Path::new(&fixture.receipt_path);

        let published = producer::publish_receipt(
            &fixture.root,
            output,
            &receipt,
            &fixture.report_binding,
            &fixture.report,
        )
        .expect("publish receipt");
        assert_eq!(published, output);

        let error = producer::publish_receipt(
            &fixture.root,
            output,
            &receipt,
            &fixture.report_binding,
            &fixture.report,
        )
        .expect_err("receipt output is immutable");
        assert!(error.to_string().contains("File exists"));
    }

    #[test]
    fn identity_and_profile_data_mutations_are_rejected() {
        let fixture = Fixture::new();
        let probe =
            ProfileProbeDiagnostics::from_stderr(0, Some(4), b"").expect("successful probe");
        let mut receipt =
            ProfileReceipt::captured(fixture.identity(), probe, fixture.data_binding())
                .expect("captured receipt");
        receipt.identity.source_revision = "b".repeat(40);
        let binding = fixture.write_receipt(&receipt);
        let error = read_and_validate(
            &fixture.root,
            &binding,
            &fixture.report_binding,
            &fixture.report,
            &[],
        )
        .expect_err("mutated identity");
        assert!(error.to_string().contains("identity does not match"));

        let probe =
            ProfileProbeDiagnostics::from_stderr(0, Some(4), b"").expect("successful probe");
        let receipt = ProfileReceipt::captured(fixture.identity(), probe, fixture.data_binding())
            .expect("captured receipt");
        let binding = fixture.write_receipt(&receipt);
        let mut bytes = std::fs::read(fixture.root.resolve_relative(Path::new(&fixture.data_path)))
            .expect("PERF data");
        let last = bytes.last_mut().expect("nonempty PERF data");
        *last ^= 1;
        std::fs::write(
            fixture.root.resolve_relative(Path::new(&fixture.data_path)),
            bytes,
        )
        .expect("mutated PERF data");
        let error = read_and_validate(
            &fixture.root,
            &binding,
            &fixture.report_binding,
            &fixture.report,
            &[],
        )
        .expect_err("mutated nested data");
        assert!(error.to_string().contains("SHA-256"));
    }

    #[test]
    fn profile_data_requires_perf_magic_and_cannot_reuse_other_artifacts() {
        let fixture = Fixture::new();
        let invalid_path = "target/benchmarks/a6-profile-row-aaaaaaaa/invalid/perf.data";
        let absolute_invalid = fixture.root.resolve_relative(Path::new(invalid_path));
        std::fs::create_dir_all(absolute_invalid.parent().expect("invalid data parent"))
            .expect("invalid data directory");
        std::fs::write(&absolute_invalid, vec![0; MIN_PERF_DATA_BYTES_USIZE])
            .expect("invalid data");
        let error = ProfileDataBinding::bind(&fixture.root, Path::new(invalid_path))
            .expect_err("invalid PERF magic");
        assert!(error.to_string().contains("PERF data magic"));

        let probe =
            ProfileProbeDiagnostics::from_stderr(0, Some(4), b"").expect("successful probe");
        let receipt = ProfileReceipt::captured(fixture.identity(), probe, fixture.data_binding())
            .expect("captured receipt");
        let binding = fixture.write_receipt(&receipt);
        let forbidden = ArtifactBinding {
            path: "target/benchmarks/other/artifact.bin".to_string(),
            sha256: fixture.data_binding().sha256,
        };
        let error = read_and_validate(
            &fixture.root,
            &binding,
            &fixture.report_binding,
            &fixture.report,
            &[forbidden],
        )
        .expect_err("reused profile digest");
        assert!(
            error
                .to_string()
                .contains("profile data reuses forbidden artifact")
        );

        let mut nonsibling = receipt;
        assert!(matches!(
            nonsibling.outcome,
            ProfileOutcome::Captured { .. }
        ));
        if let ProfileOutcome::Captured { data } = &mut nonsibling.outcome {
            data.path = invalid_path.to_string();
            data.sha256 = hex::encode(Sha256::digest(perf_data()));
        }
        let error = validate_against(
            &nonsibling,
            &fixture.root,
            &binding,
            &fixture.report_binding,
            &fixture.report,
            &[],
        )
        .expect_err("nonsibling data");
        assert!(error.to_string().contains("must be a sibling"));
    }

    #[test]
    fn receipt_probe_and_raw_data_bounds_are_enforced() {
        let fixture = Fixture::new();
        let error = ProfileProbeDiagnostics::from_stderr(
            1,
            Some(4),
            &vec![b'x'; MAX_PROBE_STDERR_BYTES + 1],
        )
        .expect_err("oversized diagnostics");
        assert!(error.to_string().contains("probe stderr"));

        let oversized_receipt_path =
            "target/benchmarks/a6-profile-row-aaaaaaaa/profile-receipt.json";
        let oversized_receipt = vec![
            b' ';
            usize::try_from(MAX_PROFILE_RECEIPT_BYTES + 1)
                .expect("receipt limit fits usize")
        ];
        std::fs::write(
            fixture
                .root
                .resolve_relative(Path::new(oversized_receipt_path)),
            &oversized_receipt,
        )
        .expect("oversized receipt");
        let oversized_binding = binding(oversized_receipt_path, &oversized_receipt);
        assert!(
            read_and_validate(
                &fixture.root,
                &oversized_binding,
                &fixture.report_binding,
                &fixture.report,
                &[],
            )
            .is_err(),
            "oversized receipt must be rejected"
        );

        let oversized_data_path = "target/benchmarks/a6-profile-row-aaaaaaaa/oversized/perf.data";
        let absolute_data = fixture
            .root
            .resolve_relative(Path::new(oversized_data_path));
        std::fs::create_dir_all(absolute_data.parent().expect("oversized data parent"))
            .expect("oversized data directory");
        let mut file = OpenOptions::new()
            .create_new(true)
            .read(true)
            .write(true)
            .open(&absolute_data)
            .expect("oversized profile data");
        file.write_all(PERF_DATA_MAGIC).expect("PERF magic");
        file.seek(SeekFrom::Start(MAX_PROFILE_DATA_BYTES))
            .expect("seek beyond profile cap");
        file.write_all(&[0]).expect("extend profile data");
        assert!(
            ProfileDataBinding::bind(&fixture.root, Path::new(oversized_data_path)).is_err(),
            "oversized profile data must be rejected"
        );
    }

    #[test]
    fn tagged_outcome_rejects_contradictory_or_unknown_fields() {
        let fixture = Fixture::new();
        let probe =
            ProfileProbeDiagnostics::from_stderr(0, Some(4), b"").expect("successful probe");
        let receipt = ProfileReceipt::captured(fixture.identity(), probe, fixture.data_binding())
            .expect("captured receipt");
        let mut value = serde_json::to_value(receipt).expect("receipt value");
        let outcome = value
            .get_mut("outcome")
            .and_then(serde_json::Value::as_object_mut)
            .expect("outcome object");
        outcome.insert(
            "reason".to_string(),
            serde_json::json!("kernel-policy-denied"),
        );
        let error = serde_json::from_value::<ProfileReceipt>(value)
            .expect_err("captured outcome cannot include unavailable reason");
        assert!(error.to_string().contains("unknown field"));
    }

    fn binding(path: &str, bytes: &[u8]) -> ArtifactBinding {
        ArtifactBinding {
            path: path.to_string(),
            sha256: hex::encode(Sha256::digest(bytes)),
        }
    }

    fn perf_data() -> Vec<u8> {
        let mut bytes = PERF_DATA_MAGIC.to_vec();
        bytes.resize(MIN_PERF_DATA_BYTES_USIZE, 0);
        bytes
    }

    fn focused_report() -> CompareReport {
        let row = CompareRowResult {
            id: "row".to_string(),
            milestone: Milestone::M7,
            threshold_class: ThresholdClass::ReportOnly.as_str().to_string(),
            runner: Runner::ContractOnly,
            comparability: ComparabilityClass::ReportOnly,
            upstream_source: "src/stim/example.perf.cc".to_string(),
            phase: "throughput".to_string(),
            measurement: "row-work".to_string(),
            status: "measured".to_string(),
            baseline_summary: String::new(),
            stab_summary: String::new(),
            note: None,
            stim_measurements: Vec::new(),
            stab_measurements: vec![Measurement {
                name: "stab_measurement".to_string(),
                seconds: 1.0,
                variance_seconds: None,
                allocation: None,
                resident_bytes: None,
                resident_delta_bytes: None,
                observations: Vec::<MeasurementObservation>::new(),
                iterations: Some(8),
            }],
            stim_median_seconds: None,
            stab_median_seconds: Some(1.0),
            relative_ratio: None,
            measurement_ratios: Vec::new(),
            stab_allocation_count_max: None,
            stab_allocation_bytes_max: None,
            stab_resident_bytes_max: None,
            stab_resident_delta_bytes_max: None,
            pass_fail_status: "not-comparable".to_string(),
            beta_gate_status: "not-checked".to_string(),
            beta_gate_waiver_reason: None,
            beta_gate_waiver_follow_up: None,
            beta_gate_error: None,
            memory_gate_status: "not-required".to_string(),
            memory_gate_baseline_bytes_max: None,
            memory_gate_allowed_bytes_max: None,
            memory_gate_baseline_resident_bytes_max: None,
            memory_gate_allowed_resident_bytes_max: None,
            memory_gate_baseline_resident_delta_bytes_max: None,
            memory_gate_allowed_resident_delta_bytes_max: None,
            memory_gate_error: None,
            regression_threshold_status: "not-configured".to_string(),
            regression_threshold_max_ratio: None,
            regression_threshold_waiver_reason: None,
            regression_threshold_waiver_follow_up: None,
            regression_threshold_error: None,
            profiler_note_status: "not-required".to_string(),
            profiler_note_path: None,
            profiler_note_error: None,
        };
        serde_json::from_value(serde_json::json!({
            "schema_version": 4,
            "generated_unix_epoch_seconds": 1,
            "machine": {
                "os": "linux",
                "arch": "x86_64",
                "family": "unix",
                "cpu_identity": "test cpu",
                "host_fingerprint": HOST_FINGERPRINT,
                "available_parallelism": 1,
                "rustc_version": "rustc test",
                "cmake_version": "cmake test"
            },
            "stim": {
                "source_path": "vendor/stim",
                "expected_tag": "v1.16.0",
                "expected_commit": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                "actual_tag": "v1.16.0",
                "actual_commit": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
            },
            "stab": {
                "commit": REVISION,
                "local_modifications": false,
                "executable_sha256": EXECUTABLE_SHA256
            },
            "command": {
                "baseline_path": "target/benchmarks/baseline/baseline.json",
                "baseline_sha256": "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
                "profile": "release",
                "milestone": null,
                "primary": false,
                "filters": ["row"],
                "cargo_features": [],
                "timing_boundary": "source-owned-row-native-v1",
                "measurement_contract_path": "benchmarks/a6-measurement-contract.json",
                "measurement_contract_sha256": "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
                "require_profiler_notes": false,
                "require_beta_gate": false,
                "beta_waivers_path": null,
                "regression_waivers_path": null,
                "require_memory_gate": false,
                "memory_baseline_path": null,
                "thresholds_path": null,
                "profiler_notes_path": null,
                "profiler_notes_paths": [],
                "track_allocations": false,
                "warmup": true,
                "measurement_runs": 1,
                "strict": true,
                "new_output": true
            },
            "rows": [row]
        }))
        .expect("focused report")
    }
}
