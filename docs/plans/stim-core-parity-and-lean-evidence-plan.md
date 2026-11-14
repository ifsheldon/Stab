# Stim Core Parity And Lean Evidence Plan

Status: P0 through P10 are complete for their recorded revisions, and no milestone is active. P10 closed on `94714e23`, the evidence descendant of measured source `1f42845e`, where `e2e-release-check` and exact-revision CI both pass. Known gaps that no milestone owns are listed in [open-items.md](open-items.md).

## Summary

Complete semantic feature parity with Stim v1.16.0 for Stab's core Rust libraries and command-line workflows, excluding deprecated Stim surfaces, documented Stim bugs, and the explicitly deferred product areas below. Prove it with a small behavior-oriented correctness suite and one end-to-end performance system.

This is a deliberate pre-1.0 contract reset. Existing Stab APIs, test ledgers, benchmark schemas, and operational commands may be removed instead of receiving compatibility shims when a cleaner direct design replaces them. Stim-visible semantics remain the target; compatibility with obsolete Stab-only interfaces is not a goal.

Component ownership and the permitted dependency graph are owned by [the architecture records](../architecture/README.md), and public API removals by [the 0.2 migration guide](../MIGRATING-0.2.md). Known gaps that no milestone owns are listed in [open-items.md](open-items.md).

## Scope

### Included

- Both frozen Stim v1.16.0 text models: circuits and detector error models.
- Every nondeprecated gate, alias, argument rule, legal target shape, tag behavior, and repeat-block behavior in the frozen gate catalog.
- Circuit parsing, printing, inspection, generation, transformation, flow analysis, sampling, detection conversion, and detector error model conversion.
- Detector error model parsing, printing, transformation, sampling, search, and satisfiability behavior that belongs to the selected core API.
- The `01`, `b8`, `r8`, `hits`, `dets`, and `ptb64` result formats, including materialized, streaming, packed, sparse, and typed-layout workflows where those representations are useful.
- The seven computational CLI commands `gen`, `convert`, `sample`, `detect`, `m2d`, `analyze_errors`, and `sample_dem`, plus the Stim `help` discovery surface. Stab's concise implemented-only help text is an explicit divergence from Stim's bundled documentation.
- Idiomatic Rust public APIs for the same stable capabilities. Stim's stable Python API defines relevant behavior, but Python object shape and source compatibility are not goals; C++ implementation and tests provide semantic examples, not a stable API contract.
- Stab-native agent inspection commands, JSON Lines diagnostics, decoder sessions, external circuit passes, and `.stim -> .stim` canonical conversion remain tested extensions, but they do not count toward Stim parity completion.
- Safe typed limits for hostile input, allocation, recursion, output size, and process control. A deliberate limit may differ from Stim when the ledger documents the divergence and tests the boundary.

### Deferred

- Python bindings.
- JavaScript and WebAssembly bindings.
- Ecosystem integrations such as Cirq, PyMatching, Sinter, and Crumble.
- `diagram`.
- `explain_errors` and full `ErrorMatcher` provenance.
- `repl` and interactive simulator products.
- Public `TableauSimulator` and `FlipSimulator` products; their internal algebra and frame semantics remain required by batch workflows.
- Python or NumPy array conversion, state-vector conversion, arbitrary-unitary import or export, and `tableau_to_unitary`; the existing bounded Rust `unitary_to_tableau` subset remains a separately owned capability.
- QASM, Quirk, and other export surfaces.
- GPU execution.
- Exact reproduction of Stim's random bit streams.

Deprecated Stim functions and flags are omitted instead of deferred. Confirmed Stim bugs are not copied silently; each intentional correction must be recorded as an explicit divergence with a pinned reproduction and a Stab regression test.

### Deprecated Exclusions

| Surface | Reason for exclusion | Pinned source |
|---|---|---|
| Legacy top-level dispatch flags `--sample`, `--detect`, `--gen`, `--m2d`, and `--analyze_errors` | Superseded by named subcommands; Stab has no compatibility normalization or second parser path for them. | `src/stim/main_namespaced.test.cc` and command-specific tests |
| `--detector_hypergraph` | Deprecated Stim spelling for `analyze_errors`; the named command is the supported surface. | `src/stim/cmd/command_analyze_errors.test.cc` |
| `sample --frame0` | Deprecated alias for `--skip_reference_sample`. | `src/stim/cmd/command_sample.cc` |
| `detect --prepend_observables` | Deprecated observable ordering that Stim itself warns against. | `src/stim/cmd/command_detect.cc` |

