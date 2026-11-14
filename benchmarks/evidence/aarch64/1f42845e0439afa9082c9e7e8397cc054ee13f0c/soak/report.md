# Stab E2E Performance Report

- Tier: `soak`
- Source: `1f42845e0439afa9082c9e7e8397cc054ee13f0c`
- Architecture: `aarch64`
- CPU: `AI TOP ATOM (0xd85,0xd87)`
- Stim parity: `Passed`
- Stab self-regression: `Passed`
- Memory: `Passed`

| Case | Stab throughput | Stim ratio | Peak RSS | Parity | Self | Memory |
| --- | ---: | ---: | ---: | --- | --- | --- |
| `generate.surface-large` | 40124286.589 | 0.643x | 4 MiB | `Passed` | `Passed` | `Passed` |
| `generate.color-large` | 42538127.609 | 0.680x | 4 MiB | `Passed` | `Passed` | `Passed` |
| `convert.sparse-r8-wide` | 1559197.151 | 0.046x | 3 MiB | `Passed` | `Passed` | `Passed` |
| `convert.dense-b8-to-01` | 941252.169 | 0.120x | 3 MiB | `Passed` | `Passed` | `Passed` |
| `convert.typed-dets-wide` | 1995354.079 | 0.226x | 3 MiB | `Passed` | `Passed` | `Passed` |
| `sample-surface.medium` | 3083705.747 | 0.769x | 3 MiB | `Passed` | `Passed` | `Passed` |
| `sample-surface.large` | 1140616.328 | 0.803x | 3 MiB | `Passed` | `Passed` | `Passed` |
| `sample-surface.d25-b8` | 89919.348 | 0.952x | 6 MiB | `Passed` | `Passed` | `Passed` |
| `sample-folded-ptb64.large` | 1682322.755 | 0.767x | 3 MiB | `Passed` | `Passed` | `Passed` |
| `detect-observables.medium` | 2973613.896 | 0.842x | 3 MiB | `Passed` | `Passed` | `Passed` |
| `detect-observables.large` | 1128488.437 | 0.800x | 3 MiB | `Passed` | `Passed` | `Passed` |
| `detect-observables.d11-b8` | 1437431.545 | 0.819x | 3 MiB | `Passed` | `Passed` | `Passed` |
| `detect-observables.d11-sparse-hits` | 1047439.268 | 0.344x | 4 MiB | `Passed` | `Passed` | `Passed` |
| `detect-observables.color-d9-b8` | 4390360.048 | 0.801x | 3 MiB | `Passed` | `Passed` | `Passed` |
| `m2d.sweep-medium` | 3848607.652 | 0.890x | 3 MiB | `Passed` | `Passed` | `Passed` |
| `m2d.sweep-large` | 3609572.240 | 0.788x | 3 MiB | `Passed` | `Passed` | `Passed` |
| `m2d.surface-d11` | 1944825.533 | 0.490x | 3 MiB | `Passed` | `Passed` | `Passed` |
| `analyze-folded.large` | 15133425.338 | 0.842x | 5 MiB | `Passed` | `Passed` | `Passed` |
| `analyze-decompose.d15` | 607945.833 | 0.528x | 25 MiB | `Passed` | `Passed` | `Passed` |
| `analyze-decompose.d25` | 333160.463 | 0.572x | 105 MiB | `Passed` | `Passed` | `Passed` |
| `sample-dem.medium` | 964190.424 | 0.960x | 4 MiB | `Passed` | `Passed` | `Passed` |
| `sample-dem.large` | 335090.979 | 0.931x | 5 MiB | `Passed` | `Passed` | `Passed` |
| `sample-dem.surface-d11-side-outputs` | 148487.201 | 0.456x | 17 MiB | `Passed` | `Passed` | `Passed` |
| `qec-cli-pipeline.large` | 683733.519 | 0.731x | 4 MiB | `Passed` | `Passed` | `Passed` |
| `qec-rust-pipeline.large` | 16091583.740 | n/a | 3 MiB | `NotApplicable` | `Passed` | `Passed` |
