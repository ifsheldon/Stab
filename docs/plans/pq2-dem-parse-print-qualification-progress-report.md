# PQ2 DEM Parse And Print Qualification Progress Report

## Status

Independent review complete; source repair implemented and fresh machine evidence pending as of 2026-07-23.

The source-current reviewed implementation inventory is performance digest `30e9df3e8004b59e43716dbb9e7aa847f472811e0adba74c43ef6bc7b243d498` and correctness digest `17d736fcbeed5b98e6ef04c1d5dee75dfde833259cd345bf40efd44ed2961942`.
It contains independent `PERFQ-M10-DEM-PARSE-CONTRACT` and `PERFQ-M10-DEM-PRINT-CONTRACT` groups, private Stab build-receipt schema version 6, adapter receipt schema version 12, worker-protocol schema version 4, contract-preflight schema version 14 with 228 ordered receipts, and qualification report schema version 33.
Clean pre-migration revision `d9e2405d18cfff05d9b5d908525394476b0edcbc` passed and replayed the complete parse and print full and soak matrix, all regressions, four rollups, and both completion receipts under performance digest `a98f57cf194f3a021d321266656cf688c9f7780fb39fa337475e8132411eb88a`.
Focused migration commit `1cfecd64cde4a5effdf07fdaabdbe51017e25a4a` retired only the exact legacy DEM timing guards, preserved both legacy memory baselines, and produced performance digest `3f51801b592b0cb8dc3b340cced3dc3b7644b913168073c7d4106188e444d83d`. Clean revision `7c3e55301b3f098497613d7dad2d624dc08a4dda` completed both post-migration probes, worker reproducibility, focused correctness, all twelve first-attempt reports and regressions, and four rollups, but independent parse-completion replay exposed a shared contract-preflight protocol defect. That complete chain is retained and review-rejected.
Clean protocol-fix revision `9497df0350cb33dcd249ea12fda802b5a68efe00` passed both probes, worker reproducibility, the exact focused correctness prerequisite, all twelve first-attempt full and soak reports, every report replay and regression, four rollup producers and replays, and both completion producers and independent replays. Parse medians range from `1.090387x` through `1.145890x`, print medians range from `0.569331x` through `0.589777x`, and the worst confidence-interval upper bound is `1.150114x`. Independent review later rejected this chain as current evidence because the exact CQ case missed five pinned-Stim DEM edge contracts, the C++ timer charged post-return assignment only to Stim, and C++ peak RSS included post-timing digest work that Rust excluded. Preserve every artifact as historical evidence under its exact source contract; no ratio or RSS observation may be relabeled under the repaired inventory.
Both groups bind the source-owned diagnosis at `benchmarks/profiler-notes/qualification/perfq-m10-dem-model.md`, whose source-current SHA-256 is `176a683c798e2d519a10eb8bf877fa00f279e3b99b4405dd9ac402810c0e3481`, so future failed or noisy formal reports can be retained and audited.

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

Schema-version-2 `benchmarks/qualification-threshold-migrations.json` binds parse as a whole-family `timing-threshold` migration with the exact legacy measurement pair and binds print as a whole-family `no-ratio-waiver` migration with null legacy measurements. Both records bind revision `d9e2405d18cfff05d9b5d908525394476b0edcbc`, pre-migration inventory `a98f57cf194f3a021d321266656cf688c9f7780fb39fa337475e8132411eb88a`, the exact completion report and preflight hashes above, migration revision `1cfecd64cde4a5effdf07fdaabdbe51017e25a4a`, migration inventory `3f51801b592b0cb8dc3b340cced3dc3b7644b913168073c7d4106188e444d83d`, the exact executable runtime measurement, and the retained memory baselines. Validator tests reject an altered reviewed record, an invented print timing pair, a missing runtime measurement, a single-scale DEM family migration, an unreviewed extra migration, a reopened legacy row, or a missing retained memory baseline.

## Retained Post-Migration Chain At `7c3e553`

Clean revision `7c3e55301b3f098497613d7dad2d624dc08a4dda` bound performance inventory `3f51801b592b0cb8dc3b340cced3dc3b7644b913168073c7d4106188e444d83d`, correctness inventory `fbaa2bdf8bc0eea01b2aca385a4e537de11c3b35099671cca0e2775950c2fbb0`, and `local_modifications=false`.

