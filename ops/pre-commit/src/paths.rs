//! Classification of staged repository-relative paths.
//!
//! This is the single home for the path lists the staged checks share, so the instruction-doc trigger, the Rust trigger, and the generated-tree exemption cannot drift apart.

use std::ffi::OsStr;
use std::path::{Component, Path};

/// Top-level directories that hold generated, vendored, or build output; anchored at the repository root so a module directory such as `crates/x/src/build/` is still checked.
const ROOT_EXCLUDED_DIRECTORIES: &[&str] =
    &["build", "coverage", "dist", "generated", "target", "vendor"];

/// Directory names that are tool caches or dependency trees wherever they appear.
const ANY_DEPTH_EXCLUDED_DIRECTORIES: &[&str] =
    &[".git", ".next", ".pytest_cache", ".turbo", "node_modules"];

/// File names whose change can alter `cargo fmt` or `cargo clippy` results.
const RUST_CONFIGURATION_FILES: &[&str] = &[
    ".clippy.toml",
    ".rustfmt.toml",
    "Cargo.lock",
    "Cargo.toml",
    "clippy.toml",
    "rust-toolchain",
    "rust-toolchain.toml",
    "rustfmt.toml",
];

const INSTRUCTION_DOC_FILES: &[&str] = &["AGENTS.md", "CLAUDE.md", "README.md"];

/// True when a change at `path` requires the instruction-document policy check.
pub(crate) fn is_docs_policy_path(path: &Path) -> bool {
    path == Path::new(".gitmodules") || file_name_in(path, INSTRUCTION_DOC_FILES)
}

/// True when a change at `path` can change Rust formatting, compilation, or lint results.
pub(crate) fn is_rust_affecting_path(path: &Path) -> bool {
    path.extension() == Some(OsStr::new("rs"))
        || file_name_in(path, RUST_CONFIGURATION_FILES)
        || path
            .parent()
            .is_some_and(|parent| parent.components().any(|c| c.as_os_str() == ".cargo"))
}

/// True when `path` lies in a generated, vendored, build-output, or dependency tree.
pub(crate) fn is_excluded_tree(path: &Path) -> bool {
    let Some(parent) = path.parent() else {
        return false;
    };
    let mut directories = parent.components().filter_map(|component| match component {
        Component::Normal(name) => Some(name),
        _ => None,
    });
    let root_excluded = directories.next().is_some_and(|first| {
        name_in(first, ROOT_EXCLUDED_DIRECTORIES) || name_in(first, ANY_DEPTH_EXCLUDED_DIRECTORIES)
    });
    root_excluded || directories.any(|name| name_in(name, ANY_DEPTH_EXCLUDED_DIRECTORIES))
}

fn file_name_in(path: &Path, names: &[&str]) -> bool {
    path.file_name().is_some_and(|name| name_in(name, names))
}

fn name_in(name: &OsStr, names: &[&str]) -> bool {
    names.iter().any(|candidate| name == OsStr::new(candidate))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rust_affecting_paths_include_toolchain_lint_format_and_cargo_config() {
        for path in [
            "crates/stab-core/src/lib.rs",
            "Cargo.toml",
            "crates/x/Cargo.toml",
            "Cargo.lock",
            "rust-toolchain.toml",
            "rust-toolchain",
            "clippy.toml",
            "crates/x/.clippy.toml",
            "rustfmt.toml",
            ".rustfmt.toml",
            ".cargo/config.toml",
            "crates/x/.cargo/config",
        ] {
            assert!(is_rust_affecting_path(Path::new(path)), "{path}");
        }
        for path in ["docs/README.md", "justfile", ".cargo", "src/cargo/x.md"] {
            assert!(!is_rust_affecting_path(Path::new(path)), "{path}");
        }
    }

    #[test]
    fn generated_tree_exclusions_are_anchored_at_the_root() {
        for path in [
            "vendor/stim/src/stim.cc",
            "target/debug/build.rs",
            "build/out.rs",
            "web/node_modules/x/index.js",
            "a/.git/hooks/x.sh",
        ] {
            assert!(is_excluded_tree(Path::new(path)), "{path}");
        }
        for path in [
            "crates/x/src/build/mod.rs",
            "crates/x/src/generated/table.rs",
            "crates/x/vendor/shim.rs",
            "build.rs",
            "crates/x/build.rs",
        ] {
            assert!(!is_excluded_tree(Path::new(path)), "{path}");
        }
    }
}
