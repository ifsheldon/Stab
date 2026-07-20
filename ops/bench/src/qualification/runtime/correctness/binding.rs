use std::collections::{BTreeMap, BTreeSet};
use std::ffi::{OsStr, OsString};
use std::io::{Read as _, Seek as _};
use std::mem::MaybeUninit;
use std::os::fd::OwnedFd;
use std::path::{Component, Path, PathBuf};

use super::CorrectnessError;

const TOP_LEVEL_NAMES: [&str; 6] = [
    "cases",
    "completion.json",
    "preflight.json",
    "report.json",
    "report.md",
    "request.json",
];
const EXECUTION_RECEIPT: &str = "execution-receipt.json";
const MAX_CASE_DIRECTORIES: usize = 8_192;
const MAX_DIRECTORY_ENTRIES: usize = MAX_CASE_DIRECTORIES + TOP_LEVEL_NAMES.len();

#[derive(Debug, Default)]
pub(in crate::qualification::runtime) struct CorrectnessArtifactBinding {
    tree: Option<BoundCorrectnessTree>,
}

impl CorrectnessArtifactBinding {
    pub(super) fn open(output_path: &Path) -> Result<Self, CorrectnessError> {
        let output = open_absolute_directory(output_path)?;
        require_names(&output, top_level_names(), output_path)?;
        let cases_path = output_path.join("cases");
        let cases = open_directory_at(&output, OsStr::new("cases"), &cases_path)?;
        Ok(Self {
            tree: Some(BoundCorrectnessTree {
                output_path: output_path.to_path_buf(),
                output,
                cases,
                top_artifacts: BTreeMap::new(),
                case_directories: BTreeMap::new(),
            }),
        })
    }

    pub(super) fn read_top_and_bind(
        &mut self,
        name: &'static str,
        maximum_bytes: usize,
    ) -> Result<Vec<u8>, CorrectnessError> {
        if !TOP_LEVEL_NAMES.contains(&name) || name == "cases" {
            return Err(CorrectnessError::ArtifactChanged(
                self.output_path().join(name),
            ));
        }
        let tree = self.tree_mut()?;
        bind_artifact(
            &tree.output,
            &mut tree.top_artifacts,
            name,
            tree.output_path.join(name),
            maximum_bytes,
        )
    }

    pub(super) fn bind_case_directories<'a>(
        &mut self,
        case_ids: impl IntoIterator<Item = &'a str>,
    ) -> Result<(), CorrectnessError> {
        let tree = self.tree_mut()?;
        if !tree.case_directories.is_empty() {
            return Err(CorrectnessError::ArtifactChanged(
                tree.output_path.join("cases"),
            ));
        }
        let expected = case_ids
            .into_iter()
            .map(OsString::from)
            .collect::<BTreeSet<_>>();
        if expected.is_empty() || expected.len() > MAX_CASE_DIRECTORIES {
            return Err(CorrectnessError::ArtifactChanged(
                tree.output_path.join("cases"),
            ));
        }
        require_names(
            &tree.cases,
            expected.clone(),
            &tree.output_path.join("cases"),
        )?;
        for case_id in expected {
            let case_path = tree.output_path.join("cases").join(&case_id);
            let descriptor = open_directory_at(&tree.cases, &case_id, &case_path)?;
            require_names(
                &descriptor,
                [OsString::from(EXECUTION_RECEIPT)].into_iter().collect(),
                &case_path,
            )?;
            tree.case_directories.insert(
                case_id,
                BoundCaseDirectory {
                    path: case_path,
                    descriptor,
                    receipt: None,
                },
            );
        }
        Ok(())
    }

    pub(super) fn read_case_receipt_and_bind(
        &mut self,
        case_id: &str,
        maximum_bytes: usize,
    ) -> Result<Vec<u8>, CorrectnessError> {
        let tree = self.tree_mut()?;
        let directory = tree
            .case_directories
            .get_mut(OsStr::new(case_id))
            .ok_or_else(|| CorrectnessError::ArtifactChanged(tree.output_path.join("cases")))?;
        if directory.receipt.is_some() {
            return Err(CorrectnessError::ArtifactChanged(
                directory.path.join(EXECUTION_RECEIPT),
            ));
        }
        let path = directory.path.join(EXECUTION_RECEIPT);
        let (bytes, artifact) = read_bound_artifact(
            &directory.descriptor,
            OsStr::new(EXECUTION_RECEIPT),
            path,
            maximum_bytes,
        )?;
        directory.receipt = Some(artifact);
        Ok(bytes)
    }

    pub(in crate::qualification::runtime) fn require_current(
        &self,
    ) -> Result<(), CorrectnessError> {
        let Some(tree) = &self.tree else {
            return Ok(());
        };
        let current_output = open_absolute_directory(&tree.output_path)?;
        require_same_file(&current_output, &tree.output, &tree.output_path)?;
        require_names(&tree.output, top_level_names(), &tree.output_path)?;
        require_directory_entry(
            &tree.output,
            OsStr::new("cases"),
            &tree.cases,
            &tree.output_path.join("cases"),
        )?;
        for (name, artifact) in &tree.top_artifacts {
            artifact.require_current(&tree.output, OsStr::new(name))?;
        }

        let expected_cases = tree.case_directories.keys().cloned().collect();
        require_names(&tree.cases, expected_cases, &tree.output_path.join("cases"))?;
        for (case_id, directory) in &tree.case_directories {
            require_directory_entry(&tree.cases, case_id, &directory.descriptor, &directory.path)?;
            require_names(
                &directory.descriptor,
                [OsString::from(EXECUTION_RECEIPT)].into_iter().collect(),
                &directory.path,
            )?;
            let receipt = directory.receipt.as_ref().ok_or_else(|| {
                CorrectnessError::ArtifactChanged(directory.path.join(EXECUTION_RECEIPT))
            })?;
            receipt.require_current(&directory.descriptor, OsStr::new(EXECUTION_RECEIPT))?;
        }
        Ok(())
    }

    fn tree_mut(&mut self) -> Result<&mut BoundCorrectnessTree, CorrectnessError> {
        self.tree
            .as_mut()
            .ok_or_else(|| CorrectnessError::ArtifactChanged(PathBuf::from("correctness")))
    }

    fn output_path(&self) -> &Path {
        self.tree.as_ref().map_or_else(
            || Path::new("correctness"),
            |tree| tree.output_path.as_path(),
        )
    }
}

