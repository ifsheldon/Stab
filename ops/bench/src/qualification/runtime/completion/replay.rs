use std::collections::BTreeMap;
use std::path::Path;

use super::{
    COMPLETION_SCHEMA_VERSION, CompletionError, CompletionManifest, CompletionReportArgs,
    CompletionReportValidation, DirectQualificationArtifactPath, LEGACY_COMPLETION_SCHEMA_VERSIONS,
    MAX_COMPLETION_MARKDOWN_BYTES, MAX_COMPLETION_PREFLIGHT_BYTES, MAX_COMPLETION_REPORT_BYTES,
    ReconstructedCompletion, ReplayedCompletion, RepositoryBinding, RepositoryEvidence,
    RetainedArtifactContext, canonical_json, completion_preflight, legacy, parse_canonical,
    read_completion_artifact, reconstruct, render_markdown, schema_version, scope, sha256_hex,
    validate_manifest_boundary,
};
use crate::root::RepoRoot;

pub(in crate::qualification::runtime) fn run_report_with_repository(
    root: &RepoRoot,
    source_root: &RepoRoot,
    repository: &RepositoryBinding,
    expected_performance_inventory_sha256: &str,
    expected_correctness_inventory_sha256: &str,
    args: CompletionReportArgs,
) -> Result<CompletionReportValidation, CompletionError> {
    let input = DirectQualificationArtifactPath::try_new(&args.input)?;
    let initial_report_json = read_completion_artifact(
        root,
        repository,
        &input,
        "report.json",
        MAX_COMPLETION_REPORT_BYTES,
    )?;
    let schema_version = schema_version(&initial_report_json)?;
    if LEGACY_COMPLETION_SCHEMA_VERSIONS.contains(&schema_version) {
        return validate_legacy(input, schema_version, &initial_report_json);
    }
    if schema_version != COMPLETION_SCHEMA_VERSION {
        return Err(CompletionError::SchemaVersion(schema_version));
    }

    let manifest: CompletionManifest = parse_canonical(&initial_report_json)?;
    let scope = scope::load(
        source_root,
        expected_performance_inventory_sha256,
        &manifest.scope_id,
    )?;
    validate_manifest_boundary(
        &manifest,
        input.as_path(),
        expected_performance_inventory_sha256,
        expected_correctness_inventory_sha256,
        &scope,
    )?;
    let rollup_paths = manifest
        .rollups
        .iter()
        .map(|rollup| DirectQualificationArtifactPath::try_new(Path::new(&rollup.artifact.path)))
        .collect::<Result<Vec<_>, _>>()?;
    let mut reconstructed = reconstruct(
        root,
        source_root,
        repository,
        expected_performance_inventory_sha256,
        expected_correctness_inventory_sha256,
        &manifest.scope_id,
        &input,
        &rollup_paths,
        manifest.generated_unix_epoch_seconds,
    )?;
    let artifact_context = RetainedArtifactContext::open(root, repository)?;
    let (completion_binding, mut artifacts) = artifact_context.read_and_bind(
        root,
        &input,
        &[
            ("report.json", MAX_COMPLETION_REPORT_BYTES),
            ("preflight.json", MAX_COMPLETION_PREFLIGHT_BYTES),
            ("report.md", MAX_COMPLETION_MARKDOWN_BYTES),
        ],
    )?;
    let report_json = take_artifact(&mut artifacts, "report.json")?;
    let preflight_json = take_artifact(&mut artifacts, "preflight.json")?;
    let markdown = take_artifact(&mut artifacts, "report.md")?;
    if report_json != initial_report_json || !artifacts.is_empty() {
        return Err(CompletionError::SourceMutation);
    }
    reconstructed.bind_source_artifacts(root, &artifact_context)?;
    require_reconstructed_artifacts(&reconstructed, &report_json, &preflight_json, &markdown)?;
    reconstructed.require_sources_current(root)?;
    completion_binding.require_current(root)?;
    require_final_repository_state(root, repository, &reconstructed.manifest.repository)?;

    Ok(CompletionReportValidation::Replayed(ReplayedCompletion {
        path: input.into_path_buf(),
        report_json,
        _artifact_binding: completion_binding,
    }))
}

