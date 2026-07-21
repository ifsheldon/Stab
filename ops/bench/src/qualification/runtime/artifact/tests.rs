use super::*;

fn direct(path: &Path) -> DirectQualificationArtifactPath {
    DirectQualificationArtifactPath::try_new(path).expect("direct qualification artifact path")
}

fn begin_output(root: &RepoRoot, path: &Path) -> Result<QualificationOutput, ArtifactError> {
    QualificationOutput::begin(root, &direct(path))
}

fn begin_new_output(root: &RepoRoot, path: &Path) -> Result<QualificationOutput, ArtifactError> {
    QualificationOutput::begin_new(root, &direct(path))
}

fn read_output_artifact(
    root: &RepoRoot,
    path: &Path,
    name: &'static str,
) -> Result<Vec<u8>, ArtifactError> {
    read_artifact(root, &direct(path), name)
}

fn read_output_artifact_bounded(
    root: &RepoRoot,
    path: &Path,
    name: &'static str,
    maximum_bytes: usize,
) -> Result<Vec<u8>, ArtifactError> {
    read_artifact_bounded(root, &direct(path), name, maximum_bytes)
}

fn overwrite_artifact(
    directory: &OwnedFd,
    name: &'static str,
    bytes: &[u8],
) -> Result<(), ArtifactError> {
    let descriptor = rustix::fs::openat(
        directory,
        name,
        rustix::fs::OFlags::WRONLY
            | rustix::fs::OFlags::CLOEXEC
            | rustix::fs::OFlags::TRUNC
            | rustix::fs::OFlags::NOFOLLOW,
        rustix::fs::Mode::empty(),
    )
    .map_err(ArtifactError::Io)?;
    let mut file = std::fs::File::from(descriptor);
    file.write_all(bytes).map_err(ArtifactError::Write)?;
    file.sync_all().map_err(ArtifactError::Write)
}

#[test]
fn output_path_rejects_escape_and_shallow_targets() {
    assert!(validate_output(Path::new("target/benchmarks/qualification/pr")).is_ok());
    assert!(validate_output(Path::new("target/benchmarks/qualification")).is_err());
    assert!(validate_output(Path::new("target/benchmarks/qualification/../outside")).is_err());
    assert!(validate_output(Path::new("/tmp/qualification")).is_err());
    assert!(
        DirectQualificationArtifactPath::try_new(Path::new("target/benchmarks/qualification/pr"))
            .is_ok()
    );
    for unsafe_name in [
        "nested/pr",
        ".publication.lock",
        ".run-1-0.staging",
        "bad|row",
        "bad`code",
        "bad\nrow",
    ] {
        let path = PathBuf::from("target/benchmarks/qualification").join(unsafe_name);
        assert!(DirectQualificationArtifactPath::try_new(&path).is_err());
    }
}

#[test]
fn atomically_replaces_known_artifacts_without_leaving_staging_directories() {
    let repository = tempfile::tempdir().expect("temporary repository");
    let root = RepoRoot::resolve(repository.path()).expect("resolve repository");
    let output = Path::new("target/benchmarks/qualification/test-run");

    let mut first = begin_output(&root, output).expect("begin first publication");
    first
        .write("report.json", b"first\n")
        .expect("write first report");
    first.commit().expect("publish first report");

    let mut second = begin_output(&root, output).expect("begin replacement");
    second
        .write("report.json", b"second\n")
        .expect("write replacement report");
    second.commit().expect("publish replacement");

    assert_eq!(
        read_output_artifact(&root, output, "report.json").expect("read replacement"),
        b"second\n"
    );
    let parent = repository.path().join("target/benchmarks/qualification");
    let staging = std::fs::read_dir(parent)
        .expect("read publication parent")
        .filter_map(Result::ok)
        .any(|entry| entry.file_name().to_string_lossy().ends_with(".staging"));
    assert!(!staging, "successful replacement left a staging directory");
}

#[test]
fn producer_publication_refuses_to_replace_existing_evidence() {
    let repository = tempfile::tempdir().expect("temporary repository");
    let root = RepoRoot::resolve(repository.path()).expect("resolve repository");
    let output = Path::new("target/benchmarks/qualification/append-only-run");

    let mut first = begin_output(&root, output).expect("begin first publication");
    first
        .write("report.json", b"first\n")
        .expect("write first report");
    first.commit_new().expect("publish first report");

    let mut second = begin_output(&root, output).expect("begin second publication");
    second
        .write("report.json", b"second\n")
        .expect("write second report");
    assert!(matches!(
        second.commit_new(),
        Err(ArtifactError::OutputAlreadyExists(path)) if path == output
    ));
    assert_eq!(
        read_output_artifact(&root, output, "report.json").expect("read retained report"),
        b"first\n"
    );
}

