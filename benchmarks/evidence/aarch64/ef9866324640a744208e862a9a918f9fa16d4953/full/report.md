# Stab E2E Performance Report

- Tier: `full`
- Source: `ef9866324640a744208e862a9a918f9fa16d4953`
- Architecture: `aarch64`
- CPU: `AI TOP ATOM (0xd85,0xd87)`
- Stim parity: `Passed`
- Stab self-regression: `Passed`
- Memory: `Passed`

| Case | Stab throughput | Stim ratio | Peak RSS | Parity | Self | Memory |
| --- | ---: | ---: | ---: | --- | --- | --- |
| `generate-surface.medium` | 6259473.875 | 0.821x | 4 MiB | `Passed` | `Passed` | `Passed` |
| `generate-surface.large` | 38189579.353 | 0.719x | 4 MiB | `Passed` | `Passed` | `Passed` |
| `convert-sparse.hits-narrow` | 325541.104 | 0.820x | 8 MiB | `Passed` | `Passed` | `Passed` |
| `convert-sparse.r8-wide` | 88598.818 | 0.809x | 24 MiB | `Passed` | `Passed` | `Passed` |
| `convert-dense.01-to-b8` | 1184488.750 | 0.632x | 24 MiB | `Passed` | `Passed` | `Passed` |
| `convert-dense.b8-to-01` | 701133.591 | 0.162x | 32 MiB | `Passed` | `Passed` | `Passed` |
| `convert-typed-dets.narrow` | 1502700.164 | 1.009x | 32 MiB | `Passed` | `Passed` | `Passed` |
| `convert-typed-dets.wide` | 528790.691 | 0.872x | 32 MiB | `Passed` | `Passed` | `Passed` |
| `sample-surface.small` | 2808649.764 | 0.872x | 32 MiB | `Passed` | `Passed` | `Passed` |
| `sample-surface.medium` | 2827432.724 | 0.902x | 32 MiB | `Passed` | `Passed` | `Passed` |
| `sample-surface.large` | 1018147.687 | 0.969x | 32 MiB | `Passed` | `Passed` | `Passed` |
| `sample-folded-ptb64.medium` | 2299428.736 | 0.971x | 32 MiB | `Passed` | `Passed` | `Passed` |
| `sample-folded-ptb64.large` | 1196879.697 | 1.147x | 32 MiB | `Passed` | `Passed` | `Passed` |
| `detect-observables.small` | 2587531.333 | 1.000x | 32 MiB | `Passed` | `Passed` | `Passed` |
| `detect-observables.medium` | 2395859.655 | 1.142x | 32 MiB | `Passed` | `Passed` | `Passed` |
| `detect-observables.large` | 913867.742 | 1.072x | 32 MiB | `Passed` | `Passed` | `Passed` |
| `m2d-packed-sweep.small` | 2712151.711 | 0.919x | 32 MiB | `Passed` | `Passed` | `Passed` |
| `m2d-packed-sweep.medium` | 3414676.439 | 1.085x | 32 MiB | `Passed` | `Passed` | `Passed` |
| `m2d-packed-sweep.large` | 2747459.137 | 1.081x | 32 MiB | `Passed` | `Passed` | `Passed` |
| `analyze-folded.small` | 2508266.947 | 0.871x | 32 MiB | `Passed` | `Passed` | `Passed` |
| `analyze-folded.medium` | 7707459.676 | 0.930x | 32 MiB | `Passed` | `Passed` | `Passed` |
| `analyze-folded.large` | 12267962.090 | 1.093x | 32 MiB | `Passed` | `Passed` | `Passed` |
| `sample-dem.small` | 2827729.422 | 0.878x | 32 MiB | `Passed` | `Passed` | `Passed` |
| `sample-dem.medium` | 828630.394 | 1.157x | 32 MiB | `Passed` | `Passed` | `Passed` |
| `sample-dem.large` | 316619.297 | 1.009x | 44 MiB | `Passed` | `Passed` | `Passed` |
| `qec-cli-pipeline.medium` | 753280.153 | 0.937x | 44 MiB | `Passed` | `Passed` | `Passed` |
| `qec-cli-pipeline.large` | 523105.497 | 1.001x | 44 MiB | `Passed` | `Passed` | `Passed` |
| `qec-rust-pipeline.medium` | 12948748.751 | n/a | 44 MiB | `NotApplicable` | `Passed` | `Passed` |
| `qec-rust-pipeline.large` | 13006952.813 | n/a | 44 MiB | `NotApplicable` | `Passed` | `Passed` |
