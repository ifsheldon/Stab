use std::collections::{BTreeMap, BTreeSet};
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

use super::support_snapshot::SupportSnapshot;
use super::{BUILD_TIMEOUT, ExecutableError, PinnedExecutable, resolve_from_path};
use crate::RepoRoot;
use crate::qualification::receipt::ExecutableIdentity;

const INCLUDE_START: &str = "#include <...> search starts here:";
const INCLUDE_END: &str = "End of search list.";

#[derive(Debug)]
pub(super) struct CompilerSupport {
    pub(super) programs: OsString,
    pub(super) libraries: OsString,
    pub(super) c_includes: OsString,
    pub(super) cxx_includes: OsString,
    program_directory: PathBuf,
    snapshot: SupportSnapshot,
    pinned_programs: Vec<(String, PinnedExecutable)>,
}

impl CompilerSupport {
    pub(super) fn digest(&self) -> &str {
        self.snapshot.digest()
    }

    pub(super) fn identities(&self) -> Vec<ExecutableIdentity> {
        self.pinned_programs
            .iter()
            .map(|(_, program)| program.identity())
            .collect()
    }

    pub(super) fn verify(&self) -> Result<(), ExecutableError> {
        self.snapshot.verify("compiler-support-snapshot")?;
        for (alias, program) in &self.pinned_programs {
            let alias = self.program_directory.join(alias);
            let actual = std::fs::read_link(&alias).map_err(build_error)?;
            if actual != program.program() {
                return Err(build_error(format!(
                    "compiler support alias {} changed",
                    alias.display()
                )));
            }
        }
        Ok(())
    }
}

pub(super) fn resolve(
    root: &RepoRoot,
    cc: &PinnedExecutable,
    cxx: &PinnedExecutable,
    home: &Path,
    scratch: &Path,
    runtime: &Path,
) -> Result<CompilerSupport, ExecutableError> {
    let environment = query_environment(home, scratch)?;
    let c_library = query_path(root, cc, "-print-file-name=libgcc.a", &environment)?;
    let cxx_library = query_path(root, cxx, "-print-file-name=libgcc.a", &environment)?;
    let c_includes = query_include_paths(root, cc, "c", &environment)?;
    let cxx_includes = query_include_paths(root, cxx, "c++", &environment)?;

    let library_directories = [c_library, cxx_library]
        .into_iter()
        .map(|library| existing_parent("support library", &library))
        .collect::<Result<Vec<_>, _>>()?;
    let program_specs = [
        ("cc1", "compiler-cc1", cc),
        ("cc1plus", "compiler-cc1plus", cxx),
        ("collect2", "compiler-collect2", cxx),
        ("lto-wrapper", "compiler-lto-wrapper", cxx),
        ("as", "compiler-assembler", cc),
        ("ld", "compiler-linker", cc),
    ];
    let mut pinned_programs = Vec::with_capacity(program_specs.len());
    for (alias, role, compiler) in program_specs {
        let path = query_program_path(root, compiler, alias, &environment)?;
        pinned_programs.push((alias.to_string(), PinnedExecutable::open(role, &path)?));
    }
    let program_directory = runtime.join("compiler-programs");
    std::fs::create_dir(&program_directory).map_err(build_error)?;
    for (alias, program) in &pinned_programs {
        std::os::unix::fs::symlink(program.program(), program_directory.join(alias))
            .map_err(build_error)?;
    }

    let library_directories = canonical_directories(&library_directories)?;
    let c_includes = canonical_directories(&c_includes)?;
    let cxx_includes = canonical_directories(&cxx_includes)?;
    let roots = library_directories
        .iter()
        .chain(c_includes.iter())
        .chain(cxx_includes.iter())
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let snapshot = SupportSnapshot::create(runtime, "compiler-support-snapshot", &roots)?;
    let mapped = roots
        .iter()
        .cloned()
        .zip(snapshot.paths().iter().cloned())
        .collect::<BTreeMap<_, _>>();
    let mapped_libraries = mapped_paths(&library_directories, &mapped)?;
    let programs = std::env::join_paths(
        std::iter::once(&program_directory).chain(mapped_libraries.iter().copied()),
    )
    .map_err(build_error)?;
    let support = CompilerSupport {
        programs,
        libraries: std::env::join_paths(mapped_libraries).map_err(build_error)?,
        c_includes: join_mapped_paths(&c_includes, &mapped)?,
        cxx_includes: join_mapped_paths(&cxx_includes, &mapped)?,
        program_directory,
        snapshot,
        pinned_programs,
    };
    support.verify()?;
    Ok(support)
}

fn query_environment(
    home: &Path,
    scratch: &Path,
) -> Result<Vec<(OsString, OsString)>, ExecutableError> {
    Ok(vec![
        (OsString::from("HOME"), home.as_os_str().to_owned()),
        (OsString::from("LANG"), OsString::from("C")),
        (OsString::from("LC_ALL"), OsString::from("C")),
        (
            OsString::from("PATH"),
            std::env::join_paths([PathBuf::from("/usr/bin"), PathBuf::from("/bin")])
                .map_err(build_error)?,
        ),
        (OsString::from("TMPDIR"), scratch.as_os_str().to_owned()),
        (OsString::from("TZ"), OsString::from("UTC")),
    ])
}

