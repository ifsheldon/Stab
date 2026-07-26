# Check product dependency direction and the portable-SIMD source boundary.
[working-directory: '..']
check:
    cargo run -q -p stab-architecture -- check