#[test]
fn producer_begin_rejects_an_existing_output_before_work() {
    let repository = tempfile::tempdir().expect("temporary repository");
    let root = RepoRoot::resolve(repository.path()).expect("resolve repository");
    let output = Path::new("target/benchmarks/qualification/append-only-preflight");

    let mut first = begin_new_output(&root, output).expect("begin first producer");
    first
        .write("report.json", b"retained\n")
        .expect("write retained report");
    first.commit_new().expect("publish retained report");

    assert!(matches!(
        begin_new_output(&root, output),
        Err(ArtifactError::OutputAlreadyExists(path)) if path == output
    ));
    assert_eq!(
        read_output_artifact(&root, output, "report.json").expect("read retained report"),
        b"retained\n"
    );
}

#[test]
fn failed_write_abort_removes_the_bound_staging_tree() {
    let repository = tempfile::tempdir().expect("temporary repository");
    let root = RepoRoot::resolve(repository.path()).expect("resolve repository");
    let output = Path::new("target/benchmarks/qualification/failed-write-abort");
    let mut publication = begin_new_output(&root, output).expect("begin publication");
    publication
        .write("report.json", b"partial\n")
        .expect("stage artifact before simulated write failure");
    let staging = repository
        .path()
        .join("target/benchmarks/qualification")
        .join(&publication.staging_name);

    let error = publication.handle_write_failure(ArtifactError::Write(std::io::Error::other(
        "injected write failure",
    )));

    assert!(matches!(error, ArtifactError::Write(_)));
    assert!(!staging.exists());
    assert!(!publication.staging_active);
}

#[test]
fn failed_write_abort_reports_staging_cleanup_failure() {
    let repository = tempfile::tempdir().expect("temporary repository");
    let root = RepoRoot::resolve(repository.path()).expect("resolve repository");
    let output = Path::new("target/benchmarks/qualification/failed-write-cleanup-error");
    let mut publication = begin_new_output(&root, output).expect("begin publication");
    publication
        .write("report.json", b"partial\n")
        .expect("stage artifact before simulated write failure");
    let parent = repository.path().join("target/benchmarks/qualification");
    let staging = parent.join(&publication.staging_name);
    let detached = parent.join("detached-failed-write-staging");
    std::fs::rename(&staging, &detached).expect("detach bound staging directory");
    std::fs::create_dir(&staging).expect("replace staging directory");
    let write = ArtifactError::Write(std::io::Error::other("injected write failure"));

    let error = publication.handle_write_failure(write);

    assert!(matches!(
        error,
        ArtifactError::WriteCleanup { write, cleanup }
            if matches!(*write, ArtifactError::Write(_))
                && matches!(*cleanup, ArtifactError::DirectoryIdentity(_))
    ));
    assert!(detached.join("report.json").exists());
}

#[test]
fn absence_admission_does_not_create_the_output_hierarchy() {
    let repository = tempfile::tempdir().expect("temporary repository");
    let root = RepoRoot::resolve(repository.path()).expect("resolve repository");
    let output = direct(Path::new(
        "target/benchmarks/qualification/read-only-admission",
    ));

    QualificationOutput::require_absent(&root, &output).expect("admit absent output");

    assert!(!repository.path().join("target").exists());
}

#[test]
fn cleanup_failure_is_reported_without_reverting_durable_replacement() {
    let repository = tempfile::tempdir().expect("temporary repository");
    let root = RepoRoot::resolve(repository.path()).expect("resolve repository");
    let output = Path::new("target/benchmarks/qualification/cleanup-failure");

    let mut first = begin_output(&root, output).expect("begin first publication");
    first
        .write("report.json", b"first\n")
        .expect("write first report");
    first.commit().expect("publish first report");

    let mut second = begin_output(&root, output).expect("begin replacement");
    second
        .write("report.json", b"second\n")
        .expect("write replacement report");
    let staging_name = second.staging_name.clone();
    assert!(matches!(
        second.commit_with_cleanup(|_, _, _| {
            Err(ArtifactError::DirectoryIdentity("injected cleanup failure"))
        }),
        Err(ArtifactError::DirectoryIdentity("injected cleanup failure"))
    ));

    assert_eq!(
        read_output_artifact(&root, output, "report.json").expect("read replacement"),
        b"second\n"
    );
    assert!(
        repository
            .path()
            .join("target/benchmarks/qualification")
            .join(staging_name)
            .exists()
    );
}