fn query_path(
    root: &RepoRoot,
    compiler: &PinnedExecutable,
    argument: &str,
    environment: &[(OsString, OsString)],
) -> Result<PathBuf, ExecutableError> {
    let output = run_query(root, compiler, [argument], environment)?;
    let text = std::str::from_utf8(&output.stdout.bytes).map_err(build_error)?;
    let path = PathBuf::from(text.trim());
    if !path.is_absolute() || !path.is_file() {
        return Err(build_error(format!(
            "compiler returned missing or non-absolute support path {}",
            path.display()
        )));
    }
    Ok(path)
}

fn query_program_path(
    root: &RepoRoot,
    compiler: &PinnedExecutable,
    name: &'static str,
    environment: &[(OsString, OsString)],
) -> Result<PathBuf, ExecutableError> {
    let argument = format!("-print-prog-name={name}");
    let output = run_query(root, compiler, [argument], environment)?;
    let text = std::str::from_utf8(&output.stdout.bytes).map_err(build_error)?;
    let path = PathBuf::from(text.trim());
    if path.is_absolute() && path.is_file() {
        return Ok(path);
    }
    if path == Path::new(name) {
        return resolve_from_path(name);
    }
    Err(build_error(format!(
        "compiler returned missing or unsupported program path {}",
        path.display()
    )))
}

fn query_include_paths(
    root: &RepoRoot,
    compiler: &PinnedExecutable,
    language: &str,
    environment: &[(OsString, OsString)],
) -> Result<Vec<PathBuf>, ExecutableError> {
    let output = run_query(
        root,
        compiler,
        ["-E", "-x", language, "-", "-v"],
        environment,
    )?;
    let text = std::str::from_utf8(&output.stderr.bytes).map_err(build_error)?;
    parse_include_paths(text)
}

fn run_query<I, S>(
    root: &RepoRoot,
    compiler: &PinnedExecutable,
    args: I,
    environment: &[(OsString, OsString)],
) -> Result<crate::ProcessOutput, ExecutableError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = crate::process::run_qualification_process_with_timeout_and_arg0(
        &compiler.program(),
        compiler.path.as_os_str(),
        args,
        &[],
        Some(&root.path),
        BUILD_TIMEOUT,
        environment,
    )
    .map_err(build_error)?;
    if !output.success() {
        return Err(build_error(output.stderr.render_for_diagnostics()));
    }
    Ok(output)
}

fn parse_include_paths(text: &str) -> Result<Vec<PathBuf>, ExecutableError> {
    let mut collecting = false;
    let mut ended = false;
    let mut paths = Vec::new();
    let mut seen = BTreeSet::new();
    for line in text.lines() {
        if line.trim() == INCLUDE_START {
            if collecting || ended {
                return Err(build_error("compiler repeated its include search list"));
            }
            collecting = true;
            continue;
        }
        if collecting && line.trim() == INCLUDE_END {
            ended = true;
            collecting = false;
            continue;
        }
        if !collecting {
            continue;
        }
        let value = line.trim();
        if value.is_empty() || value.ends_with(" (framework directory)") {
            return Err(build_error(
                "compiler emitted an empty or unsupported include directory",
            ));
        }
        let path = PathBuf::from(value);
        if !path.is_absolute() || !path.is_dir() {
            return Err(build_error(format!(
                "compiler include directory {} is missing or non-absolute",
                path.display()
            )));
        }
        if seen.insert(path.clone()) {
            paths.push(path);
        }
    }
    if collecting || !ended || paths.is_empty() {
        return Err(build_error(
            "compiler did not emit one complete non-empty include search list",
        ));
    }
    Ok(paths)
}

fn existing_parent(kind: &str, path: &Path) -> Result<PathBuf, ExecutableError> {
    path.parent()
        .filter(|parent| parent.is_absolute() && parent.is_dir())
        .map(Path::to_path_buf)
        .ok_or_else(|| build_error(format!("compiler {kind} has no existing absolute parent")))
}

fn canonical_directories(paths: &[PathBuf]) -> Result<Vec<PathBuf>, ExecutableError> {
    paths
        .iter()
        .map(|path| std::fs::canonicalize(path).map_err(build_error))
        .collect()
}

fn join_mapped_paths(
    sources: &[PathBuf],
    mapped: &BTreeMap<PathBuf, PathBuf>,
) -> Result<OsString, ExecutableError> {
    std::env::join_paths(mapped_paths(sources, mapped)?).map_err(build_error)
}

fn mapped_paths<'a>(
    sources: &[PathBuf],
    mapped: &'a BTreeMap<PathBuf, PathBuf>,
) -> Result<Vec<&'a PathBuf>, ExecutableError> {
    sources
        .iter()
        .map(|source| {
            mapped.get(source).ok_or_else(|| {
                build_error(format!(
                    "support snapshot omitted source {}",
                    source.display()
                ))
            })
        })
        .collect()
}

fn build_error(reason: impl ToString) -> ExecutableError {
    ExecutableError::Build {
        step: "compiler support-path resolution",
        reason: reason.to_string().into_boxed_str(),
    }
}

#[cfg(test)]
mod tests {
    use super::parse_include_paths;

    #[test]
    fn include_search_list_requires_existing_absolute_directories() {
        let text = format!(
            "noise\n#include <...> search starts here:\n {}\n {}\nEnd of search list.\n",
            std::env::temp_dir().display(),
            std::env::current_dir()
                .expect("current directory")
                .display()
        );
        let paths = parse_include_paths(&text).expect("valid include list");
        assert_eq!(paths.len(), 2);

        assert!(parse_include_paths("noise only").is_err());
        assert!(
            parse_include_paths(
                "#include <...> search starts here:\n relative\nEnd of search list."
            )
            .is_err()
        );
    }
}
