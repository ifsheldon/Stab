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

    pub(crate) fn manifest(&self) -> PathBuf {
        self.path.join("benchmarks").join("manifest.csv")
    }

    pub(crate) fn performance_qualification(&self) -> PathBuf {
        self.path
            .join("benchmarks")
            .join("stim-qualification-suite.json")
    }

    pub(crate) fn correctness_manifest(&self) -> PathBuf {
        self.path.join("oracle").join("qualification-manifest.json")
    }

    pub(crate) fn feature_checklist(&self) -> PathBuf {
        self.path.join("docs").join("stab-feature-checklist.md")
    }

    pub(crate) fn primary_thresholds(&self) -> PathBuf {
        self.path
            .join("benchmarks")
            .join("m12-primary-thresholds.json")
    }

    pub(crate) fn primary_beta_waivers(&self) -> PathBuf {
        self.path
            .join("benchmarks")
            .join("m12-primary-beta-waivers.json")
    }

    pub(crate) fn primary_regression_waivers(&self) -> PathBuf {
        self.path
            .join("benchmarks")
            .join("m12-primary-regression-waivers.json")
    }

    pub(crate) fn compatibility_matrix(&self) -> PathBuf {
        self.path.join("oracle").join("compatibility-matrix.csv")
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

    pub(crate) fn stim_perf_binary(&self) -> PathBuf {
        self.build_dir()
            .join("out")
            .join(format!("stim_perf{}", std::env::consts::EXE_SUFFIX))
    }

    pub(crate) fn resolve_relative(&self, path: &Path) -> PathBuf {
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.path.join(path)
        }
    }

    pub(crate) fn benchmark_output_dir(&self, path: &Path) -> Result<PathBuf, BenchError> {
        if path.is_absolute() {
            return Err(BenchError::InvalidBenchmarkOutputDir {
                path: path.to_path_buf(),
                reason: "absolute paths are not allowed".to_string(),
            });
        }
        if path.components().any(unsafe_component) {
            return Err(BenchError::InvalidBenchmarkOutputDir {
                path: path.to_path_buf(),
                reason: "path must not contain root, parent, prefix, or current-dir components"
                    .to_string(),
            });
        }
        let mut components = path.components();
        if components.next() != Some(Component::Normal("target".as_ref()))
            || components.next() != Some(Component::Normal("benchmarks".as_ref()))
        {
            return Err(BenchError::InvalidBenchmarkOutputDir {
                path: path.to_path_buf(),
                reason: "path must be under target/benchmarks".to_string(),
            });
        }
        Ok(self.path.join(path))
    }

    pub(crate) fn create_benchmark_output_dir(&self, path: &Path) -> Result<PathBuf, BenchError> {
        let output_dir = self.benchmark_output_dir(path)?;
        self.reject_existing_benchmark_output_symlink(path)?;
        std::fs::create_dir_all(&output_dir).map_err(|source| BenchError::CreateOutputDir {
            path: output_dir.clone(),
            source,
        })?;
        self.check_benchmark_output_contained(&output_dir)?;
        Ok(output_dir)
    }

    fn check_benchmark_output_contained(&self, path: &Path) -> Result<(), BenchError> {
        let benchmark_root = std::fs::canonicalize(self.benchmark_root()).map_err(|source| {
            BenchError::CreateOutputDir {
                path: self.benchmark_root(),
                source,
            }
        })?;
        let output_dir =
            std::fs::canonicalize(path).map_err(|source| BenchError::CreateOutputDir {
                path: path.to_path_buf(),
                source,
            })?;
        if output_dir.starts_with(&benchmark_root) {
            Ok(())
        } else {
            Err(BenchError::BenchmarkOutputEscaped {
                path: output_dir,
                root: benchmark_root,
            })
        }
    }

    fn reject_existing_benchmark_output_symlink(&self, path: &Path) -> Result<(), BenchError> {
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

fn unsafe_component(component: Component<'_>) -> bool {
    matches!(
        component,
        Component::Prefix(_) | Component::RootDir | Component::ParentDir | Component::CurDir
    )
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::RepoRoot;

    #[cfg(unix)]
    #[test]
    fn benchmark_output_rejects_existing_symlink_component_before_creation() {
        let repo = tempfile::tempdir().expect("repo tempdir");
        let outside = tempfile::tempdir().expect("outside tempdir");
        let benchmark_root = repo.path().join("target").join("benchmarks");
        std::fs::create_dir_all(&benchmark_root).expect("create benchmark root");
        std::os::unix::fs::symlink(outside.path(), benchmark_root.join("link"))
            .expect("create symlink");
        let root = RepoRoot::resolve(repo.path()).expect("resolve root");

        let error = root
            .create_benchmark_output_dir(Path::new("target/benchmarks/link/new"))
            .expect_err("reject symlink output");

        assert!(error.to_string().contains("symlink"));
        assert!(!outside.path().join("new").exists());
    }
}
