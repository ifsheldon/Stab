use std::assert_matches;
use std::cell::Cell;
use std::fs;
use std::rc::Rc;

use super::*;

#[cfg(unix)]
#[test]
fn descriptor_and_path_stats_share_file_identity_normalization() {
    let root = tempfile::tempdir().expect("root");
    let path = root.path().join("artifact");
    fs::write(&path, b"artifact").expect("artifact");
    let file = File::open(&path).expect("file");
    let directory = File::open(root.path()).expect("directory");

    let descriptor_identity = file_identity(&file, &path).expect("descriptor identity");
    let path_identity =
        identity_at(&directory, OsStr::new("artifact"), &path).expect("path identity");

    assert_eq!(descriptor_identity, path_identity);
}

#[cfg(unix)]
#[test]
fn retained_program_rejects_source_path_replacement() {
    let root = tempfile::tempdir().expect("root");
    let path = root.path().join("program");
    fs::write(&path, b"original").expect("program");
    let file = open_regular_file(&path).expect("file");
    let program = descriptor_program(&file, &path).expect("retained program");

    fs::rename(&path, root.path().join("displaced")).expect("displace program");
    fs::write(&path, b"replacement").expect("replacement program");

    assert_matches!(
        program.revalidate(),
        Err(ReleaseError::FileIdentityChanged(_))
    );
}

#[test]
fn retained_directory_rejects_replacement_and_symlink_entries() {
    let root = tempfile::tempdir().expect("root");
    fs::create_dir(root.path().join("target")).expect("target");
    let directory =
        RetainedDirectory::create_new_under(root.path(), Path::new("target/release-review"), None)
            .expect("retained directory");
    directory
        .write_new(OsStr::new("reviewed.crate"), b"reviewed")
        .expect("reviewed file");

    fs::rename(
        root.path().join("target/release-review"),
        root.path().join("target/displaced"),
    )
    .expect("displace directory");
    fs::create_dir(root.path().join("target/release-review")).expect("replacement");
    assert_matches!(
        directory.revalidate(),
        Err(ReleaseError::FileIdentityChanged(_))
    );
}

#[test]
fn descriptor_enumeration_is_bounded_and_exact() {
    let root = tempfile::tempdir().expect("root");
    fs::create_dir(root.path().join("target")).expect("target");
    let directory =
        RetainedDirectory::create_new_under(root.path(), Path::new("target/release-review"), None)
            .expect("retained directory");
    directory
        .write_new(OsStr::new("alpha"), b"a")
        .expect("alpha");
    directory.write_new(OsStr::new("beta"), b"b").expect("beta");

    let mut names = directory.entry_names(2).expect("entry names");
    names.sort();
    assert_eq!(names, [OsString::from("alpha"), OsString::from("beta")]);
    assert_matches!(
        directory.entry_names(1),
        Err(ReleaseError::DirectoryTooLarge { limit: 1, .. })
    );
}

#[cfg(unix)]
#[test]
fn descriptor_cleanup_removes_nested_entries_without_following_symlinks() {
    use std::os::unix::fs::symlink;

    let root = tempfile::tempdir().expect("root");
    fs::create_dir(root.path().join("target")).expect("target");
    let work =
        RetainedDirectory::create_new_under(root.path(), Path::new("target/release-work"), None)
            .expect("work");
    let nested = work.create_directory(OsStr::new("nested")).expect("nested");
    nested
        .write_new(OsStr::new("artifact"), b"bytes")
        .expect("artifact");
    let outside = root.path().join("outside");
    fs::write(&outside, b"sentinel").expect("outside");
    symlink(&outside, work.path().join("alias")).expect("symlink");
    let work_path = work.path().to_path_buf();

    work.remove_tree().expect("remove tree");
    assert!(!work_path.exists());
    assert_eq!(fs::read(&outside).expect("outside bytes"), b"sentinel");
}

