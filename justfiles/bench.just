# Compile benchmark targets as an M0 smoke check.
[working-directory: '..']
smoke:
    cargo test --workspace --benches --no-run
