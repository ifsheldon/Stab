# Stab Documentation

Index and operating rules for the `docs/` tree.
This file is the effective `AGENTS.md` source for this directory: `AGENTS.md` and `CLAUDE.md` are symlinks to it.

## Contents

- [plans/](plans/): milestone plans and progress reports. `plans/GOAL.md` is the short active execution contract and `plans/stim-core-parity-and-lean-evidence-plan.md` is the active implementation plan. Earlier rewrite, entropy, and qualification plans are historical or transitional inputs, not parallel roadmaps.
- [architecture/](architecture/): product dependency rules, compilation phases, extension seams, and architecture decision records.
- [MIGRATING-0.2.md](MIGRATING-0.2.md): coordinated Rust package, facade-tier, and public-path migration guide for Stab 0.2.
- [RELEASING.md](RELEASING.md): coordinated crates.io and GitHub release preflight, publication order, recovery, and verification procedure.
- [stim-parity.md](stim-parity.md): generated current Stim v1.16.0 core parity view, rendered from the atomic source ledger in `oracle/stim-v1.16-parity.toml`.
- [stab-feature-checklist.md](stab-feature-checklist.md): historical Stab feature assessment retained as a P0 input, not a current status source.
- [stim-feature-list.md](stim-feature-list.md): historical Stim v1.16.0 inventory retained as a P0 input, not a current scope or status source.
- [qualification-status.md](qualification-status.md): generated status for the transitional qualification system until the lean parity and E2E evidence sources replace it.

## Documentation Policy

- Historical implementation, CQ, and PQ plans preserve prior decisions and evidence but do not define current feature scope or the destination evidence architecture. Any embedded `active` or `current` wording in those preserved records applies only to their recorded revision. The active plan replaces their surviving tooling only after its lean correctness and E2E systems pass, and P9 then deletes superseded prose after retaining durable decisions.
- When changing planned scope, milestone order, compatibility targets, public CLI behavior, or benchmark acceptance gates, update the matching plan document in the same change set.
- Use `.agents/skills/milestone-audit` when auditing whether a milestone implementation satisfies its objective, tasks, linked tests, benchmarks, and done criteria, or when implementation reveals milestone loopholes or under-specified scope.

## Correctness Qualification Contracts

The commands below remain the transitional verification surface during P0 and P1. Do not extend their inventories with new per-export ownership or treat them as the active parity roadmap; the active plan replaces them with the parity ledger and behavior-oriented suite before deleting them.

- Use `just qualification::correctness-list` and `just qualification::correctness-check` for the CQ0 case and public-API inventory. Use `just qualification::correctness-regenerate` only to replace the checked manifest after reviewed source ownership changes, then update the frozen digest and run the canonical check. `oracle/qualification-cases.json` is the source-owned exact-parent ledger for collapsing reviewed upstream, public-API, and oracle owners onto independently selectable qualification cases; stale, duplicate, cross-feature, comparator-mismatched, or shared-primary mappings must fail closed.
- Use `just qualification::correctness-provenance-probe` to rebuild private Stab and Stim binaries, execute one real source-owned case through the normal qualification runner, and validate the published request, execution, report, completion, and preflight bindings.
- Use `just qualification::correctness-run --tier pr`, `--tier full`, or `--tier soak` to execute source-owned CQ1 evidence; qualification outputs must stay below `target/qualification/`, and dirty reports are diagnostic rather than promotable evidence.
- CQ1 runs must retain fresh private Stab and Stim builds, immutable sealed copies of the canonical direct-executable identity ledger, Cargo invocation from `/` with absolute manifests and private config-free homes, a private Git index reconstructed from `HEAD`, descriptor-owned fixture side outputs and support cleanup, the hashed explicit child environment, exact per-comparison statistical completion accounting, sticky process-group cancellation, and repository-anchored descriptor-owned publication; do not replace these contracts with shared mutable binaries, inherited configuration, path-reopened artifacts, or exit-status-based shot credit.
- CQ1 qualification execution is Linux-only and must fail closed elsewhere because its timeout and publication contracts require process-group termination and atomic directory exchange.
- Use `just qualification::correctness-report --out <report-directory>` to validate `request.json`, `report.json`, `completion.json`, every case execution receipt, and the derived Markdown and preflight artifacts, then use `just qualification::correctness-preflight --out <report-directory> --case <qualification-case-id> --request-sha256 <run-request-sha256> --completion-sha256 <run-completion-sha256>` to verify the controller-approved selection and outcomes before dependent performance work.
- Use `--allow-deferred` only with explicit correctness `--case` filters for diagnostic visibility; a report containing deferred cases is never valid preflight evidence.
- Existing Cargo primary selectors in the correctness manifest must select one concrete libtest case with `--exact`; broad filters are supporting evidence only and cannot close a planned atomic owner.
- Every selected public item still needs inventory ownership, but ordinary derived traits, trivial accessors, marker declarations, and Rust `Debug` formatting do not need standalone runtime assertions unless their behavior or representation is part of the compatibility contract. Test resource promises through bounded allocation, capacity, and cancellation behavior instead of pointer identity unless the public API explicitly promises storage identity.

## Oracle Corpus Workflows

The strict result-format corpus and pinned Stim executable remain durable compatibility evidence. The broader compatibility matrix and blocker ledgers are transitional inputs to P0 and must not acquire new duplicate status rows.

- Use `just oracle::parity-check` to validate the atomic parity ledger, pinned Stim identity and references, complete gate, alias, command, dialect, format, and format-route partition, and exact canonical owner selectors. Use `just oracle::parity-run --tier pr|full|soak` to execute each completed owner independently, and use `just oracle::parity-render --check` to reject drift in [stim-parity.md](stim-parity.md).
- Use `just oracle::gates` to build a small helper against pinned `libstim` and compare all canonical names, aliases, inverses, categories, argument and target rules, and classification flags against `stab-model`'s canonical gate metadata.
- Use `just oracle::version` to validate that `vendor/stim` is pinned to Stim v1.16.0, and use `just oracle::run --case smoke/help` plus `just oracle::run --case smoke/tiny-circuit` for M0 oracle smoke checks.
- Use `just oracle::list` to inspect and validate the M2 fixture corpus, including coverage of planned M4 through M11 P0/P1 C++ compatibility-matrix rows by upstream source, milestone, and parity mode; use `just oracle::list --milestone Mx` and `just oracle::run --milestone Mx` for milestone-scoped fixture work, `just oracle::record --check-clean` to verify committed runnable exact-output fixtures against pinned Stim, `just oracle::run --implemented-only` for implemented fixture parity, and `just oracle::run --all` to report red or manifest-only future fixtures.
- Use `just oracle::matrix --check` to validate the M1 compatibility matrix, and use `just oracle::matrix --milestone Mx` to inspect acceptance rows for implementation milestones.
- Use `just oracle::blockers` to validate and summarize the source-owned non-deferred blocker closure ledger, use `just oracle::blockers --list` to inspect every owned PFM-B subcase and its planned, implemented, or evidence-close state, and use `just oracle::blockers --check-selectors` to prove every claimed existing Cargo test selector resolves to at least one test.
- Use `just rust::parser-fuzz` as the local long-running M4 `.stim` parser fuzz-smoke target.
- Treat the M0 `stab-cli sample` path as a hidden oracle smoke shim only; it is not real `stim sample` compatibility, which belongs to M8.

## Performance Work

Performance qualification contracts and benchmark workflows live in [../benchmarks/AGENTS.md](../benchmarks/AGENTS.md).