Stim's MBQC decomposition helper is also excluded because it feeds rich help rendering instead of a stable computational API. Stab's concise structural help is already an explicit divergence. Python-only deprecated aliases are outside the selected Rust and CLI product boundary.

## Single Parity Source Of Truth

`oracle/stim-v1.16-parity.toml` is the sole current feature-parity ledger manifest. It names bounded, normalized family fragments under `oracle/stim-v1.16-parity/` so the logical ledger remains one validated source without creating an oversized contract file. Each entry records:

- a stable behavior-family ID;
- the user-observable contract;
- one implementation status: `done`, `missing`, `deferred`, or `divergence`;
- the owning product crate;
- a separate evidence status: `verified` with one canonical semantic test selector, or `needs-owner` with the milestone that replaces broad historical evidence;
- the pinned Stim source or test references;
- the completion milestone when behavior is `missing`;
- a rationale when status is `deferred` or `divergence`.

There is no `partial` status. A broad row that contains both implemented and missing behavior must be split until every row has one honest implementation status. Missing canonical evidence never changes an implemented row back to `missing`; it changes only the evidence state. Entries describe behavior families, not every Rust export, every upstream test file, or every fixture.

Schema version 3 removes oracle execution tiers and the former status-only `required_fixture_ids` list. The fixture manifest owns each supporting row's command, comparator, source attribution, and inputs, and every row executes in CI against pinned Stim. One `just oracle::parity-run` executes the complete canonical owner set.

The ledger generates `docs/stim-parity.md`. That generated document is the only status view; hand-maintained feature lists, checklists, and counts are not kept.

## Completed Milestones

The tasks, tests, and checkpoints of the completed milestones are retained in Git history; this plan is last present in full at commit `7b0aa4f3`.

| Milestone | Outcome |
| --- | --- |
| P0 | Scope frozen and status ledgers replaced by the one parity ledger. |
| P1 | A lean correctness suite with one canonical owner test per completed family. |
| P2 | The public architecture reset: thin facade, direct component owners, one plan, session, and sink model. |
| P3 | Model, gate, and record-format parity closed. |
| P4 | Sampling and detection parity closed. |
| P5 | Analysis, transform, search, and algebra parity closed. |
| P6 | CLI and Rust workflow parity closed. |
| P7 | The performance machinery replaced by one E2E suite owned by `benchmarks/suite.toml`. |
| P8 | User-visible regressions against Stim optimized within the `1.25x` ceiling of the suite of that revision. |
| P9 | One formal AArch64 evidence pair published and superseded history retired. |

The measurement contract, release matrix, and evidence procedure those milestones introduced are now owned by [benchmarks/README.md](../../benchmarks/README.md), [benchmarks/AGENTS.md](../../benchmarks/AGENTS.md), `benchmarks/suite.toml`, and [GOAL.md](GOAL.md).

## Milestone P10: Repair Evidence And Rebalance Tests (Complete)

A November test and benchmark audit found real defects hidden by evidence that looked stronger than it was. DEM decimal printing selected its notation before rounding and diverged from pinned Stim just below powers of ten; `convert` could exit successfully after an unflushed stdout write failed; standard-stream redirection could alias and truncate a command input; repeated flags and arguments after `--` diverged from Stim without a ledger record; `Tableau::random` was a bounded random walk rather than Stim's uniform sampler; and the benchmark runner's spawn path folded the runner's own high-water RSS into every child's reported peak, so the memory ceilings never measured Stab. Separately, most fixture rows only re-ran cargo tests, live-Stim fixtures never ran in CI, several statistical tolerances could not fail, CLI route owners used all-zero data, and most E2E cases were dominated by process startup.

### Tasks

1. Fix the product defects above, record the strict argument grammar and standard-stream policy as Stab divergences, and port uniform random tableau sampling.
2. Retire `cargo-test`, placeholder, and red fixture rows together with their milestone, parity-mode, and status columns and the status-only `required_fixture_ids` list; run every remaining live-Stim fixture and the pinned-Stim probes on an AArch64 CI job.
3. Remove duplicated, label, harness-only, and self-comparing tests, repointing ledger owners to the strongest surviving test.
4. Add the missing high-value coverage: falsifiable statistical bounds, sparse-noise and Bernoulli statistics, independent conversion oracles, fold and decomposition properties, model and record properties, binary result-format differentials, non-zero CLI route data, and Stim-versus-Stab two-sample sampling differentials.
5. Measure child peak RSS through a freshly executed launcher, bump the timing boundary, and replace startup-dominated E2E cases with user-scale QEC workflows within the existing case and family caps.
6. Share one bounded process supervisor across operations crates and replace hand-rolled confined filesystem primitives with maintained crates where the reduction is real.
7. Produce one fresh unseeded controlled AArch64 full-and-soak pair, seed baselines from it, and pass `e2e-release-check` with exact-revision CI.

