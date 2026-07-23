# Post-Review Compatibility And Evidence Repair

## Status

Active remediation plan as of 2026-07-23.

This plan supersedes the current-acceptance interpretation of the DEM evidence produced at revision `80fb5405fb077c694a8a8a18e64a3a5831e20a5e`. Those artifacts remain immutable historical `raw-work-v1` evidence, but independent review found a result-format compatibility defect, destructive CLI path-alias behavior, an unsafe legacy benchmark process runner, public status drift, and a qualification timing-boundary defect. No affected correctness or performance claim may be promoted until this plan closes.

The frozen compatibility target remains Stim v1.16.0 at commit `e2fc1eca7fd21684d433aa5f10f4504ea4860d07`. The exact Stim random stream and all already-deferred products remain outside this remediation.

## Objective

Repair every verified external-review finding and the Rust qualification timing defect without weakening the existing `1.25x` performance gates. The result must prevent explicit CLI path aliases from destroying input, match pinned Stim's public `01`, HITS, and DETS grammars and DETS semantics, use one bounded process supervisor throughout benchmark operations, identify the corrected raw-work timing boundary in every worker receipt, and rebuild affected evidence from one clean committed revision.

## Non-Negotiable Rules

- Use test-driven development. Add an independently selectable failing regression before changing each production contract when practical.
- Treat path data loss and result-format incompatibility as release blockers.
- Keep the `1.25x` timing gates, comparators, semantic work, source-owned thresholds, and memory rules unchanged. Fix regressions instead of adding waivers.
- Keep every failed, superseded, or review-rejected artifact at its original path and under its original schema and source identity.
- Never reuse a failed or previously published artifact path.
- Do not relabel `raw-work-v1` receipts as `raw-work-v2`.
- Generate promotable correctness and performance evidence only from a clean committed revision with `local_modifications=false`.
- Do not create commits unless the user explicitly requests them. Until then, dirty-tree runs are diagnostic only.
- Disable recorded swap only immediately before formal timing and restore the exact prior swap configuration on success, failure, or interruption.

## Public Interface Changes

- Add public `DetsLayout`, `DetsResultType`, and `DetsToken` types.
- Add layout-aware materialized DETS reading, dense and packed DETS visitors, typed-token visitors, and `SparseShot` visitors.
- Preserve existing width-based result-reader signatures. Their DETS behavior becomes explicitly measurement-only and rejects `D` and `L`.
- Add private CLI `FileRole` and retained file-identity types plus `CliError::ConflictingFileRoles`, whose diagnostic names both conflicting flags.
- Replace both benchmark subprocess implementations with one bounded internal process supervisor.
- Add the explicit timing-boundary identity `raw-work-v2` to qualification worker receipts and contracts.

## R0: Freeze Claims And Scope

### Tasks

1. Make this document the active remediation plan and rewrite `docs/plans/GOAL.md` as its execution contract.
2. Reopen the broad result-format, `01`, HITS, DETS, convert, and CQ2 result-format completion claims in `docs/stab-feature-checklist.md`.
3. Replace the README's broad verified-drop-in claim with the narrower statement that selected implemented surfaces have pinned compatibility evidence.
4. Mark the `80fb540` DEM chain review-rejected and historical under `raw-work-v1` in the active goal, DEM progress report, and benchmark adapter documentation.
5. Regenerate correctness and performance inventories after the public API changes land, then update the PQ0 report and test-porting plan from generated counts instead of manual arithmetic.

### Tests

- Search public documentation for stale current-qualification language and affected `Done` rows.
- Run correctness and performance inventory regeneration checks after R3.

### Acceptance

No public document claims qualification for an affected reopened surface, and no document presents `80fb540` as current promotable evidence.

## R1: Prevent File-Alias Data Loss

### Design

