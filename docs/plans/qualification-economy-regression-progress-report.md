# Qualification Economy And Regression Progress Report

## Status

Q0 through Q8 completed on 2026-07-23. The replacement qualification contracts are implemented, the repaired result-format surface has fresh clean-revision correctness evidence, the representative DEM matrix passed the unchanged Stim parity gate on controlled Linux AArch64, and 18 architecture-specific Stab self-regression identities are seeded from the accepted full and soak rollups.

The source evidence revision is `68d107a42f655254f31628f0cbedc55479f6c0f3`. Its correctness inventory is `7a0f0fd50bc46221d4c1b489f9bb3d52f0a2e8ced996087f5714c72699645c7b`, and its performance inventory is `2ff0f012fa9d17b272e2afeecbe69fded8f38b099647a6e9191dbfe21e1d6776`.

The original completion manifest correctly records Stab self-regression as `unseeded`. The reviewed baselines were created and committed afterward, so the first run seeds future comparisons and cannot retroactively become a self-regression pass.

The post-evidence audit and full code review completed against clean revision `f465b6f6c4b2ef9ab543bca76e6d9b399f600032`. Both seeded self-regression checks consumed the sealed full and soak rollups successfully: DEM parse passed 9 of 9 identities and DEM print passed 9 of 9 identities.

## Source Changes

The qualification simplification landed in focused commits covering the documentation freeze, shared compatibility corpus, curated performance matrix, separate parity and regression policies, reduced worker preflight, representative DEM families, simplified completion manifest, contract CI, generated status, and review fixes.

Two publication defects were found only after the formal evidence existed:

- `f9633840c257af4bab478d5c19451433e94a29a3` allows the descriptor-safe artifact publisher to create the documented `candidate.json` baseline artifact.
- `d9c9368384c80dfb738eed7dc7f006edc40970fe` lets baseline generation and self-regression inspect sealed historical rollups read-only from a newer clean review commit while keeping explicit rollup replay revision-bound.
- `c0019590eed2c0f7f4293a5bd4cc9494dd024528` fixes the publication cycle where `Reopened -> Done` checklist presentation was treated as workload drift. The checker now preserves frozen presentation fields while validating live structural scope; `Partial`, `Deferred`, and whole selected ownership changes remain semantic.
- `f465b6f6c4b2ef9ab543bca76e6d9b399f600032` makes runtime-contract tests consume the checked frozen inventory and makes generated-status tests validate the published checkpoint instead of the obsolete pre-evidence state.

The formal parser optimization is `68d107a42f655254f31628f0cbedc55479f6c0f3`. It keeps common DEM payloads inline, preserves generic fallbacks, adds fixed-allocation regression coverage, and leaves the workload and `1.25x` threshold unchanged.

## Correctness Evidence

All correctness runs used clean revision `68d107a42f655254f31628f0cbedc55479f6c0f3` with `local_modifications=false`.

| Tier | Result | Request SHA-256 | Report SHA-256 | Completion SHA-256 |
| --- | --- | --- | --- | --- |
| PR | 406 of 406 passed | `edfaaeeb1275b761b6c8c0d9b84a6ddd91177a5179a5d923d9114490c9a125f7` | `19c0163f992f9228ff7dd4182e096c4687f5fe9d7ec73071da7cac5f1e0f627a` | `bdc3625e93cc2de71d95bc1a67184e48d4805b79c7e78a38d532b5bb1ef203da` |
| Full | 612 of 612 passed | `f9632712aa928b721a391cb48d4d76b65100b4b5e29ef5d4fec1fb8758a4468d` | `ecfdda8b86f236bf62d2e96b5ba726e107725022ac1c92a1384e12ce026a869e` | `223eb85a7d39c36f451cf6e7b1df12b69cb555be7bb32f4b2048832d9f7e1ac5` |
| Soak | 612 of 612 passed | `bf76b9d2089cf4cc161e7a0733c65e3131eee6a1058a239b258a3ee781848d7f` | `019b73d88093a0b1d44fbb13002a82c341e6c188c2bc2ee580f36716ea6a4029` | `9364f4655c44871079b2c100f5a720961c9075e513392cae5019e802809f17c2` |
| Exact DEM prerequisite | 4 of 4 passed | `1134c375d303e14ec2e174e17c791696ea56a49775199321b8df175d506e1c7e` | `492ba3a81452b4e5a41e8a5285eeb9fe90110cd683974691a219021896fd90ca` | `65b5a1edb5972cafde3155052a7f84a4d55ce7f4426f3f88bcd8548b4817d207` |

The live pinned-Stim result-format oracle passed all 62 checked cases: 20 accepted cases matched canonical output and 42 malformed cases were rejected by both implementations. This closes the repaired `01`, HITS, DETS, convert, replay, and `m2d` qualification reopening.

## Worker And Adapter Evidence

Private-worker reproducibility passed with pinned-Stim worker digest `3a75e21779f82cd625ac8de21c7bb05be2e148817d6f9a739170fbe160d6ca99` and Stab worker digest `b9cb5e0bd0632193c80140471818864da4237aebd10e5a4318a680662593e9e1`.

The exact-output adapter probes passed before formal timing. The parse probe reported diagnostic ratio `1.291274x`, and the print probe reported `0.770883x`. Probe ratios are diagnostic only and are not substituted for paired formal evidence.

