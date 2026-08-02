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

The Rust release tool reruns the architecture policy, validates the exact package set and topological order, executes `cargo package --locked --no-verify` for all ten crates, and writes archive lengths and SHA-256 digests to `report.json`. It rejects a dirty or changing repository, an existing report path, path traversal, symlinked output ancestors, missing shared package documentation, incomplete metadata, and inexact internal versions. `--no-verify` is necessary before internal dependencies exist on crates.io; workspace tests, Clippy, Stable component checks, and external-consumer checks remain separate release gates.

Review every assembled archive before the first upload. Do not reuse a failed preflight output path, and do not publish from a revision other than the report's exact commit.

## Crates.io Publication

Provide the crates.io credential interactively with `cargo login` or through a secret `CARGO_REGISTRY_TOKEN` environment variable. Never put the token in a command argument, tracked file, generated report, log, or task transcript.

Publish one package at a time in the printed order:

```text
cargo publish --locked -p stab-kernels-simd
cargo publish --locked -p stab-model
cargo publish --locked -p stab-bits
cargo publish --locked -p stab-records
cargo publish --locked -p stab-algebra
cargo publish --locked -p stab-analysis
cargo publish --locked -p stab-decoder
cargo publish --locked -p stab-engine
cargo publish --locked -p stab-core
cargo publish --locked -p stab-cli
```

After each upload, wait until `cargo info <package>@0.2.0` resolves from crates.io before publishing a dependent package. Stop immediately on an identity, registry, or package error.

If publication stops partway through, already published versions cannot be replaced. Verify that every published crate belongs to the reviewed preflight source, then resume at the first missing package after its prerequisites become visible. If any published archive has the wrong source or a source correction is required, do not mix revisions under `0.2.0`; coordinate a new patch version and document the partial release.

## Tag And GitHub Release

After all ten crates resolve from crates.io, create an annotated or signed tag at the exact preflight commit and push it:

```text
git tag -a v0.2.0 -m "Stab 0.2.0"
git push origin v0.2.0
```

Create the GitHub release from that existing tag. The release workflow explicitly checks out the tag, verifies that it is an annotated tag at the clean current revision, builds `stab-cli`, and invokes the Rust release tool to produce `stab-linux-aarch64`, `stab-macos-aarch64`, and their SHA-256 sidecars. Manual workflow dispatch must name the same existing tag.

Verify the GitHub release source identity, both binaries, both checksum files, and the checksums themselves. Preserve the preflight report, final qualification checkpoint, release URL, crates.io package links, and workflow run in the A9 progress report.
