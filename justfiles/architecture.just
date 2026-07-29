# Check product dependency direction and the portable-SIMD source boundary.
[working-directory: '..']
check:
    cargo run -q -p stab-architecture -- check

# Compile external Stable, scalar facade, portable facade, and mixed feature consumers.
[working-directory: '..']
consumer-check:
    cargo run -q -p stab-architecture -- consumer-check
