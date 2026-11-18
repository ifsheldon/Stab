# Open Items

This list holds known gaps that no active milestone owns.
It was collected on 2026-11-14 when the fearless SIMD plan and the engine speed and architecture plan were completed and retired to Git history; both are last present at commit `7b0aa4f3` under `docs/plans/`.
An item leaves this list when a plan adopts it or a change closes it.
Timings are shared-host diagnostics from the AArch64 development host and promote no performance claim; its cores are Cortex-A725 and faster Cortex-X925, so a ratio is meaningful only when both programs are pinned to the same core.

## Speed

- The x86-64 kernel paths run in CI at whatever instruction level the hosted `ubuntu-latest` runner has, but they and the per-call level check have never been timed on a controlled x86-64 host, and no run covers each level from SSE2 through AVX-512 separately.

## Memory

- The CLI does not charge standard output's 64 KiB buffer or `convert`'s output-file buffers against `--memory-budget`, and `m2d`, which cannot know its record count up front, is charged the worst-case pending-output bound and computes its text record cap before that hold.
- DEM parsing does not charge the display text of an opaque non-UTF-8 tag, and fusing circuit instructions can briefly hold the old and new target buffers of a reallocation while charging one.

## Cancellation

- Sampling polls are spaced by estimated work rather than wall-clock time, and three stretches run without polls: zeroing the shot-major records at session creation (70 to 145 ms for 320 MB, which needs a chunked constructor in `stab-bits`), dropping session storage (22 to 40 ms), and the caller's sink.

## Structure And Evidence

- The engine's admission counting inside lowering does not use `FoldedCircuitSummary`: it needs a running measurement count in source order to validate record lookbacks, the sweep-bit and simulated-qubit maxima, and counts that depend on the lowering itself, none of which the whole-circuit summary carries. The detecting-region snapshot walk takes its entry counts from the summary but still counts ticks per repeat body by hand.