- Parse adapter probe passed at diagnostic ratio `1.163259x`, with Stim `0.002095042` seconds, Stab `0.002437076` seconds, and parent peaks 270,336 and 311,296 bytes.
- Print adapter probe passed at diagnostic ratio `0.288561x`, with Stim `0.010537213` seconds, Stab `0.003040628` seconds, and parent peaks 4,956,160 and 401,408 bytes.
- Worker reproducibility passed with pinned-Stim binary SHA-256 `cb484542faaeba73156a1ba5d7a1f35104b697320847ad33994b9bb1f33b67d4` and Stab binary SHA-256 `db8f229fd34c9bd39eb72f8cd052c3b04dbb42c3bf0cc603f45d278a425705d2`.
- The exact focused CQ prerequisite passed, replayed, and passed preflight at `target/qualification/pq2-dem-cq-full-7c3e553`. Its request, report, completion, preflight, and Markdown SHA-256 values were `84cf1c7c9795c8a30724c26c3003e1487de6b7fada20dce904deb89f0254cc62`, `bad12ea58969478c332e2acef35fa3c1fda7c2c21de0bab0c4d6fa8a49b720ce`, `4eb73cc9a473a24364ffd1ebda08d8951c1ce10ad983bfd4887db276bb61d489`, `251f650b0588d9f542434411340741f378e222423eb6c1a6488d1bef3e4d6cd1`, and `ee843b78bb7d68a6b02813dda07dae4e4dd437f9cdbd4f34d1cd1b2a2ea9f5fd`.

All twelve timing producers passed on their first attempts, replayed, and passed regression. No noisy rerun was used, and swap was restored after every producer.

| Measurement | Tier | Scale | Median ratio | Bootstrap 95 percent interval | Paired rMAD |
| --- | --- | --- | ---: | --- | ---: |
| Parse | Full | Small | `1.071705x` | `[1.064093x, 1.075730x]` | `0.003755` |
| Parse | Full | Medium | `1.092194x` | `[1.085637x, 1.108301x]` | `0.005611` |
| Parse | Full | Large | `1.127531x` | `[1.114857x, 1.140006x]` | `0.007714` |
| Parse | Soak | Small | `1.071768x` | `[1.067766x, 1.074858x]` | `0.002883` |
| Parse | Soak | Medium | `1.085059x` | `[1.079462x, 1.089559x]` | `0.005158` |
| Parse | Soak | Large | `1.127413x` | `[1.125091x, 1.134799x]` | `0.003985` |
| Print | Full | Small | `0.580438x` | `[0.577600x, 0.587752x]` | `0.004890` |
| Print | Full | Medium | `0.562553x` | `[0.558892x, 0.567117x]` | `0.004452` |
| Print | Full | Large | `0.560365x` | `[0.551476x, 0.563345x]` | `0.005317` |
| Print | Soak | Small | `0.581659x` | `[0.580141x, 0.583931x]` | `0.003897` |
| Print | Soak | Medium | `0.562111x` | `[0.559348x, 0.563917x]` | `0.004891` |
| Print | Soak | Large | `0.558642x` | `[0.557710x, 0.567815x]` | `0.005608` |

The exact report, preflight, and Markdown SHA-256 triples were:

