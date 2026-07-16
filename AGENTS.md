# Instructions for Agents

## Conversation Requirements

- Ask for clarification when the task specification is ambiguous or a reasonable assumption would be risky.
- Share honest engineering thoughts before acting when a request has design, architecture, security, compatibility, or workflow implications.
- Think proactively and point out likely sources of rework, especially around Stim compatibility, public file formats, and performance claims.
- Do not make commits unless the user explicitly asks for a commit.

## Documentation

- `docs/plans/rust-stim-drop-in-rewrite.md` is the current implementation roadmap for the Rust Stim drop-in rewrite.
- `docs/plans/GOAL.md` is the active execution contract, and it currently points at `docs/plans/comprehensive-correctness-qualification-plan.md` and `docs/plans/comprehensive-stim-performance-qualification-plan.md`.
- CQ1 and PQ1 harness acceptance is recorded in `docs/plans/cq1-correctness-harness-progress-report.md` and `docs/plans/pq1-performance-harness-progress-report.md`; CQ2 is active at `docs/plans/cq2-deterministic-qualification-progress-report.md`, and any later correctness or performance inventory digest change makes its clean refresh historical until the affected tiers are rerun from a clean committed revision.
- During the qualification program, do not promote a correctness claim from file-level or broad shared evidence, and do not promote a Stim performance ratio until its exact correctness prerequisites and equivalent semantic work pass.
- Use `just qualification::correctness-list`, `just qualification::correctness-check`, and `just qualification::correctness-regenerate --check` for the CQ0 case and public-API inventory; update the frozen digest and checked manifest together when reviewed source ownership changes. `oracle/qualification-cases.json` is the source-owned exact-parent ledger for collapsing reviewed upstream, public-API, and oracle owners onto independently selectable qualification cases; stale, duplicate, cross-feature, comparator-mismatched, or shared-primary mappings must fail closed.
- Use `just qualification::correctness-provenance-probe` to rebuild private Stab and Stim binaries, execute one real source-owned case through the normal qualification runner, and validate the published request, execution, report, completion, and preflight bindings.
- Use `just qualification::correctness-run --tier pr`, `--tier full`, or `--tier soak` to execute source-owned CQ1 evidence; qualification outputs must stay below `target/qualification/`, and dirty reports are diagnostic rather than promotable evidence.
- CQ1 runs must retain fresh private Stab and Stim builds, immutable sealed copies of the canonical direct-executable identity ledger, Cargo invocation from `/` with absolute manifests and private config-free homes, a private Git index reconstructed from `HEAD`, descriptor-owned fixture side outputs and support cleanup, the hashed explicit child environment, exact per-comparison statistical completion accounting, sticky process-group cancellation, and repository-anchored descriptor-owned publication; do not replace these contracts with shared mutable binaries, inherited configuration, path-reopened artifacts, or exit-status-based shot credit.
- CQ1 qualification execution is Linux-only and must fail closed elsewhere because its timeout and publication contracts require process-group termination and atomic directory exchange.
- Use `just qualification::correctness-report --out <report-directory>` to validate `request.json`, `report.json`, `completion.json`, every case execution receipt, and the derived Markdown and preflight artifacts, then use `just qualification::correctness-preflight --out <report-directory> --case <qualification-case-id> --request-sha256 <run-request-sha256> --completion-sha256 <run-completion-sha256>` to verify the controller-approved selection and outcomes before dependent performance work.
- Use `--allow-deferred` only with explicit correctness `--case` filters for diagnostic visibility; a report containing deferred cases is never valid preflight evidence.
- Existing Cargo primary selectors in the correctness manifest must select one concrete libtest case with `--exact`; broad filters are supporting evidence only and cannot close a planned atomic owner.
- Use `just bench::qualification-list`, `just bench::qualification-check`, and `just bench::qualification-regenerate --check` for the PQ0 performance disposition ledger; update the frozen digest and checked inventory together when reviewed checklist, API, manifest, threshold, waiver, or upstream perf ownership changes.
- Use `just bench::qualification-probe --group pq1-process-contract-smoke` and `just bench::qualification-probe --group pq1-adapter-protocol-smoke` to reproduce the bounded process and pinned-Stim adapter contracts independently; product adapter probes additionally include `pq2-circuit-parse-adapter-smoke`, `pq2-circuit-canonical-print-adapter-smoke`, `pq2-gate-name-hash-adapter-smoke`, `pq2-simd-word-popcount-adapter-smoke`, and `pq2-simd-bits-xor-adapter-smoke`.
- Use `just bench::qualification-run --tier pr`, `--tier full`, or `--tier soak` to execute PQ1 paired evidence below `target/benchmarks/qualification/`, then use `just bench::qualification-report --input <report-directory>` and `just bench::qualification-regression --input <report-directory>` to revalidate derived evidence and source-owned thresholds.
- The PQ1 `pq1-adapter-protocol-smoke` group is diagnostic infrastructure, never a product performance ratio: it does not accept correctness evidence, remains report-only, and cannot be promoted even from a clean verified full or soak run.
- PQ1 offline validation must compare reports to the currently checked correctness and performance inventory digests, replay the current pinned Rust toolchain, bind the report output directory, regenerate the exact preflight bytes, and use compare-and-swap publication so a stale refresh cannot overwrite newer evidence.
- Future product groups must reconstruct canonical CQ request, report, completion, preflight, and execution receipts. Cargo and property selector digests are hashes of their displayed selectors; oracle-fixture and ops-check digests are resolved source-contract digests frozen into the controller-approved request and must not be replaced by hashes of weaker display selectors.
- PQ1 must recompute the pinned-Stim adapter build fingerprint from the recorded source, library, tools, and canonical compiler arguments; derive expected work in the parent; perform semantic preflight at the exact common calibrated batch shape; bind every subsequent validation, warmup, sample, and memory digest to that preflight; and audit tracked, staged, and untracked repository state through a private Git index reconstructed from `HEAD`. Calibration probes select the batch size and are work-bound but do not produce ratio evidence.
- Promotable future performance groups must bind exact clean CQ preflight evidence, equal work and semantic output, a clean unchanged revision, verified host policy, symmetric worker identities, full or soak samples, and a source-owned regression rule. `--allow-unverified-host` is diagnostic only.
- The controlled PQ1 host profiles require stable thermal-zone identity and readings at or below 85000 millidegrees Celsius before and after a run in addition to affinity, load, memory, swap, and frequency-governor checks.
- PQ1 calibration targets at least 350 milliseconds so a separately executed common batch can jitter without silently crossing the contractual 250-millisecond acceptance floor; reports must retain both values and reject common validation outside 250 milliseconds through 2 seconds.
- File-writing qualification workers must wait on the start barrier before writing so the controller installs the child regular-file limit before releasing measured work; do not replace this with post-run file-size inspection.
- When changing planned scope, milestone order, compatibility targets, public CLI behavior, or benchmark acceptance gates, update the matching plan document in the same change set.
- Use `.agents/skills/milestone-audit` when auditing whether a milestone implementation satisfies its objective, tasks, linked tests, benchmarks, and done criteria, or when implementation reveals milestone loopholes or under-specified scope.
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
- When adding a new dependency, search for its latest stable version on crates.io before adding it. Do not recall a version from your memory.
- Use the workspace structure described in `docs/plans/rust-stim-drop-in-rewrite.md` unless the plan is deliberately revised.
- Pin Nightly Rust in `rust-toolchain.toml` before using `portable_simd`.
- This workspace currently pins `nightly-2026-06-20` so local and CI checks use the same toolchain.
- Keep direct `std::simd` usage isolated in bit-kernel modules, with scalar reference implementations available for tests.
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
- Use `just oracle::version` to validate that `vendor/stim` is pinned to Stim v1.16.0, and use `just oracle::run --case smoke/help` plus `just oracle::run --case smoke/tiny-circuit` for M0 oracle smoke checks.
- Use `just oracle::list` to inspect and validate the M2 fixture corpus, including coverage of planned M4 through M11 P0/P1 C++ compatibility-matrix rows by upstream source, milestone, and parity mode; use `just oracle::list --milestone Mx` and `just oracle::run --milestone Mx` for milestone-scoped fixture work, `just oracle::record --check-clean` to verify committed runnable exact-output fixtures against pinned Stim, `just oracle::run --implemented-only` for implemented fixture parity, and `just oracle::run --all` to report red or manifest-only future fixtures.
- Use `just oracle::matrix --check` to validate the M1 compatibility matrix, and use `just oracle::matrix --milestone Mx` to inspect acceptance rows for implementation milestones.
- Use `just oracle::blockers` to validate and summarize the source-owned non-deferred blocker closure ledger, use `just oracle::blockers --list` to inspect every owned PFM-B subcase and its planned, implemented, or evidence-close state, and use `just oracle::blockers --check-selectors` to prove every claimed existing Cargo test selector resolves to at least one test.
- Use `just rust::parser-fuzz` as the local long-running M4 `.stim` parser fuzz-smoke target.
- Treat the M0 `stab-cli sample` path as a hidden oracle smoke shim only; it is not real `stim sample` compatibility, which belongs to M8.
- Use `just bench::list` to inspect M3 benchmark contracts, `just bench::smoke` to validate them without long runs, `just bench::baseline --stim vendor/stim` to record pinned C++ Stim baselines under `target/benchmarks/baseline/latest`, and `just bench::compare --milestone Mx` to inspect planned Stab-vs-Stim comparison rows.
- After committing changes to private worker sources, build inputs, or receipt policy, use `just bench::qualification-worker-reproducibility` from the clean unchanged commit; each receipt must hash the ordered framed collection of `worker.rs` and every executable child module from the materialized source it builds, the Stim adapter receipt must bind CMake's resolved `libstim` compile flags into the standalone adapter build, and each sealed binary must confirm its source and build identity through the worker protocol. Every normal qualification run and the standalone reproducibility check must execute the frozen protocol vector, one- and two-iteration popcount accumulation vectors, the true maximum accepted popcount width, and pre-barrier rejection of the first unsupported circuit-parse scale, an 83-item partial gate-table sweep, a below-minimum popcount width, an in-range unaligned width, and the first over-cap aligned width. The canonical preflight digest must include both workers' exact source, build-fingerprint, and binary identities, and report replay must reject a refingerprinted preflight transplanted from another worker pair. Two isolated release builds must produce identical Stim and Stab source, build-fingerprint, binary-digest, and canonical-contract-preflight identities.
- Use `just bench::qualification-rollup --group <group> --tier <full-or-soak> --input <scale-report> ... --out <rollup-directory>` from the same clean committed revision recorded by the source reports to bind exactly one current promotable report per source-owned scale into an architecture-scoped family. A family must also share exact Stim and Stab source, build-fingerprint, binary-digest, and canonical-contract-preflight identities. Use `just bench::qualification-rollup-report --input <rollup-directory>` to replay the checked contract and source report and preflight digests, reject altered derived evidence, and atomically regenerate the rollup Markdown and preflight. Keep full and soak rollups separate, and never mix AArch64 and x86-64 reports.
- Benchmark baseline `--out` paths must be repository-relative paths under `target/benchmarks/`, and `--only` filters must exactly match benchmark row ids or milestone names such as `M7`.
- The pre-commit hook must stay shell-script-free in this repository: build and install the Rust binary instead of adding a tracked shell launcher.
- The hook should treat submodules as pointer updates, run Rust checks only for staged Rust-affecting paths, scan staged source blobs for oversized files, and check Stab's instruction-document policy only when instruction docs or `.gitmodules` change.
- Every scanned `README.md` must have a colocated `AGENTS.md`, and every effective `AGENTS.md` source must have at least one `CLAUDE.md` symlink pointing to it.
- Document new operational workflows in the matching docs when adding or changing them.

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
- Keep platform-owned metadata and scratch files outside user-writable paths when possible.
- Do not treat storage quotas, container quotas, network policy, log limits, scratch cleanup, and metadata ownership as substitutes for one another if runner or artifact infrastructure is added later.

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
