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

## Preflight

From the final clean reviewed revision, choose a new output path containing the abbreviated commit and run:

```text
just release::check --out target/releases/v0.2.0-<commit>-preflight
```

The Rust release tool reruns the architecture policy, validates the exact package set and topological order, creates all ten normalized archives in a new isolated Cargo target, and validates each archive's `.cargo_vcs_info.json` against the clean current commit. It bounds compressed bytes, expanded bytes, declared payload bytes, and entry count while rejecting unsafe paths and links. Every Cargo child receives a private `HOME`, `CARGO_HOME`, target, and temporary directory, an explicit toolchain, a source-owned `cargo:token` configuration, and no ambient environment. The preflight executes one coordinated multi-package `cargo publish --dry-run --locked --registry crates-io` invocation so Cargo resolves unpublished exact-version siblings through its temporary registry and compiles the normalized registry packages without uploading them. It also generates and validates the canonical crates.io metadata that will accompany each archive. Fresh reviewed archives and metadata are copied under the unique preflight directory, made read-only, and recorded in schema-version-4 `report.json` with their exact lengths, SHA-256 digests, Git commit, Cargo identity, rustc identity, and active Rustup toolchain. The tool never reads archives from the workspace's shared `target/package` directory.

Review every immutable archive under the preflight directory before the first upload. Do not reuse a failed preflight output path, modify a reviewed archive, or publish from a revision or toolchain other than the report's exact identities.

## Crates.io Publication

Provide the crates.io credential only through the secret `CARGO_REGISTRY_TOKEN` environment variable immediately before publication. The release operation intentionally ignores ambient Cargo homes and credentials, so `cargo login` is not a release credential source. Never put the token in a command argument, tracked file, generated report, log, or task transcript. Before it reads the token or queries crates.io, the operation runs `qualification-status --check --require-release-completion` in an isolated Cargo environment and stops unless the current clean revision owns the authenticated A9 completion checkpoint.

Publish the reviewed set with an explicit irreversible-operation confirmation:

```text
just release::publish-reviewed --preflight target/releases/v0.2.0-<commit>-preflight --confirm-version 0.2.0
```

Before each upload, the Rust operation rebuilds that package from the report's exact clean commit into a new isolated target and requires the result to match the reviewed archive byte for byte. It then reopens the immutable canonical metadata and archive through retained no-follow descriptors, revalidates source and toolchain identity, and uses Cargo's official crates.io protocol library to submit those exact bytes directly. No `cargo publish` child performs the upload, no upload-time repackaging occurs, and `CARGO_REGISTRY_TOKEN` is exposed only to the exact authenticated HTTP request. After the request, the operation revalidates the reviewed and rebuilt inputs and waits until the crates.io API reports the exact reviewed checksum before advancing to a dependent package. Missing versions are published, already-visible matching versions are accepted for safe resumption, and any existing mismatched checksum stops the release. `SIGINT` or `SIGTERM` cancels the shared operation, terminates any active process group, and interrupts registry polling before another package begins.

If publication stops partway through, rerun the same command with the same reviewed preflight. The operation rechecks every visible checksum and resumes at the first missing package. Already published versions cannot be replaced. If any published archive has the wrong checksum or a source correction is required, do not mix revisions under `0.2.0`; coordinate a new patch version and document the partial release.

## Tag And GitHub Release

After all ten crates resolve from crates.io, create an annotated or signed tag at the exact preflight commit and push it:

```text
git tag -a v0.2.0 -m "Stab 0.2.0"
git push origin v0.2.0
```

Dispatch the `Release` workflow with the existing tag. Every third-party action in the workflow is pinned to a reviewed full commit SHA, including the checkout action used by the `contents: write` draft job; `just architecture::check` opens workflow files through no-follow descriptors and rejects mutable action refs across all tracked workflows. Each native runner checks out that tag and invokes the Rust release operation, which builds into a new isolated target, requires `stab --version` to report exactly `0.2.0` with one LF, validates the runnable AArch64 executable format for the target operating system, and emits the binary, checksum sidecar, and source-provenance manifest.

The final job invokes one `just release::create-draft --assets target/releases/assets --tag v0.2.0 --confirm-version 0.2.0` operation. That operation verifies the machine-owned A9 completion before reading `GITHUB_TOKEN`, opens and retains the exact six expected regular files before any remote mutation, rejects missing or extra entries and every manifest, checksum, architecture, version, commit, or path-identity mismatch, and verifies that the existing remote tag is annotated and resolves to the reviewed commit. It creates only a private draft through the GitHub API, uploads the retained bytes without reopening workspace paths, and checks the returned and final GitHub asset names, upload states, sizes, and `sha256:` digests. An existing release or duplicate asset causes failure; the operation has no replace or publish path.

Inspect the private draft, verify its source identity, both binaries, both checksum files, both provenance manifests, and all GitHub-recorded asset digests, then publish the draft manually. A failed workflow can leave an incomplete private draft; delete that draft only after reviewing its assets, then rerun. Existing release assets are never replaced. Preserve the preflight report, final qualification checkpoint, release URL, crates.io package links, and workflow run in the A9 progress report.
