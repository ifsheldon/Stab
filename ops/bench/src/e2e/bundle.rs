use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};

use clap::Args;
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

use super::error::BundleError;
use super::host::validate_recorded_host_transition;
use super::model::{SelfBaseline, Suite, TIMING_BOUNDARY, Tier};
use super::report::{
    CORRECTNESS_SCHEMA_VERSION, CorrectnessReport, DerivedReport, GateOutcome, RawSamples,
    RunMetadata, markdown,
};
use crate::error::BenchError;
use crate::root::RepoRoot;

const CURRENT_EVIDENCE: &str = "benchmarks/current-aarch64-evidence.toml";

#[derive(Clone, Debug, Args)]
pub(crate) struct ReplayArgs {
    /// Existing repository-relative bundle directory under target/benchmarks.
    #[arg(long)]
    input: PathBuf,
}

#[derive(Clone, Debug, Args)]
pub(crate) struct BaselineCandidateArgs {
    /// Accepted full bundle.
    #[arg(long)]
    full: PathBuf,
    /// Accepted soak bundle.
    #[arg(long)]
    soak: PathBuf,
    /// New directory receiving a reviewed TOML baseline candidate.
    #[arg(long)]
    out: PathBuf,
}

pub(crate) fn replay(root: &RepoRoot, args: ReplayArgs) -> Result<(), BenchError> {
    let directory = root.benchmark_output_dir(&args.input)?;
    let bundle = verify_bundle(&directory)?;
    println!(
        "[stab-bench] replayed {} E2E cases from {}",
        bundle.report.cases.len(),
        directory.display()
    );
    Ok(())
}

pub(crate) fn baseline_candidate(
    root: &RepoRoot,
    args: BaselineCandidateArgs,
) -> Result<(), BenchError> {
    let full = verify_bundle(&root.benchmark_output_dir(&args.full)?)?;
    let soak = verify_bundle(&root.benchmark_output_dir(&args.soak)?)?;
    validate_candidate_sources(&full, &soak)?;

    let baselines = candidate_baselines(&full, &soak)?;
    let out = root.create_new_benchmark_output_dir(&args.out)?;
    let candidate = toml::to_string_pretty(&BaselineCandidate {
        self_baselines: baselines,
    })
    .map_err(BundleError::SerializeCandidate)?;
    write_new(&out.join("baseline-candidate.toml"), candidate.as_bytes())?;
    println!(
        "[stab-bench] wrote self-regression candidate to {}",
        out.display()
    );
    Ok(())
}

pub(crate) fn release_check(root: &RepoRoot) -> Result<(), BenchError> {
    let loaded = super::load_suite(root)?;
    let pointer = CurrentEvidence::read(root)?;

    let full = verify_bundle(&evidence_bundle_path(root, &pointer.full_bundle)?)?;
    let soak = verify_bundle(&evidence_bundle_path(root, &pointer.soak_bundle)?)?;
    validate_candidate_sources(&full, &soak)?;
    validate_release_pair(&loaded.suite, &pointer, &full, &soak)?;
    validate_release_source(root, &pointer.measured_source_commit)?;
    println!(
        "[stab-bench] release evidence passes for measured source {}",
        pointer.measured_source_commit
    );
    Ok(())
}

