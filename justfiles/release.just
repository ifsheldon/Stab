# Validate and package the coordinated product crates from a clean revision.
check *args:
    cargo run -q --locked -p stab-release -- check {{args}}

# Print the source-owned crates.io publication order.
publish-order:
    cargo run -q --locked -p stab-release -- publish-order

# Package one tagged stab binary and its SHA-256 sidecar.
package-binary *args:
    cargo run -q --locked -p stab-release -- package-binary {{args}}
