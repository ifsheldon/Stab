use std::collections::BTreeMap;
use std::fs::File;
use std::path::Path;
use std::time::Duration;

use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::{
    RELEASE_TAG, RELEASE_VERSION, ReleaseError, artifact, authorization,
    cancellation::ReleaseCancellation, repository,
};

mod ruleset;

const API_HOST: &str = "https://api.github.com";
const UPLOAD_HOST: &str = "https://uploads.github.com";
const API_VERSION: &str = "2022-11-28";
const REPOSITORY: &str = "ifsheldon/Stab";
const MAX_RESPONSE_BYTES: u64 = 1 << 20;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);
const RELEASE_TITLE: &str = "Stab 0.2.0";
const RELEASE_NOTES: &str = "Stab 0.2.0";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RemoteReleaseState {
    Draft,
    Published,
}

pub(crate) fn create_verified_draft(
    root: &Path,
    assets: &Path,
    tag: &str,
    confirmation: &str,
) -> Result<(), ReleaseError> {
    if confirmation != RELEASE_VERSION {
        return Err(ReleaseError::PublicationConfirmation {
            expected: RELEASE_VERSION.to_string(),
            actual: confirmation.to_string(),
        });
    }
    let mut reviewed = artifact::review_assets(root, assets, tag)?;
    let commit = reviewed.commit().to_string();
    let cancellation = ReleaseCancellation::for_signals()?;
    authorization::require_a9_release(root, &cancellation)?;
    reviewed.revalidate()?;
    repository::require_unchanged(root, &commit)?;

    let token = GitHubToken::from_environment()?;
    let mut publisher = GitHubApi::new(cancellation.clone());
    publish_draft(
        &mut publisher,
        &mut reviewed,
        tag,
        &commit,
        &token,
        &cancellation,
    )?;
    drop(token);
    reviewed.revalidate()?;
    repository::require_unchanged(root, &commit)?;
    Ok(())
}

fn publish_draft(
    publisher: &mut impl DraftPublisher,
    reviewed: &mut artifact::ReviewedAssets,
    tag: &str,
    commit: &str,
    token: &GitHubToken,
    cancellation: &ReleaseCancellation,
) -> Result<(), ReleaseError> {
    cancellation.check("GitHub draft publication")?;
    with_stable_remote_tag(publisher, tag, commit, token, |publisher| {
        reviewed.revalidate()?;
        let created = publisher.create_draft(tag, commit, token)?;
        validate_release(&created, tag, &[], RemoteReleaseState::Draft)?;

        for asset in reviewed.assets_mut() {
            cancellation.check("GitHub asset upload")?;
            let upload = asset.upload_file()?;
            let recorded = publisher.upload_asset(created.id, asset.name(), upload, token)?;
            validate_asset(&recorded, asset.name(), asset.bytes(), asset.sha256())?;
        }
        cancellation.check("GitHub draft verification")?;
        let recorded = publisher.release_by_tag(tag, token)?;
        if recorded.id != created.id {
            return Err(ReleaseError::GitHubRelease(format!(
                "GitHub returned release {} after creating release {}",
                recorded.id, created.id
            )));
        }
        let expected_assets = reviewed
            .assets()
            .iter()
            .map(|asset| ExpectedAsset {
                name: asset.name().to_string(),
                bytes: asset.bytes(),
                sha256: asset.sha256().to_string(),
            })
            .collect::<Vec<_>>();
        validate_release(&recorded, tag, &expected_assets, RemoteReleaseState::Draft)
    })
}

pub(crate) fn verify_remote_release(
    root: &Path,
    assets: &Path,
    tag: &str,
    expected_state: RemoteReleaseState,
) -> Result<(), ReleaseError> {
    let reviewed = artifact::review_assets(root, assets, tag)?;
    let commit = reviewed.commit().to_string();
    let cancellation = ReleaseCancellation::for_signals()?;
    authorization::require_a9_release(root, &cancellation)?;
    reviewed.revalidate()?;
    repository::require_unchanged(root, &commit)?;

    let token = GitHubToken::from_environment()?;
    let expected_assets = reviewed
        .assets()
        .iter()
        .map(|asset| ExpectedAsset {
            name: asset.name().to_string(),
            bytes: asset.bytes(),
            sha256: asset.sha256().to_string(),
        })
        .collect::<Vec<_>>();
    let mut verifier = GitHubApi::new(cancellation.clone());
    with_stable_remote_tag(&mut verifier, tag, &commit, &token, |verifier| {
        cancellation.check("GitHub release verification")?;
        let recorded = verifier.release_by_tag(tag, &token)?;
        validate_release(&recorded, tag, &expected_assets, expected_state)
    })?;
    drop(token);
    reviewed.revalidate()?;
    repository::require_unchanged(root, &commit)
}

