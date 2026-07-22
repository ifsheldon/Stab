# PQ2 DEM Parse And Print Qualification Progress Report

## Status

In progress as of 2026-07-23.

The source-current implementation inventory is performance digest `a98f57cf194f3a021d321266656cf688c9f7780fb39fa337475e8132411eb88a` and correctness digest `fbaa2bdf8bc0eea01b2aca385a4e537de11c3b35099671cca0e2775950c2fbb0`.
It contains independent `PERFQ-M10-DEM-PARSE-CONTRACT` and `PERFQ-M10-DEM-PRINT-CONTRACT` groups, private Stab build-receipt schema version 6, adapter receipt schema version 12, contract-preflight schema version 13 with 228 ordered receipts, and qualification report schema version 32.
Parse has passing full evidence at small and medium scale under the preceding inventories, but its retained large-scale report failed; print has no formal timing evidence, and neither group has legacy-migration authorization yet.
Both groups bind the source-owned diagnosis at `benchmarks/profiler-notes/qualification/perfq-m10-dem-model.md`, whose new SHA-256 is `0af72ac0d94b9fc0f9a4dcf6011a4c7061cd9b7c782aedcdcd4cb809fd6a6bc4`, so future failed or noisy formal reports can be retained and audited.

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

### Clean Post-Optimization Checkpoint At `3a78eb74`

- Parse adapter probe: passed exact fixture and output parity with `work=16384`, `stim_seconds=0.003617204`, `stab_seconds=0.002420963`, diagnostic ratio `0.669291x`, Stim parent peak RSS `5111808`, and Stab parent peak RSS `94208`.
- Print adapter probe: passed exact fixture and output parity with `work=16384`, `stim_seconds=0.005289254`, `stab_seconds=0.006054023`, diagnostic ratio `1.144589x`, Stim parent peak RSS `4923392`, and Stab parent peak RSS `6434816`.
- The probes ran sequentially. Their short timings remain diagnostic and cannot satisfy the formal paired-sample gate; the unusually small parse parent RSS is retained as an observation, not a memory claim.
- Worker reproducibility: passed with pinned-Stim digest `cb484542faaeba73156a1ba5d7a1f35104b697320847ad33994b9bb1f33b67d4` and Stab digest `9a2205c5522144b4a25facc69dd60b2a5cd7bf6f9ef6d574e8513b126d930382`.
- Focused correctness attempt: rejected before execution and publication because generated correctness digest `b9ed22cfcf637e740e68501b32b764015e937e6bc8d75ced3328f62ecc20cf40` did not match the frozen pre-optimization digest `648e7ea5a66997a810498dc871257bd654c7f9af9304651d43a88103eded0289`.
- Artifact disposition: no `target/qualification/pq2-dem-cq-full-3a78eb7` directory or partial correctness artifact exists.
- Inventory review: regeneration changed exactly 108 `source_line` fields in `crates/stab-core/src/dem.rs` plus the derived digest. It did not change a selected parent, owner, selector, disposition, case ID, count, or the exact DEM primary case `cq-evidence-qualification-0908c21b917526e3`.

These results prove that the optimized code and private workers were internally coherent before the source inventory was refreshed. They do not cross the inventory boundary. Both probes, worker reproducibility, and the exact focused correctness chain must run again from the clean inventory-refresh revision before any post-fix timing evidence is produced.

### Clean Inventory-Refresh Checkpoint At `d8de73d5`