Add workspace dependency `same-file = "1.0.6"`. Introduce a command-wide I/O plan that opens and retains all active explicit path inputs before opening any active output. Open every active output in one batch with create/write access but without truncation, compare existing file identities across every input/output and output/output pair, and truncate regular files only after all pairings pass. Devices and other stream-like outputs must remain usable without `set_len` or seeking. Input/input aliases remain valid.

The I/O owner must reuse the validated retained handles. It must not validate one path identity and later reopen that path. Failed-preflight cleanup must not unlink by pathname because a replacement race could delete a file Stab did not create; absent a descriptor-relative race-free cleanup primitive, a newly created empty output may remain after rejection. Commands with multiple modes must construct the role plan only after validating the mode, so inactive paths are neither opened nor truncated.

### Command Role Matrices

| Command | Input roles | Output roles |
| --- | --- | --- |
| `sample` | `--in` | `--out` |
| `detect` | `--in` | `--out`, `--obs_out` |
| `m2d` | `--circuit`, `--in`, `--sweep` | `--out`, `--obs_out` |
| `analyze_errors` | `--in` | `--out` |
| `sample_dem` | `--in`, `--replay_err_in` | `--out`, `--obs_out`, `--err_out` |
| `convert` | `--in`, `--circuit`, `--dem` | `--out`, `--obs_out` |

### Tests

- Table-drive every meaningful input/output and output/output role pair.
- Exercise direct equality, normalized relative aliases, symlinks, and hardlinks.
- Require nonzero exit status and both flag names in stderr.
- Seed each existing input and output with distinct sentinel bytes and prove none are truncated on rejection.
- Cover zero-shot `sample`, `detect`, and `sample_dem` paths.
- Cover a failure after successful preflight to prove aliases are checked before any output truncation.
- Cover `/dev/null` and a mixed regular/special multi-output command.
- Cover mode-specific inactive roles, including `.stim -> .stim` conversion with `--obs_out`, `--circuit`, or `--dem`.
- Preserve distinct-path output, stdout, missing-path, and permission behavior.

### Acceptance

Every explicit conflict fails before truncation, direct `File::create` and `std::fs::write` output paths are removed from command implementations, and distinct-path behavior is unchanged.

## R2: Implement Byte-Exact Text Grammars And Typed DETS

### Design

Create a focused byte-oriented text-record lexer module in `stab-core`. Materialized, dense, packed, sparse, convert, replay, and `m2d` consumers must use it. Public grammar validation must not use `trim()`, `split_whitespace()`, or empty-token skipping.

`01` records require exactly the configured bits followed by LF or CRLF. EOF immediately after data is an error. HITS requires a strict comma-separated unsigned-decimal grammar followed by LF or CRLF. DETS accepts pinned leading whitespace before `shot`, requires exactly one ASCII space before each typed token, applies independent M/D/L bounds, and follows pinned Stim's DETS EOF rule.

Dense and packed DETS consumers set addressed bits to true. Raw typed-token visitors preserve token type, order, and duplicates. `SparseShot` retains measurement and detector hits and applies parity only to its observable mask. HITS dense duplicate parity remains unchanged.

### Public API Contract

- `DetsLayout` stores checked measurement, detector, and observable counts and computes typed offsets and total width.
- `DetsResultType` represents measurement, detector, or observable namespaces.
- `DetsToken` stores a result type and namespace-local index.
- `read_dets_records` materializes layout-aware dense records.
- Layout-aware dense and packed visitors reuse one record buffer.
- The typed-token visitor reuses one token buffer while preserving source order and duplicates.
- The `SparseShot` visitor reuses hit and observable-mask buffers with Stim-compatible parity semantics.
- Existing width-based DETS readers delegate to `DetsLayout::measurement_only` and reject `D` or `L`.
- `stab convert` uses the public layout-aware parser instead of a private DETS parser.

### Tests