## Controlled AArch64 Evidence

The controlled host used CPU identity `CPU implementer=0x41, CPU architecture=8, CPU variant=0x0, CPU part=0xd87, CPU revision=1`. Swap `/swap.img` was disabled immediately before formal timing and restored afterward with its original file identity, 17,179,865,088-byte size, and priority `-2`. No qualification process remained after the run.

All 36 first-publication reports passed without reruns:

| Operation | Reports | Median range | Worst 95% upper bound | Faster than Stim | Slower than Stim |
| --- | ---: | ---: | ---: | ---: | ---: |
| DEM parse | 18 | `0.464633x` to `1.201529x` | `1.215806x` | 6 | 12 |
| DEM print | 18 | `0.429038x` to `0.740240x` | `0.742927x` | 18 | 0 |
| Combined | 36 | `0.429038x` to `1.201529x` | `1.215806x` | 24 | 12 |

Every paired median and bootstrap upper confidence bound remained at or below the unchanged `1.25x` Stim parity gate. The tightest result was soak `coordinate-sparse-medium` DEM parse at median `1.201529x` and upper bound `1.215806x`.

Both accepted-maximum memory probes passed and were published at `target/benchmarks/qualification/q8-68d107a-dem-parse-max-memory` and `target/benchmarks/qualification/q8-68d107a-dem-print-max-memory`.

Four rollups passed all nine measurements each:

- `target/benchmarks/qualification/q8-68d107a-dem-parse-full-rollup`
- `target/benchmarks/qualification/q8-68d107a-dem-parse-soak-rollup`
- `target/benchmarks/qualification/q8-68d107a-dem-print-full-rollup`
- `target/benchmarks/qualification/q8-68d107a-dem-print-soak-rollup`

The `dem-r6` completion manifest and offline replay passed at `target/benchmarks/qualification/q8-68d107a-dem-r6-completion`. Its report SHA-256 is `48b858ddced6f6f77f4d57c5f985ce11fd1dcd88133b0c5ab70f52832472c967`.

## Regression Baseline

The reviewed parse and print candidates each contributed nine identities. The checked baseline now contains 18 Linux AArch64 entries and has SHA-256 `b39962aa7adae87eeb179c327a703b2609c51bf7e81aa635afc1662061b4bc6c`.

Each accepted baseline takes the worse full-or-soak median and the worse full-or-soak upper bound independently. Future runs on the exact matching architecture, CPU, host profile, target, toolchain, Stim build, timing boundary, and workload contract must remain within the source-owned self-regression tolerance. x86-64 remains unseeded.

## Legacy Diagnostics

The legacy primary suite remains diagnostic compatibility coverage rather than the formal DEM release evidence. A fresh run from clean revision `f465b6f6c4b2ef9ab543bca76e6d9b399f600032` recorded:

- Timing thresholds: 68 configured rows passed, 15 rows were not configured, and 3 rows retained source-owned no-ratio waivers.
- Beta summary: 77 rows passed, 3 non-comparable rows were waived, and 6 report-only rows could not prove a Stim ratio.
- Memory summary: 78 rows passed, 4 allocation rows failed their legacy baselines, and 4 rows had no baseline.

The pre-existing memory failures are the DETS reader and two high-repeat analyzer rows. The newly visible graphlike-search failure records the parser layout tradeoff made by the qualified DEM optimization: peak allocation count fell from 814 to 557 while peak allocated bytes rose from 178,456 to 216,280 because five common targets now stay inline. The formal accepted-maximum DEM memory probes passed, the legacy memory baseline was not moved, and no waiver was added.

The final diagnostic artifacts use unique paths under `target/benchmarks/q8-final-f465b6f-primary-{baseline,beta,timing,memory}`. The beta and memory commands intentionally returned nonzero because their report-only and failed or missing-baseline rows prevent a false all-pass claim.

## Post-Evidence Audit

The milestone audit and full code review found no unresolved compatibility, filesystem-safety, process-supervision, timing-boundary, parity-policy, self-regression, evidence-publication, or CI-contract defect.

The audit did find that several runtime-contract tests regenerated live checklist presentation and therefore no longer matched the frozen evidence inventory after the checklist was closed. It also found that the generated-status integration test still expected the pre-evidence `not started` message. Commit `f465b6f6c4b2ef9ab543bca76e6d9b399f600032` fixed both without weakening semantic drift detection.

Explicit completion replay remains revision-bound by design. Replaying the `68d107a` completion requires its recorded source revision, which restores the exact unseeded regression policy identity; newer clean revisions may inspect sealed rollups for self-regression and baseline generation but cannot relabel the historical completion.

No new milestone under-specification was found. Qualification Rust sources remain below the 1,200-line repository limit, no tracked operational shell script was introduced, swap is restored, and no qualification process remains.

## Historical Evidence

Failed and review-rejected chains remain preserved under their exact revisions and schemas. In particular, the complete `61bf222` three-family run records the pre-optimization parse failures, and the older `9497df0` and `80fb540` chains remain historical `raw-work-v1` evidence. None is relabeled as current.

## Verification

The complete verification set passed against the closure source state:

```text
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
just oracle::result-formats --check
just qualification::correctness-check
just qualification::correctness-regenerate --check
just bench::qualification-check
just bench::qualification-regenerate --check
just qualification::status --check
just bench::smoke
just maintenance::pre-commit
```
