# Validate and package the coordinated product crates from a clean revision.
check *args:
    cargo run -q --locked -p stab-release -- check {{args}}

# Print the source-owned crates.io publication order.
publish-order:
    cargo run -q --locked -p stab-release -- publish-order

# Build the release operator without exposing publication credentials to Cargo or build scripts.
build-operator:
    env -u CARGO_REGISTRY_TOKEN -u CARGO_REGISTRIES_CRATES_IO_TOKEN -u GITHUB_TOKEN -u GH_TOKEN cargo build -q --locked -p stab-release

# Publish reviewed crates one at a time and verify crates.io checksums.
publish-reviewed *args: build-operator
    exec ./target/debug/stab-release publish-reviewed {{args}}

# Build and package one tagged stab binary with checksum and provenance.
build-binary *args:
    cargo run -q --locked -p stab-release -- build-binary {{args}}

# Backward-compatible name for the source-building binary operation.
package-binary *args:
    cargo run -q --locked -p stab-release -- build-binary {{args}}

# Verify the complete tagged binary asset set before draft publication.
verify-assets *args:
    cargo run -q --locked -p stab-release -- verify-assets {{args}}

# Verify exact assets, create a private GitHub draft, and check remote digests.
create-draft *args: build-operator
    exec ./target/debug/stab-release create-draft {{args}}
