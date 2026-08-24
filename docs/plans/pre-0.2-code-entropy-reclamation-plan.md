# Pre-0.2 Code Entropy Reclamation Plan

Status: Active until E0 through E7 are complete. This plan temporarily supersedes the release-oriented A9 execution sequence in [GOAL.md](GOAL.md); the architecture plan remains the long-term product roadmap.

## Purpose

Reduce Stab's accidental maintenance surface before `0.2.0` without weakening the compatibility, filesystem-safety, process-supervision, or benchmark-science guarantees that already carry real risk. The work follows the project-local `reclaim-code-entropy` skill: every deletion must have consumer evidence, an explicit tradeoff, and a decisive verification step.

This plan responds to two external reviews. The first found correctness and data-integrity defects that justified strict result-format, file-identity, and subprocess controls. The second found that the repaired tests and measurement method were strong, but the qualification inventory, evidence ceremony, historical attestation code, release rehearsal lane, and unused public surfaces had become more expensive than their current value. The right response is therefore selective deletion, not broad simplification.

## Decisions And Rationales

### Keep load-bearing controls

- Keep the pinned Stim v1.16.0 differential corpus, strict byte grammars, typed DETS layouts, and path-alias role matrix because they independently protect compatibility and user data.
- Keep descriptor-safe artifact access, immutable output paths, bounded subprocess supervision, process-group cleanup, output limits, and `raw-work-v2` timing boundaries because each closes a demonstrated correctness or reproducibility failure mode.
- Keep independent Rust and C++ benchmark workers, paired alternating samples, exact workload and output validation, separate parity and Stab self-regression policies, and the active SIMD compare/report path because these are the scientific core of the performance claims.
- Keep the production release workflow and legacy M12 diagnostics. The production workflow is a user-facing release boundary, while M12 remains useful non-authoritative trend evidence during migration.

### Remove accidental obligations

- Retire the scratch release repository and its special binary, workflow, commands, target rules, and documentation. The rehearsal proved the production path once; maintaining a second GitHub repository and a parallel release product is not a permanent product requirement.
- Make executable runtime groups the detailed performance owner. The generated API and checklist inventories remain coverage maps, but they no longer fabricate one detailed future benchmark contract per public item.
- Remove public error variants and advanced-analysis exports with no production or external consumer. Stab is pre-`0.2.0`, and the project explicitly does not preserve compatibility for unused speculative APIs.
- Archive the executable A6 attestation implementation. Its historical artifacts remain readable records, but current qualification uses the active runtime-group, report, rollup, and completion contracts.
- Consolidate only duplicate representations whose semantics and consumers have been proved equivalent. Similar algorithms remain independent when their behavior, error context, or comparator role differs.

### Preserve history without executing it

Historical reports, tags, digests, and review outcomes remain append-only. Removing an obsolete producer does not turn old evidence into current evidence and does not erase the path by which the project reached its current contract.

## Scope

### In scope

- Project-local entropy-review instructions and a main-checkout-only development policy.
- Scratch release-lane retirement.
- Performance inventory and runtime-group ownership compaction.
- Removal of proved-unused public residue.
- A6 executable attestation archival.
- Proven parser, enum, value-type, test, and command consolidation.
- CI and generated-status synchronization.
- Milestone audit, full code review, and complete exact-head verification.

### Out of scope

- Product feature additions, new Stim parity surfaces, or changes to probabilistic behavior.
- Relaxing the `1.25x` Stim parity gate, `1.15x` Stab self-regression policy, memory limits, or comparator requirements.
- Formal controlled-host timing while refactoring source or contracts.
- Python, JS/WASM, GPU, dynamic plugins, runtime gate registration, external decoder transport, or public execution IR.
- Replacing production release safety, result-format compatibility, filesystem identity checks, or subprocess resource controls.

## E0: Freeze The Simplification Contract

### Tasks

1. Install `reclaim-code-entropy` under `.agents/skills/` and record that current work happens directly on `main` without linked worktrees.
2. Add this plan and rewrite `GOAL.md` as a short temporary execution contract.
3. Record the precise keep, remove, archive, and defer decisions above before changing production or operational code.
4. Preserve the previous A9 state in the architecture progress report instead of rewriting it as though the release sequence never existed.