fn validate_release_pair(
    current_suite: &Suite,
    pointer: &CurrentEvidence,
    full: &VerifiedBundle,
    soak: &VerifiedBundle,
) -> Result<(), BenchError> {
    if full.metadata.source_commit != pointer.measured_source_commit
        || soak.metadata.source_commit != pointer.measured_source_commit
    {
        return Err(BundleError::PointerSourceMismatch.into());
    }
    if full.metadata.host_before.architecture != "aarch64"
        || soak.metadata.host_before.architecture != "aarch64"
    {
        return Err(BundleError::NotAarch64.into());
    }
    let expected_cases = current_suite
        .cases()
        .map(|(_, _, id)| id)
        .collect::<Vec<_>>();
    if full.metadata.selected_cases != expected_cases
        || soak.metadata.selected_cases != expected_cases
    {
        return Err(BundleError::IncompleteCoverage.into());
    }
    let current_contract = current_suite
        .contract_digest()
        .map_err(BundleError::ContractDigest)?;
    let measured_contract = full
        .suite
        .contract_digest()
        .map_err(BundleError::ContractDigest)?;
    if current_contract != measured_contract
        || soak
            .suite
            .contract_digest()
            .map_err(BundleError::ContractDigest)?
            != measured_contract
    {
        return Err(BundleError::ContractMismatch.into());
    }
    if full.report.parity != GateOutcome::Passed
        || soak.report.parity != GateOutcome::Passed
        || full.report.memory != GateOutcome::Passed
        || soak.report.memory != GateOutcome::Passed
    {
        return Err(BundleError::ReleaseGates.into());
    }
    if full.suite.self_baselines.is_empty() {
        if full.report.self_regression != GateOutcome::Unseeded
            || soak.report.self_regression != GateOutcome::Unseeded
        {
            return Err(BundleError::FirstEvidenceSeeded.into());
        }
        let expected = candidate_baselines(full, soak)?;
        if current_suite.self_baselines != expected {
            return Err(BundleError::MissingFirstBaseline.into());
        }
    } else if full.report.self_regression != GateOutcome::Passed
        || soak.report.self_regression != GateOutcome::Passed
        || current_suite.self_baselines != full.suite.self_baselines
    {
        return Err(BundleError::SeededEvidence.into());
    }
    Ok(())
}

fn validate_release_source(root: &RepoRoot, measured_source: &str) -> Result<(), BenchError> {
    let source = super::execution::SourceIdentity::capture(root)?;
    if !source.clean {
        return Err(BundleError::DirtySource.into());
    }
    crate::process::run_checked_status(
        "git",
        ["merge-base", "--is-ancestor", measured_source, "HEAD"],
        &root.path,
    )?;
    let range = format!("{measured_source}..HEAD");
    if let Some(path) = changed_paths(&root.path, &range)?
        .iter()
        .find(|path| !release_descendant_path(path))
    {
        return Err(BundleError::NonEvidencePath { path: path.clone() }.into());
    }
    Ok(())
}

/// Every path a commit range touches. Rename detection is off, so a rename reports both its
/// source and its destination, and `-z` output is split on NUL so no path spelling can forge or
/// hide an entry.
fn changed_paths(repository: &Path, range: &str) -> Result<Vec<String>, BenchError> {
    let changed = super::commands::command_output(
        Path::new("git"),
        &[
            "diff",
            "--name-only",
            "--no-renames",
            "-z",
            "--diff-filter=ACDMRTUXB",
            range,
        ],
        b"",
        repository,
        1 << 20,
        None,
        60,
    )?;
    changed
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
        .map(|path| {
            String::from_utf8(path.to_vec())
                .map_err(|source| BundleError::NonUtf8DiffPath(source).into())
        })
        .collect()
}

fn release_descendant_path(path: &str) -> bool {
    path == CURRENT_EVIDENCE
        || path == "benchmarks/suite.toml"
        || path == "benchmarks/SUITE.md"
        || path == "README.md"
        || path == "benchmarks/README.md"
        || path == "docs/README.md"
        || path == "docs/plans/GOAL.md"
        || path == "docs/plans/stim-core-parity-and-lean-evidence-plan.md"
        || path.starts_with("benchmarks/evidence/aarch64/")
}

fn evidence_bundle_path(root: &RepoRoot, relative: &Path) -> Result<PathBuf, BenchError> {
    if relative.is_absolute()
        || relative
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(BundleError::AbnormalEvidencePath {
            path: relative.to_path_buf(),
        }
        .into());
    }
    let components = relative
        .components()
        .map(|component| component.as_os_str())
        .collect::<Vec<_>>();
    if components.len() < 5
        || components.first().copied() != Some("benchmarks".as_ref())
        || components.get(1).copied() != Some("evidence".as_ref())
        || components.get(2).copied() != Some("aarch64".as_ref())
    {
        return Err(BundleError::EvidenceOutsideRoot {
            path: relative.to_path_buf(),
        }
        .into());
    }
    let path = root.path.join(relative);
    let mut current = root.path.clone();
    for component in relative.components() {
        current.push(component.as_os_str());
        reject_symlink(&current)?;
    }
    Ok(path)
}