- Parse small full: `61ecf62d6ebaf81b61296f6ec07d389b8425e58659d3ba529fce74ceb58ac3bc`, `fb7f6a292e9be1a9325789bfda0e8bd7f5b4a138a86a8f7adc54a60ff66fa550`, and `7b8c4d16d308144cf9862bce84aad7aefeb004ca51dd9a335a28594fabf1689d`.
- Parse medium full: `9ac77406babd08f2db583fb43546833cef338ae35692e3dc9a172f656635fa92`, `15e492c68451cb20fcf11ecc309580975301e332f6956c58580ab59b9e6e24cf`, and `54e9988140b85370c69008518fa1834f450d78b7990726821494f70bcade0575`.
- Parse large full: `1d03e0c4139f002a43b3365ed4edbfbdee09b608c6ab2ac59ff0078bc0d953c8`, `86af34252918b02ec31b36ff9bcef0817deb27743e438041d16e9806cc90076a`, and `7d33993099daf4315840cf262383f251509add4d25ac9f6dc24e14442174218d`.
- Parse small soak: `f27a631493de0f7e58db400d0795c70700da9c0566c926a436c9088fbd8de86e`, `833ccc7d1effa81701fd7e44e3c3a979e4b1d1794f994f3623ac4d9e92d5a62a`, and `bf042b91b9d733e718cfe46ac491784585a71ecaed1feb922c629392eccaac87`.
- Parse medium soak: `cf45b07923059c62b02a39b44bfcff5ab9fca569b3a15a74b778918871816904`, `5cd6b5d5018aae1a976d61c498a76af9cac02ec8aaf2bb562591301cfe99a5c3`, and `39765cbe5377c72d102c12a280e63a19bfacd57020afa2db82657bcfb6d247ac`.
- Parse large soak: `06aea28843415e64bb7fe2e851eca8ddd6dfb80087502dedeae986a53fecc04f`, `1028304e02a27ffa4cd930cbcb508fb21a582588aefd23bf661a2c7b0b889f5a`, and `8b34285d268395da70187fea4f93a8a16c07d8385faf613c250957bed6a1aaa5`.
- Print small full: `146509f3c0155ed9b3a1ce16c111eba59a9dee21e9cee14ef754454197b0b982`, `5bb7c436faa5c30509313be34303efb0d541f39a556d87244f64c39bbb5f7df7`, and `1e9c9851a559047c5100982aa1eab4897077d384a4a8e7d517ee62c404988fa1`.
- Print medium full: `07e2bca8e2a36c02c5e6d7df07a285e100d3e9418ee022beac574e5ad135210d`, `680c5f228005bfd703cf879642f3210a6669be39f2689e4c889e13b2ffc5b7f1`, and `2f924f58a1cd77d0e9910b44dc67631fb7f51cdc60e27c81bf4eb26343f6538d`.
- Print large full: `0a8f8aebfb4ce6d47be5c0556bebccfad409016622cd72204984dd7d6a761b73`, `e4d35e6c645d4d0dc183539ea99b02bce1bf2d53cf11cd3e1c0e7794fc345ee2`, and `bfa8ded0b162dca03ed094ebeae03354e7095c2f5ff8feaecf440462c2475cdb`.
- Print small soak: `c6e5140f04dd56b755fad629e14ea250d8fae835cfe61bf0580ef34061035586`, `281209b48e0d7c791bf3cff0f4d1bf38ba6cae274ffdefe49a2cc169296b5922`, and `5d422db48c41a809402f043d43537c72da3143c2021c140031d76663a01debcf`.
- Print medium soak: `11622a1bebafcd2b07803d90a0068875100aae934afb20542874904cf29d78c7`, `1e5b6bdc5fada6b1d6839361884c29af4d28717029e93b63d678d8779ead1812`, and `4aded597eaa138df339ac8741bbccec2a141d40e14fc1f0142bc75e3cadcc1cd`.
- Print large soak: `77eab36e561600527079a6f8fa918c46d7b68ed397b72e0a6fedbe656c75089c`, `3cce1f244aa96a6339132169440bc0d059b27bce412855d41fcd94b0b068f832`, and `d2b077e1f273acb3437153d05bd9e104efbc88c78e6e12970b21aa11cfd25f79`.

The parse full, parse soak, print full, and print soak rollups all published and independently replayed. Their report and preflight SHA-256 pairs were `a450e9408e85597c08d07e87d691fde15170f3d5fe11ea202a9bb78486ad0b7d` with `7bd28a9af1f016e57dddcba8e4304937411ae9e3d64a3d897f7d7ce53bea882c`, `f7a983b951e3064e3af5d0843d7f06fcea2e3a995d64dd2c89c4d98e0bd1b157` with `dd84aa6a815713122988c9ebea3cde690d74422ef7aa10a0e085d59801941a5e`, `10adf57a54c48dc3628d6139ef8f9cab131766f1a82aac24d2a53fe5b75b12b4` with `f1296b9f5635dd275174dc5e218b5f59ded5851da2ccdb08691732d80779ebeb`, and `8dfc85c9ab767c6fadff7c20622a565b9b91676c1fe9afa70902e3af9b4acb7b` with `624c751f50c36422820748a3a1491bcd6037fdd6b91a44014a6c7d544456cc88`.

The parse completion producer published `target/benchmarks/qualification/pq2-dem-7c3e553-parse-completion` and passed its internal action sequence. Its report, preflight, and Markdown SHA-256 values were `3bc6844592eb85c8adaf9452504f0a180d11641002034e6a4c77cf87ed405581`, `ff48a8f534e84aa2e3b2bce6ed350903101553b3358381ba6292cc632fabff3a`, and `37cacaea8310c64e2fd2085ecbbfbbafa6fe6e44368070b2fe18d9d4e33a5df5`. The producer's fresh parse adapter diagnostic also passed at `1.055284x`.

