use std::path::{Path, PathBuf};

use super::*;
use crate::qualification::runtime::run::sha256_hex;

const DESCRIPTOR_HELPER: &str =
    "qualification::runtime::artifact::retained::tests::release_matrix_descriptor_helper";
const DESCRIPTOR_ENV: &str = "STAB_BENCH_RETAINED_DESCRIPTOR_HELPER";
const SIMULATED_CORRECTNESS_DESCRIPTORS: usize = 192;

fn direct(path: &Path) -> DirectQualificationArtifactPath {
    DirectQualificationArtifactPath::try_new(path).expect("direct qualification artifact path")
}

fn write_fixture(root: &RepoRoot, name: &str) -> PathBuf {
    let relative = PathBuf::from("target/benchmarks/qualification").join(name);
    let directory = root.path.join(&relative);
    std::fs::create_dir_all(&directory).expect("create retained artifact fixture");
    std::fs::write(directory.join("report.json"), b"report\n").expect("write retained report");
    std::fs::write(directory.join("preflight.json"), b"preflight\n")
        .expect("write retained preflight");
    std::fs::write(directory.join("report.md"), b"markdown\n").expect("write retained markdown");
    relative
}

fn read_binding(
    root: &RepoRoot,
    context: &Arc<RetainedArtifactContext>,
    relative: &Path,
) -> (Arc<RetainedArtifactDirectory>, RetainedArtifactBytes) {
    context
        .read_and_bind(
            root,
            &direct(relative),
            &[
                ("report.json", 1024),
                ("preflight.json", 1024),
                ("report.md", 1024),
            ],
        )
        .expect("read and bind artifacts")
}

#[test]
fn returns_bound_bytes_and_rejects_late_mutation() {
    let repository = tempfile::tempdir().expect("temporary repository");
    let root = RepoRoot::resolve(repository.path()).expect("resolve repository");
    let relative = write_fixture(&root, "retained-bytes");
    let live_repository = RepositoryBinding::open(&root).expect("bind repository");
    let context = RetainedArtifactContext::open(&root, &live_repository).expect("open context");
    let (binding, artifacts) = read_binding(&root, &context, &relative);

    assert_eq!(
        artifacts.get("report.json").map(Vec::as_slice),
        Some(b"report\n".as_slice())
    );
    assert_eq!(
        artifacts.get("preflight.json").map(Vec::as_slice),
        Some(b"preflight\n".as_slice())
    );
    assert_eq!(
        artifacts.get("report.md").map(Vec::as_slice),
        Some(b"markdown\n".as_slice())
    );
    binding.require_current(&root).expect("binding is current");

    std::fs::write(root.path.join(&relative).join("report.json"), b"changed\n")
        .expect("mutate retained report");
    assert!(matches!(
        binding.require_current(&root),
        Err(ArtifactError::ConcurrentReplacement("report.json"))
    ));
}

#[test]
fn rejects_directory_substitution_and_wrong_digest() {
    let repository = tempfile::tempdir().expect("temporary repository");
    let root = RepoRoot::resolve(repository.path()).expect("resolve repository");
    let relative = write_fixture(&root, "retained-directory");
    let live_repository = RepositoryBinding::open(&root).expect("bind repository");
    let context = RetainedArtifactContext::open(&root, &live_repository).expect("open context");
    let expected = [
        ("report.json", sha256_hex(b"report\n"), 1024),
        ("preflight.json", sha256_hex(b"preflight\n"), 1024),
        ("report.md", sha256_hex(b"markdown\n"), 1024),
    ];
    let expected = expected
        .iter()
        .map(|(name, digest, limit)| (*name, digest.as_str(), *limit))
        .collect::<Vec<_>>();
    let binding = context
        .bind_digests(&root, &direct(&relative), &expected)
        .expect("bind artifact digests");

    let original = root.path.join(&relative);
    let displaced = original.with_extension("displaced");
    std::fs::rename(&original, &displaced).expect("displace retained directory");
    write_fixture(&root, "retained-directory");
    assert!(matches!(
        binding.require_current(&root),
        Err(ArtifactError::DirectoryIdentity(_))
    ));

    let digest_fixture = write_fixture(&root, "retained-digest");
    assert!(matches!(
        context.bind_digests(
            &root,
            &direct(&digest_fixture),
            &[("report.json", &"0".repeat(64), 1024)],
        ),
        Err(ArtifactError::ConcurrentReplacement("report.json"))
    ));
}