fn reject_symlink(path: &Path) -> Result<(), BundleError> {
    let metadata = fs::symlink_metadata(path).map_err(|source| BundleError::Inspect {
        path: path.to_path_buf(),
        source,
    })?;
    if metadata.file_type().is_symlink() {
        return Err(BundleError::EvidenceSymlink {
            path: path.to_path_buf(),
        });
    }
    Ok(())
}

fn candidate_baselines(
    full: &VerifiedBundle,
    soak: &VerifiedBundle,
) -> Result<Vec<SelfBaseline>, BundleError> {
    let soak_cases = soak
        .report
        .cases
        .iter()
        .map(|case| (case.case_id.as_str(), case))
        .collect::<BTreeMap<_, _>>();
    let mut baselines = Vec::new();
    for full_case in &full.report.cases {
        let soak_case =
            soak_cases
                .get(full_case.case_id.as_str())
                .ok_or_else(|| BundleError::SoakOmits {
                    case: full_case.case_id.clone(),
                })?;
        if full_case.case_digest != soak_case.case_digest {
            return Err(BundleError::CandidateDigest {
                case: full_case.case_id.clone(),
            });
        }
        baselines.push(SelfBaseline {
            architecture: full.metadata.host_before.architecture.clone(),
            cpu_model: full.metadata.host_before.cpu_model.clone(),
            rustc: full.metadata.rustc.clone(),
            target: full.metadata.target.clone(),
            timing_boundary: TIMING_BOUNDARY.to_string(),
            case_id: full_case.case_id.clone(),
            case_digest: full_case.case_digest.clone(),
            median_seconds_per_work: full_case
                .timing
                .stab_seconds_per_work
                .median
                .max(soak_case.timing.stab_seconds_per_work.median),
            upper_seconds_per_work: full_case
                .timing
                .stab_seconds_per_work
                .confidence_upper
                .max(soak_case.timing.stab_seconds_per_work.confidence_upper),
        });
    }
    if baselines.len() != soak.report.cases.len() {
        return Err(BundleError::CandidateCaseSets);
    }
    Ok(baselines)
}

pub(super) fn publish_bundle(
    directory: &Path,
    suite_bytes: &[u8],
    metadata: &RunMetadata,
    correctness: &CorrectnessReport,
    samples: &RawSamples,
    report: &DerivedReport,
) -> Result<(), BenchError> {
    write_new(&directory.join("suite.toml"), suite_bytes)?;
    write_json_new(&directory.join("run.json"), metadata)?;
    write_json_new(&directory.join("correctness.json"), correctness)?;
    write_json_new(&directory.join("samples.json"), samples)?;
    write_json_new(&directory.join("report.json"), report)?;
    write_new(&directory.join("report.md"), markdown(report).as_bytes())?;
    let files = BUNDLE_CONTENTS
        .iter()
        .map(|name| Ok(((*name).to_string(), file_sha256(&directory.join(name))?)))
        .collect::<Result<BTreeMap<_, _>, BenchError>>()?;
    write_json_new(
        &directory.join("replay.json"),
        &ReplayManifest {
            schema_version: REPLAY_SCHEMA_VERSION,
            files,
        },
    )
}

pub(super) fn write_new(path: &Path, bytes: &[u8]) -> Result<(), BenchError> {
    use std::io::Write as _;

    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|source| BundleError::Create {
            path: path.to_path_buf(),
            source,
        })?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|source| {
            BundleError::Write {
                path: path.to_path_buf(),
                source,
            }
            .into()
        })
}

