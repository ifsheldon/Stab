use std::path::Path;

use super::{ArtifactError, QualificationOutputDir};
use crate::RepoRoot;

fn root(path: &Path) -> RepoRoot {
    RepoRoot {
        path: path.to_path_buf(),
    }
}

#[test]
fn output_root_rejects_absolute_and_traversing_paths() {
    let temporary = tempfile::tempdir().expect("temporary root");
    let root = root(temporary.path());

    assert!(QualificationOutputDir::parse(&root, Path::new("/tmp/report")).is_err());
    assert!(
        QualificationOutputDir::parse(&root, Path::new("target/qualification/../escaped")).is_err()
    );
    assert!(QualificationOutputDir::parse(&root, Path::new("target/qualification")).is_err());
}

#[cfg(unix)]
#[test]
fn output_root_rejects_non_utf8_components() {
    use std::os::unix::ffi::OsStringExt as _;

    let temporary = tempfile::tempdir().expect("temporary root");
    let root = root(temporary.path());
    let path = std::path::PathBuf::from(std::ffi::OsString::from_vec(
        b"target/qualification/correctness/\xff".to_vec(),
    ));

    assert!(QualificationOutputDir::parse(&root, &path).is_err());
}

#[test]
fn artifact_paths_reject_parent_traversal() {
    let temporary = tempfile::tempdir().expect("temporary root");
    let root = root(temporary.path());
    let output =
        QualificationOutputDir::parse(&root, Path::new("target/qualification/correctness/test"))
            .expect("output root");

    let error = output
        .write(Path::new("cases/../../escaped"), b"bad")
        .expect_err("reject traversal");

    assert!(matches!(error, ArtifactError::InvalidArtifact { .. }));
}

#[test]
fn artifact_round_trip_preserves_non_utf8_bytes() {
    let temporary = tempfile::tempdir().expect("temporary root");
    let root = root(temporary.path());
    let output = QualificationOutputDir::parse(
        &root,
        Path::new("target/qualification/correctness/non-utf8"),
    )
    .expect("output root");
    let bytes = [0xff, 0x00, 0x80, b'\n'];

    output
        .write(Path::new("cases/case-a/stderr.bin"), &bytes)
        .expect("write raw artifact");

    assert_eq!(
        output
            .read(Path::new("cases/case-a/stderr.bin"), bytes.len())
            .expect("read raw artifact"),
        bytes
    );
}

#[cfg(target_os = "linux")]
#[test]
fn staged_run_atomically_replaces_the_complete_previous_directory() {
    let temporary = tempfile::tempdir().expect("temporary root");
    let root = root(temporary.path());
    let final_output =
        QualificationOutputDir::parse(&root, Path::new("target/qualification/correctness/atomic"))
            .expect("final output");

    let first = final_output.begin_run().expect("first staged run");
    first
        .write(Path::new("stale.bin"), b"old")
        .expect("write old artifact");
    first.commit().expect("publish first run");

    let second = final_output.begin_run().expect("second staged run");
    second
        .write(Path::new("fresh.bin"), b"new")
        .expect("write new artifact");
    second.commit().expect("publish second run");

    assert_eq!(
        final_output
            .read(Path::new("fresh.bin"), 3)
            .expect("read fresh artifact"),
        b"new"
    );
    assert!(final_output.read(Path::new("stale.bin"), 3).is_err());
}

