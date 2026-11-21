# Instructions for Agents

## Conversation Requirements

- Ask for clarification when the task specification is ambiguous or a reasonable assumption would be risky.
- Share honest engineering thoughts before acting when a request has design, architecture, security, compatibility, or workflow implications.
- Think proactively and point out likely sources of rework, especially around Stim compatibility, public file formats, and performance claims.
- Do not make commits unless the user explicitly asks for a commit.
- Perform current development directly on the `main` checkout. Do not create another branch or linked worktree unless the user explicitly changes this policy.

## Priorities

When goals conflict, the higher tier wins.

1. Good, well-designed, and correct APIs; performance comparable to or better than Stim; and a well-designed modular architecture.
2. Stim compatibility.
3. Backward compatibility.

- The three goals in the first tier rank equally; when they pull against each other, raise the tradeoff with the user instead of choosing silently.
- This repository is experimental, so change a public API whenever the result is a better and correct design; do not keep a weaker signature or defer a redesign only to avoid breakage.
- Diverge from Stim only when the first tier requires it, and record every deliberate divergence in the parity ledger as a design choice with its rationale and a test.
- Still document each public change in the matching migration notes, and do not treat this order as permission to promote an unmeasured performance claim.

## Documentation

- Parity-ledger and oracle workflows live in `docs/AGENTS.md` (served by `docs/README.md`); the single E2E performance contract lives in `benchmarks/AGENTS.md`. Read the matching file before correctness or benchmark work.
- Do not promote a correctness claim from file-level or broad shared evidence, and do not promote a Stim performance ratio until the exact parity prerequisites and equivalent semantic work pass.
- When changing implemented behavior, public APIs, CLI flags, supported file formats, operational workflows, or developer workflows, update the matching documentation in the same change set.
- If generated documentation, schemas, API references, or compatibility matrices are introduced, regenerate them when changing the source of truth.
- When editing Markdown prose, do not insert hard line breaks in the middle of a sentence; keep each sentence on one physical line unless a table, list, code block, quoted source, or other format requires line breaks.

## Coding Requirements

### General Engineering Guidelines

- Prioritize code quality, maintainability, correctness, and measured performance over quick transliteration from C++ Stim.
- Do not rebuild the wheel when a well-maintained Rust crate solves a feature well, unless the user explicitly asks for an in-house implementation or the dependency would compromise Stim compatibility.
- Do not write trivial or low-value tests.
- Tests should protect meaningful behavior, contracts, regressions, security properties, file-format compatibility, CLI compatibility, statistical equivalence, or performance-sensitive invariants.
- Keep source files below 1200 lines when practical.
- If a source file grows past 1200 lines, propose a refactor before adding more unrelated functionality to it.
- When fixing lint findings, preserving behavior is mandatory.
- Replacing `unwrap`, `expect`, indexing, assertions, or other panic-prone code must not silently continue, skip work, substitute defaults, or weaken limits when the previous code would fail fast.
- Prefer eliminating impossible states by construction, for example by building the correctly typed value directly instead of constructing a generic value and then asserting its shape.
- If a failure can happen at runtime, handle it with a clear domain error.
- If the old code represented an internal invariant that cannot be eliminated, convert it to a precise internal error instead of a vague fallback.
- No unsafe fixes should be applied merely because a linter suggests them.
- Avoid `unsafe` unless there is a measured, documented need and a safe abstraction boundary with tests.

### Rust And Cargo

- Treat this as a Cargo workspace project, even while the repository is still small.
- When adding a new dependency, search crates.io for its latest stable version that was published at least three days ago and add that one. Do not recall a version from your memory.
- `.cargo/config.toml` sets `registry.global-min-publish-age` to three days, so the resolver ignores younger crates.io versions unless `Cargo.lock` already pins them; do not lower or override that cooldown to pick up a fresher release.
- Use the component ownership and dependency graph described in `docs/architecture/README.md` unless an architecture decision deliberately revises it.
- The workspace pins Stable Rust `1.100.0` in `rust-toolchain.toml`, every product crate declares 1.100 as its minimum supported Rust version, and unstable features must not be introduced.
- Keep direct `fearless_simd` usage isolated in `stab-kernels-simd`, with scalar reference implementations available for tests.
- Prefer small crates and clear module boundaries over large cross-cutting modules.
- Avoid public APIs that expose awkward lifetimes, unnecessary generic parameters, or borrowed internals that will be painful to wrap with Python bindings later.
- During iteration, prefer targeted `cargo test` commands over expensive full-suite runs.
- Do not run formatters that rewrite files unless formatting is part of the task or you are preparing a requested commit.
- Before a requested commit, run the relevant full verification for touched areas; once the workspace exists, this should include `cargo fmt --check`, `cargo clippy --workspace --all-targets`, and `cargo test --workspace` unless the project documents a stricter command.

