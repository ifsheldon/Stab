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
