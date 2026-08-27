# Agent CLI Capabilities Schema Version 5

This document defines the successful machine-output contract for `stab capabilities --format=json`.

The command is a Stab extension and does not claim Stim v1.16.0 command or byte parity. JSON mode writes one complete document followed by LF to stdout; diagnostics remain on stderr under the independent `--error-format` contract.

## Report

The top-level object contains exactly:

- `schema_version`, fixed to `5`;
- `stab_version`;
- `stim_compatibility_version`;
- descriptor-derived `commands`, `dialects`, `gates`, `codecs`, and `compilers` arrays.

Commands contain `name` and `summary`. Gate entries describe accepted syntax, not universal execution support. Codec entries report name, encoding, decode and encode support, typed-layout requirements, and records per group.

Each dialect contains `name` and `default_parse_limits`. The parse-limit object contains:

- `source_bytes`, fixed to 67,108,864;
- `source_lines`, fixed to 1,000,000;
- `represented_instructions`, fixed to 1,000,000;
- `represented_targets`, fixed to 32,000,000;
- `repeat_nesting`, fixed to 256.

The values are inclusive parser defaults. Represented instructions count compact source declarations before repeat expansion and circuit fusion; represented targets count retained target values, including circuit combiners and DEM separators.

Each compiler entry contains:

- `operation`, one of `sample`, `m2d`, `detect`, or `sample_dem`;
- `input_dialect`;
- `compiler_schema_version`;
- `request_fingerprint_schema_version`, or `null` when no public request identity exists;
- `configurable_limits`.

There is no selectable-backend field. Sampling has one scalar executable implementation. Its actual identity appears in sampling plan fingerprints, while `portable-simd` remains a build-time leaf-kernel feature.

## Change From Version 4

Version 5 adds `source_bytes`, `represented_instructions`, and `represented_targets` to each dialect's `default_parse_limits`. Stab does not emit a version-4 compatibility view.

Adding or removing a field, changing a field type or meaning, changing a closed enum, or changing successful stream framing requires a schema-version increment.
