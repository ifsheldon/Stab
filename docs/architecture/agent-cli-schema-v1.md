# Agent CLI Schema Version 1

This document defines the successful machine-output contract for the Stab-native `capabilities`, `inspect`, and `plan sample` commands.

These commands are additive Stab extensions.

They do not claim matching Stim v1.16.0 command names or output bytes.

## Invocation And Streams

Each command uses `--format=human|json`.

Human output is the default.

JSON mode writes one complete schema-version-1 JSON document followed by LF to stdout.

Warnings and failures continue to use the independent global `--error-format=human|json` contract.

JSON diagnostics are one JSON object per line on stderr.

Successful machine output and structured diagnostics therefore use separate flags and separate streams.

The commands do not accept output paths.

`inspect` and `plan sample` accept one optional positional input path and otherwise read stdin.

All input paths reuse the CLI's retained-handle preflight and existing 64 MiB circuit-input admission envelope.

## Design Rationale

Capabilities are assembled from the owning product descriptors instead of qualification inventories, feature checklists, or duplicated help tables.

Gate entries describe accepted circuit syntax only.

They do not imply that every execution or analysis operation accepts every gate or target shape.

The selectable-backend array is empty until A4 introduces a real caller-selectable backend boundary.

Inspection parses and fingerprints a model but does not compile or execute it.

Sampling plans compile for validation but never call a sampling method.

The backend-neutral compilation-request fingerprint excludes shots, seed, reference mode, result codec, filesystem paths, and the accepted `--skip_loop_folding` compatibility no-op.

This separation prevents run configuration from masquerading as compiler identity.

## Common Identity Objects

A model fingerprint is:

```json
{
  "schema_version": 1,
  "algorithm": "sha256",
  "dialect": "stim-circuit",
  "digest": "64 lowercase hexadecimal characters"
}
```

The complete digest contract is defined by [model fingerprint schema version 1](model-fingerprint-schema-v1.md).

A compilation-request fingerprint is:

```json
{
  "schema_version": 1,
  "algorithm": "sha256",
  "digest": "64 lowercase hexadecimal characters"
}
```

The complete digest contract is defined by [compilation request fingerprint schema version 1](compilation-request-fingerprint-schema-v1.md).

## Estimate Objects

Every resource quantity is represented as:

```json
{
  "class": "exact",
  "value": 123
}
```

`class` is `exact`, `upper-bound`, or `unknown`.

`value` is an unsigned integer for exact and upper-bound values.

`value` is `null` for unknown values.

Unknown values must not be interpreted as zero.

Resource reports use the fields `input_bytes`, `input_items`, `expanded_operations`, `folded_traversal`, `scratch_bytes`, `resident_bytes`, `output_bytes`, and `work_units`.

## `stab capabilities`

`stab capabilities --format=json` reports:

- `schema_version`: capability schema version.
- `stab_version`: package version.
- `stim_compatibility_version`: frozen compatibility target.
- `commands`: recursively discovered Clap command paths and summaries.
- `dialects`: model names and default parse limits.
- `gates`: closed gate syntax descriptors.
- `codecs`: records-owned result codec descriptors.
- `compilers`: engine-owned compiler registrations.
- `selectable_backends`: caller-selectable backend identifiers.

Each gate reports its canonical name, accepted aliases, category, argument rule, target rule, target grouping, and `support_scope`.

Schema version 1 fixes `support_scope` to `accepted-circuit-syntax`.

Each codec reports its name, physical encoding, decode and encode availability, typed-layout requirement, and records per complete group.

Each compiler reports its operation, input dialect, compiler schema, request-fingerprint schema, configurable-limit availability, and backend-selection availability.

The command list comes from the same Clap graph that parses requests.

The gate list comes from `Gate::all()`.

The codec list comes from the records-owned codec registry.

The compiler list comes from operation registrations colocated with compiler entry points.

## `stab inspect`

The command shape is:

```text
stab inspect [INPUT] [--type=stim|dem] [--format=human|json]
```

For a path ending in `.stim` or `.dem`, the model type is inferred case-insensitively.

Explicit `--type` overrides path inference.

Stdin and unrecognized path extensions require explicit `--type`.

Every report contains:

- `schema_version`.
- `executes`, always `false`.
- `source.bytes` and `source.physical_lines`.
- `parse_estimate`.
- a dialect-tagged `model`.

A circuit model reports its fingerprint, top-level item count, qubit count, measurement count, detector count, observable count, and sweep-bit count.

A detector error model reports its fingerprint, top-level item count, detector count, and observable count.

Fields that do not belong to a dialect are absent instead of being populated with invented zeroes.

## `stab plan sample`

The command shape is:

```text
stab plan sample [INPUT] [--shots=N] [--out_format=FORMAT] [--seed=N] [--skip_reference_sample] [--skip_loop_folding] [--format=human|json]
```

The command parses the circuit, validates PTB64 shot grouping when selected, compiles the current measurement sampler, calculates identities and estimates, renders the report, and exits.

It does not execute a shot.

Every report contains:

- `schema_version`.
- `operation`, fixed to `sample`.
- `executes`, always `false`.
- source facts.
- model identity.
- compilation identity and validation state.
- run configuration.
- resource estimates.

The `compilation` object contains:

- `request_fingerprint`.
- `compiler_schema_version`.
- `normalized_options`, empty in schema version 1.
- `configurable_limits`, empty in schema version 1.
- `selectable_backend`, `null` in schema version 1.
- `validated`, `true` only after compilation succeeds.

The `run` object contains:

- `shots`.
- `random_policy`, either `seeded` or `entropy`.
- `seed`, an unsigned integer or `null`.
- `reference_mode`, either `normal` or `skip-reference-sample`.
- `output_format`.
- `skip_loop_folding_requested`.
- `skip_loop_folding_effect`, fixed to `accepted-no-op` in schema version 1.

Fixed-width output estimates include Stim v1.16.0's one-shot CLI rule that hides heralded-noise measurement columns on the normal-reference path.

The heralded-column adjustment traverses folded circuit structure with checked arithmetic, so compact repeat blocks are not expanded merely to estimate output width.

Sparse result codecs report unknown output bytes because sampled values determine their encoded size.

## Schema Evolution

Adding or removing a field, changing a field meaning, changing an enum spelling, changing successful stream framing, or moving run configuration into compilation identity requires a schema-version increment.

Adding a real selectable backend requires A4's backend-bearing plan identity and cannot silently reinterpret the schema-version-1 `null`.

Changes to model or compilation-request digest bytes require the corresponding fingerprint schema increment even when this JSON schema remains unchanged.

Human output is structural documentation, not a byte-stable compatibility format.