struct VerifiedBundle {
    suite: Suite,
    metadata: RunMetadata,
    report: DerivedReport,
}

fn verify_bundle(directory: &Path) -> Result<VerifiedBundle, BenchError> {
    validate_directory_entries(directory)?;
    let replay = read_json::<ReplayManifest>(&directory.join("replay.json"))?;
    validate_replay_files(directory, &replay)?;
    let suite_bytes = fs::read(directory.join("suite.toml")).map_err(BundleError::ReadSuite)?;
    let suite_text = std::str::from_utf8(&suite_bytes).map_err(BundleError::SuiteUtf8)?;
    let suite = toml::from_str::<Suite>(suite_text).map_err(BundleError::SuiteToml)?;
    suite.validate().map_err(BundleError::from)?;
    let metadata = read_json::<RunMetadata>(&directory.join("run.json"))?;
    let correctness = read_json::<CorrectnessReport>(&directory.join("correctness.json"))?;
    let samples = read_json::<RawSamples>(&directory.join("samples.json"))?;
    let expected = read_json::<DerivedReport>(&directory.join("report.json"))?;
    let suite_digest = hex::encode(Sha256::digest(&suite_bytes));
    if suite_digest != metadata.suite_sha256 {
        return Err(BundleError::SuiteDigest.into());
    }
    validate_metadata(&suite, &metadata)?;
    validate_correctness(&metadata, &correctness)?;
    let actual = DerivedReport::derive(&suite, &metadata, &correctness, &samples)
        .map_err(BundleError::from)?;
    if actual != expected {
        return Err(BundleError::ReportDiffers.into());
    }
    if json_bytes(&actual)?
        != fs::read(directory.join("report.json")).map_err(|source| BundleError::ReadMember {
            name: "report.json",
            source,
        })?
    {
        return Err(BundleError::NonCanonicalReport.into());
    }
    if markdown(&actual)
        != fs::read_to_string(directory.join("report.md")).map_err(|source| {
            BundleError::ReadMember {
                name: "report.md",
                source,
            }
        })?
    {
        return Err(BundleError::MarkdownDiffers.into());
    }
    Ok(VerifiedBundle {
        suite,
        metadata,
        report: actual,
    })
}

fn validate_metadata(suite: &Suite, metadata: &RunMetadata) -> Result<(), BenchError> {
    if metadata.stim_commit != suite.stim.commit {
        return Err(BundleError::WrongStimCommit.into());
    }
    let expected_formal = metadata.tier != Tier::Smoke;
    if metadata.formal != expected_formal || (metadata.formal && !metadata.source_clean) {
        return Err(BundleError::FormalStatus.into());
    }
    for (name, value, length) in [
        ("source commit", metadata.source_commit.as_str(), 40),
        ("Stim commit", metadata.stim_commit.as_str(), 40),
        ("Stim binary", metadata.stim_binary_sha256.as_str(), 64),
        ("Stab binary", metadata.stab_binary_sha256.as_str(), 64),
        (
            "benchmark binary",
            metadata.bench_binary_sha256.as_str(),
            64,
        ),
    ] {
        if value.len() != length || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(BundleError::InvalidIdentity(name).into());
        }
    }
    if metadata.rustc.is_empty() || metadata.target.is_empty() {
        return Err(BundleError::MissingToolchain.into());
    }
    validate_recorded_host_transition(
        &metadata.host_before,
        &metadata.host_after,
        suite.policy.maximum_temperature_millidegrees,
        metadata.formal,
    )
    .map_err(|source| BundleError::from(source).into())
}