#[derive(Debug)]
struct BoundCorrectnessTree {
    output_path: PathBuf,
    output: OwnedFd,
    cases: OwnedFd,
    top_artifacts: BTreeMap<&'static str, BoundCorrectnessArtifact>,
    case_directories: BTreeMap<OsString, BoundCaseDirectory>,
}

#[derive(Debug)]
struct BoundCaseDirectory {
    path: PathBuf,
    descriptor: OwnedFd,
    receipt: Option<BoundCorrectnessArtifact>,
}

#[derive(Debug)]
struct BoundCorrectnessArtifact {
    path: PathBuf,
    descriptor: OwnedFd,
    sha256: String,
    len: usize,
    maximum_bytes: usize,
}

impl BoundCorrectnessArtifact {
    fn require_current(&self, directory: &OwnedFd, name: &OsStr) -> Result<(), CorrectnessError> {
        let current = open_regular_at(directory, name, &self.path)?;
        require_same_file(&current, &self.descriptor, &self.path)?;
        let bytes = read_descriptor(&current, &self.path, self.maximum_bytes)?;
        if bytes.len() != self.len || super::super::run::sha256_hex(&bytes) != self.sha256 {
            return Err(CorrectnessError::ArtifactChanged(self.path.clone()));
        }
        Ok(())
    }
}

fn bind_artifact(
    directory: &OwnedFd,
    artifacts: &mut BTreeMap<&'static str, BoundCorrectnessArtifact>,
    name: &'static str,
    path: PathBuf,
    maximum_bytes: usize,
) -> Result<Vec<u8>, CorrectnessError> {
    if artifacts.contains_key(name) {
        return Err(CorrectnessError::ArtifactChanged(path));
    }
    let (bytes, artifact) = read_bound_artifact(directory, OsStr::new(name), path, maximum_bytes)?;
    artifacts.insert(name, artifact);
    Ok(bytes)
}

fn read_bound_artifact(
    directory: &OwnedFd,
    name: &OsStr,
    path: PathBuf,
    maximum_bytes: usize,
) -> Result<(Vec<u8>, BoundCorrectnessArtifact), CorrectnessError> {
    let descriptor = open_regular_at(directory, name, &path)?;
    let bytes = read_descriptor(&descriptor, &path, maximum_bytes)?;
    let artifact = BoundCorrectnessArtifact {
        path,
        descriptor,
        sha256: super::super::run::sha256_hex(&bytes),
        len: bytes.len(),
        maximum_bytes,
    };
    Ok((bytes, artifact))
}

fn top_level_names() -> BTreeSet<OsString> {
    TOP_LEVEL_NAMES.into_iter().map(OsString::from).collect()
}

fn require_names(
    directory: &OwnedFd,
    expected: BTreeSet<OsString>,
    path: &Path,
) -> Result<(), CorrectnessError> {
    if directory_names(directory, path)? == expected {
        Ok(())
    } else {
        Err(CorrectnessError::ArtifactChanged(path.to_path_buf()))
    }
}

