use std::ffi::{OsStr, OsString};
use std::path::Path;

use crate::error::BenchError;
use crate::process::{check_success, run_checked_status, run_process};
use crate::root::RepoRoot;

pub(crate) fn validate_stim_source(
    stim_source: &Path,
    expected_tag: &str,
    expected_commit: &str,
) -> Result<(), BenchError> {
    if !stim_source.is_dir() {
        return Err(BenchError::MissingStimSource(stim_source.to_path_buf()));
    }
    let commit = git_output(stim_source, ["rev-parse", "HEAD"])?;
    if commit != expected_commit {
        return Err(BenchError::WrongStimCommit {
            actual: commit,
            expected: expected_commit.to_string(),
        });
    }
    let tag = git_output(stim_source, ["describe", "--tags", "--exact-match"])?;
    if tag != expected_tag {
        return Err(BenchError::WrongStimTag {
            actual: tag,
            expected: expected_tag.to_string(),
        });
    }
    let status = git_output(
        stim_source,
        ["status", "--porcelain", "--untracked-files=no"],
    )?;
    if !status.is_empty() {
        return Err(BenchError::DirtyStimSource {
            status: status.into_boxed_str(),
        });
    }
    Ok(())
}

pub(crate) fn ensure_stim_cli(root: &RepoRoot, stim_source: &Path) -> Result<(), BenchError> {
    let build_dir = root.build_dir();
    std::fs::create_dir_all(&build_dir).map_err(|source| BenchError::CreateOutputDir {
        path: build_dir.clone(),
        source,
    })?;
    run_checked_status(
        "cmake",
        [
            OsString::from("-S"),
            stim_source.as_os_str().to_os_string(),
            OsString::from("-B"),
            build_dir.as_os_str().to_os_string(),
            OsString::from("-DCMAKE_BUILD_TYPE=Release"),
        ],
        &root.path,
    )?;
    run_checked_status(
        "cmake",
        [
            OsString::from("--build"),
            build_dir.as_os_str().to_os_string(),
            OsString::from("--target"),
            OsString::from("stim"),
            OsString::from("--parallel"),
        ],
        &root.path,
    )?;
    if !root.stim_binary().is_file() {
        return Err(BenchError::MissingStimBinary(root.stim_binary()));
    }
    Ok(())
}

fn git_output<I, S>(working_dir: &Path, args: I) -> Result<String, BenchError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let args = args
        .into_iter()
        .map(|arg| OsString::from(arg.as_ref()))
        .collect::<Vec<_>>();
    let output = run_process(Path::new("git"), &args, b"", working_dir, true)?;
    check_success(Path::new("git"), &output)?;
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}
