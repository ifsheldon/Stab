use std::path::Path;

use super::{PublicationLock, QualificationOutputDir, publish_staged_directory_with_check};
use crate::RepoRoot;

fn root(path: &Path) -> RepoRoot {
    RepoRoot {
        path: path.to_path_buf(),
    }
}

#[cfg(target_os = "linux")]
#[test]
fn concurrent_publications_serialize_without_orphaning_staging_directories() {
    let temporary = tempfile::tempdir().expect("temporary root");
    let root = root(temporary.path());
    let final_output = QualificationOutputDir::parse(
        &root,
        Path::new("target/qualification/correctness/concurrent"),
    )
    .expect("final output");
    let first = final_output.begin_run().expect("first staged run");
    first
        .write(Path::new("report.json"), b"first")
        .expect("first report");
    let second = final_output.begin_run().expect("second staged run");
    second
        .write(Path::new("report.json"), b"second")
        .expect("second report");
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(3));
    let first_barrier = std::sync::Arc::clone(&barrier);
    let first_thread = std::thread::spawn(move || {
        first_barrier.wait();
        first.commit()
    });
    let second_barrier = std::sync::Arc::clone(&barrier);
    let second_thread = std::thread::spawn(move || {
        second_barrier.wait();
        second.commit()
    });
    barrier.wait();

    first_thread
        .join()
        .expect("first publication thread")
        .expect("first publication");
    second_thread
        .join()
        .expect("second publication thread")
        .expect("second publication");
    let report = final_output
        .read(Path::new("report.json"), 6)
        .expect("published report");
    assert!(report == b"first" || report == b"second");
    let parent = final_output.absolute.parent().expect("output parent");
    let staging = std::fs::read_dir(parent)
        .expect("output parent entries")
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with(".stab-correctness-")
        })
        .count();
    assert_eq!(staging, 0);
}

#[cfg(target_os = "linux")]
#[test]
fn publication_rechecks_cancellation_after_waiting_for_the_repository_lock() {
    use std::sync::atomic::{AtomicBool, Ordering};

    let temporary = tempfile::tempdir().expect("temporary root");
    let root = root(temporary.path());
    let final_output = QualificationOutputDir::parse(
        &root,
        Path::new("target/qualification/correctness/cancelled-publication"),
    )
    .expect("final output");
    let staged = final_output.begin_run().expect("staged run");
    staged
        .write(Path::new("report.json"), b"must not publish")
        .expect("staged report");
    let repository = crate::safe_file::open_directory(temporary.path())
        .expect("independent repository descriptor");
    let held_lock = PublicationLock::acquire(&repository).expect("hold publication lock");
    let cancelled = std::sync::Arc::new(AtomicBool::new(false));
    let thread_cancelled = std::sync::Arc::clone(&cancelled);
    let (started_tx, started_rx) = std::sync::mpsc::channel();
    let publication = std::thread::spawn(move || {
        started_tx.send(()).expect("signal publication start");
        let result = publish_staged_directory_with_check(
            staged.staged.as_ref().expect("staged descriptor"),
            || {
                if thread_cancelled.load(Ordering::Acquire) {
                    Err(std::io::Error::new(
                        std::io::ErrorKind::Interrupted,
                        "qualification controller cancelled",
                    ))
                } else {
                    Ok(())
                }
            },
        );
        (staged, result)
    });
    started_rx.recv().expect("publication started");
    std::thread::sleep(std::time::Duration::from_millis(50));
    cancelled.store(true, Ordering::Release);
    drop(held_lock);

    let (staged, result) = publication.join().expect("publication thread");
    assert!(result.is_err());
    drop(staged);
    assert!(!final_output.absolute.exists());
}

#[cfg(target_os = "linux")]
#[test]
fn abandoned_staged_run_is_removed_without_touching_published_output() {
    let temporary = tempfile::tempdir().expect("temporary root");
    let root = root(temporary.path());
    let final_output = QualificationOutputDir::parse(
        &root,
        Path::new("target/qualification/correctness/abandoned"),
    )
    .expect("final output");
    let first = final_output.begin_run().expect("first staged run");
    first
        .write(Path::new("report.json"), b"published")
        .expect("write published report");
    first.commit().expect("publish first run");

    {
        let abandoned = final_output.begin_run().expect("abandoned staged run");
        abandoned
            .write(Path::new("report.json"), b"unpublished")
            .expect("write abandoned report");
    }

    assert_eq!(
        final_output
            .read(Path::new("report.json"), 32)
            .expect("read published report"),
        b"published"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn publication_rejects_a_swapped_target_without_following_it() {
    use std::os::unix::fs::symlink;

    let temporary = tempfile::tempdir().expect("temporary root");
    let outside = tempfile::tempdir().expect("outside root");
    std::fs::write(outside.path().join("keep"), b"outside").expect("outside fixture");
    let root = root(temporary.path());
    let final_output = QualificationOutputDir::parse(
        &root,
        Path::new("target/qualification/correctness/target-swap"),
    )
    .expect("final output");
    let first = final_output.begin_run().expect("first staged run");
    first
        .write(Path::new("report.json"), b"published")
        .expect("write published report");
    first.commit().expect("publish first run");

    let second = final_output.begin_run().expect("second staged run");
    let moved = final_output.absolute.with_extension("moved");
    std::fs::rename(&final_output.absolute, &moved).expect("move published directory");
    symlink(outside.path(), &final_output.absolute).expect("swap target with symlink");

    assert!(second.commit().is_err());
    assert_eq!(
        std::fs::read(outside.path().join("keep")).expect("outside remains"),
        b"outside"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn abandoned_cleanup_rejects_a_swapped_staging_root() {
    use std::os::unix::fs::symlink;

    let temporary = tempfile::tempdir().expect("temporary root");
    let outside = tempfile::tempdir().expect("outside root");
    std::fs::write(outside.path().join("keep"), b"outside").expect("outside fixture");
    let root = root(temporary.path());
    let final_output = QualificationOutputDir::parse(
        &root,
        Path::new("target/qualification/correctness/drop-swap"),
    )
    .expect("final output");
    let abandoned = final_output.begin_run().expect("staged run");
    abandoned
        .write(Path::new("report.json"), b"unpublished")
        .expect("write staged report");
    let moved = abandoned.absolute.with_extension("moved");
    std::fs::rename(&abandoned.absolute, &moved).expect("move staging directory");
    symlink(outside.path(), &abandoned.absolute).expect("swap staging with symlink");

    drop(abandoned);

    assert_eq!(
        std::fs::read(outside.path().join("keep")).expect("outside remains"),
        b"outside"
    );
    assert_eq!(
        std::fs::read_dir(&moved)
            .expect("detached staging directory")
            .count(),
        0,
        "descriptor cleanup should empty the owned staging tree"
    );
}

#[cfg(unix)]
#[test]
fn report_write_rejects_symlinked_parent() {
    use std::os::unix::fs::symlink;

    let temporary = tempfile::tempdir().expect("temporary root");
    let outside = tempfile::tempdir().expect("outside root");
    let qualification = temporary.path().join("target/qualification");
    std::fs::create_dir_all(&qualification).expect("qualification root");
    symlink(outside.path(), qualification.join("link")).expect("symlink output");
    let root = root(temporary.path());
    let output =
        QualificationOutputDir::parse(&root, Path::new("target/qualification/link/report"))
            .expect("syntactically valid output");

    assert!(output.write(Path::new("report.json"), b"{}").is_err());
    assert!(!outside.path().join("report/report.json").exists());
}
