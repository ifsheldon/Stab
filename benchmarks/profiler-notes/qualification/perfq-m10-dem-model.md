# PERFQ-M10-DEM-MODEL Profiler Note

Owners: `stab-core/dem-parser` for `PERFQ-M10-DEM-PARSE-CONTRACT` and `stab-core/dem-printer` for `PERFQ-M10-DEM-PRINT-CONTRACT`.

Status: clean revision `f23386bdc12258eab97b9997b3f478841caa050c` produced and replayed the first faithful parse/small/full report after this note was bound. The report failed the `1.25x` gate with median ratio `1.773450x`, bootstrap 95 percent interval `[1.767688x, 1.793239x]`, and paired relative MAD `0.003249`. It is stable failed evidence, not a noisy result, and must not be rerun from the same revision. The fixture, semantic-work denominator, comparator, output obligations, scales, common-iteration policy, and threshold remain unchanged and unwaived.

## Initial Diagnosis

Dominant cost: per-instruction allocation and generic string processing are confirmed material costs, although post-fix formal closure is not yet proven. Before optimization, parsing the 4,096-item qualification family performed 14,859 allocation calls, approximately 29 per eight-item cycle. Stab allocated a lowercase instruction name, argument vector, and target vector on common instructions and repeatedly grew the top-level and spill-sized target vectors. Pinned Stim parses bytes directly and commits argument, target, and tag payloads into monotonic buffers. The frozen workload intentionally repeats tags, numeric arguments, separators, detector and observable targets, shifts, and nested blocks, so removing any of those features to improve the ratio would invalidate the comparison.

The print path is not currently implicated by the diagnostic probe. Both workers retain owned-string production and replacement in timing, and only the final known terminal-newline difference is normalized after timing.

A stack-sampling profile is unavailable on this host because `/proc/sys/kernel/perf_event_paranoid` is `4`. This does not permit changing the benchmark contract. Allocation instrumentation, direct worker scaling, and source inspection supplied the evidence used for the first production change.

## Implemented Optimization

Commit `fb089098406892756572ea14439452a1001df57a` keeps up to two DEM arguments and one target inline, pre-sizes spill target and top-level storage, parses instruction names without allocating lowercase strings, parses unsigned targets with checked decimal accumulation, and bypasses character-by-character comment scanning on comment-free lines. A preallocation cap prevents newline-heavy hostile input from causing an unbounded speculative top-level allocation before the parser enforces its line limit.

The new public regressions preserve mixed-case instruction names, Unicode target whitespace, trailing comments, hashes inside tags, escaped closing brackets, payloads larger than the inline capacities, canonical reparsing, and the exact qualification family. The 4,096-item allocation guard admits at most 4,100 calls, down from the measured 14,859-call pre-fix result. All `stab-core` tests, workspace tests, formatting, workspace Clippy, and staged pre-commit checks passed before the implementation commit.

A dirty-tree source-owned adapter probe after the change reported `stim_seconds=0.001916643`, `stab_seconds=0.002525429`, and diagnostic ratio `1.317631x` over 16,384 top-level item operations. A separate dirty-tree Stab worker executed 10,000 owned parses of the 64-item fixture in `0.096216406` seconds. These are optimization guidance only: neither result is clean paired evidence, neither can satisfy the gate, and both are superseded by the required clean note-binding revision.

Clean note-binding revision `3a78eb74ef62d22631709b10618186567a1ece17` passed both sequential source-owned adapter probes with exact output parity. The parse probe reported `stim_seconds=0.003617204`, `stab_seconds=0.002420963`, and diagnostic ratio `0.669291x`; the print probe reported `stim_seconds=0.005289254`, `stab_seconds=0.006054023`, and diagnostic ratio `1.144589x`. Worker reproducibility also passed with pinned-Stim digest `cb484542faaeba73156a1ba5d7a1f35104b697320847ad33994b9bb1f33b67d4` and Stab digest `9a2205c5522144b4a25facc69dd60b2a5cd7bf6f9ef6d574e8513b126d930382`.

The focused CQ producer at that revision then stopped before execution or publication because the parser optimization moved 108 rustdoc source lines and the generated correctness digest no longer matched the frozen inventory. No correctness artifact was created. Regeneration confirmed that only those 108 source-line fields and the derived digest changed: selected parents, owners, selectors, dispositions, case IDs, counts, and the exact DEM prerequisite remained unchanged. The clean probes and worker check prove the pre-refresh implementation contract but cannot be promoted across the inventory change.

## Required Owner Action

Next owner action: commit the reviewed source-line-only correctness and derived performance inventory refresh with this updated note, then regenerate the exact CQ prerequisite, worker reproducibility, and both adapter probes from that clean revision before publishing the first post-fix full and soak report at every parse and print scale without rerunning a non-noisy result.

1. Reproduce both sealed workers and probes from the clean note-binding revision, then publish the first post-fix full and soak result at every scale without rerunning a non-noisy failure.
2. Inspect retained raw samples, calibration decisions, paired-ratio relative MAD, confidence bounds, setup and peak RSS, and Stab allocation behavior before accepting the optimization.
3. If parse still fails the gate, diagnose the retained post-fix report independently and change production code only when new evidence identifies the remaining cost.
4. If print fails or is noisy in formal evidence despite its diagnostic probe, diagnose it independently; do not average parse and print or transfer evidence between the groups.
5. After any parser, printer, worker, adapter, note, schema, or runtime-contract change, regenerate the affected source-owned contracts and rerun correctness, reproducibility, probes, full and soak reports, regressions, rollups, and completion receipts from the fix revision.

Current acceptance still requires both the median paired ratio and bootstrap 95 percent upper bound to be at most `1.25x` at all three scales, with no waiver path. Failed, noisy, host-rejected, and controller-rejected attempts remain visible in the progress report and cannot be replaced by favorable reruns.
