use std::collections::BTreeMap;
use std::fs::File;
use std::path::Path;
use std::time::Duration;

use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::{
    RELEASE_VERSION, ReleaseError, artifact, authorization, cancellation::ReleaseCancellation,
    repository,
};

#[cfg(test)]
mod retry_tests;
mod ruleset;
mod target;
#[cfg(test)]
mod tests;

const API_HOST: &str = "https://api.github.com";
const UPLOAD_HOST: &str = "https://uploads.github.com";
const API_VERSION: &str = "2022-11-28";
const MAX_RESPONSE_BYTES: u64 = 1 << 20;
// GitHub's by-tag release endpoint returns published releases only; drafts are
// visible solely through the paginated release list, so draft verification
// scans that list under an explicit page bound and fails closed past it.
const RELEASE_LIST_PAGE_SIZE: usize = 100;
const RELEASE_LIST_PAGE_BOUND: usize = 10;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);
const GET_MAX_ATTEMPTS: u32 = 3;
const GET_RETRY_BASE_DELAY: Duration = Duration::from_secs(1);
pub(crate) fn require_production_tag(tag: &str) -> Result<(), ReleaseError> {
    target::PRODUCTION.require_tag(tag)
}

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
    let target = target::PRODUCTION;
    target.require_tag(tag)?;
    let cancellation = ReleaseCancellation::for_signals()?;
    authorization::require_a9_release(root, &cancellation)?;
    reviewed.revalidate()?;
    repository::require_unchanged(root, &commit)?;

    let token = GitHubToken::from_environment()?;
    let mut publisher = GitHubApi::new(target, cancellation.clone());
    publish_draft(
        &mut publisher,
        &mut reviewed,
        tag,
        &commit,
        target,
        &token,
        &cancellation,
    )?;
    drop(token);
    reviewed.revalidate()?;
    repository::require_unchanged(root, &commit)
}

