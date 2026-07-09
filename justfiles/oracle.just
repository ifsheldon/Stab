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

# Validate and inspect the non-deferred blocker closure ledger.
[working-directory: '..']
blockers *args:
    cargo run -q -p stab-oracle -- blockers {{args}}