fn with_stable_remote_tag<P, F>(
    publisher: &mut P,
    tag: &str,
    commit: &str,
    token: &GitHubToken,
    operation: F,
) -> Result<(), ReleaseError>
where
    P: DraftPublisher,
    F: FnOnce(&mut P) -> Result<(), ReleaseError>,
{
    publisher.require_release_tag_ruleset(token)?;
    publisher.require_remote_annotated_tag(tag, commit, token)?;
    operation(publisher)?;
    publisher.require_remote_annotated_tag(tag, commit, token)?;
    publisher.require_release_tag_ruleset(token)
}

trait DraftPublisher {
    fn require_release_tag_ruleset(&mut self, token: &GitHubToken) -> Result<(), ReleaseError>;

    fn require_remote_annotated_tag(
        &mut self,
        tag: &str,
        commit: &str,
        token: &GitHubToken,
    ) -> Result<(), ReleaseError>;

    fn create_draft(
        &mut self,
        tag: &str,
        commit: &str,
        token: &GitHubToken,
    ) -> Result<RemoteRelease, ReleaseError>;

    fn upload_asset(
        &mut self,
        release_id: u64,
        name: &str,
        file: File,
        token: &GitHubToken,
    ) -> Result<RemoteAsset, ReleaseError>;

    fn release_by_tag(
        &mut self,
        tag: &str,
        token: &GitHubToken,
    ) -> Result<RemoteRelease, ReleaseError>;
}

struct GitHubToken(String);

impl GitHubToken {
    fn from_environment() -> Result<Self, ReleaseError> {
        let token = std::env::var_os("GITHUB_TOKEN").ok_or_else(|| {
            ReleaseError::GitHubRelease(
                "GITHUB_TOKEN is required for GitHub release operations".to_string(),
            )
        })?;
        let token = token.into_string().map_err(|_| {
            ReleaseError::GitHubRelease("GITHUB_TOKEN must contain valid UTF-8".to_string())
        })?;
        if token.is_empty() {
            return Err(ReleaseError::GitHubRelease(
                "GITHUB_TOKEN must not be empty".to_string(),
            ));
        }
        Ok(Self(token))
    }

    fn expose(&self) -> &str {
        &self.0
    }
}

struct GitHubApi {
    agent: ureq::Agent,
    api_host: String,
    upload_host: String,
    cancellation: ReleaseCancellation,
}

impl GitHubApi {
    fn new(cancellation: ReleaseCancellation) -> Self {
        Self::with_hosts(API_HOST.to_string(), UPLOAD_HOST.to_string(), cancellation)
    }

    fn with_hosts(
        api_host: String,
        upload_host: String,
        cancellation: ReleaseCancellation,
    ) -> Self {
        let config = ureq::Agent::config_builder()
            .timeout_global(Some(REQUEST_TIMEOUT))
            .http_status_as_error(false)
            .build();
        Self {
            agent: config.into(),
            api_host,
            upload_host,
            cancellation,
        }
    }

    fn get_json<T: DeserializeOwned>(
        &self,
        url: &str,
        token: &GitHubToken,
        operation: &str,
    ) -> Result<T, ReleaseError> {
        self.cancellation.check(operation)?;
        let authorization = format!("Bearer {}", token.expose());
        let response = self
            .agent
            .get(url)
            .header("Accept", "application/vnd.github+json")
            .header("Authorization", &authorization)
            .header("X-GitHub-Api-Version", API_VERSION)
            .header("User-Agent", "stab-release/0.2.0")
            .call()
            .map_err(|error| transport_error(operation, error))?;
        self.read_json(response, ureq::http::StatusCode::OK, operation)
    }

