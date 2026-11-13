# Check product dependency direction and the SIMD library boundary.
[working-directory: '..']
check:
    cargo run -q -p stab-architecture -- check

# Validate external Stable component, facade, and mixed direct-component consumers.
[working-directory: '..']
consumer-check:
    cargo run -q -p stab-architecture -- consumer-check

# Validate repository-owned Markdown links and heading anchors.
[working-directory: '..']
docs-check:
    cargo run -q -p stab-architecture -- docs-check
