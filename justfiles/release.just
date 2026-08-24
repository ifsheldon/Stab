repository_root := justfile_directory()
operator_target := join(repository_root, "target/releases/operators", uuid())
operator_binary := join(operator_target, "debug/stab-release")

# Validate and package the coordinated product crates from a clean revision.
[working-directory: '..']
[arg("out", long)]
check out:
    cargo run -q --locked -p stab-release -- check --out {{quote(out)}}

# Print the source-owned crates.io publication order.
[working-directory: '..']
publish-order:
    cargo run -q --locked -p stab-release -- publish-order

# Build the release operator without exposing publication credentials to Cargo or build scripts.
[working-directory: '..']
build-operator:
    /usr/bin/env -u CARGO_REGISTRY_TOKEN -u CARGO_REGISTRIES_CRATES_IO_TOKEN -u GITHUB_TOKEN -u GH_TOKEN /usr/bin/install -d -m 700 {{quote(operator_target)}}
    /usr/bin/env -u CARGO_REGISTRY_TOKEN -u CARGO_REGISTRIES_CRATES_IO_TOKEN -u GITHUB_TOKEN -u GH_TOKEN CARGO_TARGET_DIR={{quote(operator_target)}} cargo build -q --locked -p stab-release

# Publish reviewed crates one at a time and verify crates.io checksums.
[working-directory: '..']
[arg("preflight", long)]
[arg("confirm_version", long="confirm-version", pattern="0\\.2\\.0")]
publish-reviewed preflight confirm_version: build-operator
    exec /usr/bin/env -u CARGO_REGISTRIES_CRATES_IO_TOKEN -u GITHUB_TOKEN -u GH_TOKEN {{quote(operator_binary)}} publish-reviewed --preflight {{quote(preflight)}} --confirm-version {{quote(confirm_version)}}

# Build and package one tagged stab binary with checksum and provenance.
[working-directory: '..']
[arg("target", long)]
[arg("out", long)]
[arg("tag", long, pattern="v0\\.2\\.0")]
build-binary target out tag:
    cargo run -q --locked -p stab-release -- build-binary --target {{quote(target)}} --out {{quote(out)}} --tag {{quote(tag)}}

# Verify the complete tagged binary asset set before draft publication.
[working-directory: '..']
[arg("assets", long)]
[arg("tag", long, pattern="v0\\.2\\.0")]
verify-assets assets tag:
    cargo run -q --locked -p stab-release -- verify-assets --assets {{quote(assets)}} --tag {{quote(tag)}}

# Verify exact assets, create a private GitHub draft, and check remote digests.
[working-directory: '..']
[arg("assets", long)]
[arg("tag", long, pattern="v0\\.2\\.0")]
[arg("confirm_version", long="confirm-version", pattern="0\\.2\\.0")]
create-draft assets tag confirm_version: build-operator
    exec /usr/bin/env -u CARGO_REGISTRY_TOKEN -u CARGO_REGISTRIES_CRATES_IO_TOKEN -u GH_TOKEN {{quote(operator_binary)}} create-draft --assets {{quote(assets)}} --tag {{quote(tag)}} --confirm-version {{quote(confirm_version)}}

# Revalidate the exact private draft and retained assets immediately before publication.
[working-directory: '..']
[arg("assets", long)]
[arg("tag", long, pattern="v0\\.2\\.0")]
verify-remote-draft assets tag: build-operator
    exec /usr/bin/env -u CARGO_REGISTRY_TOKEN -u CARGO_REGISTRIES_CRATES_IO_TOKEN -u GH_TOKEN {{quote(operator_binary)}} verify-remote-draft --assets {{quote(assets)}} --tag {{quote(tag)}}

# Verify the exact public release and retained assets after publication.
[working-directory: '..']
[arg("assets", long)]
[arg("tag", long, pattern="v0\\.2\\.0")]
verify-published-release assets tag: build-operator
    exec /usr/bin/env -u CARGO_REGISTRY_TOKEN -u CARGO_REGISTRIES_CRATES_IO_TOKEN -u GH_TOKEN {{quote(operator_binary)}} verify-published-release --assets {{quote(assets)}} --tag {{quote(tag)}}
