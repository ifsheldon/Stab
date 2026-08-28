# Validate the sole source-owned E2E performance suite without timing it.
[working-directory: '..']
e2e-check *args:
    cargo run -q -p stab-bench --features portable-simd -- e2e-check {{args}}

# Run release-built E2E workflows and publish one replayable bundle.
[working-directory: '..']
e2e-run *args:
    cargo run -q -p stab-bench --features portable-simd --profile release -- e2e-run {{args}}

# Reconstruct an E2E report from the raw bundle without executing workloads.
[working-directory: '..']
e2e-replay *args:
    cargo run -q -p stab-bench --features portable-simd --profile release -- e2e-replay {{args}}

# Generate a reviewed self-regression baseline candidate from full and soak bundles.
[working-directory: '..']
e2e-baseline-candidate *args:
    cargo run -q -p stab-bench --features portable-simd --profile release -- e2e-baseline-candidate {{args}}

# Validate current controlled-host evidence before release.
[working-directory: '..']
e2e-release-check:
    cargo run -q -p stab-bench --features portable-simd --profile release -- e2e-release-check