### Rationale

Entropy work becomes unsafe when "simplify" is allowed to mean weakening validation or deleting history. Freezing the boundaries first gives each later deletion a stable test: it must reduce a contract or duplicate truth without removing a load-bearing guarantee.

### Acceptance

- The project-local skill and main-checkout policy are committed.
- `GOAL.md` points only to this active sequence and remains concise.
- No formal A9 evidence is generated from an intermediate simplification revision.

## E1: Retire The Scratch Release Lane

### Tasks

1. Delete `.github/workflows/release-rehearsal.yml`.
2. Remove the `stab-release-rehearsal` binary target and all scratch-repository dispatch, draft, ruleset, authorization, retry, and target-selection branches from `ops/release`.
3. Remove rehearsal-only `just` recipes and arguments while retaining the production release preflight, reviewed publication, asset verification, draft creation, and final verification commands.
4. Remove the fixed `ifsheldon/Stab-release-rehearsal` target and any policy that treats a second repository as an active release dependency.
5. Update `docs/RELEASING.md`, the architecture plan, progress report, and command documentation. Preserve old rehearsal run and private-draft identifiers as historical records, with an append-only retirement note.
6. Verify that no tracked source, workflow, recipe, or active document can dispatch to or publish into the deleted repository.

### Rationale

The rehearsal lane was a temporary risk-reduction device. Keeping it after the repository was deleted creates a second release state machine, authorization model, binary, workflow, and documentation path that cannot provide current value. The production release path already owns the durable contract.

### Tests

- Targeted `ops-release` unit and integration tests for production target validation, authorization, retries, asset verification, and immutable preflight behavior.
- Static searches for `Stab-release-rehearsal`, `release-rehearsal`, and `stab-release-rehearsal` in active code and workflows; historical reports may retain quoted references.
- `just release::publish-order` and the non-mutating production release checks documented by `docs/RELEASING.md`.

### Acceptance

- Exactly one active release workflow and one release operator product remain.
- Production release behavior and security boundaries are unchanged.
- Historical rehearsal evidence is readable but no longer executable.

## E2: Compact Performance Qualification Ownership

### Contract changes

- Bump performance inventory schema `4` to `5`.
- Bump runtime-group schema `10` to `11`.
- Make `benchmarks/qualification-runtime-groups.json` the detailed executable owner for feature, origin, API, checklist, workload, scale, correctness, comparator, profiler-note, and policy relationships.
- Keep `benchmarks/stim-qualification-suite.json` as a compact disposition and inherited-manifest index. Remove detailed mirrored public-API, checklist, and speculative future-group contracts.

### Tasks

1. Extend runtime-group records with typed `feature_id`, `origin`, public-API ownership, checklist ownership, and any disposition links needed to generate status without consulting a second detailed workload model.
2. Replace per-item generated performance records with aggregate source-owned dispositions: active parent, not performance relevant, future candidate, or inherited diagnostic row.
3. Delete generated future groups that have no executable contract. Retain exactly the 28 active runtime groups, 94 scales, 20 parity-policy groups, source-owned workload contracts, profiler notes, and architecture-specific accepted regression baselines.
4. Move validation to the runtime-group owner. Reject duplicate ownership, orphan groups, missing executable contracts, unknown feature/API/checklist references, stale policy entries, and release/diagnostic cap violations.
5. Preserve workload-contract digests and accepted regression identities when their semantic inputs are unchanged. Any schema or ownership-only digest migration must be explicit and tested; semantic changes invalidate the affected baseline.
6. Remove the deprecated hidden `qualification-regression` alias and redundant `qualification-*-regenerate --check` paths. The canonical `*-check` commands already regenerate in memory and byte-compare checked artifacts.

### Rationale

The complete API inventory is valuable as a coverage map, but a generated detailed benchmark contract for every API turns absence of a benchmark into thousands of speculative obligations. One executable runtime-group owner reduces mirrored truth while retaining exact release and diagnostic workload coverage.

### Tests

- Schema migration and deterministic regeneration tests.
- Unknown, duplicate, orphaned, over-cap, missing-contract, missing-prerequisite, stale-policy, and active-threshold-on-future-candidate rejection tests.
- Digest stability tests for ownership-only compaction and digest change tests for semantic workload changes.
- Existing adapter protocol, DEM parse, SIMD compare, parity, regression, memory, report, rollup, and completion tests.

