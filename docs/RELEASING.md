# Releasing Stab

Stab product crates are published as one coordinated version because their pre-1.0 internal dependencies require exact sibling versions. Crates.io publication is irreversible and non-atomic, so release work starts only after source-current correctness, performance evidence, final audits, exact-revision CI, and documentation are complete.

## Release Set

`just release::publish-order` prints and validates the source-owned order:

1. `stab-kernels-simd`
2. `stab-model`
3. `stab-bits`
4. `stab-records`
5. `stab-algebra`
6. `stab-analysis`
7. `stab-decoder`
8. `stab-engine`
9. `stab-core`
10. `stab-cli`

Ops and test-support crates remain unpublished. Every product package must be version `0.2.0`, include `README.crates.md`, carry complete crates.io metadata, and require internal publishable path dependencies with exact version `=0.2.0`.

## Retired Scratch Rehearsal

The temporary scratch-repository rehearsal used while hardening the release operator was retired before `0.2.0`. Its successful and failed runs remain immutable history in the architecture progress report, but it is no longer an active release prerequisite, workflow, binary, repository target, or command surface. The production operator remains fixed to `ifsheldon/Stab` and still requires source-current qualification completion before reading release credentials.

Read-only GitHub GET operations make at most three attempts after a connection, I/O, protocol, DNS, or timeout failure, including while reading a bounded response body, or after an HTTP 408, 500, 502, 503, or 504 response. Retries use cancellable one-second then two-second backoff. Draft creation and asset-upload POST operations are never retried automatically because their remote mutation may have succeeded even when the response was lost.

## Preflight

The A9 procedure permits exactly one status-only descendant after the measured source revision. Once that descendant exists, set `RELEASE_COMMIT` to its full commit identity and verify that the worktree is clean. The release procedure then requires the human operator to verify that every required GitHub CI check for that exact revision has passed. This external exact-revision CI check is mandatory: the local Rust release commands validate source and qualification identities, but they do not query GitHub CI and do not replace this check. Do not continue from the measured evidence parent, an earlier status revision, or a revision whose required CI is pending or failed.

```text
RELEASE_COMMIT="$(git rev-parse HEAD^{commit})"
RELEASE_TAG="v0.2.0"
git status --short
```

From that final clean reviewed revision, choose a new output path containing the abbreviated release commit and run:

```text
just release::check --out target/releases/v0.2.0-<commit>-preflight
```

The Rust release tool reruns the architecture policy, validates the exact package set and topological order, creates all ten normalized archives in a new isolated Cargo target, and validates each archive's `.cargo_vcs_info.json` against the clean current commit. It bounds compressed bytes, expanded bytes, declared payload bytes, and entry count while rejecting unsafe paths and links. Every Cargo child receives a private `HOME`, `CARGO_HOME`, target, and temporary directory, an explicit toolchain, a source-owned `cargo:token` configuration, and no ambient environment. The preflight executes one coordinated multi-package `cargo publish --dry-run --locked --registry crates-io` invocation so Cargo resolves unpublished exact-version siblings through its temporary registry and compiles the normalized registry packages without uploading them. It also generates and validates the canonical crates.io metadata that will accompany each archive. Fresh reviewed archives and metadata are copied under the unique preflight directory, made read-only, and recorded in schema-version-4 `report.json` with their exact lengths, SHA-256 digests, Git commit, Cargo identity, rustc identity, and active Rustup toolchain. The tool never reads archives from the workspace's shared `target/package` directory.

Review every immutable archive under the preflight directory before the first upload, and confirm that the report's full Git commit equals `RELEASE_COMMIT`. Do not reuse a failed preflight output path, modify a reviewed archive, or publish from a revision or toolchain other than the report's exact identities.

## Crates.io Publication

Provide the crates.io credential only through the secret `CARGO_REGISTRY_TOKEN` environment variable immediately before publication. Run the irreversible local recipe from an isolated user session with no unrelated same-UID process. The repository prevents accidental shell evaluation and stale shared-output reuse, but ordinary Linux process access does not let it promise secrecy against a malicious same-UID process that can inspect another process's environment. The `release::publish-reviewed` recipe accepts typed `--preflight` and `--confirm-version` options, shell-quotes their exact values, creates a fresh owner-only operator target, builds `stab-release` there with `CARGO_REGISTRY_TOKEN`, `CARGO_REGISTRIES_CRATES_IO_TOKEN`, `GITHUB_TOKEN`, and `GH_TOKEN` removed from the build environment, then executes that unique binary without another Cargo process while removing every recognized release credential except `CARGO_REGISTRY_TOKEN`. The binary rejects direct publication invocations that also carry `CARGO_REGISTRIES_CRATES_IO_TOKEN`, `GITHUB_TOKEN`, or `GH_TOKEN`, and diagnostics name variables without exposing their values. The release operation intentionally ignores ambient Cargo homes and credentials, so `cargo login` is not a release credential source. Never put the token in a command argument, tracked file, generated report, log, or task transcript. Before it reads the token or queries crates.io, the operation runs `qualification-status --check --require-release-completion` in an isolated token-free Cargo environment and stops unless the current clean revision owns the authenticated A9 completion checkpoint.

