# Rust Modernization Reference

This guide summarizes Rust 1.80.0 through Rust 1.95.0 features that should shape future Stab code.
It is a full-code-review skill reference for agents working in this repository, not a complete Rust release changelog.

Stab uses Rust 2024 edition and plans to use Nightly Rust for `portable_simd`.
Before recommending a feature below, still check the active toolchain, `rust-toolchain.toml`, and any future `rust-version` policy in `Cargo.toml`.

## Prefer These Language Features

### Let Chains In `if` And `while`

Use let chains when a validation path currently needs nested `if let` blocks.
This is useful in parsers, CLI validation, result-format decoding, and optional metadata handling.

```rust
if let Some(path) = output_path.as_ref()
    && !path.is_safe_output_path()
{
    return Err(CliError::InvalidOutputPath(path.display().to_string()));
}
```

This is clearer than nesting when each condition is part of the same guard.
Keep regular `match` when the branches have distinct behavior.

Official announcement: <https://blog.rust-lang.org/2025/06/26/Rust-1.88.0/>

### `if let` Guards In `match` Arms

Use `if let` guards when a `match` arm should only apply after a secondary fallible pattern match.
This is useful for parser states, CLI mode dispatch, and simulator state-machine code that already uses `match`.

```rust
match instruction.kind {
    InstructionKind::Repeat
        if let Some(body) = instruction.repeat_body()
            && body.is_empty() =>
    {
        return Err(ParseError::EmptyRepeatBlock);
    }
    InstructionKind::Repeat => analyze_repeat(instruction)?,
    InstructionKind::Gate => analyze_gate(instruction)?,
}
```

Do not rely on guard patterns for exhaustiveness.
Rust does not count guard conditions as proving that the overall `match` is exhaustive.

Official announcement: <https://blog.rust-lang.org/2026/04/16/Rust-1.95.0/>

### Async Closures

Use async closures for small async callbacks that need to borrow local state.
This may simplify oracle or benchmark helpers that run multiple command variants.

```rust
let run_case = async |case: &OracleCase| {
    let rust_output = rust_stim.run(case).await?;
    let cpp_output = cpp_stim.run(case).await?;
    compare_outputs(rust_output, cpp_output)
};
```

Use a named `async fn` when the logic is non-trivial, reused, or needs a clear error boundary.

Official announcement: <https://blog.rust-lang.org/2025/02/20/Rust-1.85.0/>

### Precise `impl Trait` Capture With `use<...>`

Use `+ use<...>` when returning `impl Trait` and the hidden type should capture only specific lifetimes, type parameters, or const parameters.
This replaces older "captures trick" style workarounds.

```rust
fn instructions<'a>(
    circuit: &'a Circuit,
) -> impl Iterator<Item = &'a CircuitInstruction> + use<'a> {
    circuit.instructions().iter()
}
```

In Rust 2024, return-position `impl Trait` captures more lifetimes by default.
Use precise capture when the default makes the API too restrictive or obscures what the returned value borrows.

Official announcements:

- <https://blog.rust-lang.org/2024/10/17/Rust-1.82.0/>
- <https://blog.rust-lang.org/2025/05/15/Rust-1.87.0/>

### Trait Upcasting

Use trait upcasting instead of adding manual `as_supertrait` methods when a trait object needs to be viewed as one of its supertraits.

```rust
trait OutputWriter: Send {
    fn flush(&mut self) -> std::io::Result<()>;
}

trait MeasurementWriter: OutputWriter {
    fn write_shot(&mut self, shot: &[bool]) -> std::io::Result<()>;
}

fn as_output_writer(writer: &mut dyn MeasurementWriter) -> &mut dyn OutputWriter {
    writer
}
```

Do not introduce trait objects only to use this feature.
Keep concrete generics where they are simpler.

Official announcement: <https://blog.rust-lang.org/2025/04/03/Rust-1.86.0/>