    fn post_json<T: DeserializeOwned, B: Serialize>(
        &self,
        url: &str,
        body: &B,
        token: &GitHubToken,
        operation: &str,
    ) -> Result<T, ReleaseError> {
        self.cancellation.check(operation)?;
        let authorization = format!("Bearer {}", token.expose());
        let body = serde_json::to_vec(body)?;
        let response = self
            .agent
            .post(url)
            .header("Accept", "application/vnd.github+json")
            .header("Authorization", &authorization)
            .header("X-GitHub-Api-Version", API_VERSION)
            .header("User-Agent", "stab-release/0.2.0")
            .header("Content-Type", "application/json")
            .send(&body)
            .map_err(|error| transport_error(operation, error))?;
        self.read_json(response, ureq::http::StatusCode::CREATED, operation)
    }

    fn post_file<T: DeserializeOwned>(
        &self,
        url: &str,
        file: File,
        token: &GitHubToken,
        operation: &str,
    ) -> Result<T, ReleaseError> {
        self.cancellation.check(operation)?;
        let authorization = format!("Bearer {}", token.expose());
        let response = self
            .agent
            .post(url)
            .header("Accept", "application/vnd.github+json")
            .header("Authorization", &authorization)
            .header("X-GitHub-Api-Version", API_VERSION)
            .header("User-Agent", "stab-release/0.2.0")
            .header("Content-Type", "application/octet-stream")
            .send(file)
            .map_err(|error| transport_error(operation, error))?;
        self.read_json(response, ureq::http::StatusCode::CREATED, operation)
    }

    fn read_json<T: DeserializeOwned>(
        &self,
        mut response: ureq::http::Response<ureq::Body>,
        expected_status: ureq::http::StatusCode,
        operation: &str,
    ) -> Result<T, ReleaseError> {
        self.cancellation.check(operation)?;
        if response.status() != expected_status {
            return Err(ReleaseError::GitHubRelease(format!(
                "{operation} returned HTTP {}, expected {expected_status}",
                response.status()
            )));
        }
        let bytes = response
            .body_mut()
            .with_config()
            .limit(MAX_RESPONSE_BYTES)
            .read_to_vec()
            .map_err(|error| {
                ReleaseError::GitHubRelease(format!(
                    "{operation} response exceeded its bound or could not be read: {error}"
                ))
            })?;
        self.cancellation.check(operation)?;
        serde_json::from_slice(&bytes).map_err(ReleaseError::from)
    }
}

impl DraftPublisher for GitHubApi {
    fn require_release_tag_ruleset(&mut self, token: &GitHubToken) -> Result<(), ReleaseError> {
        let url = format!(
            "{}/repos/{REPOSITORY}/rulesets/{}",
            self.api_host,
            ruleset::ID,
        );
        let ruleset: ruleset::RemoteRuleset =
            self.get_json(&url, token, "GitHub release-tag ruleset verification")?;
        ruleset::validate(&ruleset)
    }

    fn require_remote_annotated_tag(
        &mut self,
        tag: &str,
        commit: &str,
        token: &GitHubToken,
    ) -> Result<(), ReleaseError> {
        require_release_tag(tag)?;
        let reference_url = format!("{}/repos/{REPOSITORY}/git/ref/tags/{tag}", self.api_host);
        let reference: RemoteReference =
            self.get_json(&reference_url, token, "GitHub tag reference query")?;
        require_annotated_reference(&reference, tag)?;
        let object_url = format!(
            "{}/repos/{REPOSITORY}/git/tags/{}",
            self.api_host, reference.object.sha
        );
        let object: RemoteTag = self.get_json(&object_url, token, "GitHub tag object query")?;
        require_tag_commit(&object, tag, commit)
    }

    fn create_draft(
        &mut self,
        tag: &str,
        commit: &str,
        token: &GitHubToken,
    ) -> Result<RemoteRelease, ReleaseError> {
        require_release_tag(tag)?;
        let url = format!("{}/repos/{REPOSITORY}/releases", self.api_host);
        self.post_json(
            &url,
            &CreateReleaseRequest {
                tag_name: tag,
                target_commitish: commit,
                name: RELEASE_TITLE,
                body: RELEASE_NOTES,
                draft: true,
                prerelease: false,
                generate_release_notes: false,
                make_latest: "false",
            },
            token,
            "GitHub draft creation",
        )
    }