fn publish_draft(
    publisher: &mut impl DraftPublisher,
    reviewed: &mut artifact::ReviewedAssets,
    tag: &str,
    commit: &str,
    target: target::GitHubTarget,
    token: &GitHubToken,
    cancellation: &ReleaseCancellation,
) -> Result<(), ReleaseError> {
    cancellation.check("GitHub draft publication")?;
    with_stable_remote_tag(publisher, tag, commit, token, |publisher| {
        reviewed.revalidate()?;
        let created = publisher.create_draft(tag, commit, token)?;
        validate_release(&created, tag, &[], RemoteReleaseState::Draft, target)?;

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
        validate_release(
            &recorded,
            tag,
            &expected_assets,
            RemoteReleaseState::Draft,
            target,
        )
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
    let target = target::PRODUCTION;
    target.require_tag(tag)?;
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
    let mut verifier = GitHubApi::new(target, cancellation.clone());
    with_stable_remote_tag(&mut verifier, tag, &commit, &token, |verifier| {
        cancellation.check("GitHub release verification")?;
        let recorded = release_in_state(verifier, tag, expected_state, &token)?;
        validate_release(&recorded, tag, &expected_assets, expected_state, target)
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
    target: target::GitHubTarget,
    cancellation: ReleaseCancellation,
    get_retry_base_delay: Duration,
}

impl GitHubApi {
    fn new(target: target::GitHubTarget, cancellation: ReleaseCancellation) -> Self {
        Self::with_hosts_and_retry_delay(
            API_HOST.to_string(),
            UPLOAD_HOST.to_string(),
            target,
            cancellation,
            GET_RETRY_BASE_DELAY,
        )
    }

    #[cfg(test)]
    fn with_hosts(
        api_host: String,
        upload_host: String,
        target: target::GitHubTarget,
        cancellation: ReleaseCancellation,
    ) -> Self {
        Self::with_hosts_and_retry_delay(
            api_host,
            upload_host,
            target,
            cancellation,
            Duration::ZERO,
        )
    }

    fn with_hosts_and_retry_delay(
        api_host: String,
        upload_host: String,
        target: target::GitHubTarget,
        cancellation: ReleaseCancellation,
        get_retry_base_delay: Duration,
    ) -> Self {
        let config = ureq::Agent::config_builder()
            .timeout_global(Some(REQUEST_TIMEOUT))
            .http_status_as_error(false)
            .build();
        Self {
            agent: config.into(),
            api_host,
            upload_host,
            target,
            cancellation,
            get_retry_base_delay,
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
        let bytes = self.get_bytes_with_retry(ureq::http::StatusCode::OK, operation, || {
            self.agent
                .get(url)
                .header("Accept", "application/vnd.github+json")
                .header("Authorization", &authorization)
                .header("X-GitHub-Api-Version", API_VERSION)
                .header("User-Agent", "stab-release/0.2.0")
                .call()
        })?;
        serde_json::from_slice(&bytes).map_err(ReleaseError::from)
    }

    fn get_public_json<T: DeserializeOwned>(
        &self,
        url: &str,
        operation: &str,
    ) -> Result<T, ReleaseError> {
        self.cancellation.check(operation)?;
        let bytes = self.get_bytes_with_retry(ureq::http::StatusCode::OK, operation, || {
            self.agent
                .get(url)
                .header("Accept", "application/vnd.github+json")
                .header("X-GitHub-Api-Version", API_VERSION)
                .header("User-Agent", "stab-release/0.2.0")
                .call()
        })?;
        serde_json::from_slice(&bytes).map_err(ReleaseError::from)
    }

    fn get_bytes_with_retry(
        &self,
        expected_status: ureq::http::StatusCode,
        operation: &str,
        mut request: impl FnMut() -> Result<ureq::http::Response<ureq::Body>, ureq::Error>,
    ) -> Result<Vec<u8>, ReleaseError> {
        let mut attempt = 1;
        loop {
            self.cancellation.check(operation)?;
            let mut response = match request() {
                Ok(response) => response,
                Err(error) if attempt < GET_MAX_ATTEMPTS && is_retryable_get_error(&error) => {
                    self.wait_for_get_retry(attempt, operation)?;
                    attempt += 1;
                    continue;
                }
                Err(error) => return Err(transport_error(operation, error)),
            };
            if response.status() != expected_status {
                if attempt < GET_MAX_ATTEMPTS && is_retryable_get_status(response.status()) {
                    self.wait_for_get_retry(attempt, operation)?;
                    attempt += 1;
                    continue;
                }
                return Err(unexpected_status(
                    operation,
                    response.status(),
                    expected_status,
                ));
            }
            match read_response_bytes(&mut response) {
                Ok(bytes) => {
                    self.cancellation.check(operation)?;
                    return Ok(bytes);
                }
                Err(error) if attempt < GET_MAX_ATTEMPTS && is_retryable_get_error(&error) => {
                    self.wait_for_get_retry(attempt, operation)?;
                    attempt += 1;
                }
                Err(error) => return Err(response_read_error(operation, error)),
            }
        }
    }

    fn wait_for_get_retry(&self, attempt: u32, operation: &str) -> Result<(), ReleaseError> {
        self.cancellation
            .sleep(self.get_retry_base_delay.saturating_mul(attempt), operation)
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
            return Err(unexpected_status(
                operation,
                response.status(),
                expected_status,
            ));
        }
        let bytes = read_response_bytes(&mut response)
            .map_err(|error| response_read_error(operation, error))?;
        self.cancellation.check(operation)?;
        serde_json::from_slice(&bytes).map_err(ReleaseError::from)
    }
}

fn read_response_bytes(
    response: &mut ureq::http::Response<ureq::Body>,
) -> Result<Vec<u8>, ureq::Error> {
    response
        .body_mut()
        .with_config()
        .limit(MAX_RESPONSE_BYTES)
        .read_to_vec()
}

fn unexpected_status(
    operation: &str,
    status: ureq::http::StatusCode,
    expected_status: ureq::http::StatusCode,
) -> ReleaseError {
    ReleaseError::GitHubRelease(format!(
        "{operation} returned HTTP {status}, expected {expected_status}"
    ))
}

fn response_read_error(operation: &str, error: ureq::Error) -> ReleaseError {
    ReleaseError::GitHubRelease(format!(
        "{operation} response exceeded its bound or could not be read: {error}"
    ))
}

fn is_retryable_get_status(status: ureq::http::StatusCode) -> bool {
    matches!(status.as_u16(), 408 | 500 | 502 | 503 | 504)
}

fn is_retryable_get_error(error: &ureq::Error) -> bool {
    matches!(
        error,
        ureq::Error::Protocol(_)
            | ureq::Error::Io(_)
            | ureq::Error::Timeout(_)
            | ureq::Error::HostNotFound
            | ureq::Error::ConnectionFailed
    )
}

impl DraftPublisher for GitHubApi {
    fn require_release_tag_ruleset(&mut self, _token: &GitHubToken) -> Result<(), ReleaseError> {
        let repository_url = format!("{}/repos/{}", self.api_host, self.target.repository);
        let repository: RemoteRepository = self.get_public_json(
            &repository_url,
            "GitHub release repository identity verification",
        )?;
        validate_repository(&repository, self.target)?;
        let url = format!(
            "{}/repos/{}/rulesets/{}",
            self.api_host, self.target.repository, self.target.ruleset.id,
        );
        let ruleset: ruleset::RemoteRuleset =
            self.get_public_json(&url, "GitHub release-tag ruleset verification")?;
        ruleset::validate(&ruleset, self.target.ruleset)
    }

    fn require_remote_annotated_tag(
        &mut self,
        tag: &str,
        commit: &str,
        token: &GitHubToken,
    ) -> Result<(), ReleaseError> {
        self.target.require_tag(tag)?;
        let reference_url = format!(
            "{}/repos/{}/git/ref/tags/{tag}",
            self.api_host, self.target.repository
        );
        let reference: RemoteReference =
            self.get_json(&reference_url, token, "GitHub tag reference query")?;
        require_annotated_reference(&reference, tag)?;
        let object_url = format!(
            "{}/repos/{}/git/tags/{}",
            self.api_host, self.target.repository, reference.object.sha
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
        self.target.require_tag(tag)?;
        let url = format!(
            "{}/repos/{}/releases",
            self.api_host, self.target.repository
        );
        self.post_json(
            &url,
            &CreateReleaseRequest {
                tag_name: tag,
                target_commitish: commit,
                name: self.target.title,
                body: self.target.notes,
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
            "{}/repos/{}/releases/{release_id}/assets?name={name}",
            self.upload_host, self.target.repository
        );
        self.post_file(&url, file, token, "GitHub asset upload")
    }

    fn published_release_by_tag(
        &mut self,
        tag: &str,
        token: &GitHubToken,
    ) -> Result<RemoteRelease, ReleaseError> {
        self.target.require_tag(tag)?;
        let url = format!(
            "{}/repos/{}/releases/tags/{tag}",
            self.api_host, self.target.repository
        );
        self.get_json(&url, token, "GitHub published release query")
    }

    fn unique_draft_release(
        &mut self,
        tag: &str,
        token: &GitHubToken,
    ) -> Result<RemoteRelease, ReleaseError> {
        self.target.require_tag(tag)?;
        let mut matches: Vec<RemoteRelease> = Vec::new();
        for page in 1..=RELEASE_LIST_PAGE_BOUND {
            let url = format!(
                "{}/repos/{}/releases?per_page={RELEASE_LIST_PAGE_SIZE}&page={page}",
                self.api_host, self.target.repository
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
    target: target::GitHubTarget,
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
        || release.name.as_deref() != Some(target.title)
        || release.body.as_deref() != Some(target.notes)
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

fn validate_repository(
    repository: &RemoteRepository,
    target: target::GitHubTarget,
) -> Result<(), ReleaseError> {
    if repository.id == target.repository_id
        && repository.full_name == target.repository
        && !repository.private
        && !repository.archived
    {
        Ok(())
    } else {
        Err(ReleaseError::GitHubRelease(format!(
            "GitHub repository {} does not match its pinned public, active numeric identity",
            target.repository
        )))
    }
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

#[derive(Clone, Debug, Deserialize)]
struct RemoteRepository {
    id: u64,
    full_name: String,
    private: bool,
    archived: bool,
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