- Parse adapter probe: passed exact fixture and output parity with `work=16384`, `stim_seconds=0.003631854`, `stab_seconds=0.002592191`, diagnostic ratio `0.713738x`, Stim parent peak RSS `5148672`, and Stab parent peak RSS `397312`.
- Print adapter probe: passed exact fixture and output parity with `work=16384`, `stim_seconds=0.005250589`, `stab_seconds=0.006093213`, diagnostic ratio `1.160482x`, Stim parent peak RSS `4935680`, and Stab parent peak RSS `6524928`.
- Worker reproducibility: passed with pinned-Stim digest `cb484542faaeba73156a1ba5d7a1f35104b697320847ad33994b9bb1f33b67d4` and Stab digest `d5be065a25c748b480165c72992a7046d4043a50c646457a66d6301021914384`.
- Exact focused correctness prerequisite: passed one selected case with zero failures, replayed successfully, and passed exact preflight under `target/qualification/pq2-dem-cq-full-d8de73d`.
- Correctness request SHA-256: `a7c1eace9e0abc36b4c11969385d5c54acd75c358671ca3000f6f6053fad6f97`.
- Correctness preflight SHA-256: `c78b62ab77722ab4576e43e573c6f3702f2dd30d1c230cc2d3fa3ea73e824af0`.
- Correctness JSON report SHA-256: `ea88180a08ca4cd2c492a9b7dcd508c1522206ff7c3dd3c355c7c13fb830b229`.
- Correctness Markdown report SHA-256: `46a815319072e274dddf0a65266b663aea45896f5b423a50317be910a3e6c11f`.
- Correctness completion SHA-256: `dfb828c7639c6f2a302e54c62e047658d30c49ecd74dc80ba85c19a31d0f075e`.

An operator invoked the correctness report command before the producer PTY had completed, so that command failed because the report did not exist yet. No artifact had been created or mutated by the premature invocation. After the producer completed, the normal replay and exact preflight both passed.

### Parse Small Full Attempt 3

- Source revision: `d8de73d50fbeb4e001ea38c784d1fdcfc76dad76`.
- Artifact: `target/benchmarks/qualification/pq2-dem-d8de73d-parse-small-full-1`.
- Host preparation: swap was disabled only during timing through an exit restoration trap and `/swap.img` was restored immediately after publication.
- Result: stable failed evidence with 29,527 common iterations per sample, 1,889,728 item operations per sample, nine pairs, median ratio `1.3729706998216054x`, bootstrap 95 percent interval `[1.366241707530459x, 1.3773752216236312x]`, and paired relative MAD `0.0032080231592692974`.
- Report SHA-256: `c32f59a9016a9c05cc6ede5f79b29e7d64ef329efabed3999f2ae37c460471c9`.
- Preflight SHA-256: `8faaac3ca19e007cbfb9d2cdcfc79e5f38e5e7bca7163b6e8bad9fa62aab1cdb`.
- Markdown SHA-256: `8f580cc26609a737f0ad42e34ba9c475f664270ed2c153785902322b279ab54c`.
- Replay: passed and reproduced the artifact.
- Regression: failed closed because measurement `parse` has non-passing outcome `Failed`, as required.
- Disposition: retain permanently as the first faithful result from the refreshed inventory. It is stable, was not rerun from the same revision, and reduced the preceding clean median failure from `1.773450x` to `1.372971x` without changing the contract.

## Second Parser Optimization

Clean implementation commit `e9b94d524167b2ca7bb953e525abc26e77641629` preserves the public DEM model and parser grammar while removing more common-path allocation and rescanning. Short UTF-8 tags now use a private 16-byte inline representation with a `String` fallback, unescaped tags take a direct closing-bracket path, target parsing is single-pass and reserves once when a second target proves spill storage is needed, and top-level capacity estimation samples at most 256 bytes instead of rescanning the entire input.

The qualification-family allocation guard now admits at most 2,050 calls, and the implementation records 2,049 calls and 963,816 allocated bytes for the exact 4,096-item input. Long ASCII tags, multi-byte UTF-8 tags, escaped tags, comments, and multi-target errors remain covered, and all `stab-core` tests, workspace tests, formatting, workspace Clippy, and staged pre-commit checks passed before the commit.

