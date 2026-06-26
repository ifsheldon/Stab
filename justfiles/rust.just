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

# Run the ignored M4 parser fuzz smoke locally.
[working-directory: '..']
parser-fuzz:
    cargo test -p stab-core --test parser_fuzz parser_fuzz_smoke -- --ignored
