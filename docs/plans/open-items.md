# Open Items

This list holds known gaps that no active milestone owns.
It was collected on 2026-11-14 when the fearless SIMD plan and the engine speed and architecture plan were completed and retired to Git history; both are last present at commit `7b0aa4f3` under `docs/plans/`.
An item leaves this list when a plan adopts it or a change closes it.
Timings are shared-host diagnostics from the AArch64 development host and promote no performance claim; its cores are Cortex-A725 and faster Cortex-X925, so a ratio is meaningful only when both programs are pinned to the same core.

## Speed

- The x86-64 kernel paths run in CI at whatever instruction level the hosted `ubuntu-latest` runner has, but they and the per-call level check have never been timed on a controlled x86-64 host, and no run covers each level from SSE2 through AVX-512 separately.