Independent `qualification-completion-report` then failed while repeating private-worker reproducibility because the pinned-Stim protocol-smoke acceptance probe completed within one `steady_clock` tick and reported zero elapsed seconds. The exact retained error was `qualification completion action worker reproducibility failed: stim qualification worker failed status 2; stderr="stim qualification adapter: adapter measured a non-positive duration\n"`. The print completion producer was not started.

This is a qualification-protocol defect, not a DEM correctness or timing failure. Semantic preflight consumed output, work, and identity but mislabeled every receipt as timing evidence and therefore imposed a positive-duration rule on a value it never used. The corrected worker protocol uses `contract` for all 228 acceptance and rejection probes, permits only finite nonnegative elapsed time in that mode, keeps `timing` and `memory` finite and strictly positive, records the mode in every preflight receipt, and keeps contract rows out of statistics. Worker protocol schema version 4, contract-preflight schema version 14, report schema version 33, and then-current performance digest `a3be3a7404d6dbc4bcffae0e3dea52e6b93797102dc5b08a776323044efdabcf` rejected the old chain.

At the end of the `7c3e553` attempt, the next required work was one complete post-fix chain from one clean revision. The following section records that replacement chain; the pre-migration ratios remain migration authorization and the `7c3e553` ratios remain review-rejected historical diagnostics.

## Clean Post-Fix Machine Chain At `9497df0`

Clean revision `9497df0350cb33dcd249ea12fda802b5a68efe00` bound performance inventory `a3be3a7404d6dbc4bcffae0e3dea52e6b93797102dc5b08a776323044efdabcf`, correctness inventory `fbaa2bdf8bc0eea01b2aca385a4e537de11c3b35099671cca0e2775950c2fbb0`, and `local_modifications=false` before every source-owned producer.

- Parse adapter probe passed with `work=16384`, Stim `0.002000530` seconds, Stab `0.004143524` seconds, diagnostic ratio `2.071213x`, and parent peak RSS observations of 270,336 and 6,414,336 bytes.
- Print adapter probe passed with `work=16384`, Stim `0.005264421` seconds, Stab `0.003044275` seconds, diagnostic ratio `0.578273x`, and parent peak RSS observations of 4,911,104 and 491,520 bytes.
- Worker reproducibility passed with pinned-Stim source SHA-256 `a0ba09b77fe8eed2c6871c11faa729d66a3414ed134e29f3df0599224741c7c1`, build fingerprint `4480b07b8a879117edff56698cdb94d0d142cd2bd8352d52e72b94c75d9b3663`, and binary SHA-256 `536e443b8f99cabb2af8bfbd5d0efa4b64a3fe5844f82c38dab4179ec1b2fef0`; the matching Stab values were `7338a6ebbbc9aac00a2d0533215eba2d9ec619a5827ed48ae3f82f5a74f772ce`, `e91b88fd5b1d728d6ad5ddf44726ff3f3622f23e0e5b3cbb829a9b4ed50a0d4d`, and `82a8b67cd177e91cd4128fe31e32c08de280194f462f7e59130d90b5fe0a9197`.
- The canonical 228-receipt contract preflight SHA-256 was `5887cda965f42d238cf67e2d61211c7edaecde100c6c7e068722c2c04f633368`.
- The exact focused CQ prerequisite selected one case, passed with zero failures, independently replayed, and passed exact preflight at `target/qualification/pq2-dem-cq-full-9497df0`.
- The correctness request, JSON report, completion, preflight, and Markdown SHA-256 values were `363fcadbcfdeea81f60144c9a57f443aa34f195623542f6f41f0ba48b9e96168`, `b4e341236e38d7d831be0e68f5434845865123ea7d136c7bc427f140e677a212`, `58aecbec712e79c50889f96271abc223ab8e90a959504e59c6523caed82c944f`, `36f740f09efc04b9e8b6a97223e82cde1b3c415f93e43e3fd80341ada100c12d`, and `2f139e4f596c6c12c6b7410e8ba22f118a74be678cab1705b3bfbc1f352bfb8d`.

All twelve formal producers passed on their first attempts, all twelve reports independently replayed, and all twelve regressions passed. No noisy or failed timing attempt was rerun. Swap was disabled only around each producer through an exit restoration trap and `/swap.img` was verified restored with zero bytes in use immediately afterward.

