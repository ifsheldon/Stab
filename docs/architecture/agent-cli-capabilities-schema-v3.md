# Agent CLI Capabilities Schema Version 3

This historical document records the schema-version-3 contract. Stab now emits [capabilities schema version 4](agent-cli-capabilities-schema-v4.md) and does not provide a version-3 compatibility view.

The command is an additive Stab extension and does not claim a matching Stim v1.16.0 command name or output bytes.

Capabilities schema version 3 superseded the capabilities portion of [agent CLI schema version 2](agent-cli-schema-v2.md).

## Invocation And Streams

The command shape is:

```text
stab capabilities [--format=human|json]
```

Human output is the default.

JSON mode writes one complete schema-version-3 JSON document followed by LF to stdout.

Warnings and failures continue to use the independent global `--error-format=human|json` contract.

Successful machine output and structured diagnostics use separate flags and streams.

The command does not accept input or output paths.

## Design Rationale

Capabilities are assembled from owning product descriptors instead of qualification inventories, feature checklists, or duplicated help tables.

Gate entries describe accepted circuit syntax only and do not imply that every operation accepts every gate or target shape.

Compiler entries describe registered compilation families independently. A missing request-fingerprint schema is represented explicitly as `null` instead of inventing a schema identity that the compiler does not expose.

## Top-Level Report

The report contains:

- `schema_version`, fixed to `3`;
- `stab_version`;
- `stim_compatibility_version`;
- descriptor-derived `commands`, `dialects`, `gates`, `codecs`, and `compilers`;
- `selectable_backends`, currently containing only `scalar`.

The command, dialect, gate, codec, parse-limit, and backend fields retain their schema-version-2 meanings.

`portable-simd` is a build-time acceleration path and is not a caller-selectable sampling backend, so it is not listed in `selectable_backends`.

## Compiler Entries

Each compiler entry contains:

- `operation`, one of `sample`, `m2d`, `detect`, or `sample_dem`;
- `input_dialect`, either `stim-circuit` or `detector-error-model`;
- `compiler_schema_version`, an unsigned integer identifying that compiler's input contract;
- `request_fingerprint_schema_version`, an unsigned integer when the compiler exposes a public request-fingerprint identity or `null` when it does not;
- `configurable_limits`, whether compilation accepts caller-configurable resource limits;
- `backend_selection`, whether compilation accepts caller-selectable execution backends.

`null` does not mean schema version zero and must not be interpreted as an unknown integer. It means that the compiler family has no public request-fingerprint schema.

The current compiler registrations are:

| Operation | Input dialect | Compiler schema | Request-fingerprint schema | Configurable limits | Backend selection |
| --- | --- | ---: | ---: | --- | --- |
| `sample` | `stim-circuit` | `1` | `1` | `false` | `true` |
| `m2d` | `stim-circuit` | `1` | `null` | `true` | `false` |
| `detect` | `stim-circuit` | `1` | `null` | `true` | `false` |
| `sample_dem` | `detector-error-model` | `1` | `null` | `false` | `false` |

## Migration From Version 2

Schema version 2 advertised only the `sample` compiler and represented `request_fingerprint_schema_version` as a required unsigned integer.

Version 3 adds the `m2d`, `detect`, and `sample_dem` compiler families and changes `request_fingerprint_schema_version` to an unsigned integer or `null`.

Consumers must select a capabilities decoder from the top-level `schema_version`. Stab does not emit a schema-version-2 compatibility view.

No `inspect` or `plan sample` field changed during this historical migration, so both commands emitted schema version 2 at that checkpoint.

## Schema Evolution

Adding or removing a field, changing a field type or meaning, changing an enum value set or spelling, changing successful stream framing, or changing the closed compiler-family set requires a capabilities schema-version increment.

Changes to compiler, request-fingerprint, model-fingerprint, or plan-fingerprint digest bytes require the corresponding identity schema increment even when this capabilities schema remains unchanged.

Human output is structural documentation, not a byte-stable compatibility format.
