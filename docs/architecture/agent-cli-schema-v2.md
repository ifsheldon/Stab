# Agent CLI Schema Version 2

This document defines the current `inspect` machine-output contract and preserves the historical schema-version-2 contracts for `plan sample` and `capabilities`.

These commands are additive Stab extensions and do not claim matching Stim v1.16.0 command names or output bytes.

Schema version 2 supersedes [schema version 1](agent-cli-schema-v1.md). It remains current for `stab inspect`; its `plan sample` section is historical because planning now emits [schema version 4](agent-cli-plan-schema-v4.md).

`stab capabilities` now emits [capabilities schema version 5](agent-cli-capabilities-schema-v5.md).

## Invocation And Streams

Each command uses `--format=human|json`.

Human output is the default.

JSON mode writes one complete document followed by LF to stdout. `inspect` emits schema version 2; `plan sample` now emits [schema version 4](agent-cli-plan-schema-v4.md).

The historical schema-version-2 `capabilities` command used the same framing.

Warnings and failures continue to use the independent global `--error-format=human|json` contract.

Successful machine output and structured diagnostics use separate flags and streams.

The commands do not accept output paths.

`inspect` and `plan sample` accept one optional positional input path and otherwise read stdin.

All input paths reuse retained-handle preflight and the existing 64 MiB circuit-input admission envelope.

## Design Rationale

Capabilities are assembled from owning product descriptors instead of qualification inventories, feature checklists, or duplicated help tables.

Gate entries describe accepted circuit syntax only and do not imply that every operation accepts every gate or target shape.

Inspection parses and fingerprints a model but does not compile or execute it.

Sampling plans compile for validation but never create a session or execute a shot.

The backend-neutral request fingerprint excludes run configuration and backend choice.

The backend-bearing plan fingerprint binds the selected backend and private executable contract.

This separation keeps model identity, lowering-request identity, executable identity, and mutable run state distinct.

## Identity Objects

A model fingerprint contains:

```json
{
  "schema_version": 1,
  "algorithm": "sha256",
  "dialect": "stim-circuit",
  "digest": "64 lowercase hexadecimal characters"
}
```

A compilation-request fingerprint contains:

```json
{
  "schema_version": 1,
  "algorithm": "sha256",
  "digest": "64 lowercase hexadecimal characters"
}
```

A plan fingerprint contains:

```json
{
  "schema_version": 1,
  "algorithm": "sha256",
  "backend": "scalar",
  "executable_contract_schema_version": 2,
  "executable_contract_digest": "64 lowercase hexadecimal characters",
  "digest": "64 lowercase hexadecimal characters"
}
```

The normative byte contracts are [model fingerprint schema version 1](model-fingerprint-schema-v1.md), [compilation request fingerprint schema version 1](compilation-request-fingerprint-schema-v1.md), and [plan fingerprint schema version 1](plan-fingerprint-schema-v1.md).

## Estimate Objects

Every resource quantity is represented as:

```json
{
  "class": "exact",
  "value": 123
}
```

`class` is `exact`, `upper-bound`, or `unknown`.

`value` is an unsigned integer for exact and upper-bound values and `null` for unknown values.

Unknown values must not be interpreted as zero.

Resource reports use `input_bytes`, `input_items`, `expanded_operations`, `folded_traversal`, `scratch_bytes`, `resident_bytes`, `output_bytes`, and `work_units`.

## `stab capabilities` (Historical)

The command shape is:

```text
stab capabilities [--format=human|json]
```

The schema-version-2 report contained:

- `schema_version`, fixed to `2`;
- `stab_version`;
- `stim_compatibility_version`;
- descriptor-derived `commands`, `dialects`, `gates`, `codecs`, and `compilers`;
- `selectable_backends`, currently containing only `scalar`.

`portable-simd` was absent from schema-version-2 sampling capabilities because A6 build-time bit and Clifford acceleration did not register a caller-selectable runtime backend.

The `compilers` array contained exactly one `sample` entry for the `stim-circuit` dialect. Its `compiler_schema_version` and `request_fingerprint_schema_version` were both unsigned integers fixed to `1`, `configurable_limits` was `false`, and `backend_selection` was `true`.