#[test]
fn publication_rejects_replaced_parent_chain() {
    let repository = tempfile::tempdir().expect("temporary repository");
    let root = RepoRoot::resolve(repository.path()).expect("resolve repository");
    let output = Path::new("target/benchmarks/qualification/parent-replacement");
    let mut publication = begin_output(&root, output).expect("begin publication");
    publication
        .write("report.json", b"detached\n")
        .expect("write staged report");
    let parent = repository.path().join("target/benchmarks/qualification");
    let moved = parent.with_extension("moved");
    std::fs::rename(&parent, &moved).expect("move publication parent");
    std::fs::create_dir(&parent).expect("replace publication parent");

    assert!(publication.commit().is_err());
    assert!(!parent.join("parent-replacement/report.json").exists());
}

#[test]
fn publication_rejects_replaced_repository_root_before_exchange() {
    let parent = tempfile::tempdir().expect("temporary parent");
    let repository = parent.path().join("repository");
    std::fs::create_dir(&repository).expect("create repository");
    let root = RepoRoot::resolve(&repository).expect("resolve repository");
    let output = Path::new("target/benchmarks/qualification/root-replacement");
    let mut publication = begin_new_output(&root, output).expect("begin publication");
    publication
        .write("report.json", b"detached\n")
        .expect("write staged report");
    let detached = parent.path().join("detached-repository");

    assert!(matches!(
        publication.commit_new_with_source_validation(|_| {
            std::fs::rename(&repository, &detached).map_err(ArtifactError::Write)?;
            std::fs::create_dir(&repository).map_err(ArtifactError::Write)
        }),
        Err(ArtifactError::RepositoryIdentity)
    ));
    assert!(!repository.join(output).exists());
    assert!(!detached.join(output).exists());
}

#[test]
fn publication_rejects_replaced_staging_directory() {
    let repository = tempfile::tempdir().expect("temporary repository");
    let root = RepoRoot::resolve(repository.path()).expect("resolve repository");
    let output = Path::new("target/benchmarks/qualification/staging-replacement");
    let mut publication = begin_output(&root, output).expect("begin publication");
    publication
        .write("report.json", b"detached\n")
        .expect("write staged report");
    let parent = repository.path().join("target/benchmarks/qualification");
    let staging = parent.join(&publication.staging_name);
    let moved = staging.with_extension("moved");
    std::fs::rename(&staging, &moved).expect("move staging directory");
    std::fs::create_dir(&staging).expect("replace staging directory");

    assert!(publication.commit().is_err());
    assert!(!parent.join("staging-replacement/report.json").exists());
}

#[test]
fn publication_rejects_in_place_staged_artifact_mutation() {
    let repository = tempfile::tempdir().expect("temporary repository");
    let root = RepoRoot::resolve(repository.path()).expect("resolve repository");
    let output = Path::new("target/benchmarks/qualification/staged-mutation");
    let mut publication = begin_output(&root, output).expect("begin publication");
    publication
        .write("report.json", b"original\n")
        .expect("write staged report");
    let staged_report = repository
        .path()
        .join("target/benchmarks/qualification")
        .join(&publication.staging_name)
        .join("report.json");
    std::fs::write(&staged_report, b"changed\n").expect("mutate staged report");

    assert!(matches!(
        publication.commit(),
        Err(ArtifactError::ConcurrentReplacement("report.json"))
    ));
    assert!(!repository.path().join(output).exists());
}

#[test]
fn publication_rejects_replaced_staged_artifact() {
    let repository = tempfile::tempdir().expect("temporary repository");
    let root = RepoRoot::resolve(repository.path()).expect("resolve repository");
    let output = Path::new("target/benchmarks/qualification/staged-replacement");
    let mut publication = begin_output(&root, output).expect("begin publication");
    publication
        .write("report.json", b"original\n")
        .expect("write staged report");
    let staged_report = repository
        .path()
        .join("target/benchmarks/qualification")
        .join(&publication.staging_name)
        .join("report.json");
    std::fs::remove_file(&staged_report).expect("remove staged report");
    std::fs::write(&staged_report, b"original\n").expect("replace staged report");

    assert!(matches!(
        publication.commit(),
        Err(ArtifactError::ConcurrentReplacement("report.json"))
    ));
    assert!(!repository.path().join(output).exists());
    assert_eq!(
        std::fs::read(&staged_report).expect("read retained replacement"),
        b"original\n"
    );
}

#[test]
fn publication_rejects_unexpected_staged_artifacts() {
    let repository = tempfile::tempdir().expect("temporary repository");
    let root = RepoRoot::resolve(repository.path()).expect("resolve repository");
    let output = Path::new("target/benchmarks/qualification/staged-unexpected");
    let mut publication = begin_output(&root, output).expect("begin publication");
    publication
        .write("report.json", b"original\n")
        .expect("write staged report");
    std::fs::write(
        repository
            .path()
            .join("target/benchmarks/qualification")
            .join(&publication.staging_name)
            .join("unexpected"),
        b"hostile\n",
    )
    .expect("add unexpected staged artifact");

    assert!(matches!(
        publication.commit(),
        Err(ArtifactError::UnexpectedStagedArtifacts(_))
    ));
    assert!(!repository.path().join(output).exists());
}

