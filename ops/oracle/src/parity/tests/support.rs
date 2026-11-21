use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

use crate::process::{ProcessBudget, run_process, run_process_with_budget};
use crate::{ProcessOutput, RepoRoot, ensure_stim_library};

pub(super) struct PinnedStimProgram {
    _directory: tempfile::TempDir,
    binary: PathBuf,
    root: PathBuf,
}

impl PinnedStimProgram {
    pub(super) fn compile(name: &str, source: &[u8]) -> Self {
        let root = RepoRoot::resolve(
            &Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../..")
                .canonicalize()
                .expect("workspace root"),
        )
        .expect("repository root");
        let library = ensure_stim_library(&root, false).expect("pinned Stim static library");

        let directory = tempfile::tempdir().expect("pinned Stim probe directory");
        let binary = directory.path().join(name);
        let args = vec![
            OsString::from("-std=c++20"),
            OsString::from("-O2"),
            OsString::from("-I"),
            root.path.join("vendor/stim/src").into_os_string(),
            OsString::from("-x"),
            OsString::from("c++"),
            OsString::from("-"),
            OsString::from("-x"),
            OsString::from("none"),
            library.into_os_string(),
            OsString::from("-pthread"),
            OsString::from("-o"),
            binary.clone().into_os_string(),
        ];
        let compile = run_process_with_budget(
            Path::new("c++"),
            &args,
            source,
            &root.path,
            ProcessBudget::BUILD,
        )
        .expect("compile pinned Stim probe");
        assert!(
            compile.success(),
            "probe compilation failed: {}",
            compile.stderr.render_for_diagnostics()
        );
        Self {
            _directory: directory,
            binary,
            root: root.path,
        }
    }

    pub(super) fn run<I, S>(&self, args: I, stdin: &[u8]) -> ProcessOutput
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        run_process(&self.binary, args, stdin, &self.root).expect("execute pinned Stim probe")
    }
}
