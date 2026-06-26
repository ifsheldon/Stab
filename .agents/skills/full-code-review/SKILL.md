---
name: full-code-review
description: Use when performing a complete Stab code review across Rust library code, CLI behavior, Stim compatibility, file formats, SIMD/performance, security, tests, and documentation alignment.
---

# Full Code Review

This skill defines the expected review bar for Stab.
Use it before broad reviews, release-readiness reviews, compatibility reviews, security reviews, performance reviews, or refactor planning.

## Review Stance

Act like a senior engineer and architect with a high quality bar.
Prioritize confirmed correctness, security, compatibility, performance, architecture, and maintainability issues over stylistic preferences.
Do not soften a real release blocker, and do not inflate taste-only concerns into bugs.

Findings must be evidence-backed:

- Lead with findings, ordered by severity.
- Include exact file paths and tight line references.
- Explain the failure mode, compatibility break, security risk, performance risk, or architectural cost.
- State whether the issue is confirmed or a residual risk.
- Suggest a concrete remediation.
- Avoid vague comments such as "clean this up" without a target design.

## Pre-Review Checks

Before manual review lanes, run a large-file check from the workspace root:

```bash
find . \
  -path './target' -prune -o \
  -path './.git' -prune -o \
  -type f \
  \( -name '*.rs' -o -name '*.md' -o -name '*.toml' \) \
  -print0 | xargs -0 wc -l | sort -nr | head -40
```

Treat Rust source files over 1200 lines as P2 maintainability findings unless they are generated or intentionally table-like data.
Treat files over 900 lines as a watch list, especially when the reviewed change adds more code to them.
If this command cannot run, record that as residual risk and use an equivalent native line-count pass.

Also inspect the current workspace health when relevant:

```bash
cargo fmt --check --all
cargo clippy --workspace --all-targets
cargo test --workspace
```

Do not treat missing future crates, oracle binaries, or benchmark infrastructure as findings unless the reviewed change claims those surfaces are implemented.

## Review Lanes

Cover these lanes when the user asks for a complete review.

### Rust Code Quality

- Flag non-idiomatic Rust, weak error handling, avoidable `unwrap` or `expect`, duplicated logic, excessive coupling, missing regression tests, and reinvented functionality that a mature crate should handle.
- Flag raw primitive values that have domain meaning after parsing.
  Important Stab examples include qubit IDs, detector IDs, observable IDs, measurement record offsets, gate names, result formats, repeat counts, probabilities, target encodings, file-format tags, and CLI mode names.
- Flag raw `_path`, `path: String`, and path-like fields such as `*_file`, `*_dir`, `*_root`, `*_prefix`, and `*_key` after external parsing boundaries.
  Prefer explicit wrappers for repository-relative paths, archive-relative paths, CLI input/output paths, scratch directories, fixture paths, generated artifact paths, and storage keys if they are added.
- Flag scattered `.trim()` or case conversion near domain parser calls when normalization belongs in the domain type constructor.
- Flag handwritten hash or commit validation if the value is really a Git object ID, a SHA-256 content digest, or a future OCI digest.
  Use a domain type backed by a suitable library or fixed bytes.
- Flag secrets stored as plain `String` beyond immediate CLI, env, or config-file boundaries if secret-bearing infrastructure is added.
  Prefer `secrecy` wrappers and require any secret exposure at the exact transmission or comparison boundary.
- Check whether code can be simplified with current Rust language features and standard-library APIs documented in `references/rust-modernization.md`.
  Prefer these updates when they remove real nesting, repeated allocation, lossy error handling, platform-specific duplication, manual time/path logic, or unsafe-code ambiguity.

### Stim Compatibility

- Review against the frozen compatibility target documented in `docs/plans/rust-stim-drop-in-rewrite.md`.
- Treat `.stim`, `.dem`, and result file formats as public contracts.
- Treat CLI stdout, stderr class, exit status, accepted flags, default values, and input/output formats as compatibility surfaces for implemented commands.
- Flag behavior that diverges from Stim v1.16.0 without an explicit plan update, test, or compatibility rationale.
- Exact C++ Stim random streams are not required.
  Statistical and semantic equivalence are required for probabilistic behavior.
- For probabilistic changes, require a test that checks the distribution or semantic invariant instead of only checking that output exists.
- For parser and printer changes, require round-trip tests and oracle cases where possible.
- For `analyze_errors`, detector error models, loop folding, gauge detectors, and decomposition behavior, treat subtle semantic drift as a high-risk compatibility issue.