### Operational Commands

- Do not add shell scripts for repository operations.
- Use a root `justfile` with modular files under `justfiles/` as the human-facing operational command surface.
- Keep `just` recipes thin and declarative.
- Use namespaced recipes such as `rust::check`, `oracle::run`, `bench::sample`, and `maintenance::large-files` instead of a growing flat command list.
- Put complex operational logic in Rust binaries under an `ops` crate, then call those binaries from `just`.
- Complex logic includes branching workflows, path validation, downloads, report generation, compatibility orchestration, benchmark orchestration, release checks, and any workflow that would otherwise become a multiline shell script.
- Use `just maintenance::setup-hooks` to install the staged-aware Rust pre-commit hook into `.git/hooks/pre-commit`.
- Use `just maintenance::pre-commit` to run the hook manually against the staged index.
- The oracle, parity, and E2E benchmark command surface is documented in `CONTRIBUTE.md`; correctness workflows follow `docs/AGENTS.md` and benchmark workflows follow `benchmarks/AGENTS.md`.
- The pre-commit hook must stay shell-script-free in this repository: build and install the Rust binary instead of adding a tracked shell launcher.
- The hook should treat submodules as pointer updates, run Rust checks only for staged Rust-affecting paths, scan staged source blobs for oversized files, and check Stab's instruction-document policy only when instruction docs or `.gitmodules` change.
- Every scanned `README.md` must have a colocated `AGENTS.md`, and every effective `AGENTS.md` source must have at least one `CLAUDE.md` symlink pointing to it.
- Document new operational workflows in the matching docs when adding or changing them.
- Use `just release::publish-order` and `just release::check --out target/releases/<unique-name>` for coordinated product-crate release preflight. Use `just release::publish-reviewed` only after reviewing that immutable preflight, use `just release::build-binary` plus `just release::verify-assets` for local tagged-artifact checks, and use the single `just release::create-draft` operation for descriptor-retained GitHub draft creation and remote digest verification. Local irreversible recipes must accept typed quoted options, build each operator with every explicit publication-token variable removed in a fresh owner-only target, and bind the needed token variable only while executing that exact operator from an isolated user session; never place the binding on Cargo, a build script, an action step, an inherited workflow or job environment, a composed shell command, an argument, or a generated report. Dispatch the release workflow from the exact annotated tag ref and require its event ref, checked-out SHA, tag peel, and operator target to agree with the reviewed release commit. Follow `docs/RELEASING.md` for irreversible crates.io publication, checksum-bound partial-release recovery, tag creation, draft publication, and final verification.

### Stim Compatibility

- Target Stim v1.16.0 as the frozen compatibility baseline until the plan is explicitly changed.
- Treat `.stim`, `.dem`, and result file formats as public contracts.
- Treat CLI stdout, stderr class, exit status, and accepted flags as compatibility surfaces for implemented commands.
- Exact C++ Stim random streams are not required.
- Statistical and semantic equivalence are required for probabilistic behavior.
- Prefer oracle tests against C++ Stim v1.16.0 whenever compatibility behavior is in question.
- Do not implement compatibility shims for behavior outside the documented target without updating the plan first.
- Preserve the CLI implementation order from the plan unless explicitly changed: `gen`, `convert`, `sample`, `detect`, `m2d`, `analyze_errors`, then `sample_dem`.
- Defer `diagram`, `explain_errors`, `repl`, Python bindings, JS/WASM, Crumble, and GPU work unless the user explicitly changes the milestone focus.

### Typed Boundaries

