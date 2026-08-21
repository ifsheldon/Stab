use crate::{RELEASE_TAG, ReleaseError};

use super::ruleset;

pub(super) const PRODUCTION: GitHubTarget = GitHubTarget {
    repository: "ifsheldon/Stab",
    repository_id: 1_281_330_379,
    ruleset: ruleset::Contract {
        id: 20_419_793,
        name: "Protect Stab v0.2.0 release tag",
        repository: "ifsheldon/Stab",
        ref_include: "refs/tags/v0.2.0",
        node_id: "RRS_lACqUmVwb3NpdG9yec5MX4zLzgE3lNE",
        created_at: "2026-08-05T00:07:23.349Z",
        updated_at: "2026-08-05T00:07:23.370Z",
    },
    title: "Stab 0.2.0",
    notes: "Stab 0.2.0",
    tag: TagContract::Production,
};

pub(crate) const REHEARSAL_REPOSITORY: &str = "ifsheldon/Stab-release-rehearsal";
pub(super) const REHEARSAL_TAG_PREFIX: &str = "v0.2.0-rehearsal-";

pub(super) const REHEARSAL: GitHubTarget = GitHubTarget {
    repository: REHEARSAL_REPOSITORY,
    repository_id: 1_342_241_032,
    ruleset: ruleset::Contract {
        id: 21_169_813,
        name: "Protect Stab rehearsal tags",
        repository: REHEARSAL_REPOSITORY,
        ref_include: "refs/tags/v0.2.0-rehearsal-*",
        node_id: "RRS_lACqUmVwb3NpdG9yec5QAPkIzgFDBpU",
        created_at: "2026-08-21T21:37:59.584Z",
        updated_at: "2026-08-21T21:37:59.597Z",
    },
    title: "Stab 0.2.0 release rehearsal",
    notes: "Stab 0.2.0 release rehearsal. This draft must never be published.",
    tag: TagContract::Rehearsal,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TagContract {
    Production,
    Rehearsal,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct GitHubTarget {
    pub(super) repository: &'static str,
    pub(super) repository_id: u64,
    pub(super) ruleset: ruleset::Contract,
    pub(super) title: &'static str,
    pub(super) notes: &'static str,
    tag: TagContract,
}

impl GitHubTarget {
    pub(super) fn require_tag(self, tag: &str, commit: &str) -> Result<(), ReleaseError> {
        let expected = match self.tag {
            TagContract::Production => RELEASE_TAG.to_string(),
            TagContract::Rehearsal => rehearsal_tag(commit)?,
        };
        if tag == expected {
            Ok(())
        } else {
            Err(ReleaseError::TagName {
                expected,
                actual: tag.to_string(),
            })
        }
    }

    pub(super) fn require_tag_shape(self, tag: &str) -> Result<(), ReleaseError> {
        match self.tag {
            TagContract::Production if tag == RELEASE_TAG => Ok(()),
            TagContract::Rehearsal => {
                let Some(commit) = tag.strip_prefix(REHEARSAL_TAG_PREFIX) else {
                    return Err(ReleaseError::TagName {
                        expected: format!("{REHEARSAL_TAG_PREFIX}<40-lowercase-hex-commit>"),
                        actual: tag.to_string(),
                    });
                };
                if valid_commit(commit) {
                    Ok(())
                } else {
                    Err(ReleaseError::TagName {
                        expected: format!("{REHEARSAL_TAG_PREFIX}<40-lowercase-hex-commit>"),
                        actual: tag.to_string(),
                    })
                }
            }
            _ => Err(ReleaseError::TagName {
                expected: RELEASE_TAG.to_string(),
                actual: tag.to_string(),
            }),
        }
    }
}

pub(crate) fn rehearsal_tag(commit: &str) -> Result<String, ReleaseError> {
    if !valid_commit(commit) {
        return Err(ReleaseError::PackageContract(format!(
            "cannot derive a rehearsal tag from invalid commit identity {commit:?}"
        )));
    }
    Ok(format!("{REHEARSAL_TAG_PREFIX}{commit}"))
}

fn valid_commit(commit: &str) -> bool {
    commit.len() == 40
        && commit
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

#[cfg(test)]
mod tests {
    use super::*;

    const COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";

    #[test]
    fn lanes_accept_only_their_exact_tags() {
        PRODUCTION
            .require_tag(RELEASE_TAG, COMMIT)
            .expect("production tag");
        let rehearsal = rehearsal_tag(COMMIT).expect("rehearsal tag");
        REHEARSAL
            .require_tag(&rehearsal, COMMIT)
            .expect("derived rehearsal tag");
        assert!(PRODUCTION.require_tag(&rehearsal, COMMIT).is_err());
        assert!(REHEARSAL.require_tag(RELEASE_TAG, COMMIT).is_err());
        assert!(REHEARSAL.require_tag_shape(&rehearsal).is_ok());
    }

    #[test]
    fn rehearsal_tag_rejects_invalid_or_non_derived_commits() {
        for commit in [
            "short",
            "0123456789ABCDEF0123456789ABCDEF01234567",
            "g123456789abcdef0123456789abcdef01234567",
        ] {
            assert!(rehearsal_tag(commit).is_err());
        }
        let wrong = format!("{REHEARSAL_TAG_PREFIX}{}", "f".repeat(40));
        assert!(REHEARSAL.require_tag(&wrong, COMMIT).is_err());
    }
}