fn directory_names(
    directory: &OwnedFd,
    path: &Path,
) -> Result<BTreeSet<OsString>, CorrectnessError> {
    let descriptor = open_directory_at(directory, OsStr::new("."), path)?;
    let mut buffer = [MaybeUninit::uninit(); 8192];
    let mut entries = rustix::fs::RawDir::new(descriptor, &mut buffer);
    let mut names = BTreeSet::new();
    while let Some(entry) = entries.next() {
        let entry = entry.map_err(read_error)?;
        let name = entry.file_name().to_bytes();
        if name == b"." || name == b".." {
            continue;
        }
        if names.len() == MAX_DIRECTORY_ENTRIES {
            return Err(CorrectnessError::ArtifactChanged(path.to_path_buf()));
        }
        use std::os::unix::ffi::OsStringExt as _;
        names.insert(OsString::from_vec(name.to_vec()));
    }
    Ok(names)
}

fn open_absolute_directory(path: &Path) -> Result<OwnedFd, CorrectnessError> {
    if !path.is_absolute() {
        return Err(CorrectnessError::ArtifactChanged(path.to_path_buf()));
    }
    let mut current =
        rustix::fs::open("/", directory_flags(), rustix::fs::Mode::empty()).map_err(read_error)?;
    for component in path.components() {
        match component {
            Component::RootDir => {}
            Component::Normal(name) => current = open_directory_at(&current, name, path)?,
            _ => return Err(CorrectnessError::ArtifactChanged(path.to_path_buf())),
        }
    }
    Ok(current)
}

fn open_directory_at(
    parent: &OwnedFd,
    name: &OsStr,
    path: &Path,
) -> Result<OwnedFd, CorrectnessError> {
    rustix::fs::openat(parent, name, directory_flags(), rustix::fs::Mode::empty())
        .map_err(|_| CorrectnessError::ArtifactChanged(path.to_path_buf()))
}

fn open_regular_at(
    directory: &OwnedFd,
    name: &OsStr,
    path: &Path,
) -> Result<OwnedFd, CorrectnessError> {
    rustix::fs::openat(
        directory,
        name,
        rustix::fs::OFlags::RDONLY
            | rustix::fs::OFlags::CLOEXEC
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::NONBLOCK,
        rustix::fs::Mode::empty(),
    )
    .map_err(|_| CorrectnessError::ArtifactChanged(path.to_path_buf()))
}

fn directory_flags() -> rustix::fs::OFlags {
    rustix::fs::OFlags::RDONLY
        | rustix::fs::OFlags::CLOEXEC
        | rustix::fs::OFlags::DIRECTORY
        | rustix::fs::OFlags::NOFOLLOW
}

fn require_directory_entry(
    parent: &OwnedFd,
    name: &OsStr,
    expected: &OwnedFd,
    path: &Path,
) -> Result<(), CorrectnessError> {
    let current = open_directory_at(parent, name, path)?;
    require_same_file(&current, expected, path)
}

fn require_same_file(
    current: &OwnedFd,
    expected: &OwnedFd,
    path: &Path,
) -> Result<(), CorrectnessError> {
    use std::os::unix::fs::MetadataExt as _;

    let current = std::fs::File::from(rustix::io::dup(current).map_err(read_error)?)
        .metadata()
        .map_err(|error| CorrectnessError::Read(error.to_string()))?;
    let expected = std::fs::File::from(rustix::io::dup(expected).map_err(read_error)?)
        .metadata()
        .map_err(|error| CorrectnessError::Read(error.to_string()))?;
    if current.dev() == expected.dev() && current.ino() == expected.ino() {
        Ok(())
    } else {
        Err(CorrectnessError::ArtifactChanged(path.to_path_buf()))
    }
}

fn read_descriptor(
    descriptor: &OwnedFd,
    path: &Path,
    maximum_bytes: usize,
) -> Result<Vec<u8>, CorrectnessError> {
    let duplicate = rustix::io::dup(descriptor).map_err(read_error)?;
    let mut file = std::fs::File::from(duplicate);
    file.rewind()
        .map_err(|error| CorrectnessError::Read(error.to_string()))?;
    let metadata = file
        .metadata()
        .map_err(|error| CorrectnessError::Read(error.to_string()))?;
    let maximum = u64::try_from(maximum_bytes).map_err(|_| CorrectnessError::SizeOverflow)?;
    if !metadata.is_file() || metadata.len() > maximum {
        return Err(CorrectnessError::ArtifactChanged(path.to_path_buf()));
    }
    let mut bytes = Vec::with_capacity(
        usize::try_from(metadata.len()).map_err(|_| CorrectnessError::SizeOverflow)?,
    );
    file.take(maximum.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| CorrectnessError::Read(error.to_string()))?;
    if bytes.len() > maximum_bytes {
        return Err(CorrectnessError::ArtifactChanged(path.to_path_buf()));
    }
    Ok(bytes)
}

fn read_error(error: rustix::io::Errno) -> CorrectnessError {
    CorrectnessError::Read(error.to_string())
}
