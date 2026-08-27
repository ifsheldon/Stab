# Agent CLI Capabilities Schema Version 4 (Historical)

This document records the historical schema-version-4 contract. Stab now emits [capabilities schema version 5](agent-cli-capabilities-schema-v5.md) and does not provide a version-4 compatibility view.

The command is a Stab extension and does not claim Stim v1.16.0 command or byte parity. JSON mode writes one complete document followed by LF to stdout; diagnostics remain on stderr under the independent `--error-format` contract.

## Report

The top-level object contains exactly:

- `schema_version`, fixed to `4`;
- `stab_version`;
- `stim_compatibility_version`;
- descriptor-derived `commands`, `dialects`, `gates`, `codecs`, and `compilers` arrays.

Commands contain `name` and `summary`. Dialects contain `name` and `default_parse_limits`. Gate entries describe accepted syntax, not universal execution support. Codec entries report name, encoding, decode and encode support, typed-layout requirements, and records per group.

Each compiler entry contains:

- `operation`, one of `sample`, `m2d`, `detect`, or `sample_dem`;
- `input_dialect`;
- `compiler_schema_version`;
- `request_fingerprint_schema_version`, or `null` when no public request identity exists;
- `configurable_limits`.

There is no selectable-backend field. Sampling has one scalar executable implementation. Its actual identity appears in sampling plan fingerprints, while `portable-simd` remains a build-time leaf-kernel feature.

## Change From Version 3

Version 4 removes `selectable_backends` and compiler-level `backend_selection`. Those fields described a choice that the product did not implement. Stab does not emit a version-3 compatibility view.

Adding or removing a field, changing a field type or meaning, changing a closed enum, or changing successful stream framing requires a schema-version increment.