Publish the reviewed set with an explicit irreversible-operation confirmation:

```text
just release::publish-reviewed --preflight target/releases/v0.2.0-<commit>-preflight --confirm-version 0.2.0
```

Before each upload, the Rust operation rebuilds that package from the report's exact clean commit into a new isolated target and requires the result to match the reviewed archive byte for byte. It then reopens the immutable canonical metadata and archive through retained no-follow descriptors, revalidates source and toolchain identity, and uses Cargo's official crates.io protocol library to submit those exact bytes directly. No `cargo publish` child performs the upload, no upload-time repackaging occurs, and `CARGO_REGISTRY_TOKEN` is exposed only to the exact authenticated HTTP request. After the request, the operation revalidates the reviewed and rebuilt inputs and waits until the crates.io API reports the exact reviewed checksum before advancing to a dependent package. Missing versions are published, already-visible matching versions are accepted for safe resumption, and any existing mismatched checksum stops the release. `SIGINT` or `SIGTERM` cancels the shared operation, terminates any active process group, and interrupts registry polling before another package begins.

If publication stops partway through, rerun the same command with the same reviewed preflight. The operation rechecks every visible version, requires its reviewed checksum, rejects a yanked version as unusable recovery state, and resumes at the first missing package. Already published versions cannot be replaced. If any published archive has the wrong checksum, has been yanked, or requires a source correction, do not publish dependents or mix revisions under `0.2.0`; coordinate a new patch version and document the partial release.

## Tag And GitHub Release

After all ten crates resolve from crates.io, create an annotated or signed tag at the exact preflight commit and push it:

```text
test "$(git rev-parse HEAD^{commit})" = "$RELEASE_COMMIT"
git tag -a v0.2.0 "$RELEASE_COMMIT" -m "Stab 0.2.0"
test "$(git cat-file -t refs/tags/v0.2.0)" = "tag"
test "$(git rev-parse 'refs/tags/v0.2.0^{commit}')" = "$RELEASE_COMMIT"
git push origin refs/tags/v0.2.0
REMOTE_RELEASE_COMMIT="$(git ls-remote --exit-code origin 'refs/tags/v0.2.0^{}' | cut -f1)"
test "$REMOTE_RELEASE_COMMIT" = "$RELEASE_COMMIT"
```

The local and remote peel checks are mandatory. Stop if either tag does not resolve to `RELEASE_COMMIT`; do not move or replace a published release tag.

Repository ruleset `20419793`, named `Protect Stab v0.2.0 release tag`, must remain active throughout tag and release work. It targets only `refs/tags/v0.2.0`, rejects tag update and deletion, and has no bypass actor; the release operator reads that exact ruleset before and after every draft or release verification and fails closed if its identity, GitHub-owned fingerprint, target, rules, enforcement, or authenticated bypass state changes.

Dispatch the `Release` workflow from the existing tag ref, not from the default branch, then capture the run identity by polling the workflow-scoped run list filtered to the dispatch event and the exact reviewed head SHA, requiring exactly one match:

```text
gh workflow run release.yml --ref "$RELEASE_TAG" -f tag="$RELEASE_TAG"
RELEASE_RUN_ID=""
for _ in 1 2 3 4 5 6 7 8 9 10 11 12; do
  sleep 5
  RELEASE_RUN_ID="$(gh api "repos/ifsheldon/Stab/actions/workflows/release.yml/runs?event=workflow_dispatch&head_sha=$RELEASE_COMMIT" --jq '[.workflow_runs[].id] | if length == 1 then .[0] else empty end')"
  test -n "$RELEASE_RUN_ID" && break
done
printf '%s\n' "$RELEASE_RUN_ID" | grep -Eq '^[0-9]+$'
RELEASE_RUN_URL="https://github.com/ifsheldon/Stab/actions/runs/$RELEASE_RUN_ID"
gh run view "$RELEASE_RUN_ID" --json databaseId,event,headBranch,headSha,status,conclusion,url,workflowName
gh run watch "$RELEASE_RUN_ID" --exit-status
gh run view "$RELEASE_RUN_ID" --json databaseId,event,headBranch,headSha,status,conclusion,url,workflowName
```

The capture must resolve to exactly one dispatch-event run for the reviewed SHA: zero matches keeps polling and fails after the bounded loop, and several matches (for example a duplicate dispatch) produce the empty string so the identity check fails closed instead of guessing; investigate and delete the unwanted run before retrying. `gh workflow run` prints no run identity on stdout, so capture never relies on its output.