#[test]
fn descriptor_cleanup_rejects_replaced_root_without_deleting_it() {
    let root = tempfile::tempdir().expect("root");
    fs::create_dir(root.path().join("target")).expect("target");
    let work =
        RetainedDirectory::create_new_under(root.path(), Path::new("target/release-work"), None)
            .expect("work");
    let work_path = work.path().to_path_buf();
    fs::rename(&work_path, root.path().join("target/displaced")).expect("displace");
    fs::create_dir(&work_path).expect("replacement");
    fs::write(work_path.join("sentinel"), b"replacement").expect("sentinel");

    assert_matches!(
        work.remove_tree(),
        Err(ReleaseError::FileIdentityChanged(_))
    );
    assert_eq!(
        fs::read(work_path.join("sentinel")).expect("replacement survives"),
        b"replacement"
    );
}

#[cfg(unix)]
#[test]
fn descriptor_cleanup_preserves_root_replacement_installed_before_tombstone_move() {
    let root = tempfile::tempdir().expect("root");
    fs::create_dir(root.path().join("target")).expect("target");
    let work =
        RetainedDirectory::create_new_under(root.path(), Path::new("target/release-work"), None)
            .expect("work");
    work.write_new(OsStr::new("artifact"), b"original")
        .expect("artifact");
    let work_path = work.path().to_path_buf();
    let displaced_path = root.path().join("target/displaced-original");
    let replacement_seen = Rc::new(Cell::new(false));
    let hook_seen = Rc::clone(&replacement_seen);
    let hook_work_path = work_path.clone();
    let hook_displaced_path = displaced_path.clone();

    let result = with_remove_tree_hook(
        move |event| {
            if event.point == RemoveTreeHookPoint::BeforeRoot
                && event.original_path == hook_work_path
                && !hook_seen.replace(true)
            {
                fs::rename(&hook_work_path, &hook_displaced_path).expect("displace retained");
                fs::create_dir(&hook_work_path).expect("replacement root");
                fs::write(hook_work_path.join("sentinel"), b"replacement")
                    .expect("replacement sentinel");
            }
        },
        || work.remove_tree(),
    );

    assert_matches!(result, Err(ReleaseError::FileIdentityChanged(_)));
    assert!(replacement_seen.get());
    assert_eq!(
        fs::read(work_path.join("sentinel")).expect("replacement survives"),
        b"replacement"
    );
    assert_eq!(
        fs::read(displaced_path.join("artifact")).expect("retained tree survives"),
        b"original"
    );
}

#[cfg(unix)]
#[test]
fn descriptor_cleanup_preserves_root_replacement_installed_after_tombstone_move() {
    let root = tempfile::tempdir().expect("root");
    fs::create_dir(root.path().join("target")).expect("target");
    let work =
        RetainedDirectory::create_new_under(root.path(), Path::new("target/release-work"), None)
            .expect("work");
    work.write_new(OsStr::new("artifact"), b"original")
        .expect("artifact");
    let work_path = work.path().to_path_buf();
    let replacement_seen = Rc::new(Cell::new(false));
    let hook_seen = Rc::clone(&replacement_seen);
    let hook_work_path = work_path.clone();

    with_remove_tree_hook(
        move |event| {
            if event.point == RemoveTreeHookPoint::RootMoved
                && event.original_path == hook_work_path
                && !hook_seen.replace(true)
            {
                let tombstone_path = event.tombstone_path.expect("tombstone path");
                assert!(tombstone_path.exists());
                fs::create_dir(&hook_work_path).expect("replacement root");
                fs::write(hook_work_path.join("sentinel"), b"replacement")
                    .expect("replacement sentinel");
            }
        },
        || work.remove_tree(),
    )
    .expect("remove retained tree");

    assert!(replacement_seen.get());
    assert_eq!(
        fs::read(work_path.join("sentinel")).expect("replacement survives"),
        b"replacement"
    );
    assert!(!work_path.join("artifact").exists());
}

#[cfg(unix)]
#[test]
fn regular_file_open_rejects_symlink_substitution() {
    use std::os::unix::fs::symlink;

    let root = tempfile::tempdir().expect("root");
    let real = root.path().join("real");
    fs::write(&real, b"bytes").expect("real file");
    let alias = root.path().join("alias");
    symlink(&real, &alias).expect("symlink");
    assert_matches!(open_regular_file(&alias), Err(ReleaseError::SymlinkPath(_)));
}
