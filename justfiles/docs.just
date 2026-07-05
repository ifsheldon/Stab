# Generate Rust API documentation under target/doc.
[working-directory: '..']
api:
    cargo doc --workspace --no-deps

# Generate Rust API documentation and fail on rustdoc warnings.
[working-directory: '..']
api-check:
    RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
