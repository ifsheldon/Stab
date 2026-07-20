use super::*;

fn run_git(repository: &Path, arguments: &[&str]) {
    let status = std::process::Command::new("/usr/bin/git")
        .current_dir(repository)
        .args(arguments)
        .status()
        .expect("run git");
    assert!(status.success(), "git command failed: {arguments:?}");
}

fn bound_correctness_tree(
    repository: &tempfile::TempDir,
) -> (
    Arc<crate::qualification::runtime::correctness::CorrectnessArtifactBinding>,
    PathBuf,
) {
    let output = repository.path().join("correctness-source");
    let case = output.join("cases/case-a");
    std::fs::create_dir_all(&case).expect("create correctness case");
    for name in [
        "completion.json",
        "preflight.json",
        "report.json",
        "report.md",
        "request.json",
    ] {
        std::fs::write(output.join(name), format!("{name}\n")).expect("write correctness artifact");
    }
    let receipt = case.join("execution-receipt.json");
    std::fs::write(&receipt, b"receipt\n").expect("write correctness receipt");
    let binding =
        crate::qualification::runtime::correctness::bind_test_artifact_tree(&output, &["case-a"])
            .expect("bind correctness tree");
    (Arc::new(binding), receipt)
}

#[test]
fn completion_publication_rolls_back_when_bound_correctness_changes_after_exchange() {
    let repository = tempfile::tempdir().expect("temporary repository");
    let root = RepoRoot::resolve(repository.path()).expect("repository root");
    let live_repository = RepositoryBinding::open(&root).expect("bind repository");
    let (correctness_binding, receipt_path) = bound_correctness_tree(&repository);
    correctness_binding
        .require_current()
        .expect("correctness binding is current before publication");
    let correctness_bindings = vec![correctness_binding];
    let output_path = DirectQualificationArtifactPath::try_new(Path::new(
        "target/benchmarks/qualification/completion-correctness-race",
    ))
    .expect("completion output path");
    let mut completion = receipt();
    completion.source_reports.clear();
    completion.rollups.clear();
    let mut validations = 0;

    let result = (CompletionPublication {
        root: &root,
        repository: &live_repository,
        output_path: &output_path,
        receipt: &completion,
        report_json: b"completion report\n",
        preflight_json: b"completion preflight\n",
        markdown: "completion markdown\n",
        existing_report_json: None,
        existing_preflight_json: None,
        existing_markdown: None,
        correctness_bindings: &correctness_bindings,
    })
    .publish_production_with(|repository_binding| {
        repository_binding.require_current(&root)?;
        validations += 1;
        if validations == 2 {
            std::fs::write(&receipt_path, b"changed\n")
                .map_err(crate::qualification::runtime::artifact::ArtifactError::Write)?;
        }
        require_current_correctness(&correctness_bindings)
    });

    assert!(matches!(
        result,
        Err(CompletionError::Artifact(
            crate::qualification::runtime::artifact::ArtifactError::ExternalSourceChanged(
                "completion source evidence"
            )
        ))
    ));
    assert_eq!(validations, 2);
    assert!(!repository.path().join(output_path.as_path()).exists());
}

#[test]
fn completion_replay_reports_old_tree_cleanup_failure() {
    let repository = tempfile::tempdir().expect("temporary repository");
    let root = RepoRoot::resolve(repository.path()).expect("repository root");
    let live_repository = RepositoryBinding::open(&root).expect("bind repository");
    let output_path = Path::new("target/benchmarks/qualification/completion-cleanup");
    publish_directory(
        &root,
        output_path,
        b"old report\n",
        b"old preflight\n",
        b"old markdown\n",
    );
    let output_path =
        DirectQualificationArtifactPath::try_new(output_path).expect("completion output path");
    let mut completion = receipt();
    completion.source_reports.clear();
    completion.rollups.clear();
    let result = CompletionPublication {
        root: &root,
        repository: &live_repository,
        output_path: &output_path,
        receipt: &completion,
        report_json: b"new report\n",
        preflight_json: b"new preflight\n",
        markdown: "new markdown\n",
        existing_report_json: Some(b"old report\n"),
        existing_preflight_json: Some(b"old preflight\n"),
        existing_markdown: Some(b"old markdown\n"),
        correctness_bindings: &[],
    }
    .publish_with(
        || Ok(()),
        |output| {
            output
                .commit_with_cleanup(|_, _, _| {
                    Err(
                        crate::qualification::runtime::artifact::ArtifactError::DirectoryIdentity(
                            "injected cleanup failure",
                        ),
                    )
                })
                .map_err(CompletionError::Artifact)
        },
    );
    assert!(matches!(
        result,
        Err(CompletionError::Artifact(
            crate::qualification::runtime::artifact::ArtifactError::DirectoryIdentity(
                "injected cleanup failure"
            )
        ))
    ));
    assert_eq!(
        std::fs::read(
            repository
                .path()
                .join(output_path.as_path())
                .join("report.json")
        )
        .expect("read published completion"),
        b"new report\n"
    );
}

#[test]
fn completion_actions_use_retained_root_during_swap_and_restore() {
    let parent = tempfile::tempdir().expect("temporary parent");
    let repository = parent.path().join("repository");
    std::fs::create_dir(&repository).expect("create repository");
    std::fs::write(repository.join("sentinel"), b"retained\n").expect("write sentinel");
    run_git(&repository, &["init", "--quiet"]);
    run_git(&repository, &["add", "sentinel"]);
    run_git(
        &repository,
        &[
            "-c",
            "user.name=Stab Test",
            "-c",
            "user.email=stab@example.invalid",
            "commit",
            "--quiet",
            "-m",
            "initial",
        ],
    );
    let root = RepoRoot::resolve(&repository).expect("repository root");
    let live_repository = RepositoryBinding::open(&root).expect("bind repository");
    let expected = crate::qualification::runtime::git::repository_state(&root)
        .expect("repository state")
        .commit;
    let detached = parent.path().join("detached-repository");
    let replacement = parent.path().join("replacement-repository");

    checked_action(
        &root,
        &live_repository,
        &expected,
        "root swap test",
        |source_root| {
            std::fs::rename(&repository, &detached).expect("detach repository");
            std::fs::create_dir(&repository).expect("create replacement repository");
            std::fs::write(repository.join("sentinel"), b"replacement\n")
                .expect("write replacement sentinel");

            assert_eq!(
                std::fs::read(source_root.path.join("sentinel")).expect("read retained sentinel"),
                b"retained\n"
            );
            let state = crate::qualification::runtime::git::repository_state(source_root)?;
            assert_eq!(state.commit, expected);

            std::fs::rename(&repository, &replacement).expect("move replacement repository");
            std::fs::rename(&detached, &repository).expect("restore repository");
            Ok::<(), crate::qualification::runtime::git::GitError>(())
        },
    )
    .expect("run descriptor-root action");
}