- Mixed `shot M0 D0 L0` with independent namespace offsets.
- Duplicate M and D tokens set dense bits and remain duplicated in typed sparse events.
- Duplicate L tokens set dense bits but cancel only in `SparseShot.obs_mask`.
- Namespace-local first and last valid indexes plus first-invalid indexes.
- Large observable indexes and checked total-width overflow.
- Empty records, empty input, LF, CRLF, and pinned DETS EOF behavior.
- Unterminated `01` and HITS rejection.
- Leading, trailing, doubled, spaced, and tabbed HITS separators.
- Missing, doubled, trailing, and tabbed DETS separators.
- Invalid bytes, numeric overflow, and out-of-range indexes.
- Visitor cancellation after the first record.
- Reused dense, packed, token, and `SparseShot` buffers.
- CLI convert, replay, and `m2d` propagation.

### Acceptance

All text consumers share the lexer, no public grammar path normalizes invalid whitespace, and focused differential tests match pinned Stim v1.16.0.

## R3: Repair Oracle And Qualification Ownership

### Corpus

Add a checked result-format corpus with:

- schema version and pinned Stim source identity;
- stable case ID;
- hex-encoded input bytes;
- format;
- M/D/L layout;
- pinned Stim acceptance class;
- canonical `01` output bytes for accepted inputs;
- applicable Stab entrypoints.

Include separator mutations, EOF placement, LF and CRLF combinations, invalid bytes, empty records, duplicate tokens, namespace crossings, numeric overflow, and bounds failures.

### Operations

Add `just oracle::result-formats --check`, implemented by the Rust `stab-oracle` binary. It must build or locate the pinned Stim and Stab CLIs through existing approved workflows, execute both over every applicable corpus case, require matching acceptance, and require exact canonical output for accepted cases.

### Qualification

- Split the false dense-duplicate owner into HITS parity and DETS set-semantics owners.
- Replace the false unterminated-`01` expectations.
- Add independently selectable owners for typed DETS APIs, strict text grammars, CLI propagation, replay and `m2d` propagation, and file-role safety.
- Regenerate and check `oracle/qualification-manifest.json`.
- Regenerate the PQ0 performance inventory after the exported API inventory changes.

### Acceptance

The checked corpus reproduces pinned Stim v1.16.0, every corrected selector is independently executable, no round-trip-only test owns compatibility, and generated inventory counts and digests are synchronized.

## R4: Replace Legacy Benchmark Process Runner

### Design

Move the bounded qualification process implementation to the shared benchmark process module and remove the legacy runner. The common supervisor must support:

- bounded captured output or explicit discard;
- inherited or cleared environments plus explicit variables;
- process groups and descendant termination;
- concurrent stdin, stdout, and stderr handling;
- cancellation;
- timeout;
- RSS observation;
- CPU affinity;
- regular-file output limits;
- complete thread joining on every exit path.

Legacy baseline callers retain a 600-second timeout and inherited environment. Captured baseline stdout and stderr are capped at 8 MiB. Callers that historically discarded stdout continue to discard it.

### Migration

Migrate Git, CMake, Stim CLI, `stim_perf`, metadata, baseline, qualification, and probe invocations to the common supervisor. Remove `wait-timeout` from workspace dependencies.

### Tests

- Child fills stdout before reading stdin.
- Independent stdout and stderr floods hit bounded limits.
- Child closes stdin early.
- Child never exits.
- Descendant retains pipe handles.
- Cancellation occurs during active I/O.
- Invalid UTF-8 diagnostics remain available as bounded bytes.
- Discard policy does not allocate captured output.
- Timeout and output-limit errors retain bounded diagnostic prefixes.

### Acceptance

`pq1-process-contract-smoke`, benchmark smoke, a pinned Stim build, and one real CLI baseline complete through the common supervisor, and no legacy runner remains.

## R5: Correct Qualification Timing Boundary

### Contract

Dispatch occurs outside each timed region. The finish clock is sampled immediately after the raw workload returns and before enum wrapping, marker construction, digesting, RSS collection, or moving the result into a protocol wrapper.

