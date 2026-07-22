# PQ2 DEM Parse And Print Qualification Progress Report

## Status

In progress as of 2026-07-23.

The source-current implementation inventory is performance digest `8094995692c48c98723467cf6e90c3c685797fb4a02bf0efe15c273d844fbfef` and correctness digest `648e7ea5a66997a810498dc871257bd654c7f9af9304651d43a88103eded0289`.
It contains independent `PERFQ-M10-DEM-PARSE-CONTRACT` and `PERFQ-M10-DEM-PRINT-CONTRACT` groups, private Stab build-receipt schema version 6, adapter receipt schema version 12, contract-preflight schema version 13 with 228 ordered receipts, and qualification report schema version 32.
Neither group has promotable timing evidence or legacy-migration authorization yet.
Both groups now bind the source-owned diagnosis at `benchmarks/profiler-notes/qualification/perfq-m10-dem-model.md`, whose SHA-256 is `453e3068b89458a3da4bc9f83ecd881cdc0fa6aa1bb0656a72be0d9e8ef3afcc`, so future failed or noisy formal reports can be retained and audited.

## Retained Attempts

### Parse Probe 1

- Source revision: `6c1e1d161b1869646ba69249c19e45ed4cd963a1`.
- Command: `just bench::qualification-probe --group pq2-dem-parse-adapter-smoke`.
- Result: failed before publishing a probe receipt.
- Exact failure: the probe expected pinned Stim's zero-width rejection to say `adapter requires --iterations and --work-items`, but the adapter's numeric parser correctly rejected the request earlier with `work-items must be positive`.
- Classification: faithful contract-test failure in the new expectation, not a product or pinned-Stim defect.
- Action: bind the actual earlier rejection text, rerun targeted checks, commit the fix, and rerun both source-owned probes from the new clean revision.

This failed attempt must remain visible and cannot be promoted or replaced by a later passing run.

### Clean Checkpoint At `b7c6c34f`

- Parse adapter probe: passed with `work=16384`, `stim_seconds=0.003630547`, `stab_seconds=0.007296854`, diagnostic ratio `2.009850x`, Stim parent peak RSS `5152768`, and Stab parent peak RSS `7802880`.
- Print adapter probe: passed with `work=16384`, `stim_seconds=0.010781847`, `stab_seconds=0.006240932`, diagnostic ratio `0.578837x`, Stim parent peak RSS `4947968`, and Stab parent peak RSS `6848512`.
- Worker reproducibility: passed with pinned-Stim digest `cb484542faaeba73156a1ba5d7a1f35104b697320847ad33994b9bb1f33b67d4` and Stab digest `c310b9e3a10d0f9c1d1bb5bb5e4acd5b9661956594fbfc0a5e8b525b823208ba`.
- Exact focused correctness prerequisite: passed one selected case with zero failures under `target/qualification/pq2-dem-cq-full-b7c6c34`.
- Correctness request SHA-256: `2fb1cc36cfff48446c4d9c33fbd1ffc27e15f08144829f206c0a54eb6c67037a`.
- Correctness preflight SHA-256: `67e6507e39d01e5ff01b94d05c16723a923c823195dcac133bb076b3a3343241`.
- Correctness JSON report SHA-256: `d9139fde4ccb90c717d934a453786ef43225e4d44509c3c94e151889c2570add`.
- Correctness Markdown report SHA-256: `a32b117350fee034f56ec3fc8eea2a73ad7221be8893c7ad5d71037b4d133095`.
- Correctness completion SHA-256: `62796700c06e8519152b343c33dc9cbcba5c687c1c0a5bb514c92cb35dc1e9a1`.

These are checkpoint diagnostics and prerequisites, not promotable timing evidence. The runtime-group profiler-note binding changes the source contract, so the probes, reproducibility check, and exact correctness prerequisite must be regenerated from the clean note-binding revision before formal evidence is accepted.

### Correctness Replay Invocation Error

- The first replay attempt used an unsupported `--input` argument instead of the documented `--out` artifact root.
- The CLI rejected the invocation before reading or changing the correctness artifact.
- Replaying with `--out target/qualification/pq2-dem-cq-full-b7c6c34` passed and reproduced the report.
- Classification: operator invocation error with no artifact mutation and no effect on the passed correctness result.

### Parse Small Full Attempt 1

- Source revision: `b7c6c34f156d5f785dc46e1f6e79c3f4bf1e6914`.
- Command shape: `just bench::qualification-run --group PERFQ-M10-DEM-PARSE-CONTRACT --scale small --tier full` with the exact correctness request and completion hashes above.
- Host preparation: swap was disabled before timing and restored immediately after the command; `/swap.img` is enabled again with zero bytes in use.
- Result: the producer reached a failed or noisy product result, then correctly refused publication with `failed or noisy product evidence lacks source-owned failure ownership`.
- Artifact disposition: no report directory or partial promotable artifact exists at `target/benchmarks/qualification/pq2-dem-b7c6c34-parse-small-full-1`.
- Classification: harness-contract omission because the new DEM runtime groups had no bound profiler note. The timing gate, fixture, semantic work, comparator, and no-waiver policy remain unchanged.
- Action: add and bind the source-owned DEM profiler note before the next clean-revision evidence run. Do not treat this no-artifact attempt as a timing result or rerun it in search of a favorable sample.