### Native Raw Pointer Syntax

Use `&raw const expr` and `&raw mut expr` in unsafe code that needs raw pointers without first creating references.
This matters for packed fields, FFI, and low-level bit/SIMD code.

```rust
let ptr = &raw const packed.not_aligned_field;
```

Stab should keep unsafe code scarce.
If unsafe code appears in bit kernels, FFI, or platform-specific code, prefer this syntax over `addr_of!` and document the safety invariant.

Official announcement: <https://blog.rust-lang.org/2024/10/17/Rust-1.82.0/>

### Rust 2024 Unsafe Boundaries

Rust 2024 tightened several safety-related defaults:

- `unsafe_op_in_unsafe_fn` warns by default.
- `extern` blocks should be written as `unsafe extern`.
- Unsafe attributes such as `no_mangle`, `link_section`, and `export_name` should be written as `#[unsafe(...)]`.
- References to `static mut` are denied by default.

When unsafe code is needed, make the unsafe operation explicit inside an `unsafe {}` block even inside an `unsafe fn`, and place a short `SAFETY:` comment on the invariant being relied on.

Official announcement: <https://blog.rust-lang.org/2025/02/20/Rust-1.85.0/>

### Exclusive Range Patterns

Use exclusive range patterns for adjacent numeric ranges in validation and classification code.

```rust
match probability {
    p if p < 0.0 => Err(ProbabilityError::Negative),
    0.0..1.0 => Ok(Probability::new_unchecked(probability)),
    1.0 => Ok(Probability::ONE),
    _ => Err(ProbabilityError::GreaterThanOne),
}
```

This avoids off-by-one constants and makes boundary ownership visible.

Official announcement: <https://blog.rust-lang.org/2024/07/25/Rust-1.80.0/>

### `_` Inference For Const Generics

Use `_` for const generic arguments in expression contexts when the compiler can infer the value from the surrounding type.

```rust
let row: [bool; 4] = [false; _];
```

This can reduce noise in tests, fixed-size bit blocks, and table-driven parser cases.

Official announcement: <https://blog.rust-lang.org/2025/08/07/Rust-1.89.0/>

### `cfg_select!` And Boolean `cfg`

Use `cfg_select!` when platform-specific code has multiple mutually exclusive branches.
This can help future CLI path handling, oracle process handling, or platform-specific benchmarking code.

```rust
let default_cache_dir = cfg_select! {
    unix => unix_cache_dir(),
    windows => windows_cache_dir(),
    _ => fallback_cache_dir(),
};
```

Use `cfg(true)` or `cfg(false)` when a generated or macro-heavy path needs an explicit always-on or always-off predicate.
For normal code, avoid clever cfg expressions.

Official announcements:

- <https://blog.rust-lang.org/2025/06/26/Rust-1.88.0/>
- <https://blog.rust-lang.org/2026/04/16/Rust-1.95.0/>

## Useful Stabilized APIs

### `LazyLock` And `LazyCell`

Use `std::sync::LazyLock` for process-wide static data that is expensive or awkward to initialize at compile time, such as static regexes, gate tables, format lookup tables, or reserved names.
Prefer it over adding a crate such as `lazy_static` or `once_cell`.

```rust
static GATE_NAMES: std::sync::LazyLock<std::collections::HashSet<&'static str>> =
    std::sync::LazyLock::new(|| std::collections::HashSet::from(["H", "CX", "M"]));
```

Do not use global lazy state for request-scoped or test-scoped configuration that should remain explicit.

Official announcement: <https://blog.rust-lang.org/2024/07/25/Rust-1.80.0/>

### `Option::take_if`

Use `Option::take_if` when validation or state transitions need to remove a value only if it satisfies a predicate.

```rust
let pending_tag = parser.pending_tag.take_if(|tag| tag.is_empty());
```

This can be clearer than a separate `if option.as_ref().is_some_and(...)` followed by `take()`.

