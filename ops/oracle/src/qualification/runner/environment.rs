use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Duration;

use super::RunError;
use crate::RepoRoot;
use crate::qualification::executables::QualificationMetadataExecutables;
use crate::qualification::model::{QualificationManifest, SemanticDigest};
use crate::qualification::receipt::ExecutableIdentity;
use crate::qualification::report::ReportMetadata;

const METADATA_TIMEOUT: Duration = Duration::from_secs(120);

pub(in crate::qualification) fn environment_metadata_from_tools(
    root: &RepoRoot,
    manifest: &QualificationManifest,
    executables: &QualificationMetadataExecutables,
) -> Result<ReportMetadata, RunError> {
    environment_metadata_with(root, manifest, executables)
}

pub(in crate::qualification) fn current_environment_evidence(
    root: &RepoRoot,
    manifest: &QualificationManifest,
) -> Result<(ReportMetadata, Vec<ExecutableIdentity>), RunError> {
    let executables = QualificationMetadataExecutables::prepare(root)
        .map_err(|source| RunError::Environment(source.to_string().into_boxed_str()))?;
    let metadata = environment_metadata_with(root, manifest, &executables)?;
    Ok((metadata, executables.identities()))
}

fn environment_metadata_with(
    root: &RepoRoot,
    manifest: &QualificationManifest,
    executables: &QualificationMetadataExecutables,
) -> Result<ReportMetadata, RunError> {
    let (stim_tag, stim_commit) = validate_stim_source(root, executables)?;
    if manifest.stim_version != stim_tag || manifest.stim_commit != stim_commit {
        return Err(RunError::Environment(
            "checked qualification manifest disagrees with the validated Stim checkout".into(),
        ));
    }
    let stab_commit = git_text(root, executables, &root.path, &["rev-parse", "HEAD"])?;
    let local_modifications = !git_text(
        root,
        executables,
        &root.path,
        &["status", "--porcelain=v1", "--untracked-files=normal"],
    )?
    .is_empty();
    let rustc = command_text(
        root,
        executables.rustc(),
        executables.environment(),
        &["-vV"],
    )?;
    let rust_toolchain = rustc
        .lines()
        .find_map(|line| line.strip_prefix("release: "))
        .ok_or_else(|| RunError::Environment("rustc -vV omitted release".into()))?;
    let target_triple = rustc
        .lines()
        .find_map(|line| line.strip_prefix("host: "))
        .ok_or_else(|| RunError::Environment("rustc -vV omitted host".into()))?;
    Ok(ReportMetadata {
        qualification_manifest_digest: semantic_digest_text(manifest.semantic_digest),
        stab_commit,
        local_modifications,
        stim_tag,
        stim_commit,
        rust_toolchain: rust_toolchain.to_string(),
        target_triple: target_triple.to_string(),
        operating_system: std::env::consts::OS.to_string(),
        architecture: std::env::consts::ARCH.to_string(),
    })
}

fn validate_stim_source(
    root: &RepoRoot,
    executables: &QualificationMetadataExecutables,
) -> Result<(String, String), RunError> {
    let stim_source = root.stim_source();
    if !stim_source.is_dir() {
        return Err(RunError::Environment(
            format!(
                "Stim source directory does not exist at {}",
                stim_source.display()
            )
            .into(),
        ));
    }
    let commit = git_text(root, executables, &stim_source, &["rev-parse", "HEAD"])?;
    if commit != crate::STIM_COMMIT {
        return Err(RunError::Environment(
            format!(
                "Stim submodule is at commit {commit}, expected {}",
                crate::STIM_COMMIT
            )
            .into(),
        ));
    }
    let tag = git_text(
        root,
        executables,
        &stim_source,
        &["describe", "--tags", "--exact-match"],
    )?;
    if tag != crate::STIM_TAG {
        return Err(RunError::Environment(
            format!(
                "Stim submodule is at tag {tag}, expected {}",
                crate::STIM_TAG
            )
            .into(),
        ));
    }
    let status = git_text(
        root,
        executables,
        &stim_source,
        &["status", "--porcelain=v1", "--untracked-files=normal"],
    )?;
    if !status.is_empty() {
        return Err(RunError::Environment(
            format!("Stim submodule is not a clean pinned checkout:\n{status}").into(),
        ));
    }
    Ok((tag, commit))
}

fn git_text(
    root: &RepoRoot,
    executables: &QualificationMetadataExecutables,
    working_dir: &Path,
    args: &[&str],
) -> Result<String, RunError> {
    let view = GitView::open(executables, working_dir)?;
    let mut controlled = vec![
        OsString::from("--git-dir"),
        view.git_dir.as_os_str().to_owned(),
        OsString::from("--work-tree"),
        working_dir.as_os_str().to_owned(),
        OsString::from("-c"),
        OsString::from("core.fsmonitor=false"),
        OsString::from("-c"),
        OsString::from("core.untrackedCache=false"),
        OsString::from("-c"),
        OsString::from("submodule.recurse=false"),
    ];
    let mut initialize = controlled.clone();
    initialize.extend([OsString::from("read-tree"), OsString::from("HEAD")]);
    controlled.extend(args.iter().map(OsString::from));
    let mut environment = executables.environment().to_vec();
    environment.push((
        OsString::from("GIT_INDEX_FILE"),
        view.index.as_os_str().to_owned(),
    ));
    environment.push((
        OsString::from("GIT_NO_REPLACE_OBJECTS"),
        OsString::from("1"),
    ));
    command_text_os_at(
        root,
        executables.git(),
        &environment,
        &initialize,
        executables.runtime_path(),
    )?;
    command_text_os_at(
        root,
        executables.git(),
        &environment,
        &controlled,
        executables.runtime_path(),
    )
}

