use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

use crate::process::run_process;
use crate::{ProcessOutput, RepoRoot, ensure_stim_binary};

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
        let stim = ensure_stim_binary(&root, false).expect("pinned Stim binary");
        let library = stim
            .parent()
            .expect("Stim output directory")
            .join("libstim.a");
        assert!(library.is_file(), "pinned Stim static library is missing");

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
        let compile = run_process(Path::new("c++"), &args, source, Some(&root.path))
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
        run_process(&self.binary, args, stdin, Some(&self.root)).expect("execute pinned Stim probe")
    }
}