Official announcement: <https://blog.rust-lang.org/2024/07/25/Rust-1.80.0/>

### `std::fs::exists`

Use `std::fs::exists` in synchronous CLI or test code when the only question is whether a path exists.
It avoids the common `metadata(...).is_ok()` idiom.

```rust
if !std::fs::exists(&input_path)? {
    return Err(CliError::MissingInput(input_path));
}
```

Official announcement: <https://blog.rust-lang.org/2024/09/05/Rust-1.81.0/>

### `HashMap::get_disjoint_mut` And Slice `get_disjoint_mut`

Use these APIs when code needs multiple mutable references from the same map or slice and the keys or indices are known to be distinct.

```rust
let [x_basis, z_basis] = table.rows.get_disjoint_mut([x_index, z_index]);
```

This is preferable to cloning, temporarily removing entries, or using interior mutability just to satisfy the borrow checker.

Official announcement: <https://blog.rust-lang.org/2025/04/03/Rust-1.86.0/>

### `std::io::pipe`

Use `std::io::pipe` for local child-process workflows that need to combine or redirect output without temporary files.
It may be useful for oracle subprocess tests or CLI compatibility checks.

Official announcement: <https://blog.rust-lang.org/2025/05/15/Rust-1.87.0/>

### Collection Filtering APIs

Use collection-native extraction APIs when removing and processing selected items:

- `Vec::extract_if`
- `LinkedList::extract_if`
- `BTreeMap::extract_if`
- `BTreeSet::extract_if`
- `VecDeque::pop_front_if`
- `VecDeque::pop_back_if`

These are good fits for parser queues, benchmark case selection, work queues in local tools, and test fixtures where matching entries need to be removed and processed.

Official announcements:

- <https://blog.rust-lang.org/2025/05/15/Rust-1.87.0/>
- <https://blog.rust-lang.org/2025/10/30/Rust-1.91.0/>
- <https://blog.rust-lang.org/2026/01/22/Rust-1.93.0/>

### Path And String Quality-Of-Life APIs

Prefer newer path helpers when they express intent directly:

- `Path::file_prefix` for archive names where only the first extension should be stripped.
- `PathBuf::add_extension` and `PathBuf::with_added_extension` for appending extensions without string formatting.
- `OsStr::display`, `OsString::display`, and `os_str::Display` for user-facing path-like values.

These are useful in CLI input/output handling, oracle fixture naming, artifact naming, and error messages.

Official announcements:

- <https://blog.rust-lang.org/2025/05/15/Rust-1.87.0/>
- <https://blog.rust-lang.org/2025/10/30/Rust-1.91.0/>

### `array_windows`

Use slice `array_windows` when scanning fixed-width windows.
It avoids manual indexing and gives the closure an array reference with a known length.

```rust
fn has_parent_dir_marker(bytes: &[u8]) -> bool {
    bytes.array_windows().any(|window: &[u8; 3]| window == b"../")
}
```

This can help in parsers, path validation, and compact test assertions.
Keep normal iterator code when the window size is dynamic.

Official announcement: <https://blog.rust-lang.org/2026/03/05/Rust-1.94.0/>

### `Duration` Constructors

Use `Duration::from_mins`, `Duration::from_hours`, and `Duration::from_nanos_u128` when they describe configuration defaults more clearly than manual multiplication.

```rust
const ORACLE_TIMEOUT: std::time::Duration = std::time::Duration::from_mins(5);
```

This is useful in oracle, benchmark, and CLI timeout code.

Official announcements:

- <https://blog.rust-lang.org/2025/10/30/Rust-1.91.0/>
- <https://blog.rust-lang.org/2026/01/22/Rust-1.93.0/>

### `bool: TryFrom<{integer}>`

Use `bool::try_from(value)` when decoding integer-backed booleans from external formats, instead of accepting any non-zero value implicitly.

