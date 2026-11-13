# Agent CLI Capabilities Schema Version 6

This document defines the successful machine-output contract for `stab capabilities --format=json`.

The command is a Stab extension and does not claim Stim v1.16.0 command or byte parity.
JSON mode writes one complete document followed by LF to stdout; diagnostics remain on stderr under the independent `--error-format` contract.

## Report

The top-level object contains exactly:

- `schema_version`, fixed to `6`;
- `stab_version`;
- `stim_compatibility_version`;
- descriptor-derived `commands`, `dialects`, `gates`, `codecs`, and `compilers` arrays;
- the `memory_budget` object.

Commands contain `name` and `summary`.
Gate entries describe accepted syntax, not universal execution support.
Codec entries report name, encoding, decode and encode support, typed-layout requirements, and records per group.

Each dialect contains `name` and `default_parse_limits`. The parse-limit object contains:

- `memory_budget_bytes`, fixed to 1,073,741,824;
- `repeat_nesting`, fixed to 256.

The memory budget bounds the source bytes plus the conservative retained size of the parsed model. Source lines, declarations, and targets are not separately capped.

The `memory_budget` object describes the one Stab-native admission option that every executing command accepts:

- `option`, fixed to `--memory-budget`;
- `default_bytes`, fixed to 1,073,741,824;
- `suffixes`, fixed to `["KiB", "MiB", "GiB"]`.

Each compiler entry contains:

- `operation`, one of `sample`, `m2d`, `detect`, or `sample_dem`;
- `input_dialect`;
- `compiler_schema_version`;
- `request_fingerprint_schema_version`, or `null` when no public request identity exists;
- `configurable_limits`, which is `true` when the compiler accepts a caller-selected memory budget.

There is no selectable-backend field.
Sampling has one scalar executable implementation.
Its actual identity appears in sampling plan fingerprints, while the unconditional SIMD leaf kernels are internal acceleration and not a backend.

## Change From Version 5

Version 6 replaces the `source_bytes`, `source_lines`, `represented_instructions`, and `represented_targets` parse limits with one `memory_budget_bytes` value, adds the top-level `memory_budget` object, and reports `configurable_limits` as `true` for `sample` now that the sampling compiler accepts a memory budget.
Stab does not emit a version-5 compatibility view.

Adding or removing a field, changing a field type or meaning, changing a closed enum, or changing successful stream framing requires a schema-version increment.
