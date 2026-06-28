# M5 Completion Report

## Milestone

M5: Portable SIMD Bit Core.

Objective: provide maintainable portable high-throughput bit primitives that simulators can use without touching raw SIMD lanes.

## Status

Complete against the clarified M5 portable-SIMD bit-core contract.

## Tests Ported Or Created

- Added `crates/stab-core/tests/bits.rs` with Rust tests adapted from Stim v1.16.0 memory and bit-utility tests.
- Covered the M5-owned subsets of `bit_ref`, `simd_word`, `simd_bits`, `simd_bits_range_ref`, `simd_bit_table`, `simd_util`, `sparse_xor_vec`, and `twiddle`.
- Added property tests for scalar-vs-SIMD bit vector operations, range XOR, matrix transpose, masked row XOR, and sparse XOR symmetric difference.
- Added focused boundary tests for empty lengths, unaligned tails, dirty padding words, 255/256/257/511/512/513/1024/1025 bit lengths, matrix storage overflow, row operations, and upstream sparse XOR examples.

## Implementation Areas

- Added `stab_core::bits` with `BitLen`, `BitBlock`, `BitSlice`, `BitVec`, `BitMatrix`, `SparseXorVec`, and bit utility helpers.
- Isolated direct `std::simd` usage in `crates/stab-core/src/bits/simd.rs`; `crates/stab-core/src/lib.rs` owns the required crate-level `#![feature(portable_simd)]` gate.
- Kept scalar word kernels internal to the bit module and used them as reference and tail kernels.
- Canonicalized owned `BitVec` padding after raw word operations and made logical `popcount`/`not_zero` ignore unused tail bits.
- Added checked `BitMatrix` construction for storage-size overflow and fallible `identity`.
- Added matrix row XOR, masked row XOR, row swap, transpose, and square in-place transpose.
- Reworked matrix row operations to use disjoint row slices and avoid hot-path row allocation except for the self-masked-row invariant case.
- Added M5 Stab benchmark compare runners and normalized rate output for bit vectors, bit matrices, sparse XOR, and popcount-like workloads.

## Oracle And Benchmark Evidence

- Updated M5 oracle fixture rows from manifest-only rows to direct `cargo-test` rows.
- Labeled M5 memory rows as M5-owned upstream subsets instead of full-file parity, with unsupported memory utility subcases documented in `docs/plans/milestone-spec-gaps.md`.
- Updated benchmark manifest descriptions and benchmark docs to explain M5 comparability notes.
- M5 benchmark compare reports normalized Stab rates plus pinned Stim timings.
- Exact optimized 10k bit-table transpose parity is explicitly deferred to M12 performance hardening; M5 reports a 128x128 contract-smoke transpose and row-XOR workload.

## Done Criteria

| Requirement | Status | Evidence |
| --- | --- | --- |
| Nightly pinned and portable SIMD enabled | Satisfied | `rust-toolchain.toml`; `crates/stab-core/src/lib.rs` |
| Direct SIMD isolated to bit kernels | Satisfied | `rg "std::simd|portable_simd" crates/stab-core/src` shows only the crate feature gate and `bits/simd.rs` direct import |
| Bit primitives implemented | Satisfied | `crates/stab-core/src/bits/` |
| Scalar reference kernels available internally | Satisfied | `crates/stab-core/src/bits/scalar.rs` |
| Randomized boundary and scalar-vs-SIMD tests | Satisfied | `cargo test -p stab-core bits` |
| M5 oracle rows run | Satisfied | `just oracle::run --milestone M5` |
| M5 benchmark compare reports required workloads | Satisfied with documented comparability notes | `just bench::compare --milestone M5 --strict` |
| Architecture-specific fallback documented as deferred | Satisfied | no architecture-specific fallback was required or implemented |

## Audit Outcome

Milestone audit found three issues: dirty tail bits could leak through public `BitSlice` inputs, matrix-level masked row operations were missing, and benchmark output did not distinguish comparable workloads from contract-smoke workloads.
All three were fixed or logged as resolved M5 specification clarifications.

Resolved M5 spec entries:

- `2026-06-27 - M5: Portable SIMD Feature Gate Location`
- `2026-06-27 - M5: Benchmark Compare Semantics`
- `2026-06-27 - M5: Memory Test Subcase Granularity`

## Full Code Review Outcome

Full code review found no P0 or P1 issues.
P2 findings were fixed by narrowing oracle coverage wording, removing extra work from the direct bit-vector XOR benchmark, replacing manual popcount with `u64::count_ones`, avoiding row-operation allocations, adding multi-block-plus-tail tests, and making raw scalar kernels internal.

## Verification Commands

- `cargo fmt --all --check`
- `cargo clippy -p stab-core --all-targets -- -D warnings`
- `cargo clippy -p stab-bench --all-targets -- -D warnings`
- `cargo test -p stab-core bits`
- `cargo test -p stab-bench`
- `just oracle::matrix --check`
- `just oracle::run --milestone M5`
- `just bench::compare --milestone M5 --strict`