struct GitView {
    _temporary: tempfile::TempDir,
    git_dir: PathBuf,
    index: PathBuf,
}

impl GitView {
    fn open(
        executables: &QualificationMetadataExecutables,
        worktree: &Path,
    ) -> Result<Self, RunError> {
        Self::open_at(executables.runtime_path(), worktree)
    }

    fn open_at(runtime: &Path, worktree: &Path) -> Result<Self, RunError> {
        let source_git_dir = resolve_git_dir(worktree)?;
        let common_dir = resolve_common_dir(&source_git_dir)?;
        let temporary = tempfile::Builder::new()
            .prefix(".git-view-")
            .tempdir_in(runtime)
            .map_err(environment_error)?;
        let git_dir = temporary.path().join("git");
        std::fs::create_dir(&git_dir).map_err(environment_error)?;
        copy_git_file(&source_git_dir.join("HEAD"), &git_dir.join("HEAD"), true)?;
        copy_git_file(
            &common_dir.join("packed-refs"),
            &git_dir.join("packed-refs"),
            false,
        )?;
        copy_git_file(&common_dir.join("shallow"), &git_dir.join("shallow"), false)?;
        link_or_create_git_directory(&common_dir.join("objects"), &git_dir.join("objects"))?;
        link_or_create_git_directory(&common_dir.join("refs"), &git_dir.join("refs"))?;
        let index = git_dir.join("index");
        Ok(Self {
            _temporary: temporary,
            git_dir,
            index,
        })
    }
}

fn resolve_git_dir(worktree: &Path) -> Result<PathBuf, RunError> {
    let marker = worktree.join(".git");
    if marker.is_dir() {
        return std::fs::canonicalize(&marker).map_err(environment_error);
    }
    let bytes = std::fs::read(&marker).map_err(environment_error)?;
    if bytes.len() > 4096 {
        return Err(RunError::Environment(
            "Git directory marker is oversized".into(),
        ));
    }
    let text = std::str::from_utf8(&bytes)
        .map_err(|_| RunError::Environment("Git directory marker is not UTF-8".into()))?;
    let relative = text
        .trim()
        .strip_prefix("gitdir: ")
        .ok_or_else(|| RunError::Environment("Git directory marker is malformed".into()))?;
    let path = PathBuf::from(relative);
    let path = if path.is_absolute() {
        path
    } else {
        worktree.join(path)
    };
    std::fs::canonicalize(path).map_err(environment_error)
}

fn resolve_common_dir(git_dir: &Path) -> Result<PathBuf, RunError> {
    let marker = git_dir.join("commondir");
    if !marker.exists() {
        return Ok(git_dir.to_path_buf());
    }
    let bytes = std::fs::read(&marker).map_err(environment_error)?;
    if bytes.len() > 4096 {
        return Err(RunError::Environment(
            "Git common-directory marker is oversized".into(),
        ));
    }
    let text = std::str::from_utf8(&bytes)
        .map_err(|_| RunError::Environment("Git common-directory marker is not UTF-8".into()))?;
    let path = PathBuf::from(text.trim());
    let path = if path.is_absolute() {
        path
    } else {
        git_dir.join(path)
    };
    std::fs::canonicalize(path).map_err(environment_error)
}

fn copy_git_file(source: &Path, destination: &Path, required: bool) -> Result<(), RunError> {
    match crate::safe_file::open_regular_file(source) {
        Ok(mut input) => {
            let mut output = std::fs::OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(destination)
                .map_err(environment_error)?;
            std::io::copy(&mut input, &mut output).map_err(environment_error)?;
            Ok(())
        }
        Err(crate::safe_file::SafeFileError::Io(source))
            if !required && source.kind() == std::io::ErrorKind::NotFound =>
        {
            Ok(())
        }
        Err(source) => Err(RunError::Environment(source.to_string().into_boxed_str())),
    }
}

fn link_or_create_git_directory(source: &Path, destination: &Path) -> Result<(), RunError> {
    if source.is_dir() {
        std::os::unix::fs::symlink(source, destination).map_err(environment_error)
    } else {
        std::fs::create_dir(destination).map_err(environment_error)
    }
}

fn environment_error(source: impl ToString) -> RunError {
    RunError::Environment(source.to_string().into_boxed_str())
}

fn command_text(
    root: &RepoRoot,
    program: std::path::PathBuf,
    environment: &[(std::ffi::OsString, std::ffi::OsString)],
    args: &[&str],
) -> Result<String, RunError> {
    command_text_at(root, program, environment, args, &root.path)
}

