# PFM5 Detecting Regions Surface Memory-X Scope

## Summary

This scope note promotes one additional generated surface-code detecting-region evidence slice for PFM5.
It covers the Rust `circuit_detecting_regions_for_targets` utility on generated `surface_code:rotated_memory_x` and `surface_code:unrotated_memory_x` circuits with distance 3 and rounds 3.
It does not claim full generated surface-code detecting-region parity.

## Owned Subcases

- Generate `surface_code:rotated_memory_x` and `surface_code:unrotated_memory_x` with distance 3 and rounds 3 through Stab's Rust surface-code generator.
- Verify default-like helper counts: 24 detectors plus 1 logical observable target for rotated memory-X, 36 detectors plus 1 logical observable target for unrotated memory-X, and ticks 0 through 20 for both.
- Query D0, D4, and L0 over Stab ticks 0 through 5.
- Match exact unsigned Pauli-region strings derived from pinned Stim v1.16.0 `Circuit.detecting_regions` for the same generated circuits.

## Explicit Non-Goals

- Full generated surface-code region tables.
- Larger distances or round counts.
- Other generated-code tasks beyond separately promoted evidence.
- Coordinate-prefix target filters.
- Python binding shape or diagram API parity.
- New performance gates.

## Evidence

- Test: `cargo test -p stab-core detecting_regions_generated_surface_code_memory_x_basis_regions --quiet`.
- Oracle row: `pf5-detecting-regions-generated-surface-memory-x-rust`.
- Comparator class: structural Rust API parity against pinned Stim v1.16.0 generated circuit detecting-region output.

## Pinned Stim Reproduction

Use pinned Stim v1.16.0's Python API to regenerate the exact expected regions:

```bash
uv run --with stim==1.16.0 python - <<'PY'
import stim

for task in ["surface_code:rotated_memory_x", "surface_code:unrotated_memory_x"]:
    circuit = stim.Circuit.generated(task, distance=3, rounds=3)
    targets = [
        stim.target_relative_detector_id(0),
        stim.target_relative_detector_id(4),
        stim.target_logical_observable_id(0),
    ]
    regions = circuit.detecting_regions(targets=targets, ticks=range(6))
    print(task)
    print("detectors", circuit.num_detectors, "observables", circuit.num_observables, "ticks", circuit.num_ticks)
    for target in targets:
        print(target)
        for tick in range(6):
            print(tick, regions[target][tick])
PY
```

## Expected Regions

### Rotated Memory-X

| Target | Tick 0 | Tick 1 | Tick 2 | Tick 3 | Tick 4 | Tick 5 |
| --- | --- | --- | --- | --- | --- | --- |
| D0 | `+_XZX______________________` | `+_XXX______________________` | `+_XX_______________________` | `+__X_______________________` | `+__X_______________________` | `+__X_______________________` |
| D4 | `+__Z_______________________` | `+__X_______________________` | `+__XX______________________` | `+_XXX_____X________________` | `+_XXX_____X________________` | `+_XXX______________________` |
| L0 | `+_X______X______X__________` | `+_X______X______X__________` | `+_X______X_____XX__________` | `+_X______X______X__________` | `+_X______XX_____X__________` | `+_X______X______X__________` |

### Unrotated Memory-X

| Target | Tick 0 | Tick 1 | Tick 2 | Tick 3 | Tick 4 | Tick 5 |
| --- | --- | --- | --- | --- | --- | --- |
| D0 | `+XZX___X__________________` | `+XXX___X__________________` | `+XX___XX__________________` | `+XX___X___________________` | `+XX_______________________` | `+_X_______________________` |
| D4 | `+________X___XZX___X______` | `+________X___XXX___X______` | `+_______XX___XX___XX______` | `+________X___XX___X_______` | `+____________XX___________` | `+_____________X___________` |
| L0 | `+X_________X_________X____` | `+X_________X_________X____` | `+X_________X_________X____` | `+X____X____X____X____X____` | `+X_________X_________X____` | `+X_________X_________X____` |

## Benchmark Policy

No separate benchmark row is added for this exact-output fixture.
The existing report-only `pf5-detecting-regions-generated-surface` benchmark continues to measure the generated surface-code detecting-region utility path on the selected rotated memory-Z workload without claiming a direct Stim timing ratio.