### Acceptance

- The performance suite contains no speculative detailed group backlog.
- All 28 runtime groups and 94 scales remain executable and source-owned.
- Generated performance status is derived from one detailed runtime-group model.
- `just bench::qualification-check` proves both regeneration and validation; no second `--check` command is required.

## E3: Remove Unused Public Residue

### Tasks

1. Remove unused `CircuitError` variants `ParseLine`, `UnterminatedRepeatBlock`, and `UnexpectedRepeatTerminator` from `stab-core`. Keep the real parser diagnostics with matching names in `stab-model`.
2. Remove unused `BitError::MatrixShapeMismatch`.
3. Stop exporting and delete support that exists only for these unused `stab_analysis::advanced` surfaces: `ReverseFlowTransition`, `reverse_flow_transition`, `check_unsigned_flows_with_sparse_tracker`, `AnalyzerProbeBudget`, `ShiftedRecurrence`, `ShiftedRecurrenceSearch`, `SparseReverseFrameTracker`, and `search_shifted_recurrence`.
4. Keep consumed advanced surfaces such as `flow_record_index`, `decomposed_single_instruction`, and matched-error views and writers.
5. Remove tests and generated API records that only restate the deleted declarations. Preserve semantic tests for surviving flow, search, sparse tracking, and analyzer behavior.

### Rationale

These symbols have no workspace production consumer, dynamic entrypoint, persisted representation, or documented compatibility obligation. Removing them before `0.2.0` prevents speculative implementation details from becoming a permanent public contract.

### Tests

- Workspace-wide symbol and alternate-path search before and after deletion.
- Targeted `stab-bits`, `stab-core`, `stab-analysis`, `stab-engine`, API-doc, architecture, and correctness-inventory checks.
- Compile-fail or inventory checks must prove no documented public item silently disappears without the planned schema update.

### Acceptance

- No deleted symbol remains in production source, generated API inventories, or current public documentation.
- All surviving advanced exports have a real production or documented external role.

## E4: Archive Historical A6 Attestation Code

### Tasks

1. Delete `ops/bench/src/a6_focused_evidence.rs` and its submodules.
2. Remove hidden A6 producer, reader, replay, and attestation command variants and dispatch branches.
3. Move retained A6 JSON records to `benchmarks/archive/a6/` without rewriting their contents or identities.
4. Update active benchmark documentation to describe A6 as historical evidence. Keep a read-only narrative of what it established, not an executable compatibility layer.
5. Keep `qualification-simd-compare` and `qualification-simd-report`, which are active current-contract tools rather than A6 residue.

### Rationale

The A6 implementation is thousands of lines of executable code for a superseded evidence lifecycle. Historical evidence needs durable files and provenance, not a permanent producer and parser in the current benchmark binary.

### Tests

- Search all command names, module paths, and artifact locations.
- Verify active benchmark CLI help no longer exposes hidden A6 paths.
- Run current SIMD compare/report tests and qualification check to prove active functionality remains.
- Validate archived JSON remains byte-identical across the move.

### Acceptance

- No executable A6 attestation code remains.
- Archived records remain tracked and documented as historical.
- Current qualification and SIMD tools pass unchanged.

## E5: Consolidate Proven Duplicate Representations

### Tasks

1. Make oracle fixtures consume the canonical compatibility-matrix parser instead of defining a second `CompatibilityRow` schema and decoder.
2. Reuse the CLI's canonical `RecordFormatArg` in `sample_dem`; remove `SampleDemRecordFormatArg` and its duplicate conversion table.
3. Move the equivalent `ObservableMask` and `DetectorIndex` value/index implementations into a shared private DEM-search module used by graphlike and hypergraph search. Keep the two algorithms and their distinct error contexts independent.
4. Remove duplicate `Target` format/parser tests from `stab-core` where `stab-model` is the canonical owner and no facade behavior is being tested.
5. Remove malformed result-format test tables only when every case and applicable consumer is already owned by the checked compatibility corpus. Retain named regression tests that explain a previous defect or exercise a separate resource/cancellation property.

### Rationale

