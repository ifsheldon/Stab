use crate::ReleaseError;

const CARGO_REGISTRY_TOKEN: &str = "CARGO_REGISTRY_TOKEN";
const CARGO_REGISTRIES_CRATES_IO_TOKEN: &str = "CARGO_REGISTRIES_CRATES_IO_TOKEN";
const GITHUB_TOKEN: &str = "GITHUB_TOKEN";
const GH_TOKEN: &str = "GH_TOKEN";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CredentialScope {
    PublishReviewed,
    CreateDraft,
    VerifyRemoteRelease,
}

impl CredentialScope {
    fn operation(self) -> &'static str {
        match self {
            Self::PublishReviewed => "publish-reviewed",
            Self::CreateDraft => "create-draft",
            Self::VerifyRemoteRelease => "verify-remote-release",
        }
    }

    fn forbidden(self) -> &'static [&'static str] {
        match self {
            Self::PublishReviewed => &[CARGO_REGISTRIES_CRATES_IO_TOKEN, GITHUB_TOKEN, GH_TOKEN],
            Self::CreateDraft | Self::VerifyRemoteRelease => &[
                CARGO_REGISTRY_TOKEN,
                CARGO_REGISTRIES_CRATES_IO_TOKEN,
                GH_TOKEN,
            ],
        }
    }
}

/// Enters a credentialed operation: makes this process non-dumpable before
/// anything else runs, then rejects credentials outside `scope`.
///
/// The token stays in this process's initial environment block, which
/// `/proc/<pid>/environ` exposes to every same-user process for as long as the
/// operator runs, including the third-party build scripts and proc macros the
/// release-evidence check builds. Removing the variable cannot help: that file
/// reflects the initial block regardless, and `std::env::remove_var` is
/// `unsafe` in this edition. A non-dumpable process denies that read to
/// unprivileged processes, so this is the control, and failing to apply it
/// stops the operation.
pub(crate) fn enter_scope(scope: CredentialScope) -> Result<(), ReleaseError> {
    protect_process_memory()?;
    require_scope_with(scope, |name| std::env::var_os(name))
}

/// Marks this process non-dumpable and confirms the kernel applied it.
pub(crate) fn protect_process_memory() -> Result<(), ReleaseError> {
    #[cfg(target_os = "linux")]
    {
        use rustix::process::{DumpableBehavior, dumpable_behavior, set_dumpable_behavior};

        let protection_error = |source: rustix::io::Errno| {
            ReleaseError::ProcessProtection(format!(
                "cannot mark the release operator non-dumpable: {source}"
            ))
        };
        set_dumpable_behavior(DumpableBehavior::NotDumpable).map_err(protection_error)?;
        if dumpable_behavior().map_err(protection_error)? != DumpableBehavior::NotDumpable {
            return Err(ReleaseError::ProcessProtection(
                "the kernel did not keep the release operator non-dumpable".to_string(),
            ));
        }
        Ok(())
    }
    #[cfg(not(target_os = "linux"))]
    {
        Err(ReleaseError::UnsupportedPlatform)
    }
}