#[test]
fn drop_does_not_remove_a_replaced_staging_entry() {
    let repository = tempfile::tempdir().expect("temporary repository");
    let root = RepoRoot::resolve(repository.path()).expect("resolve repository");
    let output = Path::new("target/benchmarks/qualification/drop-staging-replacement");
    let mut publication = begin_output(&root, output).expect("begin publication");
    publication
        .write("report.json", b"detached\n")
        .expect("write staged report");
    let parent = repository.path().join("target/benchmarks/qualification");
    let staging = parent.join(&publication.staging_name);
    let detached = staging.with_extension("detached");
    std::fs::rename(&staging, &detached).expect("detach staging directory");
    std::fs::create_dir(&staging).expect("replace staging directory");
    std::fs::write(staging.join("sentinel"), b"retained\n").expect("write sentinel");

    drop(publication);

    assert_eq!(
        std::fs::read(staging.join("sentinel")).expect("read replacement sentinel"),
        b"retained\n"
    );
    assert_eq!(
        std::fs::read(detached.join("report.json")).expect("read detached report"),
        b"detached\n"
    );
}

#[test]
fn unexpected_existing_artifact_blocks_replacement_without_damaging_evidence() {
    let repository = tempfile::tempdir().expect("temporary repository");
    let root = RepoRoot::resolve(repository.path()).expect("resolve repository");
    let output = Path::new("target/benchmarks/qualification/test-run");
    let mut first = begin_output(&root, output).expect("begin first publication");
    first
        .write("report.json", b"first\n")
        .expect("write first report");
    first.commit().expect("publish first report");
    std::fs::write(
        repository
            .path()
            .join("target/benchmarks/qualification/test-run/unexpected"),
        b"hostile",
    )
    .expect("write unexpected artifact");

    let mut replacement = begin_output(&root, output).expect("begin blocked replacement");
    replacement
        .write("report.json", b"second\n")
        .expect("write staged replacement");
    assert!(replacement.commit().is_err());
    assert_eq!(
        read_output_artifact(&root, output, "report.json").expect("read preserved report"),
        b"first\n"
    );
}

#[test]
fn stale_refresh_cannot_replace_newer_published_evidence() {
    let repository = tempfile::tempdir().expect("temporary repository");
    let root = RepoRoot::resolve(repository.path()).expect("resolve repository");
    let output = Path::new("target/benchmarks/qualification/test-run");

    let mut first = begin_output(&root, output).expect("begin first publication");
    first
        .write("report.json", b"first\n")
        .expect("write first report");
    first.commit().expect("publish first report");
    let stale = read_output_artifact(&root, output, "report.json").expect("read stale report");

    let mut second = begin_output(&root, output).expect("begin newer publication");
    second
        .write("report.json", b"second\n")
        .expect("write newer report");
    second.commit().expect("publish newer report");

    let mut refresh = begin_output(&root, output).expect("begin stale refresh");
    assert!(matches!(
        refresh.require_current_artifact("report.json", &stale),
        Err(ArtifactError::ConcurrentReplacement("report.json"))
    ));
    drop(refresh);
    assert_eq!(
        read_output_artifact(&root, output, "report.json").expect("read newer report"),
        b"second\n"
    );
}

#[test]
fn source_validation_cannot_exchange_a_replaced_target() {
    let repository = tempfile::tempdir().expect("temporary repository");
    let root = RepoRoot::resolve(repository.path()).expect("resolve repository");
    let output = Path::new("target/benchmarks/qualification/pre-exchange-target-race");
    let mut first = begin_output(&root, output).expect("begin first publication");
    first
        .write("report.json", b"first\n")
        .expect("write first report");
    first.commit().expect("publish first report");

    let mut stale = begin_output(&root, output).expect("begin stale refresh");
    stale
        .write("report.json", b"stale\n")
        .expect("write stale report");
    let target = repository.path().join(output);
    let detached = target.with_extension("detached");

    assert!(matches!(
        stale.commit_with_source_validation(|_| {
            std::fs::rename(&target, &detached).map_err(ArtifactError::Write)?;
            std::fs::create_dir(&target).map_err(ArtifactError::Write)?;
            std::fs::write(target.join("report.json"), b"newer\n").map_err(ArtifactError::Write)
        }),
        Err(ArtifactError::DirectoryIdentity(
            "replaced qualification directory changed before cleanup"
        ))
    ));
    assert_eq!(
        std::fs::read(target.join("report.json")).expect("read newer target"),
        b"newer\n"
    );
    assert_eq!(
        std::fs::read(detached.join("report.json")).expect("read detached old target"),
        b"first\n"
    );
}

