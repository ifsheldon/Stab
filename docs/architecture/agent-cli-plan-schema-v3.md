# Agent CLI Plan Schema Version 3

This historical document records the schema-version-3 contract. Stab now emits [plan schema version 4](agent-cli-plan-schema-v4.md) and does not provide a version-3 compatibility view.

This document defines the successful machine-output contract for `stab plan sample --format=json`.

The command parses a circuit, validates output grouping, compiles the sole scalar sampling plan, reports identities and honest resource estimates, and exits without executing a shot.

## Invocation

```text
stab plan sample [INPUT] [--shots=N] [--out_format=FORMAT] [--seed=N] [--skip_reference_sample] [--skip_loop_folding] [--format=human|json]
```

Passing the removed `--backend` flag is an argument error.

## Report

The top-level object contains exactly `schema_version` fixed to `3`, `operation`, `executes`, `source`, `model`, `compilation`, `run`, and `estimates`.

The `compilation` object contains:

- `request_fingerprint`;
- `plan_fingerprint`, including the actual scalar backend and executable-contract identity;
- `compiler_schema_version`;
- `normalized_options`, currently empty;
- `configurable_limits`, currently empty;
- `validated`.

The `run` object contains shots, random policy, optional seed, reference mode, output format, and the accepted no-op loop-folding compatibility flag. Run configuration does not alter request or plan fingerprints.

Fixed-width output estimates are exact when representable. Sparse formats and runtime work remain unknown when sampled values or execution details determine their size.

## Change From Version 2

Version 3 removes `selected_backend`; the plan fingerprint already contains the actual executable implementation identity. It also removes the caller-facing backend flag. Stab does not emit a version-2 plan compatibility view. `stab inspect` continues to emit schema version 2 because its report did not change.

Adding or removing a field, changing a field type or meaning, changing a closed enum, or changing successful stream framing requires a schema-version increment.
