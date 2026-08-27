# Agent CLI Plan Schema Version 4

This document defines the successful machine-output contract for `stab plan sample --format=json`.

The command parses a circuit, validates output grouping, compiles the sole scalar sampling plan, reports semantic request and executable-plan identities, and exits without executing a shot.

## Invocation

```text
stab plan sample [INPUT] [--shots=N] [--out_format=FORMAT] [--seed=N] [--skip_reference_sample] [--skip_loop_folding] [--format=human|json]
```

Passing the removed `--backend` flag is an argument error.

## Report

The top-level object contains exactly `schema_version` fixed to `4`, `operation`, `executes`, `source`, `model`, `compilation`, `run`, and `estimates`.

The `compilation` object contains:

- `request_fingerprint`, which binds the canonical circuit and sampling compiler schema but excludes executable strategy;
- `plan_fingerprint`, which binds the scalar backend, private executable variant, executable-contract schema, and reference-repeat policy;
- `compiler_schema_version`;
- `normalized_options`, containing exactly `reference-loop-policy=fold` or `reference-loop-policy=iterate`;
- `configurable_limits`, currently empty;
- `validated`.

Sampling compiler schema version 5 also applies one fixed one-million-expanded-operation boundary to measurement-bearing shots. Because this is fixed compiler admission rather than a caller option, it is bound by `compiler_schema_version` and does not appear in `configurable_limits`.

The `run` object contains shots, random policy, optional seed, reference mode, output format, whether `--skip_loop_folding` was requested, and its selected effect. The effect is `fold-invariant-reference-repeats` by default and `iterate-reference-repeats` when the flag is present. The fold policy authorizes reuse after exact recurrence when its optional snapshot fits the existing session-storage ceiling; it falls back to iteration without changing admission or output when that snapshot would not fit.

Shot count, random seed, reference mode, and result encoding do not alter either compilation identity. The reference-repeat policy leaves the backend-neutral request fingerprint unchanged because it does not change accepted circuit semantics or lowering, but it changes the executable plan fingerprint because it changes how reference-sample repeat work is performed.

Fixed-width output estimates are exact when representable. Sparse formats and runtime work remain unknown when sampled values or execution details determine their size.

## Change From Version 3

Version 4 turns `--skip_loop_folding` from an accepted no-op into an executable reference-sample policy. It adds the normalized policy value, replaces the no-op effect with the selected fold or iterate effect, and reports the corresponding executable-plan fingerprint. Stab does not emit a version-3 plan compatibility view. `stab capabilities` remains schema version 5 and `stab inspect` remains schema version 2.

Adding or removing a field, changing a field type or meaning, changing a closed enum, or changing successful stream framing requires a schema-version increment.