#[test]
fn bound_refresh_replaces_the_validated_target() {
    let repository = tempfile::tempdir().expect("temporary repository");
    let root = RepoRoot::resolve(repository.path()).expect("resolve repository");
    let output = Path::new("target/benchmarks/qualification/bound-refresh");

    let mut first = begin_output(&root, output).expect("begin first publication");
    first
        .write("report.json", b"first\n")
        .expect("write first report");
    first.commit().expect("publish first report");

    let mut refresh = begin_output(&root, output).expect("begin refresh");
    refresh
        .require_current_artifact("report.json", b"first\n")
        .expect("bind current report");
    refresh
        .write("report.json", b"second\n")
        .expect("write refreshed report");
    refresh.commit().expect("publish bound refresh");

    assert_eq!(
        read_output_artifact(&root, output, "report.json").expect("read refreshed report"),
        b"second\n"
    );
}

#[test]
fn bound_refresh_rejects_in_place_target_mutation_before_exchange() {
    let repository = tempfile::tempdir().expect("temporary repository");
    let root = RepoRoot::resolve(repository.path()).expect("resolve repository");
    let output = Path::new("target/benchmarks/qualification/in-place-target");

    let mut first = begin_output(&root, output).expect("begin first publication");
    first
        .write("report.json", b"first\n")
        .expect("write first report");
    first.commit().expect("publish first report");

    let mut refresh = begin_output(&root, output).expect("begin refresh");
    refresh
        .require_current_artifact("report.json", b"first\n")
        .expect("bind current report");
    refresh
        .write("report.json", b"second\n")
        .expect("write refreshed report");
    std::fs::write(
        repository.path().join(output).join("report.json"),
        b"changed\n",
    )
    .expect("mutate report in place");

    assert!(matches!(
        refresh.commit(),
        Err(ArtifactError::ConcurrentReplacement("report.json"))
    ));
    assert_eq!(
        std::fs::read(repository.path().join(output).join("report.json"))
            .expect("read concurrent report"),
        b"changed\n"
    );
}

#[test]
fn bound_refresh_rolls_back_displaced_target_mutation() {
    let repository = tempfile::tempdir().expect("temporary repository");
    let root = RepoRoot::resolve(repository.path()).expect("resolve repository");
    let output = Path::new("target/benchmarks/qualification/in-place-displaced-target");

    let mut first = begin_output(&root, output).expect("begin first publication");
    first
        .write("report.json", b"first\n")
        .expect("write first report");
    first.commit().expect("publish first report");

    let mut refresh = begin_output(&root, output).expect("begin refresh");
    refresh
        .require_current_artifact("report.json", b"first\n")
        .expect("bind current report");
    refresh
        .write("report.json", b"second\n")
        .expect("write refreshed report");

    assert!(matches!(
        refresh.commit_with_after_exchange(true, |previous| {
            overwrite_artifact(
                previous.ok_or(ArtifactError::DirectoryIdentity("missing displaced target"))?,
                "report.json",
                b"changed\n",
            )
        }),
        Err(ArtifactError::ConcurrentReplacement("report.json"))
    ));
    assert_eq!(
        std::fs::read(repository.path().join(output).join("report.json"))
            .expect("read rolled-back report"),
        b"changed\n"
    );
}

#[test]
fn refresh_rolls_back_published_staged_artifact_mutation() {
    let repository = tempfile::tempdir().expect("temporary repository");
    let root = RepoRoot::resolve(repository.path()).expect("resolve repository");
    let output = Path::new("target/benchmarks/qualification/published-staged-mutation");

    let mut first = begin_output(&root, output).expect("begin first publication");
    first
        .write("report.json", b"first\n")
        .expect("write first report");
    first.commit().expect("publish first report");

    let mut refresh = begin_output(&root, output).expect("begin refresh");
    refresh
        .require_current_artifact("report.json", b"first\n")
        .expect("bind current report");
    refresh
        .write("report.json", b"second\n")
        .expect("write refreshed report");

    assert!(matches!(
        refresh.commit_with_after_exchange(true, |_| {
            std::fs::write(
                repository.path().join(output).join("report.json"),
                b"changed\n",
            )
            .map_err(ArtifactError::Write)
        }),
        Err(ArtifactError::ConcurrentReplacement("report.json"))
    ));
    assert_eq!(
        std::fs::read(repository.path().join(output).join("report.json"))
            .expect("read restored report"),
        b"first\n"
    );
}