    fn upload_asset(
        &mut self,
        release_id: u64,
        name: &str,
        file: File,
        token: &GitHubToken,
    ) -> Result<RemoteAsset, ReleaseError> {
        validate_asset_name(name)?;
        let url = format!(
            "{}/repos/{REPOSITORY}/releases/{release_id}/assets?name={name}",
            self.upload_host
        );
        self.post_file(&url, file, token, "GitHub asset upload")
    }

    fn release_by_tag(
        &mut self,
        tag: &str,
        token: &GitHubToken,
    ) -> Result<RemoteRelease, ReleaseError> {
        require_release_tag(tag)?;
        let url = format!("{}/repos/{REPOSITORY}/releases/tags/{tag}", self.api_host);
        self.get_json(&url, token, "GitHub release query")
    }
}

fn validate_release(
    release: &RemoteRelease,
    tag: &str,
    expected_assets: &[ExpectedAsset],
    expected_state: RemoteReleaseState,
) -> Result<(), ReleaseError> {
    let valid_state = match expected_state {
        RemoteReleaseState::Draft => release.draft && release.published_at.is_none(),
        RemoteReleaseState::Published => {
            !release.draft
                && release
                    .published_at
                    .as_deref()
                    .is_some_and(|value| !value.is_empty())
        }
    };
    if release.id == 0
        || release.tag_name != tag
        || release.name.as_deref() != Some(RELEASE_TITLE)
        || release.body.as_deref() != Some(RELEASE_NOTES)
        || !valid_state
        || release.prerelease
    {
        return Err(ReleaseError::GitHubRelease(format!(
            "GitHub release is not the exact requested {} state",
            match expected_state {
                RemoteReleaseState::Draft => "private draft",
                RemoteReleaseState::Published => "published release",
            }
        )));
    }
    let expected = expected_assets
        .iter()
        .map(|asset| (asset.name.as_str(), (asset.bytes, asset.sha256.as_str())))
        .collect::<BTreeMap<_, _>>();
    let mut actual = BTreeMap::new();
    for asset in &release.assets {
        if actual.insert(asset.name.as_str(), asset).is_some() {
            return Err(ReleaseError::GitHubRelease(format!(
                "GitHub release repeats asset name {:?}",
                asset.name
            )));
        }
    }
    if actual.len() != expected.len() || actual.keys().ne(expected.keys()) {
        return Err(ReleaseError::GitHubRelease(format!(
            "GitHub release asset names differ: expected {:?}, found {:?}",
            expected.keys().collect::<Vec<_>>(),
            actual.keys().collect::<Vec<_>>()
        )));
    }
    for (name, (bytes, sha256)) in expected {
        let asset = actual.get(name).ok_or_else(|| {
            ReleaseError::GitHubRelease(format!("GitHub release is missing asset {name}"))
        })?;
        validate_asset(asset, name, bytes, sha256)?;
    }
    Ok(())
}

fn require_annotated_reference(reference: &RemoteReference, tag: &str) -> Result<(), ReleaseError> {
    if reference.object.kind == "tag" && is_sha1(&reference.object.sha) {
        Ok(())
    } else {
        Err(ReleaseError::GitHubRelease(format!(
            "remote {tag} is not an annotated tag object"
        )))
    }
}

fn require_tag_commit(object: &RemoteTag, tag: &str, commit: &str) -> Result<(), ReleaseError> {
    if object.tag == tag && object.object.kind == "commit" && object.object.sha == commit {
        Ok(())
    } else {
        Err(ReleaseError::GitHubRelease(format!(
            "remote annotated tag {tag} does not resolve to reviewed commit {commit}"
        )))
    }
}

fn validate_asset(
    asset: &RemoteAsset,
    name: &str,
    bytes: u64,
    sha256: &str,
) -> Result<(), ReleaseError> {
    let expected_digest = format!("sha256:{sha256}");
    if asset.name != name
        || asset.state != "uploaded"
        || asset.size != bytes
        || asset.digest.as_deref() != Some(expected_digest.as_str())
    {
        return Err(ReleaseError::GitHubRelease(format!(
            "GitHub asset {name} does not match its reviewed name, size, state, and digest"
        )));
    }
    Ok(())
}

