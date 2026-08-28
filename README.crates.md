# Stab Rust Components

Stab is an agent-native, safe-Rust toolkit for quantum error correction research and a compatibility-oriented rewrite of selected Stim v1.16.0 surfaces.

The coordinated Stab 0.2 release contains independently usable crates for typed circuit and detector-error-model values, result records, packed bits, Pauli and Clifford algebra, pure analysis, execution engines, decoder interoperability, the curated `stab-core` facade, and the `stab` command-line interface.

All Stab product crates use the same `0.2.0` version. Internal dependencies require that exact version because pre-1.0 minor releases may contain coordinated API changes.

- Repository and documentation: <https://github.com/ifsheldon/Stab>
- Rust 0.2 migration guide: <https://github.com/ifsheldon/Stab/blob/main/docs/MIGRATING-0.2.md>
- Generated Stim parity status: <https://github.com/ifsheldon/Stab/blob/main/docs/stim-parity.md>
- Compatibility target: Stim v1.16.0

The portable-SIMD kernel crate, the `portable-simd` feature, `stab-core`, and `stab-cli` use the repository's pinned Nightly toolchain. The stable component crates declare Rust 1.97.1 as their minimum supported Rust version.
