# PQ2 DEM Parse And Print Qualification Progress Report

## Status

In progress as of 2026-07-23.

The source-current post-migration implementation inventory is performance digest `3f51801b592b0cb8dc3b340cced3dc3b7644b913168073c7d4106188e444d83d` and correctness digest `fbaa2bdf8bc0eea01b2aca385a4e537de11c3b35099671cca0e2775950c2fbb0`.
It contains independent `PERFQ-M10-DEM-PARSE-CONTRACT` and `PERFQ-M10-DEM-PRINT-CONTRACT` groups, private Stab build-receipt schema version 6, adapter receipt schema version 12, contract-preflight schema version 13 with 228 ordered receipts, and qualification report schema version 32.
Clean pre-migration revision `d9e2405d18cfff05d9b5d908525394476b0edcbc` passed and replayed the complete parse and print full and soak matrix, all regressions, four rollups, and both completion receipts under performance digest `a98f57cf194f3a021d321266656cf688c9f7780fb39fa337475e8132411eb88a`.
Focused migration commit `1cfecd64cde4a5effdf07fdaabdbe51017e25a4a` retired only the exact legacy DEM timing guards, preserved both legacy memory baselines, and produced the current performance digest. The source-current post-migration machine chain has not run yet, so the pre-migration ratios authorize migration but are not relabeled as source-current evidence.
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

## Clean Third-Optimization Pre-Migration Checkpoint At `d9e2405d`

Clean revision `d9e2405d18cfff05d9b5d908525394476b0edcbc` bound performance inventory `a98f57cf194f3a021d321266656cf688c9f7780fb39fa337475e8132411eb88a` and correctness inventory `fbaa2bdf8bc0eea01b2aca385a4e537de11c3b35099671cca0e2775950c2fbb0` with `local_modifications=false`.

- Both exact adapter probes passed the fixture, output, accepted-maximum, first-rejection, odd/even, and group-isolation contracts.
- Two isolated private-worker builds reproduced byte-for-byte. The pinned-Stim binary SHA-256 was `cb484542faaeba73156a1ba5d7a1f35104b697320847ad33994b9bb1f33b67d4`, and the Stab binary SHA-256 was `eaa49b9cf69909072f66ca716f033226d594452c140fd1b1f6b0f956f0963920`.
- The exact focused one-case CQ full prerequisite passed and replayed at `target/qualification/pq2-dem-cq-full-d9e2405`.
- The correctness request SHA-256 was `9aa534dcc4462f2cba04d1847a99a1c9348261cf25baff5c2675ff5475fbd5e1`, report SHA-256 was `4eb8082b480801880da8b475197306bcee3cdee0fc661e379d04cc7333aa9cac`, completion SHA-256 was `4924900528ac342998dc92e37d99153cfb745f84a1329d9c3d5b1deb0645efec`, preflight SHA-256 was `90491440f7ac38aafe375379f8639ebb700163048f317397d0eeb9ac82f0d9b8`, and Markdown SHA-256 was `4cc7a4d8dad1f11acd5a2af2225a54fc3e135c52842cfb418bbc6037e1d3a044`.

All twelve formal reports were first attempts, passed the independent `1.25x` median and bootstrap-upper-bound gates, replayed byte-for-byte, and passed regression. No noisy rerun was used.

| Measurement | Tier | Scale | Median ratio | Bootstrap 95 percent interval | Paired rMAD | Common iterations | Work per sample |
| --- | --- | --- | ---: | --- | ---: | ---: | ---: |
| Parse | Full | Small | `1.102449x` | `[1.097206x, 1.126126x]` | `0.007552` | 29,457 | 1,885,248 |
| Parse | Full | Medium | `1.125210x` | `[1.114810x, 1.137153x]` | `0.005516` | 481 | 1,970,176 |
| Parse | Full | Large | `1.153484x` | `[1.143317x, 1.167830x]` | `0.008724` | 30 | 1,966,080 |
| Parse | Soak | Small | `1.105525x` | `[1.099588x, 1.114671x]` | `0.005371` | 29,504 | 1,888,256 |
| Parse | Soak | Medium | `1.119705x` | `[1.115279x, 1.122029x]` | `0.003275` | 481 | 1,970,176 |
| Parse | Soak | Large | `1.159075x` | `[1.154724x, 1.165342x]` | `0.004423` | 30 | 1,966,080 |
| Print | Full | Small | `0.578301x` | `[0.575003x, 0.582326x]` | `0.005656` | 15,048 | 963,072 |
| Print | Full | Medium | `0.565507x` | `[0.562219x, 0.570063x]` | `0.005816` | 244 | 999,424 |
| Print | Full | Large | `0.561637x` | `[0.557170x, 0.573558x]` | `0.007954` | 16 | 1,048,576 |
| Print | Soak | Small | `0.580445x` | `[0.579638x, 0.585548x]` | `0.006093` | 15,088 | 965,632 |
| Print | Soak | Medium | `0.564138x` | `[0.562188x, 0.575506x]` | `0.004866` | 242 | 991,232 |
| Print | Soak | Large | `0.559351x` | `[0.554981x, 0.562265x]` | `0.005209` | 15 | 983,040 |