fn require_final_repository_state(
    root: &RepoRoot,
    repository: &RepositoryBinding,
    expected: &RepositoryEvidence,
) -> Result<(), CompletionError> {
    let current = super::super::run::bound_repository_state(root, repository)?;
    super::super::run::require_current_repository_state(&current, expected)?;
    Ok(())
}

fn validate_legacy(
    input: DirectQualificationArtifactPath,
    schema_version: u32,
    report_json: &[u8],
) -> Result<CompletionReportValidation, CompletionError> {
    let summary = match schema_version {
        1 => legacy::parse_v1(report_json)?,
        2 => legacy::parse_v2(report_json)?,
        _ => return Err(CompletionError::SchemaVersion(schema_version)),
    };
    if Path::new(&summary.output) != input.as_path() {
        return Err(CompletionError::OutputBinding);
    }
    Ok(CompletionReportValidation::HistoricalReadable {
        path: input.into_path_buf(),
        schema_version,
    })
}

fn take_artifact(
    artifacts: &mut BTreeMap<&'static str, Vec<u8>>,
    name: &'static str,
) -> Result<Vec<u8>, CompletionError> {
    artifacts
        .remove(name)
        .ok_or(CompletionError::SourceMutation)
}

fn require_reconstructed_artifacts(
    reconstructed: &ReconstructedCompletion,
    report_json: &[u8],
    preflight_json: &[u8],
    markdown: &[u8],
) -> Result<(), CompletionError> {
    let reconstructed_json = canonical_json(&reconstructed.manifest)?;
    let reconstructed_preflight = canonical_json(&completion_preflight(
        &reconstructed.manifest,
        &reconstructed_json,
    ))?;
    let reconstructed_markdown =
        render_markdown(&reconstructed.manifest, &sha256_hex(&reconstructed_json));
    if reconstructed_json != report_json
        || reconstructed_preflight != preflight_json
        || reconstructed_markdown.as_bytes() != markdown
    {
        return Err(CompletionError::Reconstruction);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::process::Command;

    use super::*;

    fn git(root: &Path, args: &[&str]) {
        let status = Command::new("/usr/bin/git")
            .args(args)
            .current_dir(root)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_AUTHOR_NAME", "Stab Test")
            .env("GIT_AUTHOR_EMAIL", "stab-test@example.invalid")
            .env("GIT_COMMITTER_NAME", "Stab Test")
            .env("GIT_COMMITTER_EMAIL", "stab-test@example.invalid")
            .status()
            .expect("git command");
        assert!(status.success());
    }

    #[test]
    fn final_repository_state_rejects_late_tracked_mutation() {
        let repository = tempfile::tempdir().expect("temporary repository");
        std::fs::write(repository.path().join("tracked.txt"), b"clean\n")
            .expect("write tracked fixture");
        git(repository.path(), &["init", "--quiet"]);
        git(repository.path(), &["add", "tracked.txt"]);
        git(repository.path(), &["commit", "--quiet", "-m", "fixture"]);
        let root = RepoRoot::resolve(repository.path()).expect("resolve repository");
        let live_repository = RepositoryBinding::open(&root).expect("bind repository");
        let clean = super::super::super::run::bound_repository_state(&root, &live_repository)
            .expect("clean repository state");
        let expected = RepositoryEvidence {
            commit_before: clean.commit.clone(),
            commit_after: clean.commit,
            local_modifications_before: false,
            local_modifications_after: false,
        };
        require_final_repository_state(&root, &live_repository, &expected)
            .expect("clean repository remains current");

        std::fs::write(repository.path().join("tracked.txt"), b"late mutation\n")
            .expect("mutate tracked fixture");
        assert!(matches!(
            require_final_repository_state(&root, &live_repository, &expected),
            Err(CompletionError::Artifact(
                super::super::super::artifact::ArtifactError::ExternalSourceChanged(
                    "repository state"
                )
            ))
        ));
    }
}