### CLI And File-Format Boundaries

- Review CLI parsing as an external boundary.
  Raw values should be parsed into typed values before reaching core logic.
- Check that file paths supplied through CLI flags cannot escape intended output roots when a command writes files or expands archives.
- Check that binary and text result formats preserve documented byte order, line endings, shot ordering, detector ordering, observable ordering, and bit-packed layout.
- Check that error messages are actionable and do not leak secrets, temporary implementation details, or huge unbounded input echoes.
- Flag compatibility-sensitive default changes unless the plan and tests are updated.

### SIMD And Performance

- Ensure direct `std::simd` usage remains isolated in bit-kernel modules.
- Require scalar reference tests for portable-SIMD kernels.
- Flag architecture-specific intrinsics unless the plan has been revised or a benchmark proves portable SIMD is insufficient.
- Look for hidden allocations, avoidable clones, cache-hostile layouts, branchy hot loops, and repeated parsing inside sampling or detector workflows.
- Review benchmarks for analysis time and per-shot throughput separately.
- Do not accept performance claims without a reproducible benchmark or profiler evidence.
- Do not recommend GPU work unless CPU portable-SIMD profiling shows a large batch-parallel bottleneck and transfer overhead is considered.

### Operational Commands

- Flag repository operational logic implemented as shell scripts.
- Review the root `justfile` and files under `justfiles/` as the human-facing command surface.
- Keep `just` recipes thin and namespaced.
  They should dispatch commands and pass arguments, not own complex branching logic.
- Complex operational behavior should live in Rust binaries under an `ops` crate.
  This includes oracle setup, compatibility runs, benchmark orchestration, release checks, report generation, downloads, and path validation.
- Flag duplicated operational logic across recipes or direct Cargo commands when a shared `ops` binary would provide safer validation and clearer errors.
- Check that operational workflow changes update `AGENTS.md`, `docs/plans/rust-stim-drop-in-rewrite.md`, or any future developer workflow docs.

### Security And Hostile Inputs

- Treat user-provided circuits, detector error models, result files, archives, paths, generated files, logs, and workspace contents as hostile input.
- Review filesystem races, permission windows, path identity checks, Unix byte and UTF-8 assumptions, panic-based denial of service, ignored cleanup errors, and silently dropped I/O errors.
- Reject ZIP/archive traversal, symlinks that escape roots, oversized artifacts, excessive file counts, and writes outside intended output directories if archive or fixture extraction is added.
- Check for unbounded memory growth in parsers, printers, DEM expansion, repeat handling, sample output, and benchmark fixture generation.
- Check for panic paths reachable from malformed external input.
  Internal invariant panics may be acceptable only when the invariant is constructed and tested locally.

### Architecture And Modularity

- Review ownership of invariants, not only file size.
  Ask which layer owns each truth: domain value, parser, printer, gate table, simulator, detector conversion, DEM analyzer, result format, CLI command, oracle harness, or benchmark harness.
- A modularity finding must name the proposed owner and target shape.
  Avoid vague advice such as "decouple this" or "split this file".
- Keep transport and command parsing in CLI crates, and keep semantic behavior in core crates.
- Keep parser/printer rules near the file-format model they validate.
- Keep bit-level storage details behind bit-kernel abstractions instead of leaking lane layout into simulators or file-format code.
- Keep oracle and benchmark code out of core logic.
- For large files, diagnose why they are growing and name the missing ownership boundary instead of only restating the line count.

### Test Quality

- Flag trivial or low-value tests as P3 findings unless they mask a higher-risk issue.
- Low-value examples include tests that only restate constants, assert fields on freshly constructed structs, check generic library behavior, assert static labels without workflow behavior, or duplicate stronger contract tests.
- Recommend deletion when a test has no meaningful regression value.
- Recommend replacement when the surrounding code needs coverage of real behavior, edge cases, security properties, file-format contracts, CLI behavior, statistical equivalence, or performance-sensitive invariants.
- Prefer focused regression tests for fixed bugs and compatibility tests for public behavior.

### Documentation Alignment

- Check that changes to planned scope, milestone order, compatibility targets, CLI behavior, public file formats, benchmark gates, and developer workflow update the matching docs.
- `docs/plans/rust-stim-drop-in-rewrite.md` is the current roadmap and should stay aligned with large implementation decisions.
- If generated docs, schemas, API references, or compatibility matrices are introduced later, verify they are regenerated from the source of truth.