#[test]
fn rollback_never_exchanges_a_substituted_displaced_directory() {
    let repository = tempfile::tempdir().expect("temporary repository");
    let root = RepoRoot::resolve(repository.path()).expect("resolve repository");
    let output = Path::new("target/benchmarks/qualification/rollback-substitution");

    let mut first = begin_output(&root, output).expect("begin first publication");
    first
        .write("report.json", b"first\n")
        .expect("write first report");
    first.commit().expect("publish first report");

    let mut refresh = begin_output(&root, output).expect("begin refresh");
    refresh
        .require_current_artifact("report.json", b"first\n")
        .expect("bind current report");
    refresh
        .write("report.json", b"second\n")
        .expect("write refreshed report");
    let parent = repository.path().join("target/benchmarks/qualification");
    let displaced = parent.join(&refresh.staging_name);
    let detached = displaced.with_extension("detached");

    assert!(matches!(
        refresh.commit_with_after_exchange(true, |_| {
            std::fs::rename(&displaced, &detached).map_err(ArtifactError::Write)?;
            std::fs::create_dir(&displaced).map_err(ArtifactError::Write)?;
            std::fs::write(displaced.join("report.json"), b"substitute\n")
                .map_err(ArtifactError::Write)?;
            Err(ArtifactError::DirectoryIdentity(
                "injected displaced-directory substitution",
            ))
        }),
        Err(ArtifactError::PublicationRollback)
    ));
    assert_eq!(
        std::fs::read(repository.path().join(output).join("report.json"))
            .expect("read canonical report"),
        b"second\n"
    );
    assert_eq!(
        std::fs::read(displaced.join("report.json")).expect("read substituted report"),
        b"substitute\n"
    );
    assert_eq!(
        std::fs::read(detached.join("report.json")).expect("read detached original report"),
        b"first\n"
    );
}

#[test]
fn cleanup_never_unlinks_a_substituted_displaced_artifact() {
    let repository = tempfile::tempdir().expect("temporary repository");
    let root = RepoRoot::resolve(repository.path()).expect("resolve repository");
    let output = Path::new("target/benchmarks/qualification/cleanup-substitution");

    let mut first = begin_output(&root, output).expect("begin first publication");
    first
        .write("report.json", b"first\n")
        .expect("write first report");
    first.commit().expect("publish first report");

    let mut refresh = begin_output(&root, output).expect("begin refresh");
    refresh
        .write("report.json", b"second\n")
        .expect("write refreshed report");
    let displaced = repository
        .path()
        .join("target/benchmarks/qualification")
        .join(&refresh.staging_name);

    assert!(matches!(
        refresh.commit_with_after_exchange(true, |_| {
            std::fs::remove_file(displaced.join("report.json")).map_err(ArtifactError::Write)?;
            std::fs::write(displaced.join("report.json"), b"substitute\n")
                .map_err(ArtifactError::Write)
        }),
        Err(ArtifactError::DirectoryIdentity(
            "replaced qualification artifact changed before cleanup"
        ))
    ));
    assert_eq!(
        read_output_artifact(&root, output, "report.json").expect("read durable replacement"),
        b"second\n"
    );
    assert_eq!(
        std::fs::read(displaced.join("report.json")).expect("read retained substitute"),
        b"substitute\n"
    );
}

#[test]
fn sibling_binding_rejects_changed_or_nested_sources() {
    let repository = tempfile::tempdir().expect("temporary repository");
    let root = RepoRoot::resolve(repository.path()).expect("resolve repository");
    let source_path = Path::new("target/benchmarks/qualification/source");
    let mut source = begin_output(&root, source_path).expect("begin source");
    source
        .write("report.json", b"current\n")
        .expect("write source");
    source.commit().expect("publish source");

    let rollup_path = Path::new("target/benchmarks/qualification/rollup");
    let mut rollup = begin_output(&root, rollup_path).expect("begin rollup");
    let source_path =
        DirectQualificationArtifactPath::try_new(source_path).expect("direct source path");
    let current_digest = super::super::run::sha256_hex(b"current\n");
    let stale_digest = super::super::run::sha256_hex(b"stale\n");
    rollup
        .require_sibling_artifact_digest(&source_path, "report.json", &current_digest, 64)
        .expect("bind current source");
    assert!(matches!(
        rollup.require_sibling_artifact_digest(&source_path, "report.json", &stale_digest, 64,),
        Err(ArtifactError::ConcurrentReplacement("report.json"))
    ));
    assert!(matches!(
        DirectQualificationArtifactPath::try_new(Path::new(
            "target/benchmarks/qualification/nested/source"
        )),
        Err(ArtifactError::NonDirectArtifact(_))
    ));
}

