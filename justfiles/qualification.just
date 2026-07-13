# Validate the comprehensive correctness qualification inventory.
[working-directory: '..']
correctness-check *args:
    cargo run -q -p stab-oracle -- qualification correctness check {{args}}

# List comprehensive correctness qualification coverage.
[working-directory: '..']
correctness-list *args:
    cargo run -q -p stab-oracle -- qualification correctness list {{args}}

# Regenerate the comprehensive correctness qualification inventory.
[working-directory: '..']
correctness-regenerate *args:
    cargo run -q -p stab-oracle -- qualification correctness regenerate {{args}}

# Rebuild and execute sealed Stab and Stim binaries and verify receipt provenance bindings.
[working-directory: '..']
correctness-provenance-probe:
    cargo run -q -p stab-oracle -- qualification correctness provenance-probe

# Execute a correctness qualification tier and publish bounded reports.
[working-directory: '..']
correctness-run *args:
    cargo run -q -p stab-oracle -- qualification correctness run {{args}}

# Validate JSON and regenerate derived correctness report artifacts.
[working-directory: '..']
correctness-report *args:
    cargo run -q -p stab-oracle -- qualification correctness report {{args}}

# Validate machine-readable correctness prerequisites for a dependent workload.
[working-directory: '..']
correctness-preflight *args:
    cargo run -q -p stab-oracle -- qualification correctness preflight {{args}}
