# Agent CLI Schema Version 2

This document defines the current `stab inspect` machine-output contract and the identity and estimate objects shared by the agent commands.

These commands are additive Stab extensions and do not claim matching Stim v1.16.0 command names or output bytes.

`stab plan sample` emits [plan schema version 4](agent-cli-plan-schema-v4.md), and `stab capabilities` emits [capabilities schema version 6](agent-cli-capabilities-schema-v6.md). Their superseded schema-version-2 contracts remain in Git history.

## Invocation And Streams

Each command uses `--format=human|json`.

Human output is the default.

JSON mode writes one complete document followed by LF to stdout.

Warnings and failures continue to use the independent global `--error-format=human|json` contract.

Successful machine output and structured diagnostics use separate flags and streams.

The commands do not accept output paths.

`inspect` and `plan sample` accept one optional positional input path and otherwise read stdin.

All input paths reuse retained-handle preflight and admit their input against the command's `--memory-budget`, one gibibyte by default.

## Design Rationale

Capabilities are assembled from owning product descriptors instead of qualification inventories, feature checklists, or duplicated help tables.

Gate entries describe accepted circuit syntax only and do not imply that every operation accepts every gate or target shape.

Inspection parses and fingerprints a model but does not compile or execute it.

Sampling plans compile for validation but never create a session or execute a shot.

The backend-neutral request fingerprint excludes run configuration and executable strategy.

The backend-bearing plan fingerprint binds the sole scalar backend and the private executable contract.

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
  "executable_contract_schema_version": 7,
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

## Schema Evolution

Adding or removing a field, changing a field meaning, changing enum spelling, changing successful stream framing, or moving run configuration into compilation identity requires a schema-version increment.

Changes to any fingerprint digest bytes require the corresponding fingerprint schema increment even when this JSON schema remains unchanged.

Adding a genuinely registered backend changes capability values but not the JSON shape. It must also produce a distinct plan fingerprint under the plan-fingerprint contract.

`inspect` remains on schema version 2; planning and capabilities are versioned by their own documents.

Human output is structural documentation, not a byte-stable compatibility format.