fn require_scope_with<T>(
    scope: CredentialScope,
    get: impl Fn(&str) -> Option<T>,
) -> Result<(), ReleaseError> {
    let present = scope
        .forbidden()
        .iter()
        .copied()
        .filter(|name| get(name).is_some())
        .collect::<Vec<_>>();
    if present.is_empty() {
        return Ok(());
    }
    Err(ReleaseError::CredentialEnvironment {
        operation: scope.operation(),
        variables: present.join(", "),
    })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::ffi::OsString;

    use super::*;

    fn require_fixture_scope(
        scope: CredentialScope,
        variables: &[(&str, &str)],
    ) -> Result<(), ReleaseError> {
        let variables = variables
            .iter()
            .map(|(name, value)| (OsString::from(name), OsString::from(value)))
            .collect::<BTreeSet<_>>();
        super::require_scope_with(scope, |name| {
            variables
                .iter()
                .find_map(|(candidate, value)| (candidate == name).then(|| value.clone()))
        })
    }

    const MARKER_NAME: &str = "STAB_RELEASE_ENVIRON_MARKER";
    const MARKER_VALUE: &str = "harmless-environ-marker";
    const PROBE_MODE: &str = "STAB_RELEASE_ENVIRON_PROBE";

    /// Runs `mode` as the parent helper, which spawns the probe child.
    fn run_environ_parent(mode: &str) -> std::process::ExitStatus {
        std::process::Command::new(std::env::current_exe().expect("test executable"))
            .args([
                "--ignored",
                "--exact",
                "credentials::tests::environ_helper",
                "--nocapture",
            ])
            .env(PROBE_MODE, mode)
            .env(MARKER_NAME, MARKER_VALUE)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .expect("run environ parent helper")
    }

    /// Regression: descendants such as build scripts could read the
    /// operator's token from `/proc/<ppid>/environ`.
    #[cfg(target_os = "linux")]
    #[test]
    fn descendants_cannot_read_a_non_dumpable_parent_environment() {
        if rustix::process::geteuid().is_root() {
            eprintln!("root bypasses non-dumpable /proc protection; skipping");
            return;
        }
        assert_eq!(
            run_environ_parent("unprotected").code(),
            Some(0),
            "the probe must read an unprotected parent's environment"
        );
        assert_eq!(
            run_environ_parent("protected").code(),
            Some(0),
            "a protected parent's environment must stay unreadable"
        );
    }

    #[test]
    #[ignore = "executed only as a subprocess by credential boundary tests"]
    fn environ_helper() {
        let mode = std::env::var(PROBE_MODE).expect("probe mode");
        match mode.as_str() {
            "unprotected" | "protected" => {
                if mode == "protected" {
                    protect_process_memory().expect("non-dumpable parent");
                }
                let probe =
                    std::process::Command::new(std::env::current_exe().expect("test executable"))
                        .args([
                            "--ignored",
                            "--exact",
                            "credentials::tests::environ_helper",
                            "--nocapture",
                        ])
                        .env_clear()
                        .env(PROBE_MODE, "probe")
                        .stdout(std::process::Stdio::piped())
                        .stderr(std::process::Stdio::null())
                        .output()
                        .expect("run environ probe");
                let read_marker = probe
                    .stdout
                    .windows(b"read-marker".len())
                    .any(|window| window == b"read-marker");
                let expected = mode == "unprotected";
                std::process::exit(if read_marker == expected { 0 } else { 1 });
            }
            "probe" => {
                let parent = rustix::process::getppid().expect("parent process");
                let environ =
                    std::fs::read(format!("/proc/{}/environ", parent.as_raw_nonzero().get()));
                let needle = format!("{MARKER_NAME}={MARKER_VALUE}");
                if environ.is_ok_and(|bytes| {
                    bytes
                        .split(|byte| *byte == 0)
                        .any(|entry| entry == needle.as_bytes())
                }) {
                    print!("read-marker");
                } else {
                    print!("denied");
                }
            }
            other => {
                eprintln!("unknown environ probe mode {other}");
                std::process::exit(125);
            }
        }
    }

    #[test]
    fn publish_scope_allows_only_the_primary_cargo_token() {
        require_fixture_scope(
            CredentialScope::PublishReviewed,
            &[("CARGO_REGISTRY_TOKEN", "reviewed-registry-secret")],
        )
        .expect("primary Cargo token is allowed");

        for forbidden in [
            "CARGO_REGISTRIES_CRATES_IO_TOKEN",
            "GITHUB_TOKEN",
            "GH_TOKEN",
        ] {
            let error = require_fixture_scope(
                CredentialScope::PublishReviewed,
                &[
                    ("CARGO_REGISTRY_TOKEN", "reviewed-registry-secret"),
                    (forbidden, "must-not-appear-in-diagnostics"),
                ],
            )
            .expect_err("unrelated publication credential must be rejected");
            let diagnostic = error.to_string();
            assert!(diagnostic.contains(forbidden));
            assert!(!diagnostic.contains("must-not-appear-in-diagnostics"));
        }
    }

    #[test]
    fn draft_scope_allows_only_the_github_token() {
        for scope in [
            CredentialScope::CreateDraft,
            CredentialScope::VerifyRemoteRelease,
        ] {
            require_fixture_scope(scope, &[("GITHUB_TOKEN", "reviewed-github-secret")])
                .expect("primary GitHub token is allowed");

            for forbidden in [
                "CARGO_REGISTRY_TOKEN",
                "CARGO_REGISTRIES_CRATES_IO_TOKEN",
                "GH_TOKEN",
            ] {
                let error = require_fixture_scope(
                    scope,
                    &[
                        ("GITHUB_TOKEN", "reviewed-github-secret"),
                        (forbidden, "must-not-appear-in-diagnostics"),
                    ],
                )
                .expect_err("unrelated publication credential must be rejected");
                let diagnostic = error.to_string();
                assert!(diagnostic.contains(forbidden));
                assert!(!diagnostic.contains("must-not-appear-in-diagnostics"));
            }
        }
    }
}