Use separate output-returning and mutation-only timing helpers. Add an injectable clock for deterministic ordering tests.

### Schema Changes

- Timing boundary: `raw-work-v2`.
- Runtime-group schema: 5 to 6.
- Worker protocol: 4 to 5.
- Contract preflight: 14 to 15.
- Qualification report: 33 to 34.
- Bump each derived preflight, rollup, or completion schema that serializes or digests the new field.
- Make both Rust and C++ workers emit and validate the same boundary identity.

### Tests

- Fake-clock ordering proves finish sampling occurs before output wrapping, result movement, markers, digesting, and RSS.
- Rust and C++ protocol vectors include `raw-work-v2`.
- Missing, stale, unknown, or mismatched timing-boundary identities fail at worker, invocation, report replay, rollup, and completion boundaries.
- Reproducibility proves both workers bind the same source-owned timing contract.

### Acceptance

All worker and report paths reject `raw-work-v1` as current evidence, while historical files remain readable only by their historical schema handlers where supported.

## R6: Rebuild Correctness And Performance Evidence

This milestone begins only after the user explicitly authorizes focused commits and the source, tests, inventories, and documentation have been committed.

### Correctness

1. Run focused PR, full, and soak qualification for every reopened result-format and file-role case.
2. Produce and replay a fresh full correctness prerequisite for both DEM groups under the regenerated inventory.
3. Run `just oracle::result-formats --check`, implemented oracle fixtures, matrix checks, and the checked correctness regeneration.

### Legacy Benchmarks

Probe `m8-measure-reader-01`, `m8-measure-reader-hits`, `m8-measure-reader-dets`, affected convert rows, and `m2d` text conversion. Update the M8 profiler note in the same source commit as any optimization or changed parser evidence. Run a fresh primary baseline, beta gate, timing regression, and memory regression without adding waivers.

### DEM Qualification

From one clean unchanged commit:

1. Reproduce both workers.
2. Run both DEM adapter probes.
3. Produce exactly twelve unique reports: parse and print, each at small, medium, and large scales, each at full and soak tiers.
4. Replay and regression-check all twelve.
5. Produce and replay four architecture rollups.
6. Produce and independently replay two completion receipts.
7. Repeat both accepted-maximum memory probes at 524,288 work items.

### Host Safety

Record the exact active swap configuration, disable it immediately before formal timing, and restore that exact configuration through an interruption-safe owner. After the run, verify swap state and prove no qualification process remains.

### Acceptance

Both DEM groups pass median and upper-confidence-bound ratios at or below `1.25x`, every source artifact has clean provenance and a unique path, every replay is byte-consistent, and no failed artifact has been overwritten.

## R7: Documentation, Audit, Review, And Closure

1. Synchronize README, feature checklist, comprehensive plans, this goal, test-porting plan, benchmark docs, generated inventories, and the DEM progress report.
2. Preserve all historical and failed evidence references and explain why the old oracle and timing boundary were invalid.
3. Run the `milestone-audit` skill against every milestone, test, benchmark, resource boundary, schema transition, and acceptance criterion.
4. Fix every implementation, correctness, evidence, resource, benchmark, and documentation finding. Log only genuine under-specification in `docs/plans/milestone-spec-gaps.md`.
5. Run the `full-code-review` skill across core compatibility, CLI filesystem safety, process supervision, benchmark science, tests, and documentation.
6. Fix every confirmed P0 through P3 issue and rerun affected checks.

### Final Verification

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --quiet
just oracle::version
just oracle::result-formats --check
just oracle::run --implemented-only
just oracle::matrix --check
just qualification::correctness-check
just qualification::correctness-regenerate --check
just bench::smoke
just bench::qualification-check
just maintenance::pre-commit
```

### Final Acceptance

The worktree is clean, swap is restored, no qualification process remains, public claims match generated evidence, and every affected compatibility and performance claim points to fresh `raw-work-v2` evidence from the same reviewed source revision.