fn validate_candidate_sources(
    full: &VerifiedBundle,
    soak: &VerifiedBundle,
) -> Result<(), BenchError> {
    if full.metadata.tier != Tier::Full || soak.metadata.tier != Tier::Soak {
        return Err(BundleError::CandidateTiers.into());
    }
    if !full.metadata.formal
        || !soak.metadata.formal
        || !full.metadata.source_clean
        || !soak.metadata.source_clean
    {
        return Err(BundleError::CandidateFormal.into());
    }
    for (name, left, right) in [
        (
            "suite",
            &full.metadata.suite_sha256,
            &soak.metadata.suite_sha256,
        ),
        (
            "source",
            &full.metadata.source_commit,
            &soak.metadata.source_commit,
        ),
        (
            "Stim commit",
            &full.metadata.stim_commit,
            &soak.metadata.stim_commit,
        ),
        (
            "Stim binary",
            &full.metadata.stim_binary_sha256,
            &soak.metadata.stim_binary_sha256,
        ),
        (
            "Stab binary",
            &full.metadata.stab_binary_sha256,
            &soak.metadata.stab_binary_sha256,
        ),
        (
            "benchmark binary",
            &full.metadata.bench_binary_sha256,
            &soak.metadata.bench_binary_sha256,
        ),
        ("rustc", &full.metadata.rustc, &soak.metadata.rustc),
        ("target", &full.metadata.target, &soak.metadata.target),
        (
            "architecture",
            &full.metadata.host_before.architecture,
            &soak.metadata.host_before.architecture,
        ),
        (
            "CPU",
            &full.metadata.host_before.cpu_model,
            &soak.metadata.host_before.cpu_model,
        ),
        (
            "kernel",
            &full.metadata.host_before.kernel_release,
            &soak.metadata.host_before.kernel_release,
        ),
    ] {
        if left != right {
            return Err(BundleError::CandidateIdentity(name).into());
        }
    }
    if full.metadata.host_before.affinity_cpu != soak.metadata.host_before.affinity_cpu
        || full.metadata.selected_cases != soak.metadata.selected_cases
    {
        return Err(BundleError::CandidateSelection.into());
    }
    if !eligible_for_baseline(&full.report) || !eligible_for_baseline(&soak.report) {
        return Err(BundleError::CandidateGates.into());
    }
    Ok(())
}

fn eligible_for_baseline(report: &DerivedReport) -> bool {
    matches!(
        report.parity,
        GateOutcome::Passed | GateOutcome::NotApplicable
    ) && matches!(
        report.self_regression,
        GateOutcome::Passed | GateOutcome::Unseeded
    ) && report.memory == GateOutcome::Passed
}

fn validate_directory_entries(directory: &Path) -> Result<(), BenchError> {
    let entry_error = |source| BundleError::DirectoryEntry {
        path: directory.to_path_buf(),
        source,
    };
    let actual = fs::read_dir(directory)
        .map_err(|source| BundleError::ReadDirectory {
            path: directory.to_path_buf(),
            source,
        })?
        .map(|entry| {
            let entry = entry.map_err(entry_error)?;
            let file_type = entry.file_type().map_err(entry_error)?;
            if !file_type.is_file() || file_type.is_symlink() {
                return Err(BundleError::IrregularMember { path: entry.path() });
            }
            entry
                .file_name()
                .into_string()
                .map_err(|name| BundleError::NonUtf8Member { name })
        })
        .collect::<Result<BTreeSet<_>, _>>()?;
    let expected = BUNDLE_CONTENTS
        .iter()
        .copied()
        .chain(["replay.json"])
        .map(str::to_string)
        .collect::<BTreeSet<_>>();
    if actual != expected {
        return Err(BundleError::DirectoryEntries.into());
    }
    Ok(())
}

fn validate_replay_files(directory: &Path, manifest: &ReplayManifest) -> Result<(), BenchError> {
    if manifest.schema_version != REPLAY_SCHEMA_VERSION {
        return Err(BundleError::ReplaySchema {
            actual: manifest.schema_version,
            expected: REPLAY_SCHEMA_VERSION,
        }
        .into());
    }
    let expected = BUNDLE_CONTENTS.iter().copied().collect::<BTreeSet<_>>();
    let actual = manifest
        .files
        .keys()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    if actual != expected {
        return Err(BundleError::ManifestFiles.into());
    }
    for (name, digest) in &manifest.files {
        if file_sha256(&directory.join(name))? != *digest {
            return Err(BundleError::ReplayDigest { file: name.clone() }.into());
        }
    }
    Ok(())
}

