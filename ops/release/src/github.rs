use std::collections::BTreeMap;
use std::fs::File;
use std::path::Path;
use std::time::Duration;

use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::{
    RELEASE_TAG, RELEASE_VERSION, ReleaseError, artifact, authorization,
    cancellation::ReleaseCancellation, repository,
};

mod ruleset;
#[cfg(test)]
mod tests;

const API_HOST: &str = "https://api.github.com";
const UPLOAD_HOST: &str = "https://uploads.github.com";
const API_VERSION: &str = "2022-11-28";
const REPOSITORY: &str = "ifsheldon/Stab";
const MAX_RESPONSE_BYTES: u64 = 1 << 20;
// GitHub's by-tag release endpoint returns published releases only; drafts are
// visible solely through the paginated release list, so draft verification
// scans that list under an explicit page bound and fails closed past it.
const RELEASE_LIST_PAGE_SIZE: usize = 100;
const RELEASE_LIST_PAGE_BOUND: usize = 10;
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
        let recorded = release_in_state(publisher, tag, RemoteReleaseState::Draft, token)?;
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
        let recorded = release_in_state(verifier, tag, expected_state, &token)?;
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

fn release_in_state<P: DraftPublisher>(
    publisher: &mut P,
    tag: &str,
    state: RemoteReleaseState,
    token: &GitHubToken,
) -> Result<RemoteRelease, ReleaseError> {
    match state {
        RemoteReleaseState::Draft => publisher.unique_draft_release(tag, token),
        RemoteReleaseState::Published => publisher.published_release_by_tag(tag, token),
    }
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

    /// Look up a published release through the by-tag endpoint, which never
    /// returns drafts.
    fn published_release_by_tag(
        &mut self,
        tag: &str,
        token: &GitHubToken,
    ) -> Result<RemoteRelease, ReleaseError>;

    /// Find exactly one draft release carrying `tag` in the release list, the
    /// only endpoint that exposes drafts; zero or several matches must fail.
    fn unique_draft_release(
        &mut self,
        tag: &str,
        token: &GitHubToken,
    ) -> Result<RemoteRelease, ReleaseError>;
}

struct GitHubToken(SecretString);

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
        Ok(Self(SecretString::from(token)))
    }

    /// The only exposure point; call it solely where the value is transmitted.
    fn expose(&self) -> &str {
        self.0.expose_secret()
    }
}

impl std::fmt::Debug for GitHubToken {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(formatter)
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

    fn published_release_by_tag(
        &mut self,
        tag: &str,
        token: &GitHubToken,
    ) -> Result<RemoteRelease, ReleaseError> {
        require_release_tag(tag)?;
        let url = format!("{}/repos/{REPOSITORY}/releases/tags/{tag}", self.api_host);
        self.get_json(&url, token, "GitHub published release query")
    }

    fn unique_draft_release(
        &mut self,
        tag: &str,
        token: &GitHubToken,
    ) -> Result<RemoteRelease, ReleaseError> {
        require_release_tag(tag)?;
        let mut matches: Vec<RemoteRelease> = Vec::new();
        for page in 1..=RELEASE_LIST_PAGE_BOUND {
            let url = format!(
                "{}/repos/{REPOSITORY}/releases?per_page={RELEASE_LIST_PAGE_SIZE}&page={page}",
                self.api_host
            );
            let releases: Vec<RemoteRelease> =
                self.get_json(&url, token, "GitHub draft release listing")?;
            let last_page = releases.len() < RELEASE_LIST_PAGE_SIZE;
            matches.extend(
                releases
                    .into_iter()
                    .filter(|release| release.tag_name == tag && release.draft),
            );
            if matches.len() > 1 {
                return Err(ReleaseError::GitHubRelease(format!(
                    "GitHub lists {} draft releases for {tag}, expected exactly one",
                    matches.len()
                )));
            }
            if last_page {
                return matches.pop().ok_or_else(|| {
                    ReleaseError::GitHubRelease(format!("GitHub lists no draft release for {tag}"))
                });
            }
        }
        Err(ReleaseError::GitHubRelease(format!(
            "GitHub release list still returned full pages after {RELEASE_LIST_PAGE_BOUND} pages \
             of {RELEASE_LIST_PAGE_SIZE}, so a unique draft for {tag} cannot be verified"
        )))
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