Dirty-tree direct-worker diagnostics for the final layout reported `0.416383174` seconds for the small diagnostic run and `0.377149828` seconds for the large diagnostic run, with observed peak RSS `34701312` bytes in the latter. Alternative two- and three-target inline layouts were slightly faster in isolated runs but were rejected because they materially enlarged every instruction model; the selected one-target inline layout plus one spill reservation retained the projected timing margin with a smaller persistent footprint. These figures are implementation guidance only, are not paired Stim evidence, and cannot satisfy any qualification gate.

Because the parser source, direct dependency graph, profiler note, and generated source inventories now change again, no evidence from `d8de73d5` can be relabeled as current. The next evidence chain must begin from a clean commit that binds this diagnosis and regenerated inventories, then repeat both probes, worker reproducibility, the exact CQ prerequisite, and every formal parse and print report.

### Clean Second-Optimization Checkpoint At `ca9fd68d`

- Parse adapter probe: passed exact fixture and output parity with `work=16384`, `stim_seconds=0.002103587`, `stab_seconds=0.002461219`, diagnostic ratio `1.170011x`, Stim parent peak RSS `184320`, and Stab parent peak RSS `352256`.
- Print adapter probe: passed exact fixture and output parity with `work=16384`, `stim_seconds=0.005327000`, `stab_seconds=0.002971140`, diagnostic ratio `0.557751x`, Stim parent peak RSS `4902912`, and Stab parent peak RSS `81920`.
- Worker reproducibility: passed with pinned-Stim digest `cb484542faaeba73156a1ba5d7a1f35104b697320847ad33994b9bb1f33b67d4` and Stab digest `34e4f33e92ac0377dd8372a83ae9214e4433f5711bc208ebf292e4464623c437`.
- Exact focused correctness prerequisite: passed one selected case with zero failures, replayed successfully, and passed exact preflight under `target/qualification/pq2-dem-cq-full-ca9fd68`.
- Correctness request SHA-256: `d906a38d6c1c63cc58083c50e81c5c9f03a43d582a5ec2a93b153723dbff0e06`.
- Correctness JSON report SHA-256: `43ed3b1fa4e4f3435b2694a5abad48e8d57449e323748e269a5298f6d8e4a9fb`.
- Correctness completion SHA-256: `4caaba719d15c8d02cb8488234b3669a5b498e604f640e4a570b9e6b06189aa1`.
- Correctness preflight SHA-256: `d05ed6126213a775b6849f91dad7251e5589b2d2623397947ebb765ce80fe54c`.
- Correctness Markdown report SHA-256: `ec4a79db8a6ef5179ad32490d5306f3d4a0e12bceb69b898fe8b8fea8110f7ca`.

### Parse Small Full Attempt 4

- Source revision: `ca9fd68d3856e9cac9cc6da16433947d056e8848`.
- Artifact: `target/benchmarks/qualification/pq2-dem-ca9fd68-parse-small-full-1`.
- Host preparation: swap was disabled only during timing through an exit restoration trap and `/swap.img` was restored with zero bytes in use immediately afterward.
- Result: passed with 29,378 common iterations, 1,880,192 item operations per sample, nine pairs, median ratio `1.1741490288680279x`, bootstrap 95 percent interval `[1.1520024305094314x, 1.1828408574028013x]`, and paired relative MAD `0.0074026621161992255`.
- Report SHA-256: `4b5ac2870af44c4edb7af1367466dab6a6aa230382dbdc6c6cb97eb507364fbb`.
- Preflight SHA-256: `ccfa71be38630c59437f7607b10411a8d4ffcd0b0d9883fe51e28d3fd9b0718a`.
- Markdown SHA-256: `87c5851242e54ea71c740b0d3f64f7d29c62fd4e68f9c7a206336b1b2ebf38bf`.
- Replay and regression: passed.

### Parse Medium Full Attempt 1