fn validate_correctness(
    metadata: &RunMetadata,
    correctness: &CorrectnessReport,
) -> Result<(), BenchError> {
    if correctness.schema_version != CORRECTNESS_SCHEMA_VERSION {
        return Err(BundleError::CorrectnessSchema {
            actual: correctness.schema_version,
        }
        .into());
    }
    let expected = metadata
        .selected_cases
        .iter()
        .map(super::model::CaseId::as_str)
        .collect::<BTreeSet<_>>();
    let actual = correctness
        .cases
        .iter()
        .map(|case| case.case_id.as_str())
        .collect::<BTreeSet<_>>();
    if actual != expected || actual.len() != correctness.cases.len() {
        return Err(BundleError::CorrectnessCases.into());
    }
    for case in &correctness.cases {
        if case.stab_outputs.is_empty() {
            return Err(BundleError::NoStabWitness {
                case: case.case_id.clone(),
            }
            .into());
        }
    }
    Ok(())
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, BenchError> {
    let bytes = fs::read(path).map_err(|source| BundleError::ReadJson {
        path: path.to_path_buf(),
        source,
    })?;
    serde_json::from_slice(&bytes).map_err(|source| {
        BundleError::ParseJson {
            path: path.to_path_buf(),
            source,
        }
        .into()
    })
}

fn write_json_new(path: &Path, value: &impl Serialize) -> Result<(), BenchError> {
    write_new(path, &json_bytes(value)?)
}

fn json_bytes(value: &impl Serialize) -> Result<Vec<u8>, BenchError> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    Ok(bytes)
}

pub(super) fn file_sha256(path: &Path) -> Result<String, BenchError> {
    let bytes = fs::read(path).map_err(|source| BundleError::ReadForDigest {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(hex::encode(Sha256::digest(bytes)))
}

const REPLAY_SCHEMA_VERSION: u16 = 1;
const BUNDLE_CONTENTS: [&str; 6] = [
    "suite.toml",
    "run.json",
    "correctness.json",
    "samples.json",
    "report.json",
    "report.md",
];

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReplayManifest {
    schema_version: u16,
    files: BTreeMap<String, String>,
}

#[derive(Serialize)]
struct BaselineCandidate {
    self_baselines: Vec<SelfBaseline>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CurrentEvidence {
    schema_version: u16,
    measured_source_commit: String,
    full_bundle: PathBuf,
    soak_bundle: PathBuf,
}

impl CurrentEvidence {
    /// Reads and validates the regular-file evidence pointer.
    fn read(root: &RepoRoot) -> Result<Self, BundleError> {
        let pointer_path = root.path.join(CURRENT_EVIDENCE);
        match fs::symlink_metadata(&pointer_path) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(BundleError::PointerSymlink {
                    pointer: CURRENT_EVIDENCE,
                });
            }
            Ok(metadata) if !metadata.is_file() => {
                return Err(BundleError::PointerNotFile {
                    pointer: CURRENT_EVIDENCE,
                });
            }
            Ok(_) => {}
            Err(source) => {
                return Err(BundleError::PointerUnavailable {
                    pointer: CURRENT_EVIDENCE,
                    source,
                });
            }
        }
        let pointer_bytes =
            fs::read(&pointer_path).map_err(|source| BundleError::PointerUnavailable {
                pointer: CURRENT_EVIDENCE,
                source,
            })?;
        let pointer = toml::from_slice::<CurrentEvidence>(&pointer_bytes)
            .map_err(BundleError::PointerToml)?;
        pointer.validate()?;
        Ok(pointer)
    }

    fn validate(&self) -> Result<(), BundleError> {
        if self.schema_version != 1
            || self.measured_source_commit.len() != 40
            || !self
                .measured_source_commit
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
            || self.full_bundle == self.soak_bundle
        {
            return Err(BundleError::PointerSchema);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