| Measurement | Tier | Scale | Median ratio | Bootstrap 95 percent interval | Paired rMAD | Common iterations | Work per sample |
| --- | --- | --- | ---: | --- | ---: | ---: | ---: |
| Parse | Full | Small | `1.090387x` | `[1.083988x, 1.095587x]` | `0.003249` | 29,129 | 1,864,256 |
| Parse | Full | Medium | `1.113522x` | `[1.105026x, 1.121210x]` | `0.006831` | 480 | 1,966,080 |
| Parse | Full | Large | `1.142606x` | `[1.127673x, 1.148037x]` | `0.004753` | 30 | 1,966,080 |
| Parse | Soak | Small | `1.094018x` | `[1.088797x, 1.102172x]` | `0.006245` | 29,336 | 1,877,504 |
| Parse | Soak | Medium | `1.104965x` | `[1.100543x, 1.118665x]` | `0.009749` | 478 | 1,957,888 |
| Parse | Soak | Large | `1.145890x` | `[1.140016x, 1.150114x]` | `0.005126` | 31 | 2,031,616 |
| Print | Full | Small | `0.588676x` | `[0.583127x, 0.593078x]` | `0.005591` | 14,805 | 947,520 |
| Print | Full | Medium | `0.576665x` | `[0.571789x, 0.605291x]` | `0.008455` | 238 | 974,848 |
| Print | Full | Large | `0.570075x` | `[0.568476x, 0.599574x]` | `0.005095` | 15 | 983,040 |
| Print | Soak | Small | `0.589777x` | `[0.586325x, 0.592537x]` | `0.005418` | 14,656 | 937,984 |
| Print | Soak | Medium | `0.575912x` | `[0.573804x, 0.585379x]` | `0.004015` | 237 | 970,752 |
| Print | Soak | Large | `0.569331x` | `[0.568587x, 0.577518x]` | `0.004258` | 15 | 983,040 |

The parse medians range from `1.090387x` through `1.145890x`; the print medians range from `0.569331x` through `0.589777x`. Every median and confidence-interval upper bound passes its independent `1.25x` rule, and the worst upper bound is parse large soak at `1.150114x`.

The exact report, preflight, and Markdown SHA-256 triples are:

- Parse small full: `f889e896e69fb6f3cc9d725d8dc68706d7d9c4c107d507d39fb7cc79728d6b41`, `09b5a4dceb97aaff3545ecb366e4f3dee2560781a8d01fab320aa710930aa0c1`, and `b1d36cb2b48d1bc50532327d325cdf0633e832341dad18c76b3008ff6f0dcadd`.
- Parse medium full: `4ddbc2a87bf55b45a5317e302ca7453da1fb13c4b5cc28f4c84a90668e0b7292`, `a10b367d5991e0af9c40b4a668fbcfd493048637efb5f95b4705c4336d9b2b6e`, and `23ecffea274cce00495e2fd255bef2abab82f7dd6c80db53b1a08f2f9c7ce427`.
- Parse large full: `d67eb5c66cf1aa599ae33f7f78407877ea72cbf38d5a09c42dad63e95df5a4ff`, `f27069acea4861ce8425ca9c878382c50e6a0cf24a7bb414cad4456c4df86e73`, and `280d8a9dd841c50ab64ced700946e10dc2604855ba43bd41dcec201a7dc4dc55`.
- Parse small soak: `1ef6e2fbd2419d77ccfe2816547c5947db0ea16eae6f354f5ea324ff596e6e9f`, `61362915bfe372be9dec2f206451e6969fd5d1791621f9478e651a6856a9700e`, and `485542bd6f83e9d011234230db942fe73c06fe66f1d3d142715718ca8947076e`.
- Parse medium soak: `d20a45e681cc920389baf53c5f0edc4efa05a93476ed25261dc545343eb9bebc`, `6c2112da93e3c2eb513cb14167ef619de6c1d48f033ba24ff99eb28c9a41a084`, and `badfffb5832bec3ef5c693f188b82e328ab8c7138ab5305ddba82af858b76fb6`.
- Parse large soak: `ab5b2f684e47885bba0461a2198a2de8a558227744e6a3f20288e3585a7cd7b9`, `4be602b7cb3682e7e84ca85d9ea6b4cc92c01d34729d80835f5022683875d290`, and `ef3a8b601708d405e7d8a07b4b0dd86f143bc7418e5fafc1e0c84ebdc2e6e842`.
- Print small full: `0b1e13e2d6804c2f5daafe8db74504e378cc8e0ee0eedb5e23bcf9eab4d42e66`, `40b3fd5d424f4d99ca75bd587f053a8156fef8c8a8d2da3b71e07519f1b6c0ab`, and `2551f5dcf056562d14d24837583ea3cac496c4d647505d5e90ac92e2c8b971fe`.
- Print medium full: `06b45d0fcdd13c516a786f58d52248ef1d19d86312c2e2a2f42bf0ab0b53d2f9`, `e598d70c82ca3ca109cac866568ab1dfba3f1c0336f44f9ce4dbd14e1721050d`, and `edc38eff111c1df3694a2d703f37b6c3b3847d4e394abb1a959eefb4cad8724e`.
- Print large full: `4b1fa914220dc483167f36c7109098c49bd2f101139b0a5ebc179b038f6e7244`, `6bde375eb80830b766ce5aa4fcb168d977148f7c56a8601abf02e88ef08eeda6`, and `33e7a89f0bc1df1debf75cd03e9eeebe84f1c4da4b907cf7fa4806302b1eb321`.
- Print small soak: `2c65fff74dcf427a28039d3e749b399ac496632e2f4f11672f8129061dbc0208`, `6cd0dae5135c2d117a19b6342a49b53d42ab249f8044cc1cbbf71e30f0e0ca09`, and `92d4d430556441474b0e32c3a57aafc88a5a32bdb48322b1796e75f9cc9a5370`.
- Print medium soak: `e5e33dae436318fead2fc68821a3984314b57adf64dee8b8878a8225be6d6342`, `b4c0808d2b0e64401160bafc91a1a16a2ecdec7c293642d510e9088a5f07472d`, and `e48fdb8baa4725e14226b3fc9763cb8e0d3516c26baae4b8283251c8a5c75bdc`.
- Print large soak: `4951db56096f1da60997615df9c752b5f30fd8171a2f6fa7bec03ece491a035f`, `ae1022ba50a816e8bcd8690f5f2147cb967ede920c177cb5fafd290d12341984`, and `bd0f5e401d128f0e19f16e6fd8a3bc65629ac460b5128c9adfaaedb219a6228f`.

