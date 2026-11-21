//! Builds of the pinned Stim oracle, its libstim helpers, and the Stab CLI.

use std::ffi::OsString;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use crate::process::{ProcessBudget, run_checked};
use crate::{OracleError, RepoRoot, StimSourceVersion, validate_stim_source};

const REVERSE_FLOW_HELPER: &str = "stab_stim_reverse_flow";
const PARSE_PRINT_HELPER: &str = "stab_stim_parse_print";

pub(crate) fn ensure_stim_binary(root: &RepoRoot, rebuild: bool) -> Result<PathBuf, OracleError> {
    let version = validate_stim_source(root)?;
    let binary = root.stim_binary();
    if binary.is_file() && !rebuild && build_stamp_matches(root, &version) {
        return Ok(binary);
    }

    let build_dir = root.build_dir();
    std::fs::create_dir_all(&build_dir).map_err(|source| OracleError::CreateBuildDir {
        path: build_dir.clone(),
        source,
    })?;

    run_checked(
        "cmake",
        [
            OsString::from("-S"),
            root.stim_source().into_os_string(),
            OsString::from("-B"),
            build_dir.clone().into_os_string(),
            OsString::from("-DCMAKE_BUILD_TYPE=Release"),
        ],
        b"",
        &root.path,
        ProcessBudget::BUILD,
    )?;
    cmake_build_target(root, &build_dir, "stim")?;
    if !binary.is_file() {
        return Err(OracleError::MissingStimBinary(binary));
    }
    write_build_stamp(root, &version)?;
    Ok(binary)
}

/// Ensures the pinned `libstim` static library that helpers and C++ probes
/// link against.
pub(crate) fn ensure_stim_library(
    root: &RepoRoot,
    rebuild_stim: bool,
) -> Result<PathBuf, OracleError> {
    ensure_stim_binary(root, rebuild_stim)?;
    let library = root.stim_library();
    if rebuild_stim || !library.is_file() {
        cmake_build_target(root, &root.build_dir(), "libstim")?;
    }
    if !library.is_file() {
        return Err(OracleError::MissingStimArtifact(library));
    }
    Ok(library)
}

pub(crate) fn ensure_stim_reverse_flow_helper(
    root: &RepoRoot,
    rebuild_stim: bool,
) -> Result<PathBuf, OracleError> {
    ensure_stim_helper(root, rebuild_stim, REVERSE_FLOW_HELPER)
}

pub(crate) fn ensure_stim_parse_print_helper(
    root: &RepoRoot,
    rebuild_stim: bool,
) -> Result<PathBuf, OracleError> {
    ensure_stim_helper(root, rebuild_stim, PARSE_PRINT_HELPER)
}

pub(crate) fn ensure_stim_helper(
    root: &RepoRoot,
    rebuild_stim: bool,
    target: &'static str,
) -> Result<PathBuf, OracleError> {
    let library = ensure_stim_library(root, rebuild_stim)?;
    let source_dir = root.stim_helper_source_dir();
    if !source_dir.is_dir() {
        return Err(OracleError::MissingStimArtifact(source_dir));
    }
    let helper_build_dir = root.stim_helper_build_dir();
    std::fs::create_dir_all(&helper_build_dir).map_err(|source| OracleError::CreateBuildDir {
        path: helper_build_dir.clone(),
        source,
    })?;
    run_checked(
        "cmake",
        [
            OsString::from("-S"),
            source_dir.into_os_string(),
            OsString::from("-B"),
            helper_build_dir.clone().into_os_string(),
            OsString::from("-DCMAKE_BUILD_TYPE=Release"),
            cmake_path_definition("STIM_SOURCE_DIR", &root.stim_source()),
            cmake_path_definition("STIM_LIBRARY", &library),
        ],
        b"",
        &root.path,
        ProcessBudget::BUILD,
    )?;
    cmake_build_target(root, &helper_build_dir, target)?;
    let binary = root.stim_helper_binary(target);
    if !binary.is_file() {
        return Err(OracleError::MissingStimBinary(binary));
    }
    Ok(binary)
}

fn cmake_build_target(root: &RepoRoot, build_dir: &Path, target: &str) -> Result<(), OracleError> {
    run_checked(
        "cmake",
        [
            OsString::from("--build"),
            build_dir.as_os_str().to_owned(),
            OsString::from("--target"),
            OsString::from(target),
            OsString::from("--parallel"),
        ],
        b"",
        &root.path,
        ProcessBudget::BUILD,
    )?;
    Ok(())
}

fn cmake_path_definition(name: &str, path: &Path) -> OsString {
    let mut value = OsString::from(format!("-D{name}="));
    value.push(path.as_os_str());
    value
}

fn build_stamp_matches(root: &RepoRoot, version: &StimSourceVersion) -> bool {
    std::fs::read_to_string(root.build_stamp())
        .is_ok_and(|actual| actual == build_stamp_content(version))
}