fn require_release_tag(tag: &str) -> Result<(), ReleaseError> {
    if tag == RELEASE_TAG {
        Ok(())
    } else {
        Err(ReleaseError::TagName {
            expected: RELEASE_TAG.to_string(),
            actual: tag.to_string(),
        })
    }
}

fn validate_asset_name(name: &str) -> Result<(), ReleaseError> {
    let known = [
        "stab-linux-aarch64",
        "stab-linux-aarch64.sha256",
        "stab-linux-aarch64.json",
        "stab-macos-aarch64",
        "stab-macos-aarch64.sha256",
        "stab-macos-aarch64.json",
    ];
    if known.contains(&name) {
        Ok(())
    } else {
        Err(ReleaseError::GitHubRelease(format!(
            "unexpected GitHub asset name {name:?}"
        )))
    }
}

fn is_sha1(value: &str) -> bool {
    value.len() == 40 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn transport_error(operation: &str, error: ureq::Error) -> ReleaseError {
    ReleaseError::GitHubRelease(format!("{operation} transport failed: {error}"))
}

#[derive(Serialize)]
struct CreateReleaseRequest<'a> {
    tag_name: &'a str,
    target_commitish: &'a str,
    name: &'a str,
    body: &'a str,
    draft: bool,
    prerelease: bool,
    generate_release_notes: bool,
    make_latest: &'a str,
}

struct ExpectedAsset {
    name: String,
    bytes: u64,
    sha256: String,
}

#[derive(Debug, Deserialize)]
struct RemoteReference {
    object: RemoteGitObject,
}

#[derive(Debug, Deserialize)]
struct RemoteTag {
    tag: String,
    object: RemoteGitObject,
}

#[derive(Debug, Deserialize)]
struct RemoteGitObject {
    sha: String,
    #[serde(rename = "type")]
    kind: String,
}

#[derive(Debug, Deserialize)]
struct RemoteRelease {
    id: u64,
    tag_name: String,
    name: Option<String>,
    body: Option<String>,
    draft: bool,
    prerelease: bool,
    #[serde(default)]
    published_at: Option<String>,
    #[serde(default)]
    assets: Vec<RemoteAsset>,
}

#[derive(Debug, Deserialize)]
struct RemoteAsset {
    name: String,
    state: String,
    size: u64,
    digest: Option<String>,
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    use super::*;

    fn remote_asset(name: &str, bytes: &[u8]) -> RemoteAsset {
        RemoteAsset {
            name: name.to_string(),
            state: "uploaded".to_string(),
            size: bytes.len() as u64,
            digest: Some(format!("sha256:{}", crate::archive::sha256_bytes(bytes))),
        }
    }

    #[test]
    fn exact_remote_asset_identity_is_required() {
        let bytes = b"reviewed asset";
        let asset = remote_asset("stab-linux-aarch64", bytes);
        validate_asset(
            &asset,
            "stab-linux-aarch64",
            bytes.len() as u64,
            &crate::archive::sha256_bytes(bytes),
        )
        .expect("exact asset");

        let mut wrong_digest = remote_asset("stab-linux-aarch64", bytes);
        wrong_digest.digest = Some(format!("sha256:{}", "0".repeat(64)));
        assert!(
            validate_asset(
                &wrong_digest,
                "stab-linux-aarch64",
                bytes.len() as u64,
                &crate::archive::sha256_bytes(bytes),
            )
            .is_err()
        );
    }

    #[test]
    fn draft_creation_requires_an_exact_version_confirmation() {
        assert!(matches!(
            create_verified_draft(
                Path::new("."),
                Path::new("target/releases/not-opened"),
                RELEASE_TAG,
                "0.2.1"
            ),
            Err(ReleaseError::PublicationConfirmation { .. })
        ));
    }

