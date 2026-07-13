use std::ffi::OsString;
use std::path::{Path, PathBuf};

use super::support_snapshot::SupportSnapshot;
use super::{BUILD_TIMEOUT, ExecutableError, PinnedExecutable};

#[derive(Debug)]
pub(super) struct CmakeSupport {
    program: PathBuf,
    module_alias: PathBuf,
    module_path: PathBuf,
    module_snapshot: SupportSnapshot,
}

impl CmakeSupport {
    pub(super) fn prepare(
        runtime: &Path,
        cmake: &PinnedExecutable,
    ) -> Result<Self, ExecutableError> {
        let query_environment = vec![
            (OsString::from("HOME"), runtime.as_os_str().to_owned()),
            (OsString::from("LANG"), OsString::from("C")),
            (OsString::from("LC_ALL"), OsString::from("C")),
            (
                OsString::from("PATH"),
                std::env::join_paths([PathBuf::from("/usr/bin"), PathBuf::from("/bin")])
                    .map_err(build_error)?,
            ),
            (OsString::from("TZ"), OsString::from("UTC")),
        ];
        let output = crate::process::run_qualification_process_with_timeout_and_arg0(
            &cmake.program(),
            cmake.path.as_os_str(),
            ["--system-information"],
            &[],
            Some(runtime),
            BUILD_TIMEOUT,
            &query_environment,
        )
        .map_err(build_error)?;
        if !output.success() {
            return Err(build_error(output.stderr.render_for_diagnostics()));
        }
        let text = std::str::from_utf8(&output.stdout.bytes).map_err(build_error)?;
        let module_root = text
            .lines()
            .find_map(|line| {
                line.strip_prefix("CMAKE_ROOT \"")
                    .and_then(|value| value.strip_suffix('"'))
            })
            .map(PathBuf::from)
            .filter(|path| path.is_absolute() && path.is_dir())
            .ok_or_else(|| {
                build_error("CMake system information omitted an existing absolute CMAKE_ROOT")
            })?;
        let module_name = module_root
            .file_name()
            .ok_or_else(|| build_error("CMake module root has no final component"))?
            .to_os_string();
        let module_snapshot = SupportSnapshot::create(
            runtime,
            "cmake-module-snapshot",
            std::slice::from_ref(&module_root),
        )?;
        let module_path = module_snapshot
            .paths()
            .first()
            .cloned()
            .ok_or_else(|| build_error("CMake module snapshot omitted its root"))?;

        let alias_root = runtime.join("cmake-root");
        let bin = alias_root.join("bin");
        let share = alias_root.join("share");
        std::fs::create_dir(&alias_root)
            .and_then(|()| std::fs::create_dir(&bin))
            .and_then(|()| std::fs::create_dir(&share))
            .map_err(build_error)?;
        let program = bin.join("cmake");
        std::os::unix::fs::symlink(cmake.program(), &program).map_err(build_error)?;
        let module_alias = share.join(module_name);
        std::os::unix::fs::symlink(&module_path, &module_alias).map_err(build_error)?;
        let support = Self {
            program,
            module_alias,
            module_path,
            module_snapshot,
        };
        support.verify(cmake)?;
        Ok(support)
    }

    pub(super) fn program(&self) -> &Path {
        &self.program
    }

    pub(super) fn digest(&self) -> &str {
        self.module_snapshot.digest()
    }

    pub(super) fn verify(&self, cmake: &PinnedExecutable) -> Result<(), ExecutableError> {
        self.module_snapshot.verify("cmake-module-snapshot")?;
        verify_alias(&self.program, &cmake.program())?;
        verify_alias(&self.module_alias, &self.module_path)?;
        Ok(())
    }
}

fn verify_alias(alias: &Path, target: &Path) -> Result<(), ExecutableError> {
    let actual = std::fs::read_link(alias).map_err(build_error)?;
    if actual == target {
        Ok(())
    } else {
        Err(build_error(format!(
            "runtime alias {} points to {}, expected {}",
            alias.display(),
            actual.display(),
            target.display()
        )))
    }
}

fn build_error(reason: impl ToString) -> ExecutableError {
    ExecutableError::Build {
        step: "CMake runtime preparation",
        reason: reason.to_string().into_boxed_str(),
    }
}
