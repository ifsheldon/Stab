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

# Inspect or validate the compatibility matrix.
[working-directory: '..']
matrix *args:
    cargo run -q -p stab-oracle -- matrix {{args}}