fn build_stamp_content(version: &StimSourceVersion) -> String {
    format!(
        "stim_tag={}\nstim_commit={}\nbuild_type=Release\n",
        version.tag, version.commit
    )
}

/// Writes the stamp through a sibling temporary file and renames it into
/// place, so an interrupted write never leaves a stamp that claims a build.
fn write_build_stamp(root: &RepoRoot, version: &StimSourceVersion) -> Result<(), OracleError> {
    let path = root.build_stamp();
    let failed = |source| OracleError::WriteBuildStamp {
        path: path.clone(),
        source,
    };
    let mut temporary = tempfile::Builder::new()
        .prefix(".stab-oracle-build-stamp-")
        .tempfile_in(root.build_dir())
        .map_err(failed)?;
    temporary
        .write_all(build_stamp_content(version).as_bytes())
        .and_then(|()| temporary.as_file().sync_all())
        .map_err(failed)?;
    temporary
        .persist(&path)
        .map_err(|error| failed(error.error))?;
    Ok(())
}

/// Builds the Stab CLI with the committed lockfile and returns the executable
/// Cargo reports, so the oracle never tests a stale or differently located
/// binary.
pub(crate) fn ensure_stab_cli_binary(root: &RepoRoot) -> Result<PathBuf, OracleError> {
    let output = run_checked(
        "cargo",
        [
            "build",
            "--locked",
            "--message-format=json-render-diagnostics",
            "-p",
            "stab-cli",
            "--bin",
            "stab",
        ],
        b"",
        &root.path,
        ProcessBudget::BUILD,
    )?;
    stab_executable_from_cargo_messages(&output.stdout.bytes)
}

/// Returns the executable of the last `stab` binary artifact in Cargo's JSON
/// message stream.
fn stab_executable_from_cargo_messages(stdout: &[u8]) -> Result<PathBuf, OracleError> {
    let mut executable = None;
    for message in cargo_metadata::Message::parse_stream(stdout) {
        if let cargo_metadata::Message::CompilerArtifact(artifact) =
            message.map_err(OracleError::CargoMessage)?
            && artifact.target.name == "stab"
            && artifact.target.is_bin()
            && let Some(path) = artifact.executable
        {
            executable = Some(path.into_std_path_buf());
        }
    }
    executable.ok_or(OracleError::MissingStabExecutable)
}

#[cfg(test)]
mod tests {
    use std::assert_matches;

    use super::stab_executable_from_cargo_messages;
    use crate::OracleError;

    const LIBRARY_ARTIFACT: &str = r#"{"reason":"compiler-artifact","package_id":"path+file:///w/crates/stab-core#0.2.0","manifest_path":"/w/crates/stab-core/Cargo.toml","target":{"kind":["lib"],"crate_types":["lib"],"name":"stab_core","src_path":"/w/crates/stab-core/src/lib.rs","edition":"2024","doc":true,"doctest":true,"test":true},"profile":{"opt_level":"0","debuginfo":2,"debug_assertions":true,"overflow_checks":true,"test":false},"features":[],"filenames":["/w/target/debug/deps/libstab_core.rlib"],"executable":null,"fresh":true}"#;
    const STAB_ARTIFACT: &str = r#"{"reason":"compiler-artifact","package_id":"path+file:///w/crates/stab-cli#0.2.0","manifest_path":"/w/crates/stab-cli/Cargo.toml","target":{"kind":["bin"],"crate_types":["bin"],"name":"stab","src_path":"/w/crates/stab-cli/src/main.rs","edition":"2024","doc":true,"doctest":false,"test":true},"profile":{"opt_level":"0","debuginfo":2,"debug_assertions":true,"overflow_checks":true,"test":false},"features":[],"filenames":["/elsewhere/target/debug/stab"],"executable":"/elsewhere/target/debug/stab","fresh":false}"#;
    const BUILD_FINISHED: &str = r#"{"reason":"build-finished","success":true}"#;

    #[test]
    fn cargo_messages_name_the_reported_stab_executable() {
        let stream = format!("{LIBRARY_ARTIFACT}\n{STAB_ARTIFACT}\n{BUILD_FINISHED}\n");

        let executable =
            stab_executable_from_cargo_messages(stream.as_bytes()).expect("stab executable");

        assert_eq!(
            executable,
            std::path::Path::new("/elsewhere/target/debug/stab")
        );
    }

    #[test]
    fn cargo_messages_without_a_stab_executable_fail() {
        let stream = format!("{LIBRARY_ARTIFACT}\n{BUILD_FINISHED}\n");

        assert_matches!(
            stab_executable_from_cargo_messages(stream.as_bytes()),
            Err(OracleError::MissingStabExecutable)
        );
    }
}