#[cfg(target_os = "linux")]
#[test]
fn nested_artifact_creation_synchronizes_each_new_parent() {
    let temporary = tempfile::tempdir().expect("temporary root");
    let root = root(temporary.path());
    let final_output = QualificationOutputDir::parse(
        &root,
        Path::new("target/qualification/correctness/nested-sync"),
    )
    .expect("final output");
    let staged = final_output.begin_run().expect("staged run");

    super::reset_created_parent_sync_count();
    staged
        .write(Path::new("cases/case-a/stdout.bin"), b"stdout")
        .expect("nested artifact");
    assert_eq!(super::created_parent_sync_count(), 2);

    super::reset_created_parent_sync_count();
    staged
        .write(Path::new("cases/case-a/stderr.bin"), b"stderr")
        .expect("sibling artifact");
    assert_eq!(
        super::created_parent_sync_count(),
        0,
        "existing parent directories must not be reported as newly synchronized"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn staged_writes_remain_bound_to_the_open_directory_after_a_path_swap() {
    let temporary = tempfile::tempdir().expect("temporary root");
    let root = root(temporary.path());
    let final_output = QualificationOutputDir::parse(
        &root,
        Path::new("target/qualification/correctness/staged-write-swap"),
    )
    .expect("final output");
    let staged = final_output.begin_run().expect("staged run");
    let original_path = staged.absolute.clone();
    let moved_path = original_path.with_extension("moved");
    std::fs::rename(&original_path, &moved_path).expect("move staged directory");
    std::fs::create_dir(&original_path).expect("create replacement staging path");

    staged
        .write(Path::new("cases/case-a/stdout.bin"), b"descriptor-owned")
        .expect("descriptor-owned write");

    assert_eq!(
        std::fs::read(moved_path.join("cases/case-a/stdout.bin"))
            .expect("read descriptor-owned artifact"),
        b"descriptor-owned"
    );
    assert!(!original_path.join("cases/case-a/stdout.bin").exists());
    assert!(staged.commit().is_err());
}

#[cfg(target_os = "linux")]
#[test]
fn locked_published_output_rejects_an_identity_swap() {
    let temporary = tempfile::tempdir().expect("temporary root");
    let root = root(temporary.path());
    let final_output = QualificationOutputDir::parse(
        &root,
        Path::new("target/qualification/correctness/locked-swap"),
    )
    .expect("final output");
    let staged = final_output.begin_run().expect("staged run");
    staged
        .write(Path::new("report.json"), b"published")
        .expect("write report");
    staged.commit().expect("publish run");
    let locked = final_output.lock_published().expect("lock published run");
    let moved = final_output.absolute.with_extension("moved");
    std::fs::rename(&final_output.absolute, &moved).expect("move published directory");
    std::fs::create_dir(&final_output.absolute).expect("replace published directory");

    locked
        .output()
        .write(Path::new("report.md"), b"derived")
        .expect("descriptor-owned derived write");

    assert_eq!(
        std::fs::read(moved.join("report.md")).expect("read moved derived report"),
        b"derived"
    );
    assert!(!final_output.absolute.join("report.md").exists());
    assert!(locked.finish().is_err());
}

#[cfg(target_os = "linux")]
#[test]
fn publication_rejects_a_replaced_output_parent_chain() {
    let temporary = tempfile::tempdir().expect("temporary root");
    let root = root(temporary.path());
    let final_output = QualificationOutputDir::parse(
        &root,
        Path::new("target/qualification/correctness/parent-swap"),
    )
    .expect("final output");
    let staged = final_output.begin_run().expect("staged run");
    staged
        .write(Path::new("report.json"), b"detached")
        .expect("write staged report");
    let parent = temporary.path().join("target/qualification/correctness");
    let moved = parent.with_extension("moved");
    std::fs::rename(&parent, &moved).expect("move output parent");
    std::fs::create_dir(&parent).expect("replace output parent");

    assert!(staged.commit().is_err());
    assert!(!parent.join("parent-swap/report.json").exists());
}

#[cfg(target_os = "linux")]
#[test]
fn report_regeneration_rejects_a_replaced_output_parent_chain() {
    let temporary = tempfile::tempdir().expect("temporary root");
    let root = root(temporary.path());
    let final_output = QualificationOutputDir::parse(
        &root,
        Path::new("target/qualification/correctness/report-parent-swap"),
    )
    .expect("final output");
    let staged = final_output.begin_run().expect("staged run");
    staged
        .write(Path::new("report.json"), b"published")
        .expect("write report");
    staged.commit().expect("publish run");
    let locked = final_output.lock_published().expect("lock published run");
    let parent = temporary.path().join("target/qualification/correctness");
    let moved = parent.with_extension("moved");
    std::fs::rename(&parent, &moved).expect("move output parent");
    std::fs::create_dir(&parent).expect("replace output parent");
    locked
        .output()
        .write(Path::new("report.md"), b"detached-derived")
        .expect("descriptor-owned derived report");

    assert!(locked.finish().is_err());
    assert!(!parent.join("report-parent-swap/report.md").exists());
    assert_eq!(
        std::fs::read(moved.join("report-parent-swap/report.md"))
            .expect("read detached derived report"),
        b"detached-derived"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn bounded_cleanup_failure_does_not_invalidate_new_publication() {
    let temporary = tempfile::tempdir().expect("temporary root");
    let root = root(temporary.path());
    let final_output = QualificationOutputDir::parse(
        &root,
        Path::new("target/qualification/correctness/bounded-cleanup"),
    )
    .expect("final output");
    let first = final_output.begin_run().expect("first staged run");
    let mut deep_artifact = std::path::PathBuf::new();
    for _ in 0..=super::MAX_CLEANUP_DEPTH {
        deep_artifact.push("nested");
    }
    deep_artifact.push("old.bin");
    first
        .write(&deep_artifact, b"old")
        .expect("write deep artifact");
    first.commit().expect("publish deep run");

    let second = final_output.begin_run().expect("second staged run");
    second
        .write(Path::new("report.json"), b"new")
        .expect("write replacement report");
    second
        .commit()
        .expect("cleanup limit must not invalidate publication");

    assert_eq!(
        final_output
            .read(Path::new("report.json"), 3)
            .expect("read new report"),
        b"new"
    );
}
