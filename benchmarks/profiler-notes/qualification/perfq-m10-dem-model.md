# PERFQ-M10-DEM-MODEL Profiler Note

Owners: `stab-core/dem-parser` for `PERFQ-M10-DEM-PARSE-CONTRACT` and `stab-core/dem-printer` for `PERFQ-M10-DEM-PRINT-CONTRACT`.

Status: the first clean source-owned adapter probes at revision `b7c6c34f156d5f785dc46e1f6e79c3f4bf1e6914` proved exact fixture and normalized output identity. The medium parse probe reported a diagnostic Stab-to-Stim ratio of `2.009850x` over 16,384 top-level item operations, while the medium canonical-print probe reported `0.578837x` over the same work. Probe timing is diagnostic and cannot satisfy the `1.25x` gate. The first formal parse/small/full producer then reached a failed or noisy result but correctly refused publication because the runtime group had no source-owned profiler note. It produced no artifact. The fixture, semantic-work denominator, comparator, output obligations, scales, common-iteration policy, and `1.25x` threshold remain unchanged and unwaived.

## Initial Diagnosis

Dominant cost: not yet proven. The current parse hypothesis is allocation and tokenization overhead. Stab parses through `str::lines`, comment and tag scans, repeated trimming and whitespace splitting, and separately owned `Vec` and `String` payloads for each instruction. Pinned Stim parses bytes directly and commits argument, target, and tag payloads into monotonic buffers. The frozen workload intentionally repeats tags, numeric arguments, separators, detector and observable targets, shifts, and nested blocks, so removing any of those features to improve the ratio would invalidate the comparison.

The print path is not currently implicated by the diagnostic probe. Both workers retain owned-string production and replacement in timing, and only the final known terminal-newline difference is normalized after timing.

A stack-sampling profile is unavailable on this host because `/proc/sys/kernel/perf_event_paranoid` is `4`. This does not permit changing the benchmark contract. Allocation instrumentation, direct worker scaling, source inspection, and any available non-perf profiler should be used to separate parser allocation, tokenization, validation, and model-construction costs before changing production code.

## Required Owner Action

Next owner action: retain the first faithful formal evidence, diagnose any failed or noisy report from its raw samples, and optimize only after the evidence isolates a production cost.

1. Bind this note into both runtime groups so failed or noisy reports can be retained instead of being discarded at publication.
2. Reproduce both sealed workers and probes from the clean note-binding revision, then publish the first faithful full and soak result at every scale without rerunning a non-noisy failure.
3. Inspect retained raw samples, calibration decisions, paired-ratio relative MAD, confidence bounds, setup and peak RSS, and Stab allocation behavior before choosing an optimization.
4. If parse fails the gate, optimize the public parser without changing fixture bytes, output identity, lifecycle, semantic work, resource limits, or error behavior. Add differential correctness, malformed-input, accepted-maximum, and allocation regressions for the changed path.
5. If print fails or is noisy in formal evidence despite the probe, diagnose it independently; do not average parse and print or transfer evidence between the groups.
6. After any parser, printer, worker, adapter, note, schema, or runtime-contract change, regenerate the affected source-owned contracts and rerun correctness, reproducibility, probes, full and soak reports, regressions, rollups, and completion receipts from the fix revision.

Current acceptance still requires both the median paired ratio and bootstrap 95 percent upper bound to be at most `1.25x` at all three scales, with no waiver path. Failed, noisy, host-rejected, and controller-rejected attempts remain visible in the progress report and cannot be replaced by favorable reruns.