    #[test]
    fn remote_tag_must_be_annotated_and_resolve_to_the_reviewed_commit() {
        let commit = "1".repeat(40);
        let reference = RemoteReference {
            object: RemoteGitObject {
                sha: "2".repeat(40),
                kind: "tag".to_string(),
            },
        };
        require_annotated_reference(&reference, RELEASE_TAG).expect("annotated reference");
        let object = RemoteTag {
            tag: RELEASE_TAG.to_string(),
            object: RemoteGitObject {
                sha: commit.clone(),
                kind: "commit".to_string(),
            },
        };
        require_tag_commit(&object, RELEASE_TAG, &commit).expect("reviewed commit");

        let lightweight = RemoteReference {
            object: RemoteGitObject {
                sha: commit.clone(),
                kind: "commit".to_string(),
            },
        };
        assert!(require_annotated_reference(&lightweight, RELEASE_TAG).is_err());
        assert!(require_tag_commit(&object, RELEASE_TAG, &"3".repeat(40)).is_err());
    }

    struct MovingTagPublisher {
        remote_commit: String,
        events: Vec<&'static str>,
    }

    impl DraftPublisher for MovingTagPublisher {
        fn require_release_tag_ruleset(
            &mut self,
            _token: &GitHubToken,
        ) -> Result<(), ReleaseError> {
            self.events.push("ruleset-check");
            Ok(())
        }

        fn require_remote_annotated_tag(
            &mut self,
            _tag: &str,
            commit: &str,
            _token: &GitHubToken,
        ) -> Result<(), ReleaseError> {
            self.events.push("tag-check");
            if self.remote_commit == commit {
                Ok(())
            } else {
                Err(ReleaseError::GitHubRelease(
                    "remote tag moved after draft validation".to_string(),
                ))
            }
        }

        fn create_draft(
            &mut self,
            _tag: &str,
            _commit: &str,
            _token: &GitHubToken,
        ) -> Result<RemoteRelease, ReleaseError> {
            Err(ReleaseError::GitHubRelease(
                "unexpected draft creation in tag-guard test".to_string(),
            ))
        }

        fn upload_asset(
            &mut self,
            _release_id: u64,
            _name: &str,
            _file: File,
            _token: &GitHubToken,
        ) -> Result<RemoteAsset, ReleaseError> {
            Err(ReleaseError::GitHubRelease(
                "unexpected asset upload in tag-guard test".to_string(),
            ))
        }

        fn release_by_tag(
            &mut self,
            _tag: &str,
            _token: &GitHubToken,
        ) -> Result<RemoteRelease, ReleaseError> {
            Err(ReleaseError::GitHubRelease(
                "unexpected release query in tag-guard test".to_string(),
            ))
        }
    }

    #[test]
    fn late_remote_tag_change_is_rejected_after_final_validation() {
        let reviewed_commit = "1".repeat(40);
        let moved_commit = "2".repeat(40);
        let mut publisher = MovingTagPublisher {
            remote_commit: reviewed_commit.clone(),
            events: Vec::new(),
        };
        let token = GitHubToken("reviewed-token".to_string());

        let result = with_stable_remote_tag(
            &mut publisher,
            RELEASE_TAG,
            &reviewed_commit,
            &token,
            |publisher| {
                publisher.events.push("draft-created");
                publisher.events.push("assets-uploaded");
                publisher.events.push("final-release-validated");
                publisher.remote_commit = moved_commit;
                Ok(())
            },
        );

        assert!(matches!(
            result,
            Err(ReleaseError::GitHubRelease(detail))
                if detail == "remote tag moved after draft validation"
        ));
        assert_eq!(
            publisher.events,
            [
                "ruleset-check",
                "tag-check",
                "draft-created",
                "assets-uploaded",
                "final-release-validated",
                "tag-check",
            ]
        );
    }

    #[test]
    fn release_protection_and_tag_identity_bracket_remote_verification() {
        let reviewed_commit = "1".repeat(40);
        let mut publisher = MovingTagPublisher {
            remote_commit: reviewed_commit.clone(),
            events: Vec::new(),
        };
        let token = GitHubToken("reviewed-token".to_string());

        with_stable_remote_tag(
            &mut publisher,
            RELEASE_TAG,
            &reviewed_commit,
            &token,
            |publisher| {
                publisher.events.push("release-validated");
                Ok(())
            },
        )
        .expect("stable protected release tag");

        assert_eq!(
            publisher.events,
            [
                "ruleset-check",
                "tag-check",
                "release-validated",
                "tag-check",
                "ruleset-check",
            ]
        );
    }