Tasks 1 through 5 are complete as of `a3260617`, and task 6 as of `6b774b9a`: `ops/bench` and `ops/oracle` run every child through the shared `ops/process` supervisor, while `ops/release` keeps its own supervisor and `cap-std` was evaluated and not adopted, for the reasons `ops/AGENTS.md` records. Task 7 was blocked on five suite cases above the `1.25x` ceiling until the November engine work closed them; the first pair was measured on `2c229ccd`, its baselines are seeded, and the release check passed on `7b0aa4f3`. Task 6 and the documentation cleanup then changed non-evidence paths, so a second pair was measured on `1f42845e`; it passes Stim parity, memory, and seeded self-regression against the unchanged baselines, and it is the pair that `benchmarks/current-aarch64-evidence.toml` names. The release check passes on its evidence descendant `94714e23`, and GitHub CI run `43576457529` passed on that exact revision, which completes task 7 and the milestone.

### Done Criteria

- Every defect above has a falsifiable regression test, and every new divergence has a verified ledger family.
- The fixture manifest contains only rows that execute pinned Stim or a stored pinned-Stim artifact, and CI runs all of them.
- No gate, threshold, or semantic workload is relaxed to obtain a pass.
- The release-evidence procedure in [GOAL.md](GOAL.md) passes for the new measured revision, including exact-revision CI.

## Test Suite Contract

The final suite has five layers:

1. **Owner tests:** deterministic semantic tests in the crate that owns the behavior.
2. **Pinned differential tests:** compact generated and curated comparisons against Stim v1.16.0 for compatibility-sensitive behavior.
3. **Property and metamorphic tests:** fixed-seed invariants such as round trips, folded/unrolled equivalence, algebraic identities, and chunking invariance.
4. **Real CLI tests:** process-level workflows covering arguments, files, streams, outputs, errors, and exit status.
5. **Focused safety tests:** hostile input, path identity, process supervision, allocation bounds, cancellation, and writer failure.

Pull-request and push CI run deterministic owner tests, the complete canonical parity-owner set, architecture checks, CLI smoke, generated-document checks, the live pinned-Stim fixture manifest and probes on an AArch64 job, and untimed benchmark preflight. Ignored fuzz helpers remain explicit local checks; no separate scheduled differential or statistical tier is advertised. Shared-host timing is diagnostic only. Formal performance evidence runs only on the controlled host.

Tests that merely assert derives, re-exports, constants, type names, private pointer identity, static labels, or bookkeeping shape are removed unless that representation is itself a documented public contract.

## Operational Commands

The human-facing command surface is:

```text
just oracle::parity-check
just oracle::parity-run
just oracle::parity-render --check
just oracle::run --all-rows
just bench::e2e-check
just bench::e2e-run --tier smoke|full|soak
just bench::e2e-replay --input <bundle>
just bench::e2e-release-check
```

Complex logic remains in Rust operations binaries.

## Documentation Contract

- `GOAL.md` stays below 80 lines and contains only current state, current milestone, blockers, next commands, non-negotiable gates, and links to active sources.
- This plan owns the active milestone and its rationale.
- `oracle/stim-v1.16-parity.toml` owns feature status.
- Generated `docs/stim-parity.md` presents that status without independent prose counts.
- `benchmarks/suite.toml` owns active workloads and thresholds.
- Architecture records own durable design decisions.
- Completed plans, completed milestone narratives, and progress reports are not active specifications; they are deleted once their durable rationale is retained in architecture, migration, or benchmark records, and Git history preserves them.

## Final Acceptance

The program is complete when every nondeprecated, nondeferred core Rust and CLI behavior in the frozen Stim v1.16.0 ledger is `done` or an approved bug/resource divergence; every surviving test protects meaningful behavior; the architecture has one owner and one public route per capability; the 25-case E2E suite is the only active performance system; controlled AArch64 evidence passes correctness, `1.25x` Stim parity, memory, and seeded `1.15x` self-regression where a prior baseline exists; the full-and-soak evidence pair replays offline; and superseded compatibility and qualification machinery has been deleted.

## Assumptions

- Stim v1.16.0 remains frozen for this program.
- Breaking obsolete Stab APIs before 1.0 is acceptable and receives migration notes rather than shims.
- Safe resource-limit divergences are acceptable when typed, documented, and tested.
- Exact Stim random streams are not required.
- The release claim is controlled AArch64 first; controlled x86-64 evidence follows later.
- Development remains directly on `main`; no branch or linked worktree is created unless the user changes that policy.
