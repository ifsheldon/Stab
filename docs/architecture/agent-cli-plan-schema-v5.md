# Agent CLI Plan Schema Version 5

This document defines the successful machine-output contract for `stab plan sample --format=json`.

The command parses a circuit, validates output grouping, compiles the sole scalar sampling plan, reports semantic request and executable-plan identities, and exits without executing a shot.

## Invocation

```text
stab plan sample [INPUT] [--shots=N] [--out_format=FORMAT] [--seed=N] [--skip_reference_sample] [--skip_loop_folding] [--format=human|json]
```

Passing the removed `--backend` flag is an argument error.

## Report

The top-level object contains exactly `schema_version` fixed to `5`, `operation`, `executes`, `source`, `model`, `compilation`, `run`, and `estimates`.

The `compilation` object contains:

- `request_fingerprint`, which binds the canonical circuit and sampling compiler schema but excludes executable strategy;
- `plan_fingerprint`, which binds the scalar backend, private executable variant, executable-contract schema, and reference-repeat policy;
- `compiler_schema_version`;
- `normalized_options`, containing exactly `reference-loop-policy=fold` or `reference-loop-policy=iterate`;
- `configurable_limits`, currently empty;
- `validated`.

Admission is by the command's `--memory-budget`, which bounds the parsed circuit, the compiled plan, and the session a run would create. The budget is not part of either compilation identity and does not appear in `configurable_limits`.

The `run` object contains `shots`, `frame_lane_width` (`64` or `256`), `random_policy`, optional `seed`, `reference_mode`, `output_format`, `skip_loop_folding_requested`, and `skip_loop_folding_effect`. `frame_lane_width` reports `stab_engine::LaneWidth::for_total_shots(shots).shots_per_batch()`: `64` for totals of 1 to 192 and 257 to 448 shots and `256` for every other total. The loop-folding effect is `fold-invariant-reference-repeats` by default and `iterate-reference-repeats` when `--skip_loop_folding` is present. The fold policy authorizes reuse after exact recurrence when its optional snapshot fits the memory budget; it falls back to iteration without changing admission or output when that snapshot would not fit.

Shot count, frame lane width, random seed, reference mode, and result encoding do not alter either compilation identity. Because `stab sample` selects its session's frame lane width from `--shots`, a seeded run's output depends on the seed and the shot count together; executable-contract schema `7` records that dependency. The reference-repeat policy leaves the backend-neutral request fingerprint unchanged because it does not change accepted circuit semantics or lowering, but it changes the executable plan fingerprint because it changes how reference-sample repeat work is performed.

Fixed-width output estimates are exact when representable. Sparse formats and runtime work remain unknown when sampled values or execution details determine their size.

## Change From Version 4

Version 5 adds `run.frame_lane_width` (`64` or `256`), reporting the Pauli-frame batch lane width that `stab_engine::LaneWidth::for_total_shots` selects for `run.shots`. Stab does not emit a version-4 plan compatibility view. `stab capabilities` is versioned separately by [capabilities schema version 6](agent-cli-capabilities-schema-v6.md), and `stab inspect` remains on [schema version 2](agent-cli-schema-v2.md).

Adding or removing a field, changing a field type or meaning, changing a closed enum, or changing successful stream framing requires a schema-version increment.