- Source revision: `ca9fd68d3856e9cac9cc6da16433947d056e8848`.
- Artifact: `target/benchmarks/qualification/pq2-dem-ca9fd68-parse-medium-full-1`.
- Host preparation: swap was disabled only during timing through an exit restoration trap and restored immediately afterward.
- Result: passed with 483 common iterations, 1,978,368 item operations per sample, nine pairs, median ratio `1.204309243184199x`, bootstrap 95 percent interval `[1.1913888243751443x, 1.2143650043774166x]`, and paired relative MAD `0.006107643956684444`.
- Report SHA-256: `1d4e870484a7a811929e3adf6103f91f25ffc92d8436771e09a5f6510b7d9018`.
- Preflight SHA-256: `047352972b660ef430e664ddad70e32ae6e9ae745c2376b03f9146c1573e9fb2`.
- Markdown SHA-256: `a589ba781d83e3cb568a54a28cb6052d0405cf120259ff9efdc660994bc2cac5`.
- Replay and regression: passed.

### Parse Large Full Attempt 1

- Source revision: `ca9fd68d3856e9cac9cc6da16433947d056e8848`.
- Artifact: `target/benchmarks/qualification/pq2-dem-ca9fd68-parse-large-full-1`.
- Host preparation: swap was disabled only during timing through an exit restoration trap and restored immediately afterward.
- Result: stable failed evidence with 31 common iterations, 2,031,616 item operations per sample, nine pairs, median ratio `1.2482205525151264x`, bootstrap 95 percent interval `[1.2297776854679268x, 1.2639196099402108x]`, and paired relative MAD `0.012577150242760585`.
- Report SHA-256: `61c46998f7b8540f57c82a4b585a6a88f88d65a411feeb423861edb0e26be291`.
- Preflight SHA-256: `cfd5b35151bfba438b49200f9df0f60315c61e527fa53a6a629fcce70866d49c`.
- Markdown SHA-256: `c946320132704217d5ef3435b58e13233b5ea6fb43781f8a4457745063b9e966`.
- Replay: passed and reproduced the artifact.
- Regression: failed closed because the confidence-interval upper bound exceeds `1.25x`.
- Disposition: retain permanently. The result is stable and must not be rerun or replaced by a favorable sample from the same revision.

## Third Parser Optimization

Clean implementation commit `430428ea93a40af25a352746acc2bc517e7ad1fd` stores spilled tags as `Box<str>`, bounds `DemInstruction` and `DemItem` at 96 bytes, parses target tokens without a preliminary digit scan, and uses one explicit decimal-overflow bound per digit. Extracting the private tag representation leaves `dem.rs` at 1,172 lines. Lowercase `d` and `l` targets now match pinned Stim and canonicalize to uppercase.

The regression suite covers the model-layout ceiling, maximum and overflowing unsigned values, malformed targets, lowercase target canonicalization, mixed ASCII and Unicode whitespace, existing tag cases, and the exact allocation ceiling. Full workspace tests, formatting, workspace Clippy, and staged pre-commit checks passed before the commit.

Five dirty-tree direct-worker diagnostics over the large 31-iteration shape improved from `0.458354964` through `0.465637638` seconds and approximately 34.7 MiB peak RSS before this optimization to `0.426247111` through `0.431028809` seconds and approximately 33.0 MiB peak RSS afterward. Dirty-tree exact-parity probes reported parse diagnostic ratio `1.137073x` and print diagnostic ratio `0.588801x`. These are optimization guidance only and cannot satisfy the gate.

The next source-contract commit must bind the updated profiler-note digest and regenerated correctness and performance inventories. The clean Git revision, private Cargo build, sealed Stab binary digest, and worker identity bind the new private tag module; comparator sources remain the pinned-Stim comparator implementation. From that clean revision, repeat both adapter probes, worker reproducibility, the exact focused CQ producer, replay, and preflight before restarting append-only formal full and soak evidence at every parse and print scale. No `ca9fd68d` report may be rerun.
