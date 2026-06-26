# Check Rust formatting.
[working-directory: '..']
fmt-check:
    cargo fmt --all --check

# Lint Rust.
[working-directory: '..']
clippy:
    cargo clippy --workspace --all-targets -- -D warnings

# Run Rust tests.
[working-directory: '..']
test:
    cargo test --workspace
