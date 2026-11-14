# Rust 1.96–1.100: Changes Relevant To Stab

Reviewed on 2026-11-13 and updated on 2026-11-14 after an adoption pass on the Stable 1.100 pin.
This is a research note, not an active contract; it is not linked from `docs/README.md`.

## Sources

Each version was read from its rust-lang.org announcement and its GitHub release notes (`RELEASES.md` / the GitHub release body, which matched item for item), plus the Cargo and Clippy changelogs that the announcements link to.

| Version | Released | Announcement | Point release |
| --- | --- | --- | --- |
| 1.96.0 | 2026-05-28 | [blog](https://blog.rust-lang.org/2026/05/28/Rust-1.96.0/) | [1.96.1](https://blog.rust-lang.org/2026/06/30/Rust-1.96.1/) (2026-06-30): MIR-optimization miscompile fix, Cargo libssh2 CVEs |
| 1.97.0 | 2026-07-09 | [blog](https://blog.rust-lang.org/2026/07/09/Rust-1.97.0/) | [1.97.1](https://blog.rust-lang.org/2026/07/16/Rust-1.97.1/) (2026-07-16): LLVM miscompile fix (rust#159035, x86_64 segfault) |
| 1.98.0 | 2026-08-20 | [blog](https://blog.rust-lang.org/2026/08/20/Rust-1.98.0/) | [1.98.1](https://blog.rust-lang.org/2026/09/03/Rust-1.98.1/) (2026-09-03): vtable miscompile with a null slot (rust#161441) |
| 1.99.0 | 2026-10-01 | [blog](https://blog.rust-lang.org/2026/10/01/Rust-1.99.0/) | [1.99.1](https://blog.rust-lang.org/2026/10/15/Rust-1.99.1/) (2026-10-15): x86_64 FFI `bool` return miscompile |
| 1.100.0 | 2026-11-12 | [blog](https://blog.rust-lang.org/2026/11/12/Rust-1.100.0/) | — |

GitHub release notes: [1.96.0](https://github.com/rust-lang/rust/releases/tag/1.96.0), [1.97.0](https://github.com/rust-lang/rust/releases/tag/1.97.0), [1.98.0](https://github.com/rust-lang/rust/releases/tag/1.98.0), [1.99.0](https://github.com/rust-lang/rust/releases/tag/1.99.0), [1.100.0](https://github.com/rust-lang/rust/releases/tag/1.100.0).

On 2026-11-13 the 1.98.0 blog URL served the 1.99.0 post; the 1.98.0 content was read from `content/Rust-1.98.0.md` in the `rust-lang/blog.rust-lang.org` repository instead.

## Where Stab Stands

- `rust-toolchain.toml` pins Stable `1.100.0` (`rustc 1.100.0 (0ef1bc386 2026-11-09)`), set by `f285fcb4` in the [fearless SIMD plan](plans/fearless-simd-stable-rust-plan.md); [ADR 0008](architecture/adr-0008-unconditional-fearless-simd-kernels.md) superseded the Nightly boundary of ADR 0005.
- Every product crate declares `rust-version = "1.100"`, so every feature in this note is usable in every crate; no MSRV or pin bump is needed for any of them.
- The pin includes every point-release fix in the table above.
- Each API adopted below was probed on the pin with a one-line program compiled by `rustc 1.100.0` with no feature gate; all of them compiled, including `core::range`, `DropGuard`, `NumBuffer`, and `assert_matches!`.
- The move to the pin already absorbed the new Clippy lints: S4 converted 22 `chunks_exact_to_as_chunks` sites and one `manual_slice_fill` site to the suggested form, and no lint was silenced with an `allow`.

## Adoption Summary

The adoption pass started from `7c7cde86` and was behavior-preserving: every CLI output, including error text, stayed byte-identical (see [Evidence](#evidence)).

| Feature | Version | Status in Stab |
| --- | --- | --- |
| `<int>::format_into(&mut NumBuffer<_>)` | 1.98 | Adopted on the circuit, DEM, and `convert` integer output paths; measured and rejected in the streaming record encoder |
| `str::trim_prefix`, `Path::trim_prefix` | 1.100 | Adopted at 7 sites |
| `str::strip_circumfix` | 1.98 | Adopted at 3 sites |
| `str::substr_range` / `[T]::subslice_range` | 1.98 | Not adopted |
| `assert_matches!` | 1.96 | Adopted at 182 test assertions in 67 files |
| `bool::ok_or` / `ok_or_else` | 1.98 | Adopted at 8 sites |
| `Option::map_or_default` | 1.98 | Adopted at 5 sites |
| `NonZero::<int>::from_str_radix`, iterable `NonZero` ranges | 1.98, 1.96 | No sites |
| `lowest_one` and the other 1.97 bit operations | 1.97 | `lowest_one` adopted at 8 set-bit scans; measured neutral |
| `core::range` types | 1.96 | Not adopted |
| `DropGuard` | 1.100 | Adopted at 1 site |
| `Path::is_empty` | 1.98 | Adopted at 2 sites |
| `String::from_utf8_lossy_owned` | 1.99 | Adopted at 2 sites |
| `std::fs::set_times`, `PathBuf::into_string` | 1.99, 1.98 | No sites |
| `bool::toggle` | 1.100 | Adopted at 5 sites |
| `Vec::from_fn`, `Box::map`, `Vec::into_parts`, `IntoIterator for Box<[T; N]>` | 1.100, 1.99 | Not adopted |
| `build.warnings = "deny"` | 1.97 | Adopted for the Clippy gate in `just`, CI, the pre-commit hook, and `CONTRIBUTE.md` |
| `[lints.cargo]` `unused_dependencies` | 1.100 | Declared in `[workspace.lints.cargo]`; no unused dependency found |
| Workspace `default-features = false` override | 1.99 | Adopted for `rand` in `stab-algebra` and `hashbrown` in `stab-analysis` |
| `chunks_exact_to_as_chunks`, `manual_isolate_lowest_one`, `manual_bit_width`, `mismatched_bit_width_type` | Clippy 1.98–1.99 | Already clean; nothing silenced |
| `f32`/`f64::algebraic_*`, `Allocator`, `mem::conjure_zst`, NEON `vdot*` | 1.98–1.100 | Rejected |
| `registry.global-min-publish-age` | 1.100 | Open decision for the project owner |

## Library Features

### Integer formatting: `format_into` (1.98)

`<int>::format_into(&mut core::fmt::NumBuffer<_>) -> &str` formats an integer into a stack buffer without `fmt` dispatch.

Adopted sites:

- `stab-records`: `append_usize_decimal` in the per-record writers (`convert` to `hits` and `dets`) replaced a one-digit-at-a-time loop and lost a `cast_possible_truncation` allow.
- `stab-model` circuit printing: `push_u64`, `push_i32`, `write_u64_io`, and `write_i32_io` now call `format_into`, the hand-rolled `encode_u64` helper and its three `expect_used` allows are gone, and the single-pass float printer's integer shortcut (`DETECTOR(1, 2, 0)` style coordinates) uses `format_into` instead of `write!`.
- `stab-model` DEM printing: `DemTarget::text_parts` splits a target into its letter and digits, and `Display for DemTarget`, the string writer, and the byte writer all use it, so `write_dem_bytes` no longer goes through `write!(out, " {target}")` and the string writer no longer allocates a `String` per target.

`decimal_len_u64` in circuit capacity estimation still counts digits by division, because no 1.96–1.100 API replaces it and `checked_ilog10` predates this window.

`format_into` on `i32` writes the sign itself, which matches the old sign-then-magnitude code, including `i32::MIN`.

Measured and rejected: `push_decimal` in the streaming record encoder (`hits` and `dets` output of `detect`, `sample`, `sample_dem`, and `m2d`) keeps its two-digit lookup table.
With `format_into` there, `detect --out_format dets` on `suite_d11.stim` at 300k shots was 3.6% faster (300.1 ms to 289.3 ms) but `--out_format hits` was 3% slower (272.9 ms to 281.1 ms), and an `#[inline]` hint did not change either number, so the table stays.

The A/B timing in [Evidence](#evidence) found every kept integer-formatting change equal or faster, by up to 14% for noisy `gen` at distance 400.

### Prefix trimming and circumfix stripping (1.98, 1.100)

- `trim_prefix` replaced `strip_prefix(x).unwrap_or(original)` and its `map_or_else(|_| path.to_path_buf(), …)` form at 7 sites: the target-error classifier in `stab-model` (`trim_prefix('-')`), the clap JSON diagnostic in `stab-cli` (`trim_prefix("error: ")`), and five root-relative path displays in `ops/architecture` and `ops/pre-commit` (`Path::trim_prefix`).
- `strip_circumfix` replaced `strip_prefix(..).and_then(|v| v.strip_suffix(']'))` for `rec[-N]` and `sweep[N]` targets in `stab-model/src/target.rs` and for `X[..]`-style flow terms in `stab-algebra/src/flow.rs`.
- The `[` and `(` argument openers in the circuit and DEM parsers were left alone, because they search for the matching closer and report a distinct error for each half.
- The fixture-token parser in `ops/oracle/src/fixtures/outputs.rs` was left alone, because it reports a missing suffix differently from a missing prefix.

### Byte offsets: `substr_range` / `subslice_range` (1.98)

Not adopted.
Parser diagnostics carry offsets through `SourceSlice` in `stab-model/src/source_text.rs`, which threads a `byte_start` with each slice by construction; `substr_range` would return an `Option` that the callers would have to turn into an internal error, so it would add a failure path without removing any arithmetic.

### `assert_matches!` / `debug_assert_matches!` (1.96)

Both macros live at the crate root and need an import on stable 1.100: `use std::assert_matches;` (or `use core::assert_matches;`).

Every `assert!(matches!(..))` in the workspace, 182 assertions in 67 files, was converted mechanically, so a failure now prints the value that did not match.
The import sits in the innermost module that uses the macro, in that module's `std` import group.

- Six assertions dropped a custom message that only repeated the asserted value with `{value:?}`, because the macro now prints it.
- Eight release tests assert on `review_assets(..).map(drop)`, because the success type `ReviewedAssets` deliberately has no `Debug` implementation.
- Three `assert!(!matches!(..))` assertions stay, because there is no negated form.
- Guarded patterns (`P if cond`) keep their hand-formatted layout, because rustfmt cannot format a macro argument that is not an expression.

### `bool::ok_or` and `Option::map_or_default` (1.98)

- `bool::ok_or_else` replaced `cond.then_some(()).ok_or_else(..)` at 2 sites and `if cond { Ok(()) } else { Err(..) }` at 5 sites, and `bool::ok_or` replaced the Linux host check in `ops/bench/src/process.rs`.
  Every allocating error stays lazy.
- The `ELSE_CORRELATED_ERROR` check in `stab-analysis/src/circuit_to_dem/reverse_fold.rs` stays an `if`/`else`, because its long message reads worse inside a closure.
- `map_or_default(f)` replaced `map(f).unwrap_or_default()` at 5 sites.
- `map_or(0, <[u8]>::len)` and similar calls stay, because the explicit zero is the clearer statement of a byte count.

### `NonZero` parsing and ranges (1.96, 1.98)

No sites.
The only `NonZero` value in the product crates is the `rec[-N]` lookback (`NonZeroI32`), which is parsed as a bounded unsigned value and negated, so `from_str_radix` does not fit.

### Integer bit operations (1.97)

The 2026-11-14 search found no `BITS - leading_zeros`, `x & x.wrapping_neg()`, or `63 - leading_zeros` idioms, and Clippy with `manual_bit_width`, `manual_isolate_lowest_one`, and `mismatched_bit_width_type` forced on reported nothing.

`lowest_one` replaced the `while bits != 0 { bits.trailing_zeros() … }` set-bit scans at 8 sites: `for_each_set_bit` in the record encoder, the bit visitors in `stab-algebra/src/conversions.rs` and `tableau/action.rs`, the two selection scans in `tableau/random.rs`, `first_set_bit` in the reference simulator, and two scans in the Bernoulli statistical tests.
The loop condition and the index now come from one call, so no index is ever read from a zero word.

The record-encoder scan is on the measured `hits`/`dets` path; an A/B against the integer-formatting build alone was neutral (see [Evidence](#evidence)).

The `reverse_fold.rs` combination loop keeps `trailing_zeros` with `mask & (mask - 1)`, because it walks a nonzero counter rather than scanning bits.

`bit_width` exists on unsigned types only (`i64::bit_width` does not exist, despite `RELEASES.md` saying `{integer}`).

### `core::range` types (1.96)

Not adopted.
The only stored span list, `Vec<Range<usize>>` in `stab-model/src/model_bytes.rs`, is read through `.start`/`.end` in production; the Copy range would only remove four `.clone()` calls in its tests while adding conversions where production pushes `start..end`.
Public signatures already take `impl RangeBounds<usize>`.

### `DropGuard` (1.100)

Adopted in `atomic_write_regular_file_unix` in `ops/oracle/src/safe_file.rs`: a `DropGuard` unlinks the temporary file on any early return and is dismissed after the rename and directory sync, replacing an immediately invoked closure plus `if result.is_err()`.
The guard is created at the same point as the old cleanup scope, so the cleanup covers exactly the same failures.

Not adopted:

- The `cfg(not(any(unix, windows)))` fallback in the same file has the same shape, but it cannot be compiled on the development host, so it was left unchanged.
- `ops/release` never deletes partial output; it writes into no-replace retained directories, so there is no rollback for a guard to own.
- The existing `Drop` implementations (`ManagedChild` in `ops/bench` and `ops/release`, the engine transactions, `BitWordsMut`) clean up state owned by their type, which is not a closure-shaped guard.

### Filesystem and string helpers (1.98, 1.99)

- `Path::is_empty` replaced `path.as_os_str().is_empty()` in the two `validate_relative` checks in `ops/release`.
- `String::from_utf8_lossy_owned` replaced `String::from_utf8_lossy(&bytes).into_owned()` for the captured stdout and stderr in `ops/pre-commit/src/support.rs`, which avoids a copy when the output is valid UTF-8.
- The opaque-tag display strings in `stab-model` stay on `from_utf8_lossy`, because they keep the original bytes and the bytes are known to be invalid.
- `std::fs::set_times` / `set_times_nofollow` and `PathBuf::into_string` have no sites: no ops tool sets timestamps, and the only path-to-`String` conversion borrows an `OsStr`.

### Other small items (1.96–1.100)

- `bool::toggle` replaced `x = !x` at 5 sites (`stabilizer_frame.rs`, `flow.rs`, and one test).
- `Vec::from_fn` was not adopted: the candidates are `(0..n).map(f).collect()` chains that usually feed a `join`, and the iterator form is just as clear.
- `Box`/`Rc`/`Arc::map`, `Vec::into_parts` / `from_parts`, `IntoIterator for Box<[T; N]>`, `Default for RepeatN`, `From<T> for LazyLock`, `FusedIterator for StepBy`, `PartialEq<Cow<[U]>>`, and `Send`/`Sync` for `CommandArgs` have no sites.

### Rejected

- **`f32` / `f64::algebraic_*`** let LLVM reassociate float arithmetic, at the cost of non-deterministic results.
  They would break Stim parity and seeded self-regression in probability, noise-model, and DEM-weight paths.
- **The `Allocator` API** with `Vec::new_in` / `Box::new_in` would need `unsafe impl Allocator` for an arena, and `unsafe_code` is denied workspace-wide.
- **`mem::conjure_zst`** is unsafe.
- **NEON `vdot*` intrinsics** compute int8 dot products, which do not match Stab's XOR/popcount bit kernels, and need `unsafe`.

## Cargo And Lints

- **`build.warnings = "deny"` (1.97).** `just rust::clippy`, the CI Clippy step, the pre-commit hook, and `CONTRIBUTE.md` now run `cargo clippy --workspace --all-targets --config "build.warnings='deny'"` instead of `-- -D warnings`.
  It exits with status 101 on the same rustc and Clippy warnings, and it shares the build cache with a plain `cargo clippy`, because the deny setting is not a compiler flag.
  It also fails on Cargo manifest-lint warnings such as `cargo::unused_dependencies`, which `-D warnings` ignored; that makes the gate stricter, and the workspace has none today.
  `RUSTDOCFLAGS="-D warnings"` in `justfiles/docs.just` is unchanged, because `build.warnings` does not cover rustdoc flags.
  The release recipes and release workflow were not touched.
- **`[lints.cargo]` `unused_dependencies` (1.100).** The lint is already `warn` by default; the workspace now states it in `[workspace.lints.cargo]`, which every member inherits.
  It fired on a deliberately unused probe dependency and reports nothing for the real workspace with `--all-targets`; it never checks dev-dependencies or dependencies behind disabled features.
- **Workspace `default-features = false` override (1.99).** `stab-algebra` now takes `rand` without default features, because it only uses the `Rng`/`RngExt` traits and `SmallRng`; its tests get the defaults back through a dev-dependency, so a standalone `stab-algebra` build no longer pulls `getrandom` or `chacha20`.
  `stab-analysis` takes `hashbrown` with only `inline-more`, because it uses `HashTable` with the std `RandomState`; that drops `foldhash`, `allocator-api2`, and `equivalent` from every product build and from the three external consumer lockfiles under `test-support/consumers/`.
  The root `Cargo.lock` is unchanged, because `ops` tools still use those crates.
  The `stab-engine` `rand` dependency keeps its defaults, because unseeded runs call `rand::random`.
- **`registry.global-min-publish-age` (1.100)** adds a dependency cooldown in `.cargo/config.toml`.
  It conflicts with the rule to add new dependencies at their latest crates.io version, because a cooldown can reject a version published within the window, so it is left as an open decision for the project owner.
- `resolver.lockfile-path` (1.97) remains a possible fit for read-only source trees such as the release operator's isolated builds; it was not adopted because no current workflow needs it.
- The `linker_messages` lint (1.97) is already `allow` in `[workspace.lints.rust]`.

## Effects Of The 1.100 Pin To Keep In Mind

1. **CLI stderr text.** 1.98 escapes more characters in `Debug` output, and 1.100 makes `str::escape_debug` always escape grapheme extenders.
   `stab-cli` diagnostics format user input with `{:?}`, so error text can differ from a pre-pin build for unusual input.
   Compatibility is defined on stderr class, so this is cosmetic.
2. **Lint behavior.** `semicolon_in_expressions_from_non_local_macros` warns for macros from other crates (1.99), `unreachable_cfg_select_predicates` joins the `unused` group (1.99), and legacy integer modules such as `std::u64::MAX` are fully deprecated (1.99); the repo has none of these.
3. **Cargo behavior.** Incremental compilation is off by default when `CI` is set (1.99).
   The new `build-dir` layout (1.100) only matters to tools that read Cargo's intermediate artifacts; Stab's ops code uses its own `target/releases/` subtree and `CARGO_BIN_EXE_*`.
   `cargo install` uses the packaged lockfile by default (1.100).
   v0 symbol mangling is the default (1.97).
4. **Semantics.** Iterating an exhausted `RangeInclusive` may leave different `start()`/`end()` values (1.99), `Iterator::min`/`max` call different `Ord` methods (1.100), `assert_eq!` gets its own temporary scope (1.98), and `derive(PartialOrd)` takes a fast path when `Ord` is also derived (1.98).
   None of these matter with lawful `Ord` impls.
5. **Never type.** 1.100 makes `!` stable and `Infallible` an alias for it.
   Stab's `type Error = Infallible` impls keep working, and the repo has no `impl … for Infallible` or `From<Infallible>` impls that could start overlapping.
6. **Benchmark evidence.** The toolchain move invalidated the AArch64 baselines (`benchmarks/AGENTS.md`), and 1.99 moved to LLVM 23, so ratios must come from evidence recorded on the 1.100 pin.

## Evidence

All timings below are best of 9 interleaved runs pinned to CPU 4 on the AArch64 development host, comparing a release build of `7c7cde86` with a release build of the adoption pass.

Integer formatting and bit-scan A/B (best of 9, milliseconds; `fmt` is the build with only the integer-formatting and trimming changes, `bits` adds `lowest_one`):

| Case | `7c7cde86` | `fmt` | Ratio | `bits` vs `fmt` |
| --- | ---: | ---: | ---: | ---: |
| `detect --out_format dets`, `suite_d11.stim`, 300k shots | 303.5 | 289.0 | 0.95 | 1.00 |
| `detect --out_format hits`, `suite_d11.stim`, 300k shots | 278.1 | 281.7 | 1.01 | 1.00 |
| `sample_dem --out_format hits`, `suite_d11.dec.dem`, 100k shots | 328.3 | 329.0 | 1.00 | 1.00 |
| `sample_dem --out_format dets`, `suite_d11.dec.dem`, 100k shots | 334.6 | 329.7 | 0.99 | 1.00 |
| `analyze_errors`, `d21r21.stim` | 262.3 | 251.9 | 0.96 | — |
| `analyze_errors`, `d15r150.stim` | 1072.3 | 1021.3 | 0.95 | — |
| `gen` rotated surface code, distance 400, 1 round | 776.3 | 712.1 | 0.92 | — |
| `gen` rotated surface code, distance 400, 3 noisy rounds | 963.6 | 821.3 | 0.85 | — |
| `convert` b8 to `dets`, 2048 bits, 40k shots | 262.9 | 256.2 | 0.97 | 1.00 |
| `convert` b8 to `hits`, 2048 bits, 40k shots | 257.5 | 254.0 | 0.99 | 1.00 |

The `hits` regression traced to `format_into` in the streaming encoder, which was reverted as described in [Integer formatting](#integer-formatting-format_into-198).

Final adoption build against `7c7cde86` over the review set plus the sparse-output cases (best of 9, milliseconds):

| Case | `7c7cde86` | Adoption | Ratio |
| --- | ---: | ---: | ---: |
| `detect` d7r1000 100k b8 | 2217.7 | 2211.4 | 1.00 |
| `detect` suite_d11 350k b8 with observables | 228.9 | 226.3 | 0.99 |
| `detect` color_d9 1M b8 | 219.6 | 219.7 | 1.00 |
| `detect` d35r1000 64 shots | 64.9 | 64.7 | 1.00 |
| `detect` d35r1000 1 shot | 63.8 | 64.2 | 1.01 |
| `detect` d35r1000 1000 shots | 680.6 | 677.6 | 1.00 |
| `sample` d35r1000 64 shots | 124.8 | 125.5 | 1.01 |
| `sample` d35r1000 1000 shots | 695.8 | 706.3 | 1.01 |
| `detect` d21r21 64 shots | 4.2 | 4.2 | 0.99 |
| `sample` d11r11 300k b8 | 181.4 | 180.7 | 1.00 |
| `m2d` d11r11 200k b8 | 87.9 | 87.5 | 1.00 |
| `sample_dem` suite_d11 100k b8 | 299.4 | 299.1 | 1.00 |
| `detect` suite_d11 300k `dets` | 302.3 | 299.6 | 0.99 |
| `detect` suite_d11 300k `hits` | 277.0 | 273.3 | 0.99 |
| `sample_dem` suite_d11 100k `hits` | 328.5 | 326.9 | 1.00 |
| `sample_dem` suite_d11 100k `dets` | 335.4 | 334.3 | 1.00 |
| `analyze_errors` d21r21 | 263.2 | 251.7 | 0.96 |
| `analyze_errors` d15r150 | 1101.2 | 1079.1 | 0.98 |
| `gen` distance 400 | 777.0 | 714.5 | 0.92 |
| `gen` distance 400, noisy | 966.8 | 823.7 | 0.85 |
| `convert` b8 to `dets` | 262.3 | 258.0 | 0.98 |
| `convert` b8 to `hits` | 257.2 | 256.7 | 1.00 |

A 15-round recheck of `sample` d35r1000 1000 shots measured 696.9 ms before and 695.0 ms after; intermediate builds of the same code ranged from 694.8 ms to 710.1 ms, so the 1.01 row is build-to-build layout noise rather than a change in the sampling code, which this pass did not touch beyond `bool::toggle`.

Output identity: 523 commands (sampling, detection, `m2d`, `convert` in every sparse format, `analyze_errors` with folding and decomposition, `gen`, `inspect`, `plan`, hostile circuits and DEMs, malformed `rec[...]` and `sweep[...]` targets, and clap and JSON diagnostics) produced the same stdout, stderr, and exit status from both release builds when both binaries had the same file name.
A first run with different binary names differed only in the three clap usage lines that print `argv[0]`.

The E2E smoke tier (`just bench::e2e-run --tier smoke --affinity-cpu 4 --allow-dirty`) completed all 25 cases with memory `Passed` and parity and self-regression `Diagnostic`; smoke ratios are diagnostic only and are not quoted here as Stim performance claims.

## Not Relevant To Stab

Nothing in 1.96–1.100 touches `std::simd` or `portable_simd`.
Platform tier changes did not touch `aarch64-unknown-linux-gnu`.
The WebAssembly, LoongArch, NVPTX, PowerPC, Windows, and embedded target changes, C-variadics, `extern "custom"`, the rustdoc flag stabilizations, and `-Cprofile-sample-use` do not affect current milestones.