The parse medians range from `1.102449x` through `1.159075x`, with worst confidence-interval upper bound `1.167830x`. The print medians range from `0.559351x` through `0.580445x`, with worst upper bound `0.585548x`.

The exact report, preflight, and Markdown SHA-256 triples are:

- Parse small full: `09577461f638408cefc8992cbf5a38dc9c096bfbc615c2625c1ff2a3214df5c1`, `3c6a8e3644993f4e0c39f74bd80aafb01640faa88f0cd0bbab57a0ebc64b16aa`, and `b9f87eca451984e8799a1383edaa5027b665d1647162a969b3643811d6058fc7`.
- Parse medium full: `13f8f5813378a9b13de058d5d935279b254b4b8da3dbd741985383eb01269ca3`, `66e37f9830a5731f08100c6357ab3955d33ffce895552d7a1953bc3afbbe58fc`, and `e0a65550914a4ee9e8c56243c9f640842d5873055d28f19054342c73d0230cb9`.
- Parse large full: `ecf2ef44dbde23b323dee4c5408bcf31bfb01c3cae412845eaf04e35a154687f`, `c11197296d42d67e5ef81d96d7d10c8922393dae0d81503b2ac9dea4db71ee15`, and `32c916a172c189d11a9ad0dc7fadeb9355f30566eb1361373160cadd17f36eb7`.
- Parse small soak: `17ad6c88a6393c71cec646c4480718fbbe14aab4ab89f039d29da5686becbe11`, `1555703d2869cb77ff2aaceecbe271de9f2b557f0a95f77228a83bbf3173dd4f`, and `77b287e9f3ebad6cdb1e74682f06d7ac89dbb8b2526dcc89191890d9835b50c9`.
- Parse medium soak: `7b806e976a26ba207069cf0e1a5d424c5040b082fd117a35fa505cd5e78a9107`, `68cb459d4c1b61f370b6789f70b59ffdb356517ebca51a44bc46a627ed8ff28b`, and `ea917121c42ce10825bb495ce034dab4680e853795740791081ec69a6b2fde68`.
- Parse large soak: `cf270015028b52dd5d268ce23da02dcda1680d429c01901231c5513d21134efd`, `eb2964969240f0e304ebae5eafc6c91c569161a5811ca75d5e8b74407088fb4c`, and `9e05fc63327d708136a375250123f3e4beba26a91cd5e239cfbc24838e751ea1`.
- Print small full: `4fa61cb4ce4a137c43a584d1cc2d064867307ca3c4d6d4375648aa22f87e8bf2`, `c28dfbbd4f31896645f766a74f978a10a19a6a60876a1c326731fee31333837b`, and `b6f0f7a1ae6cbec2ccf6a0116b5c73f33caa1c8d015f81cb83956b77b92b8cda`.
- Print medium full: `3c46c87e5c25ed995c1ba57e2fbf9de92c3af5730a1b5a3af0325f2c7fe6a5ed`, `51c2709594e447667f301756a752281381bf74915cf46699d51ec4ef83983241`, and `cc951137c10c61410c33d59f8b945c697788eb45071bb06cca7b9760d52121ae`.
- Print large full: `53cf2e645e6bd801b32c0c7e12636a2d2828206c24ada4ccd6633d45392670a5`, `02a7a4c7d88e10d61c89e46ebc95429f7422a1ab1a62d067c30d5752d6d300b2`, and `9e4fdc641adfa979b4a7a87e7aac09d71702cbd7fd70fabab8345536b2724645`.
- Print small soak: `de6ede1b7f0de6746fede1d595e4d293db895d56f899096dd3b31455672a7872`, `8835bee04c65f597a6ff657d3d26f25860087be7c7e5a7a63edb973a82996930`, and `10863d46cfca807a45c126a314a15544f07e331c5eb0a80c54802f02c3eb1e1d`.
- Print medium soak: `62e1756f39c9542d210fc9b3a037a365e18a90164911bd53905fa34af3a4a9db`, `52136515c6b5f6c318364b2edfd5cdfd6371958a1e85d0ca51588cc6805868e0`, and `005a49df5aed4602e044ee3d496668b6ba1abb55c38e3194481c8d23c0fdc466`.
- Print large soak: `709de684865abc75633a6c16b2a00a220caba7187c8b632332c9b5aae56aeaef`, `57e605ec8a84251db468349572791f645a8cc5d4410063b2aa794838a2979af7`, and `a46bc5fe102f48b84d1f9dfcf3ab73d71b447b97d35bc9f923b587ee1920467a`.

