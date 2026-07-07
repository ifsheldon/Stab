# PFM5 Detecting Regions Unrotated Surface Scope

## Summary

This scope note promotes one additional generated surface-code detecting-region evidence slice for PFM5.
It covers the Rust `circuit_detecting_regions_for_targets` utility on a generated `surface_code:unrotated_memory_z` circuit with distance 3 and rounds 3.
It does not claim full generated surface-code detecting-region parity.

## Owned Subcases

- Generate `surface_code:unrotated_memory_z` with distance 3 and rounds 3 through Stab's Rust surface-code generator.
- Verify default-like helper counts: 36 detectors plus 1 logical observable target, and ticks 0 through 20.
- Query D0, D4, and L0 over Stab ticks 0 through 5.
- Match exact unsigned Pauli-region strings derived from pinned Stim v1.16.0 `Circuit.detecting_regions` for the same generated circuit.

## Explicit Non-Goals

- Full generated surface-code region tables.
- Larger distances or round counts.
- Other unrotated or rotated tasks beyond separately promoted evidence.
- Coordinate-prefix target filters.
- Python binding shape or diagram API parity.
- New performance gates.

## Evidence

- Test: `cargo test -p stab-core detecting_regions_generated_unrotated_surface_code_filters_and_regions --quiet`.
- Oracle row: `pf5-detecting-regions-generated-unrotated-surface-rust`.
- Comparator class: structural Rust API parity against pinned Stim v1.16.0 generated circuit detecting-region output.

## Pinned Stim Reproduction

Use pinned Stim v1.16.0's Python API to regenerate the exact expected regions:

```bash
uv run --with stim==1.16.0 python - <<'PY'
import stim

circuit = stim.Circuit.generated("surface_code:unrotated_memory_z", distance=3, rounds=3)
targets = [
    stim.target_relative_detector_id(0),
    stim.target_relative_detector_id(4),
    stim.target_logical_observable_id(0),
]
regions = circuit.detecting_regions(targets=targets, ticks=range(6))
for target in targets:
    print(target)
    for tick in range(6):
        print(tick, regions[target][tick])
PY
```

## Expected Regions

| Target | Tick 0 | Tick 1 | Tick 2 | Tick 3 | Tick 4 | Tick 5 |
| --- | --- | --- | --- | --- | --- | --- |
| D0 | `+Z____ZZ___Z______________` | `+Z____ZZ___Z______________` | `+Z____Z____Z______________` | `+Z____Z___________________` | `+_____Z___________________` | `+_____Z___________________` |
| D4 | `+____Z___ZZ____Z__________` | `+____Z___ZZ____Z__________` | `+___ZZ___ZZ___ZZ__________` | `+____Z___ZZ___Z___________` | `+________ZZ_______________` | `+_________Z_______________` |
| L0 | `+Z_Z_Z____________________` | `+Z_Z_Z____________________` | `+ZZZZZ____________________` | `+ZZZZZ____________________` | `+ZZZZZ____________________` | `+Z_Z_Z____________________` |

## Benchmark Policy

No separate benchmark row is added for this exact-output fixture.
The existing report-only `pf5-detecting-regions-generated-surface` benchmark continues to measure the generated surface-code detecting-region utility path on the selected rotated memory-Z workload without claiming a direct Stim timing ratio.
