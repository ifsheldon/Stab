# Rust 1.96–1.100: Changes Relevant To Stab

Reviewed on 2026-11-13. This is a research note, not an active contract; it is not linked from `docs/README.md`.

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

- The pinned toolchain `nightly-2026-06-20` is `rustc 1.98.0-nightly (f428d123a 2026-06-19)`.
- Stable component crates declare `rust-version = "1.97.1"` (ADR 0005).
- So everything up to 1.97 is usable everywhere, most 1.98 library APIs already compile on the pin but need an MSRV bump before stable crates may use them, and 1.99/1.100 features need both a pin bump and an MSRV bump.
- 1.98 availability on the pin was checked by compiling a probe per API with `rustc +nightly-2026-06-20` and no feature gates; the 1.97 bit-op signatures below were checked with `rustc +1.97.1`.

## Recommendation: Consider Bumping The Nightly Pin

The pin predates three point-release miscompilation fixes: 1.96.1 (MIR optimization), 1.98.1 (vtable with a null function slot, which is UB), and possibly 1.97.1 (LLVM; reported on x86_64, Stab's primary host is AArch64). Whether the pinned nightly actually contains each bug was not verified. 1.99 also moves to LLVM 23, which can shift SIMD codegen in either direction.

A pin bump is not free: `benchmarks/AGENTS.md` binds baselines to the Rust toolchain, so a new pin needs fresh E2E evidence before any Stim performance ratio is promoted, and newer Clippy brings lints that will fail the `-D warnings` CI gates (see [Pin-bump checklist](#pin-bump-checklist)).

## Usable Now In Every Crate (≤ 1.97.1)

### Integer bit operations (1.97)

New on all unsigned integers and `NonZero` (probed on 1.97.1):

| Method | `u64` result | `NonZero<u64>` result |
| --- | --- | --- |
| `highest_one()` | `Option<u32>` (bit index) | `u32` |
| `lowest_one()` | `Option<u32>` | `u32` |
| `isolate_highest_one()` | `u64` | `NonZero<u64>` |
| `isolate_lowest_one()` | `u64` | `NonZero<u64>` |
| `bit_width()` | `u32` | `u32` |

`bit_width` exists on unsigned types only (`i64::bit_width` does not exist, despite `RELEASES.md` saying `{integer}`). They give names to idioms such as `u64::BITS - x.leading_zeros()` and `x & x.wrapping_neg()`; a 2026-11-13 search of `crates/` found neither idiom, so they mainly matter for new bit-kernel code. Clippy 1.98 adds `manual_isolate_lowest_one` (complexity) and 1.99 adds `manual_bit_width` (pedantic) and `mismatched_bit_width_type` (suspicious) to push code toward them. The bit-scan loop in `crates/stab-analysis/src/circuit_to_dem/reverse_fold.rs:648` (`trailing_zeros` then `mask & (mask - 1)`) clears rather than isolates, so it is already idiomatic.

### `core::range` types (1.96)

`core::range::{Range, RangeFrom, RangeToInclusive}` are `Copy` and implement `IntoIterator` rather than `Iterator`. They suit stored spans such as parser diagnostics byte ranges or record windows that currently store `start`/`end` pairs or clone `ops::Range`. Range syntax still produces the legacy `core::ops` types until a future edition, so prefer `impl RangeBounds` in public signatures, as the announcement recommends.

### `assert_matches!` / `debug_assert_matches!` (1.96)

These print the value's `Debug` form when they fail. The workspace has about 150 `assert!(matches!(..))` calls in tests, and they would give more useful failure output after migrating. Both macros must be imported (`use core::assert_matches;`). Migrate opportunistically; a mass rewrite has little value.

### Other small items

- `NonZero` integer ranges are iterable (1.96), useful if typed IDs are `NonZero`-backed.
- `Default for RepeatN` (1.97) and `From<T> for LazyLock<T, F>` (1.96).
- `char::is_control` is const (1.97).

### Cargo and CI (1.97)

- `build.warnings = "deny"` (`CARGO_BUILD_WARNINGS=deny`) replaces `RUSTFLAGS=-Dwarnings` without invalidating the build cache. Stab passes `-- -D warnings` to Clippy in `justfiles/rust.just` and `.github/workflows/ci.yml`; those runs could switch, and `just rust::*` recipes could share one cache across checked and unchecked runs. It does not cover `RUSTDOCFLAGS="-D warnings"` in `justfiles/docs.just`.
- The `linker_messages` lint is warn-by-default and outside the `warnings` group. Stab already sets `linker_messages = "allow"` in `[workspace.lints.rust]`.
- `resolver.lockfile-path` allows a custom lockfile path, which helps read-only source trees such as the release operator's isolated builds.
- `cargo clean` refuses a `--target-dir` that does not look like a Cargo target directory.
- v0 symbol mangling is the stable default. It affects backtrace text and older profilers only; it does not affect the pinned nightly, which already used v0.

## Usable On The Pin, Needs An MSRV Bump For Stable Crates (1.98)

All of these compile on `nightly-2026-06-20` except the `Atomic::from_mut*` APIs.

- **`<int>::format_into(&mut core::fmt::NumBuffer<_>) -> &str`** formats integers without `fmt` dispatch, with `itoa`-like speed. `append_usize_decimal` in `crates/stab-records/src/result_formats.rs:817` is a hand-rolled version of this on the `dets` output path. Replacing it removes code and a `cast_possible_truncation` allow, but `stab-records` is a 1.97.1-MSRV crate and the output path is benchmarked, so it needs an MSRV decision and an A/B measurement.
- **`NonZero::<int>::from_str_radix`** parses directly into `NonZero` typed values, in line with the typed-boundary rules.
- **`str::strip_circumfix` / `[T]::strip_circumfix`** strip a prefix and a suffix in one call, for example bracketed parser arguments.
- **`str::substr_range` / `[T]::subslice_range`** recover a byte range from a subslice, which helps point parser diagnostics at a location.
- **`bool::ok_or` / `ok_or_else`** and **`Option` / `Result::map_or_default`** shorten validation chains that end in domain errors.
- **`Path::is_empty`** and **`PathBuf::into_string`** are useful for typed path validation in the `ops` crates.
- **`Send` / `Sync` for `std::process::CommandArgs`.**
- **`f32` / `f64::algebraic_*`** let LLVM reassociate float arithmetic, at the cost of non-deterministic results. Do not use them in probability, noise-model, or DEM-weight paths where Stim parity or seeded self-regression must stay reproducible.
- **NEON `vdot*` intrinsics** (aarch64) are stable. They compute int8 dot products, which do not match Stab's XOR/popcount bit kernels, and they would need `unsafe`, which is denied workspace-wide. They are not relevant.

## Needs A Pin Bump And An MSRV Bump (1.99, 1.100)

### Most relevant

- **`DropGuard` (1.100)** runs a closure on drop unless `DropGuard::dismiss` is called. It suits rollback-on-error code such as the `self.buffer.truncate(old_len)` error path in `crates/stab-records/src/record_stream.rs:468`, and partial-output cleanup in `ops/release`.
- **`str::trim_prefix` / `trim_suffix`, `[T]::trim_*`, `Path::trim_prefix` (1.100)** replace `strip_prefix(x).unwrap_or(original)`. Current sites include `crates/stab-model/src/circuit/parser.rs:778`, `ops/architecture/src/markdown.rs:633`, and `ops/architecture/src/workflow_actions.rs:75`.
- **Workspace `default-features` override (1.99, Cargo, edition 2024 members only)**: `dep = { workspace = true, default-features = false }` is now honored even when the workspace definition enables defaults. This lets product crates trim features from shared `[workspace.dependencies]` entries.
- **`[lints.cargo]` with `cargo::unused_dependencies` (1.100, warn by default)** helps keep the dependency graph minimal, in line with ADR 0007. It only fires when every target that could use a dependency is built, and it never reports dev-dependencies.
- **`registry.global-min-publish-age` (1.100)** adds a dependency cooldown in `.cargo/config.toml` as a supply-chain defense. It interacts with the rule to add new dependencies at their latest crates.io version: a cooldown can make Cargo reject a version published within the window. Decide that interaction before adopting it.
- **`std::fs::set_times` / `set_times_nofollow` (1.99)** set file timestamps by path, with a no-follow variant that fits the symlink-hostile filesystem rules in `ops/release`.
- **`String::from_utf8_lossy_owned` / `FromUtf8Error::into_utf8_lossy` (1.99)** avoid a copy when lossily decoding owned bytes, for example captured subprocess output in the oracle and bench tooling.

### Also available

- 1.99:
  - `IntoIterator` for `Box<[T; N]>` and its references.
  - `Vec::into_parts` / `from_parts`.
  - `FusedIterator for StepBy`.
  - `Box::into_non_null` / `from_non_null`.
  - Layout queries on raw pointers.
  - Defining C-variadic functions (not needed).
- 1.100:
  - `Vec::from_fn`.
  - `bool::toggle`.
  - `Box`/`Rc`/`Arc::map`.
  - `PartialEq<Cow<[U]>>` for `Vec` and slices.
  - The stable `Allocator` API with `Vec::new_in` / `Box::new_in`. An arena for parser or analysis allocations would need `unsafe impl Allocator`, so it would need an ADR-level exception to `unsafe_code = "deny"` and is not recommended now.
  - `mem::conjure_zst` (unsafe).
- 1.100 makes the never type `!` stable, and `Infallible` becomes an alias for `!`. Stab's `type Error = Infallible` impls keep working. The repo has no `impl … for Infallible` or `From<Infallible>` impls that could start overlapping.

## Pin-Bump Checklist

These items will need attention when the pinned nightly moves past 2026-06-19:

1. **New Clippy lints under `-D warnings`.**
   - `chunks_exact_to_as_chunks` (style, 1.98) suggests `as_chunks` for constant chunk sizes; there are about 25 `chunks_exact(` calls in `crates/`, and some sit in hot kernels, so measure before accepting the suggestion.
   - `manual_isolate_lowest_one` (complexity, 1.98) and `mismatched_bit_width_type` (suspicious, 1.99) also fire by default.
   - `nonminimal_bool` and `overly_complex_bool_expr` moved to pedantic in 1.97, so they stop firing.
   - `unwrap_used`'s `allow-unwrap-in-tests` now covers integration tests (1.100). Check `clippy.toml` before relying on it, because the workspace warns on `unwrap_used`.
2. **Benchmark evidence.** A toolchain change invalidates the AArch64 baselines (`benchmarks/AGENTS.md`), and the 1.99 LLVM 23 update makes a codegen shift likely. Re-run the E2E evidence before quoting ratios.
3. **CLI stderr text.** 1.98 escapes more characters in `Debug` output, and 1.100 makes `str::escape_debug` always escape grapheme extenders. `stab-cli` diagnostics format user input with `{:?}`, so error text can change for unusual input. Compatibility is defined on stderr class, so this is cosmetic, but snapshot-style tests may need updating.
4. **Lint behavior.**
   - The `unsafe_code` lint now fires consistently on all unsafe attributes (1.98; already in the pin).
   - `semicolon_in_expressions_from_non_local_macros` now warns for macros from other crates (1.99).
   - `unreachable_cfg_select_predicates` joins the `unused` group (1.99).
   - Legacy integer modules such as `std::u64::MAX` are fully deprecated (1.99); the repo has none.
5. **Cargo behavior.**
   - Incremental compilation is off by default when `CI` is set (1.99), which can change CI build times.
   - The new `build-dir` layout (1.100) only matters to tools that read Cargo's intermediate artifacts. Stab's ops code uses its own `target/releases/` subtree and `CARGO_BIN_EXE_*`, so no impact is expected.
   - `cargo install` uses the packaged lockfile by default (1.100), which makes `--locked` redundant but harmless.
6. **Semantics to keep in mind.**
   - Iterating an exhausted `RangeInclusive` may now leave different `start()`/`end()` values (1.99).
   - `Iterator::min`/`max` call different `Ord` methods (1.100).
   - `assert_eq!` gets its own temporary scope (1.98).
   - `derive(PartialOrd)` takes a fast path when `Ord` is also derived (1.98).
   - None of these should matter with lawful `Ord` impls.

## Not Relevant To Stab

Nothing in 1.96–1.100 touches `std::simd` or `portable_simd`. Platform tier changes did not touch `aarch64-unknown-linux-gnu`. The WebAssembly, LoongArch, NVPTX, PowerPC, Windows, and embedded target changes, C-variadics, `extern "custom"`, the rustdoc flag stabilizations, and `-Cprofile-sample-use` do not affect current milestones.