The parse full and soak rollups published and replayed at `target/benchmarks/qualification/pq2-dem-d9e2405-parse-full-rollup` and `target/benchmarks/qualification/pq2-dem-d9e2405-parse-soak-rollup`; their report and preflight SHA-256 pairs were `475debeef6b263cba6840384b463db414b921a5d74edad3cbdcdcb48bcbd9e50` with `276ba8dfe60283ff7de33db17cdbbc35d9d8e2797d6258a02744ad4e0445e15f`, and `d94e76163d5eb1fde76b4756747e57623ba89248499521bc91fd2b9abc92781a` with `7b0e2410fa7e2be308ea283ef306fc3b8eb34dace87a66c3ae1d9a31dc45c462`.

The print full and soak rollups published and replayed at `target/benchmarks/qualification/pq2-dem-d9e2405-print-full-rollup` and `target/benchmarks/qualification/pq2-dem-d9e2405-print-soak-rollup`; their report and preflight SHA-256 pairs were `2beea68abb17ee988fee0b9302432a160d16480bf2bc35b69c120bce9ee7ffba` with `4c231c9cd26c180973e2458aeee0ef3752cb3e6c661daf42101c68d900efd146`, and `370ad0acfe9ea6b4ecbbe0238cdfa25e2e0c4b3b931b0a2f81a56f8a9eb3cd26` with `34017e760d3dd8684ed087f23bff24b61152b4de5f47964a3b1c60660918cbc2`.

The parse completion published and independently replayed at `target/benchmarks/qualification/pq2-dem-d9e2405-parse-completion`, with report SHA-256 `3445c71b5453c7b1c31906bf0021e905271f317320333ecaf7592df5864e31e9`, preflight SHA-256 `62a6124b861d95865245199dc90b0d80c51703323a4e59ad7c34fda742c0972d`, and Markdown SHA-256 `7bc94e666fa71222dd83507a64a4a03c171d7bc7e48a06c70826c7446ea87422`.

The print completion published and independently replayed at `target/benchmarks/qualification/pq2-dem-d9e2405-print-completion`, with report SHA-256 `4597e10933cf211a5f7984de377b8946a1ab4f1f4569a77983e86df22e67c38b`, preflight SHA-256 `2b2a8c2691a4caba611e0a2a18f8bdd78f584b5ae221966abbc4621a949a33f9`, and Markdown SHA-256 `281cc6429dba5f7ce276eef7870d776eb7d8f2917c923cf9e719df12890840c3`.

Peak RSS remained report-only. Across the formal scale matrix, observed pinned-Stim versus Stab parent peaks ranged from 3,903,488 versus 5,201,920 bytes at parse small full through 27,594,752 versus 33,153,024 bytes at parse large soak, and from 3,903,488 versus 5,464,064 bytes at print small full through 21,835,776 versus 23,928,832 bytes at print large soak. These observations do not replace PQ6 growth evidence.

## Focused Legacy Migration

Commit `1cfecd64cde4a5effdf07fdaabdbe51017e25a4a` used the replayed pre-migration completions to retire only the legacy `m10-dem-parse-contract` timing threshold and `m10-dem-print-contract` no-ratio waiver. It marked both legacy rows `non-primary-report-only` and `superseded/duplicate`, removed their replacement mappings, preserved both entries in `benchmarks/m12-primary-memory-baseline.json`, and regenerated performance inventory digest `3f51801b592b0cb8dc3b340cced3dc3b7644b913168073c7d4106188e444d83d`.

Schema-version-2 `benchmarks/qualification-threshold-migrations.json` binds parse as a whole-family `timing-threshold` migration with the exact legacy measurement pair and binds print as a whole-family `no-ratio-waiver` migration with null legacy measurements. Both records bind revision `d9e2405d18cfff05d9b5d908525394476b0edcbc`, pre-migration inventory `a98f57cf194f3a021d321266656cf688c9f7780fb39fa337475e8132411eb88a`, the exact completion report and preflight hashes above, migration revision `1cfecd64cde4a5effdf07fdaabdbe51017e25a4a`, current inventory `3f51801b592b0cb8dc3b340cced3dc3b7644b913168073c7d4106188e444d83d`, the exact executable runtime measurement, and the retained memory baselines. Validator tests reject an altered reviewed record, an invented print timing pair, a missing runtime measurement, a single-scale DEM family migration, an unreviewed extra migration, a reopened legacy row, or a missing retained memory baseline.

The next promotable work is one complete post-migration chain from one clean revision: both adapter probes, worker reproducibility, the exact focused CQ producer and replay, all twelve full and soak scale reports with immediate replay and regression, four rollups and replays, and two completion receipts with independent replay. Until that chain passes, the ratios above remain migration authorization rather than source-current timing evidence.