```rust
let bit = bool::try_from(raw_bit).map_err(|_| FormatError::InvalidBit(raw_bit))?;
```

This is useful for strict result-format or binary-format import paths.

Official announcement: <https://blog.rust-lang.org/2026/04/16/Rust-1.95.0/>

## Lints And Diagnostics To Respect

### `mismatched_lifetime_syntaxes`

This lint warns when a function signature hides a lifetime in one position while showing or eliding it differently elsewhere.

Prefer spelling `'_` in return types when it makes a borrowed result obvious:

```rust
fn instructions(circuit: &Circuit) -> std::slice::Iter<'_, CircuitInstruction> {
    circuit.instructions.iter()
}
```

Official announcement: <https://blog.rust-lang.org/2025/08/07/Rust-1.89.0/>

### Dangling Raw Pointer Lint

Do not return raw pointers to local variables.
If unsafe code needs a pointer, make ownership explicit and keep the pointee alive for the required duration.

Official announcement: <https://blog.rust-lang.org/2025/10/30/Rust-1.91.0/>

### Never-Type Fallback Lints

If never-type future-compatibility lints fire, fix the type inference rather than allowing the lint.
Add explicit types around `?`, `return`, `panic!`, or diverging closures when needed.

Official announcement: <https://blog.rust-lang.org/2025/12/11/Rust-1.92.0/>

## Project Guidance

- Prefer these features when they remove real nesting, cloning, temporary variables, or unsafe-code ambiguity.
- Do not refactor working code solely to demonstrate a new Rust feature.
- When touching CLI platform-specific code, consider `cfg_select!` before duplicating `#[cfg(unix)]` and `#[cfg(not(unix))]` helper functions.
- When touching validation code, consider let chains, `Option::take_if`, exclusive range patterns, `bool::try_from`, and direct path APIs.
- When touching parser code, consider `array_windows`, let chains, and precise iterator lifetimes before manual indexing.
- When touching table, matrix, or simulator code, consider `get_disjoint_mut` before reaching for clones or interior mutability.
- When adding unsafe code, follow Rust 2024 unsafe-boundary style and keep the safety invariant local to the unsafe operation.

## Release Announcement Links

- Rust 1.80.0: <https://blog.rust-lang.org/2024/07/25/Rust-1.80.0/>
- Rust 1.81.0: <https://blog.rust-lang.org/2024/09/05/Rust-1.81.0/>
- Rust 1.82.0: <https://blog.rust-lang.org/2024/10/17/Rust-1.82.0/>
- Rust 1.83.0: <https://blog.rust-lang.org/2024/11/28/Rust-1.83.0/>
- Rust 1.84.0: <https://blog.rust-lang.org/2025/01/09/Rust-1.84.0/>
- Rust 1.85.0: <https://blog.rust-lang.org/2025/02/20/Rust-1.85.0/>
- Rust 1.86.0: <https://blog.rust-lang.org/2025/04/03/Rust-1.86.0/>
- Rust 1.87.0: <https://blog.rust-lang.org/2025/05/15/Rust-1.87.0/>
- Rust 1.88.0: <https://blog.rust-lang.org/2025/06/26/Rust-1.88.0/>
- Rust 1.89.0: <https://blog.rust-lang.org/2025/08/07/Rust-1.89.0/>
- Rust 1.90.0: <https://blog.rust-lang.org/2025/09/18/Rust-1.90.0/>
- Rust 1.91.0: <https://blog.rust-lang.org/2025/10/30/Rust-1.91.0/>
- Rust 1.92.0: <https://blog.rust-lang.org/2025/12/11/Rust-1.92.0/>
- Rust 1.93.0: <https://blog.rust-lang.org/2026/01/22/Rust-1.93.0/>
- Rust 1.94.0: <https://blog.rust-lang.org/2026/03/05/Rust-1.94.0/>
- Rust 1.95.0: <https://blog.rust-lang.org/2026/04/16/Rust-1.95.0/>