    #[test]
    fn complete_private_draft_identity_is_required() {
        let identities = [
            ("stab-linux-aarch64", b"linux".as_slice()),
            ("stab-linux-aarch64.sha256", b"linux checksum".as_slice()),
            ("stab-linux-aarch64.json", b"linux manifest".as_slice()),
            ("stab-macos-aarch64", b"macos".as_slice()),
            ("stab-macos-aarch64.sha256", b"macos checksum".as_slice()),
            ("stab-macos-aarch64.json", b"macos manifest".as_slice()),
        ];
        let expected = identities
            .iter()
            .map(|(name, bytes)| ExpectedAsset {
                name: (*name).to_string(),
                bytes: bytes.len() as u64,
                sha256: crate::archive::sha256_bytes(bytes),
            })
            .collect::<Vec<_>>();
        let mut release = RemoteRelease {
            id: 7,
            tag_name: RELEASE_TAG.to_string(),
            name: Some(RELEASE_TITLE.to_string()),
            body: Some(RELEASE_NOTES.to_string()),
            draft: true,
            prerelease: false,
            published_at: None,
            assets: identities
                .iter()
                .map(|(name, bytes)| remote_asset(name, bytes))
                .collect(),
        };
        validate_release(&release, RELEASE_TAG, &expected, RemoteReleaseState::Draft)
            .expect("complete draft");

        release.assets.pop();
        assert!(
            validate_release(&release, RELEASE_TAG, &expected, RemoteReleaseState::Draft).is_err()
        );
        release.assets = identities
            .iter()
            .map(|(name, bytes)| remote_asset(name, bytes))
            .collect();
        release.draft = false;
        assert!(
            validate_release(
                &release,
                RELEASE_TAG,
                &expected,
                RemoteReleaseState::Published
            )
            .is_err()
        );
        release.published_at = Some("2026-08-04T00:00:00Z".to_string());
        validate_release(
            &release,
            RELEASE_TAG,
            &expected,
            RemoteReleaseState::Published,
        )
        .expect("complete published release");
    }

    #[test]
    fn draft_request_cannot_publish_the_release() {
        let request = CreateReleaseRequest {
            tag_name: RELEASE_TAG,
            target_commitish: "1",
            name: RELEASE_TITLE,
            body: RELEASE_NOTES,
            draft: true,
            prerelease: false,
            generate_release_notes: false,
            make_latest: "false",
        };
        let value = serde_json::to_value(request).expect("request JSON");
        assert_eq!(
            value,
            serde_json::json!({
                "tag_name": RELEASE_TAG,
                "target_commitish": "1",
                "name": RELEASE_TITLE,
                "body": RELEASE_NOTES,
                "draft": true,
                "prerelease": false,
                "generate_release_notes": false,
                "make_latest": "false"
            })
        );
    }