Do not discover the run with `gh run list`; a concurrent dispatch could make a latest-run query select unrelated work. Do not accept the captured run unless its database ID and URL agree with `RELEASE_RUN_ID` and `RELEASE_RUN_URL`, `workflowName` is `Release`, `event` is `workflow_dispatch`, `headBranch` is `v0.2.0`, `headSha` equals `RELEASE_COMMIT`, and the final status and conclusion are `completed` and `success`. Every third-party action in the workflow is pinned to a reviewed full commit SHA, including the checkout action used by the `contents: write` draft job; `just architecture::check` opens workflow files through no-follow descriptors and rejects mutable action refs across all tracked workflows. The architecture contract also freezes the complete release workflow execution context: exact jobs, runners, timeout, permissions, environment, shell-bearing steps, action inputs, and commands. Each native runner checks out immutable `github.sha`, verifies that the input is exactly `v0.2.0`, the event ref is exactly `refs/tags/v0.2.0`, and the local tag, event ref, and `HEAD` all resolve to that SHA. It then invokes the Rust release operation, which builds into a new isolated target, requires `stab --version` to report exactly `0.2.0` with one LF, validates the runnable AArch64 executable format for the target operating system, and emits the binary, checksum sidecar, and source-provenance manifest.

The final job builds `stab-release` without explicit publication-token variables into `${{ runner.temp }}/stab-release-operator-${{ github.sha }}`, then exposes a step-local `GITHUB_TOKEN` binding only while invoking that SHA-scoped prebuilt operator with the exact `create-draft --assets target/releases/assets --tag "$RELEASE_TAG" --confirm-version 0.2.0` argv as the final job step. The step declares no Cargo token or `GH_TOKEN`, and the binary rejects a direct draft invocation carrying any of them. `just architecture::check` requires immutable checkout with full history and credential persistence disabled, requires the operator build immediately before the final invocation, and recursively rejects recognized release-token expressions hidden under aliases, action inputs, or inline commands. The reviewed full-SHA actions still execute under the draft job's declared GitHub permission model. The draft operation verifies the machine-owned A9 completion before reading `GITHUB_TOKEN`, opens and retains the exact six expected regular files before any remote mutation, rejects missing or extra entries and every manifest, checksum, architecture, version, commit, complete target-aware pinned-toolchain identity, or path-identity mismatch, and verifies that the exact release-tag ruleset is active and the existing remote tag is annotated and resolves to the reviewed commit before mutation and again immediately before success. It creates only a private draft through the GitHub API, uploads the retained bytes without reopening workspace paths, and checks the returned and final GitHub asset names, upload states, sizes, and `sha256:` digests. Because the by-tag release endpoint returns published releases only, the final draft re-read and `release::verify-remote-draft` scan the paginated repository release list under a fixed page bound and require exactly one draft carrying the release tag, while `release::verify-published-release` keeps the by-tag query. An existing release or duplicate asset causes failure; the operation has no replace or publish path.

Download the two exact workflow artifacts from the captured run into a new local directory, then run the read-only remote verifier immediately before manual publication:

```text
RELEASE_ASSETS="target/releases/v0.2.0-<commit>-workflow-assets"
test ! -e "$RELEASE_ASSETS"
mkdir "$RELEASE_ASSETS"
gh run download "$RELEASE_RUN_ID" --name stab-linux-aarch64 --dir "$RELEASE_ASSETS"
gh run download "$RELEASE_RUN_ID" --name stab-macos-aarch64 --dir "$RELEASE_ASSETS"
GITHUB_TOKEN="$(gh auth token)" just release::verify-remote-draft --assets "$RELEASE_ASSETS" --tag "$RELEASE_TAG"
```

Inspect the private draft, verify its source identity, both binaries, both checksum files, both provenance manifests, and all GitHub-recorded asset digests, then publish the draft manually. Do not publish if time or unrelated activity intervenes after the read-only verification; run `release::verify-remote-draft` again immediately before publication. A failed workflow can leave an incomplete private draft; delete that draft only after reviewing its assets, then rerun. Existing release assets are never replaced.

Immediately after manual publication, verify the exact public state and retained assets, then repeat the remote annotated-tag peel:

```text
GITHUB_TOKEN="$(gh auth token)" just release::verify-published-release --assets "$RELEASE_ASSETS" --tag "$RELEASE_TAG"
REMOTE_RELEASE_COMMIT="$(git ls-remote --exit-code origin 'refs/tags/v0.2.0^{}' | cut -f1)"
test "$REMOTE_RELEASE_COMMIT" = "$RELEASE_COMMIT"
```

Both read-only verifiers rerun the A9 machine authorization before reading `GITHUB_TOKEN`, retain and revalidate the exact local six-file asset set, rehash every retained descriptor against its reviewed length and SHA-256 digest, require the exact active no-bypass release-tag ruleset, bracket the GitHub read with remote annotated-tag checks, and require exact release state, names, sizes, successful upload states, and GitHub-recorded SHA-256 digests. The post-publication verifier additionally requires a nonempty publication timestamp.

The single status descendant is the final source revision for this release. Do not edit the A9 progress report or create a post-publication source commit to record release results. Preserve the final release URL, crates.io package URLs and reviewed checksums, GitHub Actions workflow run identity, remote annotated-tag peel, and release-asset digests in external release records outside the source tree. Crates.io, the remote tag, the published GitHub release and its assets, and the GitHub Actions run remain the authoritative public records; retain any consolidated operator ledger outside the repository. Keep the immutable local preflight report and final qualification checkpoint with the release records according to the project's artifact-retention policy.
