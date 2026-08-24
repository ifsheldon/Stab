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
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct GitHubTarget {
    pub(super) repository: &'static str,
    pub(super) repository_id: u64,
    pub(super) ruleset: ruleset::Contract,
    pub(super) title: &'static str,
    pub(super) notes: &'static str,
}

impl GitHubTarget {
    pub(super) fn require_tag(self, tag: &str, _commit: &str) -> Result<(), ReleaseError> {
        if tag == RELEASE_TAG {
            Ok(())
        } else {
            Err(ReleaseError::TagName {
                expected: RELEASE_TAG.to_string(),
                actual: tag.to_string(),
            })
        }
    }

    pub(super) fn require_tag_shape(self, tag: &str) -> Result<(), ReleaseError> {
        if tag == RELEASE_TAG {
            Ok(())
        } else {
            Err(ReleaseError::TagName {
                expected: RELEASE_TAG.to_string(),
                actual: tag.to_string(),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn production_accepts_only_the_exact_tag() {
        PRODUCTION
            .require_tag(RELEASE_TAG, "unused")
            .expect("production tag");
        assert!(PRODUCTION.require_tag("v0.2.1", "unused").is_err());
        assert!(PRODUCTION.require_tag_shape(RELEASE_TAG).is_ok());
        assert!(PRODUCTION.require_tag_shape("v0.2.1").is_err());
    }
}
