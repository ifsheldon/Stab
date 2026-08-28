# Initialize, update, and validate the pinned Stim oracle submodule.
[working-directory: '..']
fetch *args:
    cargo run -q -p stab-oracle -- fetch {{args}}

# Print and assert the pinned Stim oracle version.
[working-directory: '..']
version *args:
    cargo run -q -p stab-oracle -- version {{args}}

# Run a named oracle smoke comparison.
[working-directory: '..']
run *args:
    cargo run -q -p stab-oracle -- run {{args}}

# List oracle fixtures grouped by milestone, parity mode, and status.
[working-directory: '..']
list *args:
    cargo run -q -p stab-oracle -- list {{args}}

# Record or check exact-output oracle fixtures from pinned Stim.
[working-directory: '..']
record *args:
    cargo run -q -p stab-oracle -- record {{args}}

# Inspect or validate the compatibility matrix.
[working-directory: '..']
matrix *args:
    cargo run -q -p stab-oracle -- matrix {{args}}

# Validate the pinned result-format acceptance and canonical-output corpus.
[working-directory: '..']
result-formats *args:
    cargo run -q -p stab-oracle -- result-formats {{args}}

# Compare the canonical Stab gate catalog against pinned Stim metadata.
[working-directory: '..']
gates *args:
    cargo run -q -p stab-oracle -- gates {{args}}

# Validate the atomic Stim v1.16.0 parity ledger.
[working-directory: '..']
parity-check *args:
    cargo run -q -p stab-oracle -- parity check {{args}}

# Run canonical parity-owner tests for a PR, full, or soak tier.
[working-directory: '..']
parity-run *args:
    cargo run -q -p stab-oracle -- parity run {{args}}

# Render or check the generated parity document.
[working-directory: '..']
parity-render *args:
    cargo run -q -p stab-oracle -- parity render {{args}}