The parse full, parse soak, print full, and print soak rollups published and independently replayed at `target/benchmarks/qualification/pq2-dem-9497df0-parse-full-rollup`, `target/benchmarks/qualification/pq2-dem-9497df0-parse-soak-rollup`, `target/benchmarks/qualification/pq2-dem-9497df0-print-full-rollup`, and `target/benchmarks/qualification/pq2-dem-9497df0-print-soak-rollup`. Their report, preflight, and Markdown SHA-256 triples were `6de965218dc9f15a5d18b2e14d919daa48876954e899c0e1c7261cc942222dbe` / `1f120041289dab48ddd6f78f9378c077117f8a74ef335834eb8cb93f1e61e754` / `e6a051a12865bf6618b92ff4afb07eeb7dbfa772098d64b9730a6b606059f7e9`, `ee3c98f5c3d44a73f0f946826b5370921f4eb0b4777104c3331745c866e725ef` / `f188be76538c740f2a2576042fc3f9525d10c0058113622246b93b0e258de78a` / `c326e160c6ce2344f157346e8f08c1c391de8ba88d581b77466a699378416f10`, `30c02bb45a6e860380e6c418d23dd6745635e277a8b29fc08a2a8a6de310a8ef` / `d6450256a8a326857430a02c88278180b716ddc8124df00c70db0cd6ab25f807` / `c46dbdc2882ac566782cf90c92b0463dcfcdfc4d1ead3b2f6720ac7961f32c08`, and `8f952fd7c32eeb72c5904c86bc696055e8f9e9a6760810af817fbfb63129aed5` / `75e531cb0286476214b7eccdef0984b5b3c1e618ceefaff88ac2c05b6c8cc462` / `5851c762e24750b6e5edb01054db4fa82327f51747bfeb076dc3bc8b890fce16`.

The parse completion published and independently replayed at `target/benchmarks/qualification/pq2-dem-9497df0-parse-completion`, with report SHA-256 `29d7b1069088f3d45625950b82a99f09a9cc70b7d951f39b9d464c0f67b2f415`, preflight SHA-256 `c65af125243d157f0773b76e316c44bdf14a638b2a7091036fdc41fd14722366`, and Markdown SHA-256 `9fa3d29604d813210cd93013b58abee76f0aa2ed1c1662bc6e582944e0cd2fad`.

