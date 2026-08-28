use std::path::{Component, Path, PathBuf};

use crate::config::{BUILD_DIR, DEFAULT_STIM_PATH};
use crate::error::BenchError;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RepoRoot {
    pub(crate) path: PathBuf,
}

impl RepoRoot {
    pub(crate) fn resolve(path: &Path) -> Result<Self, BenchError> {
        let path = std::fs::canonicalize(path).map_err(|source| BenchError::ResolveRoot {
            path: path.to_path_buf(),
            source,
        })?;
        Ok(Self { path })
    }

    pub(crate) fn e2e_suite(&self) -> PathBuf {
        self.path.join("benchmarks").join("suite.toml")
    }

    pub(crate) fn e2e_suite_doc(&self) -> PathBuf {
        self.path.join("benchmarks").join("SUITE.md")
    }

    pub(crate) fn default_stim_source(&self) -> PathBuf {
        self.path.join(DEFAULT_STIM_PATH)
    }

    pub(crate) fn build_dir(&self) -> PathBuf {
        self.path.join(BUILD_DIR)
    }

    pub(crate) fn benchmark_root(&self) -> PathBuf {
        self.path.join("target").join("benchmarks")
    }

    pub(crate) fn stim_binary(&self) -> PathBuf {
        self.build_dir()
            .join("out")
            .join(format!("stim{}", std::env::consts::EXE_SUFFIX))
    }

    pub(crate) fn benchmark_output_dir(&self, path: &Path) -> Result<PathBuf, BenchError> {
        validate_output_path(path)?;
        Ok(self.path.join(path))
    }

    pub(crate) fn create_new_benchmark_output_dir(
        &self,
        path: &Path,
    ) -> Result<PathBuf, BenchError> {
        let output = self.benchmark_output_dir(path)?;
        self.reject_existing_symlink(path)?;
        let parent = output.parent().ok_or_else(|| BenchError::CreateOutputDir {
            path: output.clone(),
            source: std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "benchmark output has no parent",
            ),
        })?;
        std::fs::create_dir_all(parent).map_err(|source| BenchError::CreateOutputDir {
            path: parent.to_path_buf(),
            source,
        })?;
        self.reject_existing_symlink(path)?;
        std::fs::create_dir(&output).map_err(|source| BenchError::CreateOutputDir {
            path: output.clone(),
            source,
        })?;
        self.require_contained(&output)?;
        Ok(output)
    }

    fn require_contained(&self, path: &Path) -> Result<(), BenchError> {
        let benchmark_root = std::fs::canonicalize(self.benchmark_root()).map_err(|source| {
            BenchError::CreateOutputDir {
                path: self.benchmark_root(),
                source,
            }
        })?;
        let output = std::fs::canonicalize(path).map_err(|source| BenchError::CreateOutputDir {
            path: path.to_path_buf(),
            source,
        })?;
        if output.starts_with(&benchmark_root) {
            Ok(())
        } else {
            Err(BenchError::BenchmarkOutputEscaped {
                path: output,
                root: benchmark_root,
            })
        }
    }

    fn reject_existing_symlink(&self, path: &Path) -> Result<(), BenchError> {
        let mut current = self.path.clone();
        for component in path.components() {
            current.push(component.as_os_str());
            match std::fs::symlink_metadata(&current) {
                Ok(metadata) if metadata.file_type().is_symlink() => {
                    return Err(BenchError::InvalidBenchmarkOutputDir {
                        path: path.to_path_buf(),
                        reason: format!("existing component {} is a symlink", current.display()),
                    });
                }
                Ok(_) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
                Err(source) => {
                    return Err(BenchError::CreateOutputDir {
                        path: current,
                        source,
                    });
                }
            }
        }
        Ok(())
    }
}

fn validate_output_path(path: &Path) -> Result<(), BenchError> {
    if path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::Prefix(_)
                    | Component::RootDir
                    | Component::ParentDir
                    | Component::CurDir
            )
        })
    {
        return Err(BenchError::InvalidBenchmarkOutputDir {
            path: path.to_path_buf(),
            reason: "path must contain only normal relative components".to_string(),
        });
    }
    let mut components = path.components();
    if components.next() != Some(Component::Normal("target".as_ref()))
        || components.next() != Some(Component::Normal("benchmarks".as_ref()))
        || components.next().is_none()
    {
        return Err(BenchError::InvalidBenchmarkOutputDir {
            path: path.to_path_buf(),
            reason: "path must name a child under target/benchmarks".to_string(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn new_output_rejects_symlink_components_without_writing_outside() {
        let repo = tempfile::tempdir().expect("repo tempdir");
        let outside = tempfile::tempdir().expect("outside tempdir");
        let benchmark_root = repo.path().join("target/benchmarks");
        std::fs::create_dir_all(&benchmark_root).expect("create benchmark root");
        std::os::unix::fs::symlink(outside.path(), benchmark_root.join("link"))
            .expect("create symlink");
        let root = RepoRoot::resolve(repo.path()).expect("resolve root");

        root.create_new_benchmark_output_dir(Path::new("target/benchmarks/link/new"))
            .expect_err("reject symlink output");

        assert!(!outside.path().join("new").exists());
    }

    #[test]
    fn new_output_rejects_existing_or_escaping_paths_without_truncation() {
        let repo = tempfile::tempdir().expect("repo tempdir");
        let output = repo.path().join("target/benchmarks/evidence");
        std::fs::create_dir_all(&output).expect("create existing output");
        std::fs::write(output.join("sentinel"), b"preserve\n").expect("write sentinel");
        let root = RepoRoot::resolve(repo.path()).expect("resolve root");

        root.create_new_benchmark_output_dir(Path::new("target/benchmarks/evidence"))
            .expect_err("existing evidence output must not be reused");
        assert!(
            root.benchmark_output_dir(Path::new("target/benchmarks/../escape"))
                .is_err()
        );
        assert_eq!(
            std::fs::read(output.join("sentinel")).expect("read sentinel"),
            b"preserve\n"
        );
    }
}
