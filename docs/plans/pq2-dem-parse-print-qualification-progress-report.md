# PQ2 DEM Parse And Print Qualification Progress Report

## Status

In progress as of 2026-07-23.

The source-current implementation inventory is performance digest `8094995692c48c98723467cf6e90c3c685797fb4a02bf0efe15c273d844fbfef` and correctness digest `648e7ea5a66997a810498dc871257bd654c7f9af9304651d43a88103eded0289`.
It contains independent `PERFQ-M10-DEM-PARSE-CONTRACT` and `PERFQ-M10-DEM-PRINT-CONTRACT` groups, private Stab build-receipt schema version 6, adapter receipt schema version 12, contract-preflight schema version 13 with 228 ordered receipts, and qualification report schema version 32.
Neither group has promotable timing evidence or legacy-migration authorization yet.

## Retained Attempts

### Parse Probe 1

- Source revision: `6c1e1d161b1869646ba69249c19e45ed4cd963a1`.
- Command: `just bench::qualification-probe --group pq2-dem-parse-adapter-smoke`.
- Result: failed before publishing a probe receipt.
- Exact failure: the probe expected pinned Stim's zero-width rejection to say `adapter requires --iterations and --work-items`, but the adapter's numeric parser correctly rejected the request earlier with `work-items must be positive`.
- Classification: faithful contract-test failure in the new expectation, not a product or pinned-Stim defect.
- Action: bind the actual earlier rejection text, rerun targeted checks, commit the fix, and rerun both source-owned probes from the new clean revision.

This failed attempt must remain visible and cannot be promoted or replaced by a later passing run.