#[test]
fn sibling_binding_rejects_an_unexpected_source_entry() {
    let repository = tempfile::tempdir().expect("temporary repository");
    let root = RepoRoot::resolve(repository.path()).expect("resolve repository");
    let source_path = Path::new("target/benchmarks/qualification/exact-source");
    let mut source = begin_output(&root, source_path).expect("begin source");
    source
        .write("report.json", b"source\n")
        .expect("write source");
    source.commit().expect("publish source");

    let output_path = Path::new("target/benchmarks/qualification/exact-derived");
    let mut output = begin_new_output(&root, output_path).expect("begin output");
    output
        .require_sibling_artifact_digest(
            &direct(source_path),
            "report.json",
            &super::super::run::sha256_hex(b"source\n"),
            64,
        )
        .expect("bind source");
    output
        .write("report.json", b"derived\n")
        .expect("write derived report");
    std::fs::write(
        repository.path().join(source_path).join("unexpected"),
        b"unexpected\n",
    )
    .expect("add unexpected source entry");

    assert!(matches!(
        output.commit_new(),
        Err(ArtifactError::BoundArtifactSetChanged { .. })
    ));
    assert!(!repository.path().join(output_path).exists());
    assert_eq!(
        std::fs::read(repository.path().join(source_path).join("unexpected"))
            .expect("read unexpected source entry"),
        b"unexpected\n"
    );
}

#[test]
fn new_publication_rolls_back_in_place_sibling_mutation() {
    let repository = tempfile::tempdir().expect("temporary repository");
    let root = RepoRoot::resolve(repository.path()).expect("resolve repository");
    let source_path = Path::new("target/benchmarks/qualification/in-place-source");
    let mut source = begin_output(&root, source_path).expect("begin source");
    source
        .write("report.json", b"source\n")
        .expect("write source");
    source.commit().expect("publish source");

    let output_path = Path::new("target/benchmarks/qualification/in-place-derived");
    let mut output = begin_new_output(&root, output_path).expect("begin output");
    output
        .require_sibling_artifact_digest(
            &direct(source_path),
            "report.json",
            &super::super::run::sha256_hex(b"source\n"),
            64,
        )
        .expect("bind source");
    output
        .write("report.json", b"derived\n")
        .expect("write derived report");

    assert!(matches!(
        output.commit_with_after_exchange(false, |_| {
            std::fs::write(
                repository.path().join(source_path).join("report.json"),
                b"changed\n",
            )
            .map_err(ArtifactError::Write)
        }),
        Err(ArtifactError::ConcurrentReplacement("report.json"))
    ));
    assert!(!repository.path().join(output_path).exists());
    assert_eq!(
        std::fs::read(repository.path().join(source_path).join("report.json"))
            .expect("read changed source"),
        b"changed\n"
    );
}

#[test]
fn external_source_validation_failure_rolls_back_each_post_exchange_checkpoint() {
    for (suffix, fail_at) in [("after-exchange", 2), ("after-sync", 3)] {
        let repository = tempfile::tempdir().expect("temporary repository");
        let root = RepoRoot::resolve(repository.path()).expect("resolve repository");
        let output_path = PathBuf::from("target/benchmarks/qualification").join(suffix);
        let mut output = begin_new_output(&root, &output_path).expect("begin output");
        output
            .write("report.json", b"derived\n")
            .expect("write derived report");
        let mut validations = 0;

        assert!(matches!(
            output.commit_new_with_source_validation(|_| {
                validations += 1;
                if validations == fail_at {
                    Err(ArtifactError::ExternalSourceChanged("injected source"))
                } else {
                    Ok(())
                }
            }),
            Err(ArtifactError::ExternalSourceChanged("injected source"))
        ));
        assert_eq!(validations, fail_at);
        assert!(!repository.path().join(&output_path).exists());
    }
}

#[test]
fn external_source_validation_failure_restores_replaced_output() {
    let repository = tempfile::tempdir().expect("temporary repository");
    let root = RepoRoot::resolve(repository.path()).expect("resolve repository");
    let output_path = Path::new("target/benchmarks/qualification/source-validated-refresh");
    let mut original = begin_output(&root, output_path).expect("begin original output");
    original
        .write("report.json", b"original\n")
        .expect("write original report");
    original.commit().expect("publish original output");

    let mut refresh = begin_output(&root, output_path).expect("begin refresh");
    refresh
        .require_current_artifact("report.json", b"original\n")
        .expect("bind original report");
    refresh
        .write("report.json", b"replacement\n")
        .expect("write replacement report");
    let mut validations = 0;
    assert!(matches!(
        refresh.commit_with_source_validation(|_| {
            validations += 1;
            if validations == 2 {
                Err(ArtifactError::ExternalSourceChanged("injected source"))
            } else {
                Ok(())
            }
        }),
        Err(ArtifactError::ExternalSourceChanged("injected source"))
    ));
    assert_eq!(validations, 2);
    assert_eq!(
        read_output_artifact(&root, output_path, "report.json").expect("read restored report"),
        b"original\n"
    );
}

