# Open Items

This list holds known gaps that no active milestone owns.
It was collected on 2026-11-14 when the fearless SIMD plan and the engine speed and architecture plan were completed and retired to Git history; both are last present at commit `7b0aa4f3` under `docs/plans/`.
An item leaves this list when a plan adopts it or a change closes it.
Timings are shared-host diagnostics from the AArch64 development host and promote no performance claim; its cores are Cortex-A725 and faster Cortex-X925, so a ratio is meaningful only when both programs are pinned to the same core.

## Speed

- Fixed parse-and-compile cost on short instructions is above Stim and outside the suite: a single-shot `sample` on 1.5 million short instructions takes 0.16 s against Stim's 0.13 s without the reference sample and 0.18 s against 0.08 s with it, while a 147 MB flat circuit takes 0.57 s against 0.71 s without the reference sample and 0.79 s against 0.84 s with it.
- That remaining cost is building the parsed circuit and the lowered program before the first shot, so peak memory is both together (335 MiB against Stim's 229 MiB on the flat circuit, 138 MiB against 122 MiB on the short instructions, and against Stim's 4 to 6 MiB with the reference sample), because Stim streams a single shot without building the circuit.
- At about 10 percent density, reading HITS or DETS input costs about 2x (100-bit records) to 14x (1,000,000-bit records) reading the same records as `b8`, whose files are 2 to 6 times smaller, although every measured `convert` and `m2d` row is faster than Stim; at about 0.1 percent density it costs no more than `b8`. The reader is within 5 percent of a bare digit loop at 1,000,000-bit records and carries about 15 ns of per-record overhead at 100-bit records; an 8-byte digit parser and a loop with fewer bounds checks both measured slower.
- Variable-width `sample_dem --replay_err_in` inputs (`01`, HITS, DETS, and `r8`) whose retained records do not fit what the replay session leaves of `--memory-budget` still decode their input twice, which costs 10 to 17 percent on sparse HITS, DETS, and `r8` and about 90 percent on `01`; a budget that just misses retention is the worst case, because the store fills, is released, and the input is decoded again. No sound single decode was found: stdout and pipes cannot be staged, and a temporary file plus rename would replace symlinks, hard links, mode, and ownership that the in-place output plan preserves. Keeping the retained prefix and seeking to the first unretained record was not built.
- A single `Tableau::append` still visits every row, about 4 ns per row at 5000 qubits where the row stride makes it memory-bound; batched callers including `tableau_to_circuit` use `TableauAppender`, and `Tableau::prepend` is the word-parallel left-action route.
- Two-qubit `Tableau::prepend` decodes its gate argument on every call, so at 500 qubits and below it stays 1 to 10 ns behind Stim's direct specialized prepends (`CX` 32 ns against 27 ns at 100 qubits), and `ISWAP` and `CY` still take the saved-rows path (`ISWAP` 204 ns against 76 ns at 100 qubits); at 10,000 qubits every measured gate is ahead of Stim on the same core.
- The x86-64 instruction-level kernel paths and the per-call level check have never been executed or measured on a controlled x86-64 host.

## Memory

- `sample` and `detect` stream 1 to 63 shots and larger non-PTB64 requests whose output planes fill at least 8 MiB per batch, so their floor is one lane width of complete records: 312 MiB from 256 shots on for a 5,000,000-round repetition code (10,000,000 measurements), against Stim's 1.03 to 2.31 GB. Going lower needs narrower lanes, which changes seeded output, or compressed records. PTB64 output still materializes its planes (462 MiB at 1024 shots) because streaming it measured 10 to 28 percent slower, and small records stay materialized because streaming them measured up to 4 percent slower.
- `m2d` with sweep correction still converts whole batches through full measurement and detector planes; without it, 256 records of that circuit hold 563 MiB, one lane width of records.
- The CLI does not charge standard output's 64 KiB buffer or `convert`'s output-file buffers against `--memory-budget`, and `m2d`, which cannot know its record count up front, is charged the worst-case pending-output bound and computes its text record cap before that hold.
- DEM parsing does not charge the display text of an opaque non-UTF-8 tag, and fusing circuit instructions can briefly hold the old and new target buffers of a reallocation while charging one.

## Cancellation

- Sampling polls are spaced by estimated work rather than wall-clock time, and three stretches run without polls: zeroing the shot-major records at session creation (70 to 145 ms for 320 MB, which needs a chunked constructor in `stab-bits`), dropping session storage (22 to 40 ms), and the caller's sink.

## Structure And Evidence

- The engine's admission counting inside lowering does not use `FoldedCircuitSummary`: it needs a running measurement count in source order to validate record lookbacks, the sweep-bit and simulated-qubit maxima, and counts that depend on the lowering itself, none of which the whole-circuit summary carries. The detecting-region snapshot walk takes its entry counts from the summary but still counts ticks per repeat body by hand.
- `stab-model`'s folded DEM visitor and DEM parser still recurse once per repeat level and refuse models deeper than 256 levels before recursing, and coordinate queries on a circuit are iterative but quadratic in nesting depth.
- `registry.global-min-publish-age` is not configured, because a publish-age cooldown conflicts with the rule to add dependencies at their latest crates.io version; that policy choice is open.