#[test]
fn rejects_identical_file_replacement_and_extra_children() {
    let repository = tempfile::tempdir().expect("temporary repository");
    let root = RepoRoot::resolve(repository.path()).expect("resolve repository");
    let relative = write_fixture(&root, "retained-file");
    let live_repository = RepositoryBinding::open(&root).expect("bind repository");
    let context = RetainedArtifactContext::open(&root, &live_repository).expect("open context");
    let (binding, _) = read_binding(&root, &context, &relative);
    let report = root.path.join(&relative).join("report.json");
    let displaced = root.path.join("displaced-report.json");
    std::fs::rename(&report, &displaced).expect("displace retained report");
    std::fs::write(&report, b"report\n").expect("write identical replacement report");
    assert!(matches!(
        binding.require_current(&root),
        Err(ArtifactError::ConcurrentReplacement("report.json"))
    ));

    let second = write_fixture(&root, "retained-extra");
    let (binding, _) = read_binding(&root, &context, &second);
    std::fs::write(root.path.join(&second).join("unexpected"), b"extra\n")
        .expect("write unexpected child");
    assert!(matches!(
        binding.require_current(&root),
        Err(ArtifactError::BoundArtifactSetChanged { .. })
    ));
}

#[test]
fn rejects_parent_and_repository_substitution() {
    let repository = tempfile::tempdir().expect("temporary repository");
    let root = RepoRoot::resolve(repository.path()).expect("resolve repository");
    let relative = write_fixture(&root, "retained-parent");
    let live_repository = RepositoryBinding::open(&root).expect("bind repository");
    let context = RetainedArtifactContext::open(&root, &live_repository).expect("open context");
    let (binding, _) = read_binding(&root, &context, &relative);
    let parent = root.path.join("target/benchmarks/qualification");
    let displaced_parent = root.path.join("target/benchmarks/qualification-displaced");
    std::fs::rename(&parent, &displaced_parent).expect("displace qualification parent");
    write_fixture(&root, "retained-parent");
    assert!(matches!(
        binding.require_current(&root),
        Err(ArtifactError::DirectoryIdentity(_))
    ));

    let repository_path = root.path.clone();
    let displaced_repository = repository_path.with_extension("displaced");
    std::fs::rename(&repository_path, &displaced_repository).expect("displace repository");
    std::fs::create_dir(&repository_path).expect("create replacement repository");
    assert!(matches!(
        binding.require_current(&root),
        Err(ArtifactError::RepositoryIdentity)
    ));
}

#[test]
fn release_matrix_fits_soft_nofile_1024() {
    let output = std::process::Command::new(std::env::current_exe().expect("test executable"))
        .args([DESCRIPTOR_HELPER, "--exact", "--ignored", "--nocapture"])
        .env_clear()
        .env(DESCRIPTOR_ENV, "1")
        .output()
        .expect("run retained descriptor helper");
    assert!(
        output.status.success(),
        "retained descriptor helper failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
#[ignore = "executed only as a subprocess by the descriptor-budget test"]
fn release_matrix_descriptor_helper() {
    assert_eq!(std::env::var(DESCRIPTOR_ENV).as_deref(), Ok("1"));
    let inherited = rustix::process::getrlimit(rustix::process::Resource::Nofile);
    assert!(inherited.maximum.is_none_or(|maximum| maximum >= 1024));
    rustix::process::setrlimit(
        rustix::process::Resource::Nofile,
        rustix::process::Rlimit {
            current: Some(1024),
            maximum: inherited.maximum,
        },
    )
    .expect("set soft descriptor limit");

    let repository = tempfile::tempdir().expect("temporary repository");
    let root = RepoRoot::resolve(repository.path()).expect("resolve repository");
    for index in 0..177 {
        write_fixture(&root, &format!("release-artifact-{index:03}"));
    }
    let simulated_correctness_descriptors = (0..SIMULATED_CORRECTNESS_DESCRIPTORS)
        .map(|_| {
            rustix::fs::open(
                "/dev/null",
                rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::CLOEXEC,
                rustix::fs::Mode::empty(),
            )
            .expect("open simulated correctness descriptor")
        })
        .collect::<Vec<_>>();
    let live_repository = RepositoryBinding::open(&root).expect("bind repository");
    let context = RetainedArtifactContext::open(&root, &live_repository).expect("open context");
    let bindings = (0..177)
        .map(|index| {
            let path = PathBuf::from(format!(
                "target/benchmarks/qualification/release-artifact-{index:03}"
            ));
            read_binding(&root, &context, &path).0
        })
        .collect::<Vec<_>>();
    for binding in &bindings {
        binding
            .require_current(&root)
            .expect("artifact remains current");
    }
    assert_eq!(bindings.len(), 177);
    assert_eq!(
        simulated_correctness_descriptors.len(),
        SIMULATED_CORRECTNESS_DESCRIPTORS
    );
}