These cuts remove duplicate truths rather than merely shortening code. The canonical parser, enum, and value types already exist and have the same semantic domain. Algorithmic paths remain separate where similarity is not proof of interchangeable behavior.

### Tests

- Oracle matrix and implemented fixture runs.
- CLI `sample_dem` parsing and exact-output tests for all six formats.
- Graphlike and hypergraph DEM search suites, including overflow and error-context cases.
- Stim-format facade and model parser/printing tests.
- Result-format corpus, replay, convert, and `m2d` propagation tests.

### Acceptance

- One compatibility-matrix schema, one CLI result-format argument enum, and one private DEM search mask/index representation remain.
- No semantic compatibility, error-context, resource, or cancellation coverage is lost.

## E6: Synchronize CI And Generated Documentation

### Tasks

1. Update CI to call only canonical contract checks and remove retired release/A6/regeneration aliases.
2. Regenerate `docs/qualification-status.md` from the compact inventories and active completion checkpoint.
3. Make README, feature checklist, benchmark docs, contributor docs, release docs, architecture docs, and agent instructions link to generated volatile counts instead of copying them.
4. Mark the old release-rehearsal and A6 procedures historical. Do not edit old result claims except for an append-only supersession or retirement note.
5. Keep `GOAL.md` below roughly 80 lines and limited to current state, blockers, next actions, gates, and sources of truth.

### Rationale

Schema, count, and checkpoint duplication is itself an entropy source. CI should enforce the source-owned contracts, while generated status should be the only current count dashboard.

### Tests

- Workflow syntax and action-version checks.
- `just qualification::status --check`.
- Instruction-document, docs-link, architecture-doc, correctness, and benchmark regeneration checks.
- Searches for retired commands and volatile copied counts in active documentation.

### Acceptance

- Current status has one generated owner.
- Every canonical non-timing contract check runs in ordinary CI.
- No active documentation directs users to a deleted repository, A6 command, or redundant check path.

## E7: Audit And Verify The Exact Head

### Tasks

1. Run the `reclaim-code-entropy` apply verification: search every deleted symbol, route, repository name, command, and duplicate type; inspect the complete diff and net maintenance reduction.
2. Run `milestone-audit` against E0 through E6. Fix implementation and evidence defects; log only genuine under-specification in `milestone-spec-gaps.md`.
3. Run `full-code-review` across product code, CLI compatibility, qualification science, release safety, tests, and documentation. Fix all confirmed findings.
4. Run the full verification sequence below from one clean committed revision and require exact-head GitHub CI.
5. Do not start formal controlled-host A9 evidence until the simplified contracts and exact-head CI are final.

### Final verification

```text
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
just docs::api-check
just architecture::check
just architecture::consumer-check
just architecture::docs-check
just oracle::matrix --check
just oracle::result-formats --check
just oracle::run --implemented-only
just qualification::correctness-check
just bench::qualification-check
just qualification::status --check
just bench::qualification-probe --group pq1-adapter-protocol-smoke
just bench::qualification-probe --group pq2-dem-parse-adapter-smoke
just bench::smoke
just maintenance::pre-commit
git diff --check
```

### Acceptance

- Every planned deletion has consumer evidence, a documented tradeoff, and a decisive passing check.
- The worktree is clean on `main`, no linked worktree or simplification branch exists, and exact-head CI passes.
- The final report lists removed files, lines, public items, commands, representations, and intentionally retained high-value complexity.

## Focused Commit Sequence

1. `chore(agent): add project entropy review policy`
2. `docs(entropy): freeze pre-0.2 simplification`
3. `refactor(release): retire scratch rehearsal lane`
4. `refactor(bench): make runtime groups the performance owner`
5. `refactor(api): remove unused pre-0.2 public residue`
6. `refactor(bench): archive A6 attestation implementation`
7. `refactor: consolidate proved duplicate representations`
8. `chore(ci): enforce compact qualification contracts`
9. Audit and review fixes split by the ownership boundary they affect.

## Formal Evidence Boundary

No timing result produced from E0 through E7 is promotable. Once this plan is complete, the architecture plan's A9 release sequence resumes from a new clean source revision. All formal correctness, performance, completion, package, and release identities must bind that final revision and use new immutable artifact paths.
