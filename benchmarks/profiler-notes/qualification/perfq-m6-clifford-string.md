# PERFQ-M6-CLIFFORD-STRING Profiler Note

Owner: `stab-core` Clifford-string storage and right multiplication.

Status: the first clean focused adapter probes proved equivalent semantic work but exposed a material performance failure in both qualification groups. The identity probe reported a diagnostic ratio of `21.933884`, and the non-identity probe reported `238.618379`, against the `1.25x` threshold. These probes use intentionally tiny calibrated work and are diagnostic infrastructure evidence rather than promotable full or soak measurements, but the non-identity scaling measurements below confirmed that the implementation architecture was unsuitable. The threshold, frozen vectors, comparator, work definitions, scales, and correctness prerequisites remain unchanged and unwaived.

## Root Cause

Dominant cost: the pre-optimization non-identity callback performed one scalar `SingleQubitClifford` composition per qubit instead of applying the Clifford algebra over packed words.

The pre-optimization `CliffordString` stored one `SingleQubitClifford` byte per qubit and called scalar single-qubit multiplication for every non-identity right operand. Direct release-worker measurements at width 10,000 showed approximately linear cost: one iteration took `92.752 us`, ten took `779.008 us`, one hundred took `7.416903 ms`, and one thousand took `79.983523 ms`. This is approximately 7.4 through 8.0 nanoseconds per logical single-qubit product after warmup. Pinned Stim represents the same operation as six packed bit planes and evaluates the Clifford algebra over machine words, so improving branch details or table lookup overhead in the scalar loop could not close the architectural gap.

A stack-level profile is unavailable on this host because `/proc/sys/kernel/perf_event_paranoid` is `4`, which rejects unprivileged performance counting and sampling. This limitation does not weaken the source-shape, allocation, scaling, correctness, or paired timing contracts used by qualification.

## Implemented Optimization

Commit `2a0ab88c44eeeaae1714b0976089ecc1809203f3` replaces the byte-per-qubit representation with six packed `BitVec` planes corresponding to Stim's Clifford representation. It applies the exact right-multiplication formulas through an isolated four-word `std::simd` kernel, retains a scalar reference implementation for differential tests, and handles remaining words without allocation. Public construction, indexing, mutation, concatenation, repetition, display, randomization, unequal-width extension, and multiplication APIs remain unchanged.

Focused tests cover every frozen all-24-by-24 Clifford product vector, portable-SIMD versus scalar results across SIMD blocks and partial tails, shorter-right preservation within and across word boundaries, identity metadata restoration after clearing the final non-identity gate, deterministic qualification vectors, and zero timed-callback allocations at every contract scale. The complete `stab-core` suite and relevant `stab-bench` Clippy and test checks passed before the optimization commit.

Direct dirty-tree release-worker diagnostics after the change showed the non-identity width-10,000 workload completing one thousand iterations in `234.608 us`, compared with `79.983523 ms` before the change, for an approximate 341-fold implementation-level improvement. The identity fast path completed one hundred thousand callbacks in `733.44 us`. These numbers are optimization guidance only: they were not paired with Stim under the clean qualification controller, do not bind the current correctness inventory, and must not be promoted as gate evidence.

## Acceptance Work

Next owner action: commit the refreshed correctness and performance inventories, then produce same-revision focused CQ evidence, reproducible workers, full and soak reports, regressions, rollups, and completion receipts for both Clifford groups before any timing claim or legacy-row migration.

Current acceptance requires a clean unchanged revision that binds this exact profiler-note digest, current correctness and performance inventories, reproducible private workers, exact CQ preflight evidence, verified host policy, equal calibrated work, semantic output equality, full and soak samples at all three source-owned scales, passing regression reports, architecture-scoped rollups, and replayable completion receipts for both `PERFQ-M6-CLIFFORD-STRING` and `PERFQ-M6-CLIFFORD-STRING-NON-IDENTITY`. Every failed, noisy, or host-unverified result must be retained instead of being replaced by an earlier passing diagnostic. Native x86-64 evidence remains independent of AArch64 evidence.
