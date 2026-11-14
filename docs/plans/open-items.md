# Open Items

This list holds known gaps that no active milestone owns.
It was collected on 2026-11-14 when the fearless SIMD plan and the engine speed and architecture plan were completed and retired to Git history; both are last present at commit `7b0aa4f3` under `docs/plans/`.
An item leaves this list when a plan adopts it or a change closes it.
Timings are shared-host diagnostics from the AArch64 development host and promote no performance claim.

## Speed

- A single-shot `sample` with a reference sample on 400 layers of `CZ` and `S` over 400 qubits takes 37 ms against Stim's 29 ms, which is above the `1.25x` ceiling and outside the suite.
- The reference sample is about 58 percent of a 64-shot `sample` at distance 35 with 1000 rounds, and reference loops with a period above one are not folded.
- Reading HITS and DETS input is slower than reading `b8` input, although still faster than Stim.
- The stabilizer solver takes 4.27 s at 5000 qubits, single-gate `Tableau` append visits every row, and Stim's timings for the algebra operations were never measured.
- The x86-64 instruction-level kernel paths and the per-call level check have never been executed or measured on a controlled x86-64 host.
- A 128-lane frame width was not built or measured, and the two 512-qubit thresholds that select dense or sparse checking in analysis were reasoned, not measured.

## Memory

- The CLI applies `--memory-budget` per stage, not per command, so one command can retain a small multiple of the budget; a 147 MB flattened circuit peaks at 1.35 GB under the 1 GiB default.
- Peak memory at large record widths is far above Stim's for shapes that Stim streams, such as 415 MB against 6 MB for a 5,000,000-round repetition-code `sample`.
- Time reversal, missing-detector discovery, flow generation, detecting regions, sampled flows, and Pauli and Clifford strings admit against the default budget and take no caller budget.

## Cancellation

- A circuit-sampling or detection-sampling session that observes cancellation inside a batch is poisoned, while the same request observed between batches is resumable.
- Polls are spaced by work units, not time, so one very wide instruction can run long between polls.
- Circuit passes, logical-error search, SAT construction, and the flow checker do not poll.
- `stab_decoder::DecodeCancellation` is a separate signal type from `stab_model::Cancellation`.

## Structure And Evidence

- Detecting regions keep a 128-level repeat-nesting bound instead of the shared 256, because the traversal is recursive and overflowed a debug test stack at 256.
- `Gate` is an 8-byte handle beside `GateKind`; making it the enum would shrink `CircuitItem` and change every byte-admission number derived from its size.
- The engine's admission counting inside lowering, the detecting-region snapshot walk, and the reverse-tracker metering do not use `FoldedCircuitSummary`.
- `stab plan` does not report the frame lane width that a shot count selects, and no pinned-Stim statistical fixture row runs at a shot count that selects 64 lanes.
- `PAULI_CHANNEL_1` probabilities differ from Stim in the last printed digit in some analyses.
- `registry.global-min-publish-age` is not configured, because a publish-age cooldown conflicts with the rule to add dependencies at their latest crates.io version; that policy choice is open.