The print completion published and independently replayed at `target/benchmarks/qualification/pq2-dem-9497df0-print-completion`, with report SHA-256 `4d5cba4c658c3aaf9ee8c7c835967dc5893f707d5c2232a4b42525ece729d2a1`, preflight SHA-256 `1b56253ade8c97db79d81ddac5b6d271918d9766a8296401a227e04879c7e876`, and Markdown SHA-256 `072492f72e78e54d5b3519284ec06416602f1e8a260b0c18cfb1138c0a4d5474`.

Peak RSS remains report-only. Across parse reports, observed pinned-Stim parent peaks range from 3,776,512 through 27,594,752 bytes and Stab peaks range from 4,669,440 through 32,964,608 bytes. Across print reports, pinned-Stim peaks range from 3,887,104 through 21,999,616 bytes and Stab peaks range from 5,005,312 through 23,527,424 bytes. These observations do not establish a cross-scale growth rule and do not replace PQ6 memory qualification.

Milestone audit found that the accepted-maximum execution contract had passed but its setup and peak RSS observations had not been recorded. The existing source-owned probes close that evidence gap without changing worker, fixture, comparator, timing, threshold, or completion contracts. From clean documentation revision `dade78f7aa91af98e2f79441ec9695c37436a21c`, `just bench::qualification-probe --group pq2-dem-parse-adapter-smoke --iterations 1 --work-items 524288 --evidence-mode memory` passed exact input, output, boundary, and group-isolation checks and reported Stim setup/peak RSS of 18,161,664 / 127,889,408 bytes and Stab setup/peak RSS of 19,148,800 / 124,170,240 bytes. The corresponding canonical-print command reported Stim setup/peak RSS of 96,497,664 / 127,696,896 bytes and Stab setup/peak RSS of 124,116,992 / 139,063,296 bytes. These are report-only accepted-maximum observations and do not alter or strengthen the PQ6 memory-growth claim.

## Independent Review Findings

Five independent GPT-5.6/max lanes completed on 2026-07-23. Hostile-input and artifact-lifecycle review and benchmark-science review found no additional issue; core DEM review, worker/adapter review, and documentation review found the following confirmed defects:

- Empty argument lists were parsed as zero arguments instead of one zero argument, so `error()`, `detector()`, and `shift_detectors()` diverged from Stim canonicalization and `logical_observable()` was accepted when Stim rejects it.
- DEM `repeat 0` was rejected because the public model reused the circuit-only nonzero `RepeatCount`. The repaired API uses a distinct zero-capable `DemRepeatCount`, and every traversal, flattening, sampling, counting, coordinate, search, and reverse-output path treats a zero block according to pinned Stim semantics.
- Textual DEM integers were accepted beyond Stim's inclusive maximum of $2^{60}-1$.
- Unicode separators and whitespace before instruction tags or argument lists were accepted even though pinned Stim requires its ASCII lexical boundaries and immediate modifiers.
- Standalone `DemTarget::from_str` accepted lowercase `d` and `l`; pinned Stim's standalone target syntax rejects them while complete DEM parsing continues to accept and canonicalize lowercase targets.
- C++ DEM parse and print moved returned values into preconstructed variables before the finishing clock sample, charging result transfer only to Stim. The shared adapter now times output construction, samples the clock, then transfers the retained result for every return-valued workload; mutation-only kernels retain the void timer.
- C++ peak RSS was sampled after post-timing canonicalization and digest work, while Rust sampled immediately after timed dispatch. Both now use the same pre-digest phase boundary.
- The PQ0 progress report advertised two stale disposition counts. Commit `c9cab96` synchronized the exact-case, planned-preflight, reworked, superseded, proxy, and duplicate counts with the checked inventory.

The source repair expands the correctness inventory from 1,974 to 1,986 public API records because `DemRepeatCount`, its two methods, and nine derived trait implementations are now explicit public owners. The exact DEM model mutation/repeat case owns all twelve records and now tests zero and nonzero repeat counts. The frozen correctness inventory is `17d736fcbeed5b98e6ef04c1d5dee75dfde833259cd345bf40efd44ed2961942`; the corrected shared adapter, profiler note, runtime contract, baseline binding, and performance inventory are frozen at `30e9df3e8004b59e43716dbb9e7aa847f472811e0adba74c43ef6bc7b243d498`.