## Targeted Search Starters

Use these searches as starting points, then inspect context manually.
Do not report a finding from a search result without reading the surrounding code.

### Rust Panic And Error Handling

```bash
rg -n "unwrap\\(|expect\\(|panic!|todo!|unimplemented!|\\[[^\\]]+\\]" crates
rg -n "\\.ok\\(\\)|unwrap_or_default|let _ =" crates
```

### Typed Boundaries

```bash
rg -n "pub [a-zA-Z0-9_]*_(id|name|path|file|dir|root|prefix|key|format|gate): String|[a-zA-Z0-9_]*_(id|name|path|file|dir|root|prefix|key|format|gate): &str" crates
rg -n "id_or_|name_or_|slug|identifier|parse_[a-zA-Z0-9_]+\\(|new_[a-zA-Z0-9_]+\\(" crates
rg -n "\\.trim\\(\\).*parse|parse_.*\\(.*\\.trim\\(|try_new\\(.*\\.trim\\(|to_lowercase\\(\\).*try_new|try_new\\(.*to_lowercase\\(" crates
```

### File And OS Boundaries

```bash
rg -n "File::create|fs::metadata|fs::set_permissions|fs::remove_file|create_dir|canonicalize|read_to_string|write\\(" crates
rg -n "from_utf8_lossy|str::from_utf8|String::from_utf8|PathBuf|Path::new" crates
```

### SIMD And Hot Paths

```bash
rg -n "std::simd|portable_simd|Simd<|unsafe|from_slice|as_ptr|as_mut_ptr" crates
rg -n "clone\\(|collect::<Vec|to_vec\\(|String::from|to_string\\(" crates
```

### CLI And Compatibility

```bash
rg -n "clap|args|stdout|stderr|exit|format|sample|detect|m2d|analyze_errors|sample_dem|convert|gen" crates
rg -n "stim|dem|detector|observable|qubit|measurement|rec\\[" crates docs
```

### Operational Commands

```bash
find . -path './target' -prune -o -path './.git' -prune -o -name '*.sh' -print
rg -n "mod |^[_a-zA-Z0-9:-]+:|cargo run -p ops|bash|sh -c|python " justfile justfiles 2>/dev/null
find ops -maxdepth 3 -type f 2>/dev/null
```

### Secrets And External Access

```bash
rg -n "token|bearer|password|secret|client_secret|api_key|Authorization|ExposeSecret|expose_secret" .
```

## Subagent Instructions

When spawning a subagent for Rust core, CLI, compatibility, security, or performance review, explicitly ask that subagent to read `references/rust-modernization.md` before reviewing code.
The subagent should report places where newer Rust features or APIs simplify Stab code without causing churn for its own sake.

For modularity and architecture review, assign at least one subagent or one manual pass to crate, module, struct, workflow, and bit-kernel boundaries.
That pass should report both findings and positive confirmation where intended boundaries are respected.
It should explain the current owner, the proposed owner, and the smallest target shape that removes mixed responsibility.

For compatibility review, assign at least one pass to compare implemented behavior against Stim v1.16.0 using oracle tests or direct inspection of upstream behavior.
The reviewer should separate exact compatibility requirements from statistical equivalence requirements.

Do not report a modularity issue only because a file is long or a helper is small.
Report it when the code mixes ownership of invariants, duplicates policy, forces distant layers to know too much, or makes meaningful tests hard to write.

## Severity Guidance

- P0: Release blocker, likely security compromise, data leak, destructive data corruption, or uncontrolled public resource exhaustion.
- P1: Serious correctness, security, compatibility, lifecycle, or scaling issue that should be fixed before enabling the affected feature publicly.
- P2: Important maintainability, reliability, compatibility, performance, or architecture concern that can be scheduled but should not be ignored.
- P3: Low-risk cleanup, test gap, or polish issue with limited blast radius.

## Validation Expectations

For implementation follow-up after review, require focused regression tests around each fixed behavior.
Before committing fixes, run the relevant checks:

```bash
cargo fmt --check --all
cargo clippy --workspace --all-targets
cargo test --workspace
```

Run oracle and benchmark checks when the reviewed change affects compatibility or performance.
Do not add tests just to increase test count.
A good test should protect a specific behavior, contract, regression, security property, workflow, or compatibility surface.
