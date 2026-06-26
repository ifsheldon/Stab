# Run the staged-aware pre-commit checks.
[working-directory: '..']
pre-commit *args:
    cargo run -q -p stab-pre-commit -- {{args}}

# Install the Rust pre-commit hook into the local Git hooks directory.
[working-directory: '..']
setup-hooks:
    cargo build -q -p stab-pre-commit
    mkdir -p .git/hooks
    cp target/debug/stab-pre-commit .git/hooks/pre-commit
    chmod +x .git/hooks/pre-commit
    @echo "Pre-commit hook installed."