    #[test]
    fn draft_creation_sends_the_private_release_contract() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
        let address = listener.local_addr().expect("address");
        let commit = "1".repeat(40);
        let expected_commit = commit.clone();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("create request");
            let request = read_request(&mut stream);
            let header_end = find_subslice(&request, b"\r\n\r\n").expect("headers") + 4;
            let headers = String::from_utf8_lossy(
                request.get(..header_end).expect("bounded request headers"),
            );
            assert!(headers.starts_with("POST /repos/ifsheldon/Stab/releases HTTP/1.1\r\n"));
            let lowercase_headers = headers.to_ascii_lowercase();
            assert!(lowercase_headers.contains("authorization: bearer reviewed-token\r\n"));
            let request_body: serde_json::Value =
                serde_json::from_slice(request.get(header_end..).expect("bounded request body"))
                    .expect("request JSON");
            assert_eq!(
                request_body,
                serde_json::json!({
                    "tag_name": RELEASE_TAG,
                    "target_commitish": expected_commit,
                    "name": RELEASE_TITLE,
                    "body": RELEASE_NOTES,
                    "draft": true,
                    "prerelease": false,
                    "generate_release_notes": false,
                    "make_latest": "false"
                })
            );
            let response = format!(
                "{{\"id\":42,\"tag_name\":\"{RELEASE_TAG}\",\"name\":\"{RELEASE_TITLE}\",\"body\":\"{RELEASE_NOTES}\",\"draft\":true,\"prerelease\":false,\"assets\":[]}}"
            );
            write!(
                stream,
                "HTTP/1.1 201 Created\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response}",
                response.len()
            )
            .expect("response");
        });
        let cancellation = ReleaseCancellation::for_test();
        let host = format!("http://{address}");
        let mut api = GitHubApi::with_hosts(host.clone(), host, cancellation);
        let release = api
            .create_draft(
                RELEASE_TAG,
                &commit,
                &GitHubToken("reviewed-token".to_string()),
            )
            .expect("create draft");
        validate_release(&release, RELEASE_TAG, &[], RemoteReleaseState::Draft)
            .expect("private draft");
        server.join().expect("server");
    }

    #[test]
    fn asset_upload_sends_exact_file_bytes_and_scopes_the_token_to_the_header() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
        let address = listener.local_addr().expect("address");
        let payload = b"reviewed asset bytes".to_vec();
        let expected_payload = payload.clone();
        let digest = crate::archive::sha256_bytes(&payload);
        let expected_digest = digest.clone();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("upload request");
            let request = read_request(&mut stream);
            let header_end = find_subslice(&request, b"\r\n\r\n").expect("headers") + 4;
            let headers = String::from_utf8_lossy(
                request.get(..header_end).expect("bounded request headers"),
            );
            assert!(headers.starts_with(
                "POST /repos/ifsheldon/Stab/releases/42/assets?name=stab-linux-aarch64 HTTP/1.1\r\n"
            ));
            let lowercase_headers = headers.to_ascii_lowercase();
            assert!(lowercase_headers.contains("authorization: bearer reviewed-token\r\n"));
            assert_eq!(headers.matches("reviewed-token").count(), 1);
            assert_eq!(
                request.get(header_end..).expect("bounded request body"),
                expected_payload
            );
            let response = format!(
                "{{\"name\":\"stab-linux-aarch64\",\"state\":\"uploaded\",\"size\":{},\"digest\":\"sha256:{expected_digest}\"}}",
                expected_payload.len()
            );
            write!(
                stream,
                "HTTP/1.1 201 Created\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response}",
                response.len()
            )
            .expect("response");
        });
        let file_root = tempfile::tempdir().expect("file root");
        let path = file_root.path().join("asset");
        std::fs::write(&path, &payload).expect("asset");
        let file = std::fs::File::open(&path).expect("open asset");
        let cancellation = ReleaseCancellation::for_test();
        let host = format!("http://{address}");
        let mut api = GitHubApi::with_hosts(host.clone(), host, cancellation);
        let asset = api
            .upload_asset(
                42,
                "stab-linux-aarch64",
                file,
                &GitHubToken("reviewed-token".to_string()),
            )
            .expect("upload");
        validate_asset(&asset, "stab-linux-aarch64", payload.len() as u64, &digest)
            .expect("recorded asset");
        server.join().expect("server");
    }

    fn read_request(stream: &mut std::net::TcpStream) -> Vec<u8> {
        let mut request = Vec::new();
        let mut buffer = [0_u8; 4096];
        let mut expected = None;
        loop {
            let read = stream.read(&mut buffer).expect("request read");
            if read == 0 {
                break;
            }
            request.extend_from_slice(buffer.get(..read).expect("bounded read"));
            if expected.is_none()
                && let Some(header_start) = find_subslice(&request, b"\r\n\r\n")
            {
                let header_end = header_start + 4;
                let headers = String::from_utf8_lossy(
                    request.get(..header_end).expect("bounded request headers"),
                );
                let content_length = headers
                    .lines()
                    .find_map(|line| {
                        line.strip_prefix("Content-Length: ")
                            .or_else(|| line.strip_prefix("content-length: "))
                    })
                    .and_then(|value| value.parse::<usize>().ok())
                    .expect("content length");
                expected = Some(header_end + content_length);
            }
            if expected.is_some_and(|length| request.len() >= length) {
                break;
            }
        }
        request
    }

    fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
        haystack
            .windows(needle.len())
            .position(|window| window == needle)
    }
}