### Clean Checkpoint At `f23386bd`

- Parse adapter probe: passed with `work=16384`, `stim_seconds=0.002048688`, `stab_seconds=0.006883295`, diagnostic ratio `3.359855x`, Stim parent peak RSS `253952`, and Stab parent peak RSS `7786496`.
- Print adapter probe: passed with `work=16384`, `stim_seconds=0.010549630`, `stab_seconds=0.003281743`, diagnostic ratio `0.311077x`, Stim parent peak RSS `4964352`, and Stab parent peak RSS `6905856`.
- The probes ran concurrently and their short diagnostic timings are not comparable to formal isolated sampling. They prove fixture, output, and hostile-request parity only.
- Worker reproducibility: passed with pinned-Stim digest `cb484542faaeba73156a1ba5d7a1f35104b697320847ad33994b9bb1f33b67d4` and Stab digest `5ba0095ef882ca010e2bbf48395bbd4b7666e352860f331c2fff0a80e15cd606`.
- Exact focused correctness prerequisite: passed one selected case with zero failures, replayed successfully, and passed exact preflight under `target/qualification/pq2-dem-cq-full-f23386b`.
- Correctness request SHA-256: `453240ebb324aa9333be8f323e1df5aa8cd8a7a42069c088e063c350e26b2879`.
- Correctness preflight SHA-256: `3d51e5ae97c3b19336cdadcb3167838b335db0f813f92ca57a735631f5ef987f`.
- Correctness JSON report SHA-256: `cf3ec21e8bbb72aed6d94493a99de76866033f5b7bddde6edc955399b3d5b88f`.
- Correctness Markdown report SHA-256: `4e26c1569f2bf679710a54780102e4f3976568fc227406be4282df9c930608da`.
- Correctness completion SHA-256: `5b1ff5f387bc11670d2db34e40b85ef88db355cf7687679b9501491edfcd43e6`.

### Parse Small Full Attempt 2

- Source revision: `f23386bdc12258eab97b9997b3f478841caa050c`.
- Artifact: `target/benchmarks/qualification/pq2-dem-f23386b-parse-small-full-1`.
- Host preparation: swap was disabled only during timing through an exit restoration trap and `/swap.img` was restored with zero bytes in use immediately after publication.
- Result: stable failed evidence with 29,547 common iterations per sample, nine pairs, median ratio `1.7734502252203892x`, bootstrap 95 percent interval `[1.7676881539960179x, 1.7932386972899312x]`, and paired relative MAD `0.0032490741168984733`.
- Report SHA-256: `2076cedfa263d43644750131afbddb934b3eafbc8e2f053442c08a77af89ca51`.
- Preflight SHA-256: `4c587b1492b38401bee894edbffab7fb412ef971565f1c8c413986c9e78d62b0`.
- Markdown SHA-256: `f13d675ac44168b4cdcf497ecc3fbf3c3d5bb489aa47c9e16ac64c4cd3e99a5e`.
- Replay: passed and reproduced the artifact.
- Regression: failed closed because measurement `parse` has non-passing outcome `Failed`, as required.
- Disposition: retain permanently as the first faithful source-owned failure. It is not noisy and was not rerun from the same revision.

## Parser Optimization

Clean implementation commit `fb089098406892756572ea14439452a1001df57a` removes the confirmed common-path allocation and string-processing costs without changing public syntax, the frozen fixture, output identity, resource limits, semantic work, or gate. Common DEM arguments and one-target payloads now stay inline; spill targets and top-level storage are pre-sized; instruction-name parsing no longer allocates lowercase strings; unsigned targets use checked decimal accumulation; and comment-free lines bypass character scanning. Speculative top-level capacity is capped before hostile line-count rejection.

The new regression suite proves mixed-case names, Unicode whitespace, comments, hashes and escapes inside tags, payloads larger than inline storage, canonical reparsing, and the exact 4,096-item qualification family. The allocation test failed before the implementation at 14,859 calls and now admits at most 4,100. All `stab-core` tests, workspace tests, formatting, workspace Clippy, and staged pre-commit checks passed for the implementation commit.

A dirty-tree parse adapter probe after the optimization passed exact parity and reported diagnostic ratio `1.317631x` with `stim_seconds=0.001916643` and `stab_seconds=0.002525429`. A separate dirty-tree Stab worker completed 10,000 owned parses of the 64-item fixture in `0.096216406` seconds. These numbers are optimization guidance only and are not clean paired evidence. The updated profiler-note binding changes the runtime source contract, so the entire prerequisite and timing chain must be regenerated from the next clean revision.