Fresh execution must start from a clean commit containing these repairs. It must reproduce both private workers and both DEM probes, publish and replay the expanded exact CQ prerequisite, rerun all twelve full and soak reports once, run every regression, publish and replay four rollups and two completion receipts, and repeat accepted-maximum memory probes at the corrected RSS boundary. A new milestone audit and independent full-code-review pass are required after that evidence; neither the initial audit below nor the first review closes the repaired source state.

## Initial Milestone Audit (Superseded By Independent Review)

The 2026-07-23 milestone audit found no DEM product-correctness, public-lifecycle, comparator-fidelity, fixture-identity, timing-boundary, threshold, migration, artifact-publication, or retained-memory-baseline defect. It found and closed three evidence or documentation findings:

- Accepted-maximum setup and peak RSS had been executed but not reported. The two clean source-owned memory probes above now record all four implementation and lifecycle values.
- Twelfth-slice task 12 still named pre-fix contract-preflight schema version 13 and report schema version 32. The task now names worker protocol version 4, contract-preflight version 14, report version 33, and the exact `contract` elapsed-time rule that resolved the retained `7c3e553` failure.
- The PQ0 progress report still described four current no-ratio waivers including DEM serialization. It now matches the checked three-row waiver file and identifies the reviewed DEM print migration as the retirement authority.

| Requirement | Audit status | Direct evidence |
| --- | --- | --- |
| 1. Independent parse and serialize groups | Satisfied | `benchmarks/qualification-runtime-groups.json`; twelve distinct `9497df0` reports |
| 2. Exact CQ prerequisite | Satisfied | `target/qualification/pq2-dem-cq-full-9497df0`; replayed request, report, completion, and preflight hashes above |
| 3. Frozen eight-item cycle | Satisfied | `runtime/worker/dem_model.rs`; `stim_adapter/dem_model_contract.h`; exact input digests |
| 4. Three scales and semantic work | Satisfied | Runtime-group inventory and twelve report work counts above |
| 5. Accepted maximum and pre-barrier guards | Satisfied | Sixteen contract receipts; focused zero, incomplete, over-cap, wrong-measurement, and overflow probe tests |
| 6. Owned parse lifecycle | Satisfied | Rust and C++ direct constructor loops, optimizer barriers, retained final model, and post-timing canonical digest |
| 7. Owned serialize lifecycle | Satisfied | Rust and C++ direct canonical-printer loops, per-iteration owned strings, optimizer barriers, and retained final string |
| 8. Single terminal-newline normalization | Satisfied | Independent normalizers, exact four-scale output digests, and nonterminal-difference rejection test |
| 9. Dedicated bounded modules | Satisfied | Dedicated Rust worker, invocation, and probe modules plus C++ header; every named source remains below 1,200 lines |
| 10. Worker and fixture tests | Satisfied | `cargo test -p stab-bench dem_model --quiet`; eleven passing focused tests plus formal cross-worker receipts |
| 11. Sixteen ordered DEM receipts | Satisfied | Canonical 228-receipt preflight SHA-256 `5887cda9...`; exact-order and failure-shape tests |
| 12. Schemas, sources, and drift rejection | Satisfied | Build 6, adapter 12, protocol 4, preflight 14, report 33; zero-elapsed contract and receipt-drift tests |
| 13. Source-owned adapter probes | Satisfied | Parse and print timing probes at `9497df0`; accepted-maximum memory probes at `dade78f` |
| 14. Scale and accepted-maximum RSS | Satisfied | Twelve formal memory sections and the two accepted-maximum observations above; both legacy memory baselines retained |
| 15. Independent `1.25x` gates | Satisfied | Twelve first-attempt passing medians and bootstrap upper bounds; worst upper bound `1.150114x` |
| 16. Focused legacy migration | Satisfied | Schema-version-2 migration ledger, pre-migration completions, three current waivers, and both retained memory rows |
| 17. Complete post-migration chain | Satisfied | One clean revision, twelve report replays and regressions, four rollup replays, and two completion replays |
| 18. Audit, review, and progress report | Superseded | The initial audit completed, but independent review revealed the correctness and measurement defects above; repeat audit and review after fresh repaired evidence |

No new milestone under-specification was revealed. The earlier contract-preflight elapsed-time loophole is already recorded as resolved in `docs/plans/milestone-spec-gaps.md`; the audit did not reopen it or identify another gap.

This table records the initial audit against the `9497df0` source contract and is retained for history. It is not acceptance of the repaired source state. Final acceptance requires the fresh evidence chain described above, repeated milestone audit and independent review, synchronized final documentation, full repository verification, restored swap, and a clean committed worktree.