Schema version 2 did not advertise the `m2d`, `detect`, or `sample_dem` compiler families, and `request_fingerprint_schema_version` was not nullable.

## `stab inspect`

The command shape is:

```text
stab inspect [INPUT] --type=stim|dem [--format=human|json]
```

`--type` may be inferred from a `.stim` or `.dem` path and is required for stdin or an unrecognized extension.

The report contains:

- `schema_version`, fixed to `2`;
- `executes`, always `false`;
- exact source byte and physical-line counts;
- a parse estimate;
- one dialect-tagged model summary.

A circuit summary reports its model fingerprint, top-level item count, qubits, measurements, detectors, observables, and sweep bits.

A detector error model summary reports its model fingerprint, top-level item count, detectors, and observables.

Fields that do not belong to a dialect are absent.

## `stab plan sample`

The command shape is:

```text
stab plan sample [INPUT] [--shots=N] [--out_format=FORMAT] [--seed=N] [--skip_reference_sample] [--skip_loop_folding] [--backend=auto|scalar|portable-simd] [--format=human|json]
```

The command parses the circuit, validates PTB64 grouping, compiles a sampling plan using the requested backend policy, calculates identities and estimates, renders the report, and exits without executing a shot.

`--backend` defaults to `auto`. `auto` selects the best backend registered by the current build, `scalar` requires the scalar backend, and `portable-simd` requires a separately registered portable-SIMD sampling backend. A syntactically valid but unavailable explicit backend fails compilation with a nonzero status; enabling portable-SIMD kernels elsewhere in Stab does not by itself register a selectable sampling backend.

Backend availability is discoverable through `stab capabilities`. `auto` is a selection policy and is not listed as a backend; a successful plan's `selected_backend` is always one of the advertised `selectable_backends` values.

Every report contains:

- `schema_version`, fixed to `2`;
- `operation`, fixed to `sample`;
- `executes`, always `false`;
- source facts;
- model identity;
- compilation identity and validation state;
- run configuration;
- resource estimates.

The `compilation` object contains:

- `request_fingerprint`;
- `plan_fingerprint`;
- `compiler_schema_version`;
- `normalized_options`, currently empty;
- `configurable_limits`, currently empty;
- `selected_backend`, the backend actually selected by compilation and currently `scalar` in supported builds;
- `validated`, true only after compilation succeeds.

The `run` object contains:

- `shots`;
- `random_policy`, either `seeded` or `entropy`;
- `seed`, an unsigned integer or `null`;
- `reference_mode`, either `normal` or `skip-reference-sample`;
- `output_format`;
- `skip_loop_folding_requested`;
- `skip_loop_folding_effect`, fixed to `accepted-no-op`.

Shots, seed, reference mode, output format, and the compatibility no-op do not alter request or plan fingerprints.

The requested backend policy never alters the backend-neutral request fingerprint. The resolved backend is part of the plan fingerprint, so `auto` and an explicit backend produce the same plan fingerprint when they resolve to the same executable backend.

Fixed-width output estimates include Stim v1.16.0's one-shot CLI rule that hides heralded-noise measurement columns on the normal-reference path.

Sparse result codecs report unknown output bytes because sampled values determine their encoded size.

## Schema Evolution

Adding or removing a field, changing a field meaning, changing enum spelling, changing successful stream framing, or moving run configuration into compilation identity requires a schema-version increment.

Changes to any fingerprint digest bytes require the corresponding fingerprint schema increment even when this JSON schema remains unchanged.

Adding a genuinely registered backend changes capability values but not the JSON shape. It must also produce a distinct plan fingerprint under the plan-fingerprint contract.

Capabilities schema version 3 changed `request_fingerprint_schema_version` from an unsigned integer to an unsigned integer or `null` and added three compiler families. Version 4 removed fictitious backend selection, version 5 exposed the complete default parser policy, and planning moved to version 4. `inspect` remains on schema version 2.

Human output is structural documentation, not a byte-stable compatibility format.