- Avoid stringly typed domain values.
- Stable identifiers, indexes, paths, probabilities, storage keys, file-format tags, gate names, result formats, detector IDs, observable IDs, qubit IDs, measurement record references, and repeat counts should use explicit Rust types after parsing.
- Raw strings are acceptable only at immediate external boundaries such as CLI arguments, environment variables, config files, and deserialization inputs before validation.
- Put canonical normalization in typed constructors, not scattered call sites.
- Avoid ad hoc `.trim()`, `.to_lowercase()`, or similar cleanup immediately before parser calls unless the domain type cannot own the normalization for a clear reason.
- Do not create free-standing domain constructors, parsers, or generators when a type method is the natural home.
- Prefer `Type::generate()`, `FromStr`, `TryFrom`, `try_new`, or associated constructors such as `Circuit::from_stim_bytes` and `DetectorErrorModel::from_dem_bytes`.
- Filesystem, archive, CLI input/output, fixture, scratch, generated artifact, and storage-key values should be typed after parsing.

### Hostile Inputs And Filesystems

- Treat user-provided circuits, detector error models, result files, archives, paths, generated files, logs, and workspace contents as hostile input unless a tighter trust boundary is documented.
- Reject path traversal, unsafe path components, unexpected symlinks, unsafe archive entries, and writes outside intended output roots.
- That rule covers paths Stab derives from input content; a path the caller names on the `stab` command line carries the caller's own authority and is opened as given, as the path trust boundary in `docs/architecture/README.md` records.
- Keep platform-owned metadata and scratch files outside user-writable paths when possible.
- Do not treat storage quotas, container quotas, network policy, log limits, scratch cleanup, and metadata ownership as substitutes for one another if runner or artifact infrastructure is added later.
- Hostile-input protection means memory safety, no panics, checked arithmetic, bounded allocation, and bounded recursion; CPU time is the caller's to bound and is not capped by fixed work limits.

### Secrets And External Processes

- Secrets such as API tokens, bearer credentials, private keys, and one-time codes must use explicit secret-handling wrappers after the external boundary when the language ecosystem provides them.
- Keep raw secret strings only at the immediate CLI, environment, or config-file boundary, and expose them only at the exact call site that must transmit, hash, compare, or store them.
- Secrets must not appear in command-line arguments, logs, error messages, debug output, default CLI output, snapshots, screenshots, or test fixtures.
- If a command requires a password, such as `sudo` or `ssh`, ask the user for help instead of trying to work around it.

## Commit Message Convention

Use a lightweight Conventional Commit style for new commits:

```text
<type>(<scope>): <imperative summary>

<body explaining why and notable details>

<footer, if needed>
```

For cross-cutting commits where one scope would be misleading, omit the scope:

```text
<type>: <imperative summary>
```

Allowed types:

- `feat`: new behavior or user-facing capability.
- `fix`: bug, security, lifecycle, compatibility, or correctness fix.
- `refactor`: restructuring without intended behavior change.
- `docs`: documentation, README, or agent instruction updates.
- `test`: test-only changes.
- `chore`: tooling, dependency, metadata, or generated-only maintenance.
- `perf`: performance improvement.
- `style`: formatting-only changes with no behavior change.

Use repo-local, concrete scopes such as `core`, `cli`, `oracle`, `bench`, `bits`, `parser`, `dem`, `docs`, or `workspace`.

Commit subject rules:

- Use imperative mood, such as `add`, `fix`, `reject`, or `document`.
- Keep the subject under about 72 characters when practical.
- Do not end the subject with a period.
- Mention the user-visible or public contract when that is the important change.

Commit body rules:

- Use a multiline body when the "why" is not obvious, behavior changes, migrations are involved, or tradeoffs matter.
- Write the body as motivation and important consequences, not a file-by-file changelog.
- Include verification notes only when useful, especially for non-obvious tests or intentionally skipped checks.
- Use footers such as `BREAKING CHANGE: ...` or `Refs #123` when they add useful context.

Use a multiline body for any commit that changes public APIs, file formats, CLI behavior, benchmark gates, security behavior, persistence behavior, or operational workflow.

Narrow docs, client, or test-only commits may stay one-line if the subject is self-explanatory.

## Technical Defaults

- Assume `uv` for Python environments unless the user or project docs explicitly choose another tool.
- Use `rg` for search and `rg --files` for file discovery.
- Use targeted tests during implementation and broader verification before requested commits.
- Do not skip tests for trivial reasons.
- If a required local service, toolchain, or oracle binary is missing, document the blocker and either install it through the project-approved workflow or ask the user for the missing external setup.