#[test]
fn replacement_revalidates_external_sources_after_cleanup() {
    let repository = tempfile::tempdir().expect("temporary repository");
    let root = RepoRoot::resolve(repository.path()).expect("resolve repository");
    let output_path = Path::new("target/benchmarks/qualification/post-cleanup-source");
    let mut original = begin_output(&root, output_path).expect("begin original output");
    original
        .write("report.json", b"original\n")
        .expect("write original report");
    original.commit().expect("publish original output");

    let mut refresh = begin_output(&root, output_path).expect("begin refresh");
    refresh
        .write("report.json", b"replacement\n")
        .expect("write replacement report");
    let staging_name = refresh.staging_name.clone();
    let mut validations = 0;
    assert!(matches!(
        refresh.commit_with_source_validation(|_| {
            validations += 1;
            if validations == 4 {
                Err(ArtifactError::ExternalSourceChanged("post-cleanup source"))
            } else {
                Ok(())
            }
        }),
        Err(ArtifactError::ExternalSourceChanged("post-cleanup source"))
    ));
    assert_eq!(validations, 4);
    assert_eq!(
        read_output_artifact(&root, output_path, "report.json").expect("read durable replacement"),
        b"replacement\n"
    );
    assert!(
        !repository
            .path()
            .join("target/benchmarks/qualification")
            .join(staging_name)
            .exists()
    );
}

#[test]
fn publication_rejects_replaced_bound_source_directory() {
    let repository = tempfile::tempdir().expect("temporary repository");
    let root = RepoRoot::resolve(repository.path()).expect("resolve repository");
    let source_path = Path::new("target/benchmarks/qualification/source-inode");
    let mut source = begin_output(&root, source_path).expect("begin source");
    source
        .write("report.json", b"source\n")
        .expect("write source");
    source.commit().expect("publish source");

    let output_path = Path::new("target/benchmarks/qualification/derived-inode");
    let mut output = begin_output(&root, output_path).expect("begin output");
    let source_path =
        DirectQualificationArtifactPath::try_new(source_path).expect("direct source path");
    output
        .require_sibling_artifact_digest(
            &source_path,
            "report.json",
            &super::super::run::sha256_hex(b"source\n"),
            64,
        )
        .expect("bind source directory");
    output
        .write("report.json", b"derived\n")
        .expect("write derived report");

    let source = repository.path().join(source_path.as_path());
    let moved = source.with_extension("detached");
    std::fs::rename(&source, &moved).expect("move bound source");
    std::fs::create_dir(&source).expect("replace bound source directory");
    std::fs::write(source.join("report.json"), b"source\n")
        .expect("write byte-identical replacement source");

    assert!(matches!(
        output.commit(),
        Err(ArtifactError::DirectoryIdentity(_))
    ));
    assert!(!repository.path().join(output_path).exists());
}

#[test]
fn publication_rejects_replaced_bound_target_directory() {
    let repository = tempfile::tempdir().expect("temporary repository");
    let root = RepoRoot::resolve(repository.path()).expect("resolve repository");
    let output_path = Path::new("target/benchmarks/qualification/target-inode");
    let mut first = begin_output(&root, output_path).expect("begin first output");
    first
        .write("report.json", b"current\n")
        .expect("write current report");
    first.commit().expect("publish current report");

    let mut refresh = begin_output(&root, output_path).expect("begin refresh");
    refresh
        .require_current_artifact("report.json", b"current\n")
        .expect("bind current target");
    refresh
        .write("report.json", b"refreshed\n")
        .expect("write refreshed report");

    let target = repository.path().join(output_path);
    let moved = target.with_extension("detached");
    std::fs::rename(&target, &moved).expect("move bound target");
    std::fs::create_dir(&target).expect("replace bound target directory");
    std::fs::write(target.join("report.json"), b"current\n")
        .expect("write byte-identical replacement target");

    assert!(matches!(
        refresh.commit(),
        Err(ArtifactError::DirectoryIdentity(_))
    ));
    assert_eq!(
        std::fs::read(target.join("report.json")).expect("read replacement target"),
        b"current\n"
    );
}

#[test]
fn bounded_reads_reject_oversized_artifacts_and_invalid_limits() {
    let repository = tempfile::tempdir().expect("temporary repository");
    let root = RepoRoot::resolve(repository.path()).expect("resolve repository");
    let output = Path::new("target/benchmarks/qualification/bounded-read");
    let mut publication = begin_output(&root, output).expect("begin publication");
    publication
        .write("report.json", &[b'x'; 65])
        .expect("write oversized-for-test report");
    publication.commit().expect("publish report");

    assert!(matches!(
        read_output_artifact_bounded(&root, output, "report.json", 64),
        Err(ArtifactError::UnsafeArtifact("report.json"))
    ));
    assert!(matches!(
        read_output_artifact_bounded(
            &root,
            output,
            "report.json",
            MAX_ARTIFACT_BYTES.saturating_add(1),
        ),
        Err(ArtifactError::InvalidReadLimit(_))
    ));
}
