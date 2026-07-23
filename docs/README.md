# Stab Documentation

Index and operating rules for the `docs/` tree.
This file is the effective `AGENTS.md` source for this directory: `AGENTS.md` and `CLAUDE.md` are symlinks to it.

## Contents

- [plans/](plans/): milestone plans and progress reports. `plans/rust-stim-drop-in-rewrite.md` is the implementation roadmap, and `plans/GOAL.md` is the active execution contract for `plans/post-review-compatibility-evidence-repair.md`.
- [stab-feature-checklist.md](stab-feature-checklist.md): Stab feature availability against Stim v1.16.0.
- [stim-feature-list.md](stim-feature-list.md): the upstream Stim v1.16.0 feature inventory that the checklist maps onto.

## Documentation Policy

- CQ1 and PQ1 harness acceptance is recorded in `plans/cq1-correctness-harness-progress-report.md` and `plans/pq1-performance-harness-progress-report.md`; CQ2 is active at `plans/cq2-deterministic-qualification-progress-report.md`, and any later correctness or performance inventory digest change makes its clean refresh historical until the affected tiers are rerun from a clean committed revision.
- When changing planned scope, milestone order, compatibility targets, public CLI behavior, or benchmark acceptance gates, update the matching plan document in the same change set.
- Use `.agents/skills/milestone-audit` when auditing whether a milestone implementation satisfies its objective, tasks, linked tests, benchmarks, and done criteria, or when implementation reveals milestone loopholes or under-specified scope.

## Correctness Qualification Contracts

- Use `just qualification::correctness-list`, `just qualification::correctness-check`, and `just qualification::correctness-regenerate --check` for the CQ0 case and public-API inventory; update the frozen digest and checked manifest together when reviewed source ownership changes. `oracle/qualification-cases.json` is the source-owned exact-parent ledger for collapsing reviewed upstream, public-API, and oracle owners onto independently selectable qualification cases; stale, duplicate, cross-feature, comparator-mismatched, or shared-primary mappings must fail closed.
- Use `just qualification::correctness-provenance-probe` to rebuild private Stab and Stim binaries, execute one real source-owned case through the normal qualification runner, and validate the published request, execution, report, completion, and preflight bindings.
- Use `just qualification::correctness-run --tier pr`, `--tier full`, or `--tier soak` to execute source-owned CQ1 evidence; qualification outputs must stay below `target/qualification/`, and dirty reports are diagnostic rather than promotable evidence.
- CQ1 runs must retain fresh private Stab and Stim builds, immutable sealed copies of the canonical direct-executable identity ledger, Cargo invocation from `/` with absolute manifests and private config-free homes, a private Git index reconstructed from `HEAD`, descriptor-owned fixture side outputs and support cleanup, the hashed explicit child environment, exact per-comparison statistical completion accounting, sticky process-group cancellation, and repository-anchored descriptor-owned publication; do not replace these contracts with shared mutable binaries, inherited configuration, path-reopened artifacts, or exit-status-based shot credit.
- CQ1 qualification execution is Linux-only and must fail closed elsewhere because its timeout and publication contracts require process-group termination and atomic directory exchange.
- Use `just qualification::correctness-report --out <report-directory>` to validate `request.json`, `report.json`, `completion.json`, every case execution receipt, and the derived Markdown and preflight artifacts, then use `just qualification::correctness-preflight --out <report-directory> --case <qualification-case-id> --request-sha256 <run-request-sha256> --completion-sha256 <run-completion-sha256>` to verify the controller-approved selection and outcomes before dependent performance work.
- Use `--allow-deferred` only with explicit correctness `--case` filters for diagnostic visibility; a report containing deferred cases is never valid preflight evidence.
- Existing Cargo primary selectors in the correctness manifest must select one concrete libtest case with `--exact`; broad filters are supporting evidence only and cannot close a planned atomic owner.

## Oracle Corpus Workflows

- Use `just oracle::version` to validate that `vendor/stim` is pinned to Stim v1.16.0, and use `just oracle::run --case smoke/help` plus `just oracle::run --case smoke/tiny-circuit` for M0 oracle smoke checks.
- Use `just oracle::list` to inspect and validate the M2 fixture corpus, including coverage of planned M4 through M11 P0/P1 C++ compatibility-matrix rows by upstream source, milestone, and parity mode; use `just oracle::list --milestone Mx` and `just oracle::run --milestone Mx` for milestone-scoped fixture work, `just oracle::record --check-clean` to verify committed runnable exact-output fixtures against pinned Stim, `just oracle::run --implemented-only` for implemented fixture parity, and `just oracle::run --all` to report red or manifest-only future fixtures.
- Use `just oracle::matrix --check` to validate the M1 compatibility matrix, and use `just oracle::matrix --milestone Mx` to inspect acceptance rows for implementation milestones.
- Use `just oracle::blockers` to validate and summarize the source-owned non-deferred blocker closure ledger, use `just oracle::blockers --list` to inspect every owned PFM-B subcase and its planned, implemented, or evidence-close state, and use `just oracle::blockers --check-selectors` to prove every claimed existing Cargo test selector resolves to at least one test.
- Use `just rust::parser-fuzz` as the local long-running M4 `.stim` parser fuzz-smoke target.
- Treat the M0 `stab-cli sample` path as a hidden oracle smoke shim only; it is not real `stim sample` compatibility, which belongs to M8.

## Performance Work

Performance qualification contracts and benchmark workflows live in [../benchmarks/AGENTS.md](../benchmarks/AGENTS.md).
