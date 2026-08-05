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

pub(crate) fn require_scope(scope: CredentialScope) -> Result<(), ReleaseError> {
    require_scope_with(scope, |name| std::env::var_os(name))
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