fn command_text_at(
    _root: &RepoRoot,
    program: std::path::PathBuf,
    environment: &[(std::ffi::OsString, std::ffi::OsString)],
    args: &[&str],
    working_dir: &Path,
) -> Result<String, RunError> {
    let args = args.iter().map(OsString::from).collect::<Vec<_>>();
    command_text_os_at(_root, program, environment, &args, working_dir)
}

fn command_text_os_at(
    _root: &RepoRoot,
    program: std::path::PathBuf,
    environment: &[(std::ffi::OsString, std::ffi::OsString)],
    args: &[OsString],
    working_dir: &Path,
) -> Result<String, RunError> {
    let output = crate::process::run_qualification_process_with_timeout(
        &program,
        args,
        &[],
        Some(working_dir),
        METADATA_TIMEOUT,
        environment,
    )
    .map_err(|source| RunError::Environment(source.to_string().into_boxed_str()))?;
    if !output.success() {
        return Err(RunError::Environment(
            format!(
                "{} {} failed: {}",
                program.display(),
                args.iter()
                    .map(|argument| argument.to_string_lossy())
                    .collect::<Vec<_>>()
                    .join(" "),
                output.stderr.render_for_diagnostics()
            )
            .into(),
        ));
    }
    let text = std::str::from_utf8(&output.stdout.bytes).map_err(|_| {
        RunError::Environment(format!("{} output is not UTF-8", program.display()).into())
    })?;
    Ok(text.trim().to_string())
}

fn semantic_digest_text(digest: SemanticDigest) -> String {
    digest.to_string()
}

#[cfg(test)]
mod tests {
    use std::process::Command;

    use super::GitView;

    #[test]
    fn git_view_excludes_repository_local_configuration() {
        let temporary = tempfile::tempdir().expect("temporary root");
        let worktree = temporary.path().join("worktree");
        let runtime = temporary.path().join("runtime");
        std::fs::create_dir(&worktree).expect("worktree");
        std::fs::create_dir(&runtime).expect("runtime");
        let git = crate::qualification::executables::resolve_from_path("git").expect("git");
        let init = Command::new(&git)
            .args(["init", "--quiet"])
            .current_dir(&worktree)
            .env_clear()
            .env("HOME", &runtime)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .output()
            .expect("initialize repository");
        assert!(init.status.success());
        std::fs::write(worktree.join("tracked"), b"tracked").expect("tracked file");
        let add = Command::new(&git)
            .args(["add", "tracked"])
            .current_dir(&worktree)
            .env_clear()
            .env("HOME", &runtime)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .output()
            .expect("stage tracked file");
        assert!(add.status.success());
        let commit = Command::new(&git)
            .args([
                "-c",
                "user.name=Qualification Test",
                "-c",
                "user.email=qualification@example.invalid",
                "commit",
                "--quiet",
                "-m",
                "fixture",
            ])
            .current_dir(&worktree)
            .env_clear()
            .env("HOME", &runtime)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .output()
            .expect("commit fixture");
        assert!(commit.status.success());

        let excludes = temporary.path().join("hostile-excludes");
        std::fs::write(&excludes, b"untracked\n").expect("hostile excludes");
        let config = worktree.join(".git/config");
        let mut config_text = std::fs::read_to_string(&config).expect("repository config");
        config_text.push_str(&format!(
            "\n[core]\n\texcludesFile = {}\n",
            excludes.display()
        ));
        std::fs::write(&config, config_text).expect("hostile repository config");
        std::fs::write(worktree.join("untracked"), b"must remain visible").expect("untracked file");
        let skip = Command::new(&git)
            .args(["update-index", "--skip-worktree", "tracked"])
            .current_dir(&worktree)
            .env_clear()
            .env("HOME", &runtime)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .output()
            .expect("mark tracked file skip-worktree");
        assert!(skip.status.success());
        std::fs::write(worktree.join("tracked"), b"modified despite skip-worktree")
            .expect("modify tracked file");

        let view = GitView::open_at(&runtime, &worktree).expect("config-free Git view");
        assert!(!view.git_dir.join("config").exists());
        let read_tree = Command::new(&git)
            .arg("--git-dir")
            .arg(&view.git_dir)
            .arg("--work-tree")
            .arg(&worktree)
            .args(["read-tree", "HEAD"])
            .current_dir(&runtime)
            .env_clear()
            .env("HOME", &runtime)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_INDEX_FILE", &view.index)
            .output()
            .expect("reconstruct private index");
        assert!(read_tree.status.success());
        let status = Command::new(&git)
            .arg("--git-dir")
            .arg(&view.git_dir)
            .arg("--work-tree")
            .arg(&worktree)
            .args(["status", "--porcelain=v1", "--untracked-files=normal"])
            .current_dir(&runtime)
            .env_clear()
            .env("HOME", &runtime)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_INDEX_FILE", &view.index)
            .output()
            .expect("query config-free status");

        assert!(status.status.success());
        assert_eq!(status.stdout, b" M tracked\n?? untracked\n");
    }
}
