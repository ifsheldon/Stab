# Run the staged-aware pre-commit checks.
[working-directory: '..']
pre-commit *args:
    cargo run -q --locked -p stab-pre-commit -- {{args}}

# Install the Rust pre-commit hook into the repository's Git hooks directory; pass --force to replace a foreign hook.
[working-directory: '..']
setup-hooks *args:
    cargo run -q --locked -p stab-pre-commit -- install {{args}}
