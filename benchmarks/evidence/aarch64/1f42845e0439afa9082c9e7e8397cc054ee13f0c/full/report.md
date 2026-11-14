# Stab E2E Performance Report

- Tier: `full`
- Source: `1f42845e0439afa9082c9e7e8397cc054ee13f0c`
- Architecture: `aarch64`
- CPU: `AI TOP ATOM (0xd85,0xd87)`
- Stim parity: `Passed`
- Stab self-regression: `Passed`
- Memory: `Passed`

| Case | Stab throughput | Stim ratio | Peak RSS | Parity | Self | Memory |
| --- | ---: | ---: | ---: | --- | --- | --- |
| `generate.surface-large` | 40122332.929 | 0.644x | 4 MiB | `Passed` | `Passed` | `Passed` |
| `generate.color-large` | 43352243.897 | 0.678x | 4 MiB | `Passed` | `Passed` | `Passed` |
| `convert.sparse-r8-wide` | 1681113.738 | 0.043x | 3 MiB | `Passed` | `Passed` | `Passed` |
| `convert.dense-b8-to-01` | 893321.571 | 0.126x | 3 MiB | `Passed` | `Passed` | `Passed` |
| `convert.typed-dets-wide` | 2010476.961 | 0.225x | 3 MiB | `Passed` | `Passed` | `Passed` |
| `sample-surface.medium` | 3067063.178 | 0.766x | 3 MiB | `Passed` | `Passed` | `Passed` |
| `sample-surface.large` | 1145511.572 | 0.779x | 3 MiB | `Passed` | `Passed` | `Passed` |
| `sample-surface.d25-b8` | 98122.285 | 0.898x | 6 MiB | `Passed` | `Passed` | `Passed` |
| `sample-folded-ptb64.large` | 1713693.166 | 0.759x | 3 MiB | `Passed` | `Passed` | `Passed` |
| `detect-observables.medium` | 2968478.398 | 0.852x | 3 MiB | `Passed` | `Passed` | `Passed` |
| `detect-observables.large` | 1121439.955 | 0.818x | 3 MiB | `Passed` | `Passed` | `Passed` |
| `detect-observables.d11-b8` | 1429953.943 | 0.822x | 3 MiB | `Passed` | `Passed` | `Passed` |
| `detect-observables.d11-sparse-hits` | 1046725.137 | 0.344x | 3 MiB | `Passed` | `Passed` | `Passed` |
| `detect-observables.color-d9-b8` | 4397606.959 | 0.802x | 3 MiB | `Passed` | `Passed` | `Passed` |
| `m2d.sweep-medium` | 3855479.751 | 0.885x | 3 MiB | `Passed` | `Passed` | `Passed` |
| `m2d.sweep-large` | 3637022.330 | 0.777x | 3 MiB | `Passed` | `Passed` | `Passed` |
| `m2d.surface-d11` | 1917254.161 | 0.493x | 3 MiB | `Passed` | `Passed` | `Passed` |
| `analyze-folded.large` | 13902858.200 | 0.914x | 5 MiB | `Passed` | `Passed` | `Passed` |
| `analyze-decompose.d15` | 604471.063 | 0.532x | 25 MiB | `Passed` | `Passed` | `Passed` |
| `analyze-decompose.d25` | 333675.986 | 0.569x | 105 MiB | `Passed` | `Passed` | `Passed` |
| `sample-dem.medium` | 925952.614 | 0.968x | 4 MiB | `Passed` | `Passed` | `Passed` |
| `sample-dem.large` | 327011.510 | 0.937x | 5 MiB | `Passed` | `Passed` | `Passed` |
| `sample-dem.surface-d11-side-outputs` | 148581.843 | 0.456x | 17 MiB | `Passed` | `Passed` | `Passed` |
| `qec-cli-pipeline.large` | 680938.728 | 0.737x | 4 MiB | `Passed` | `Passed` | `Passed` |
| `qec-rust-pipeline.large` | 16086318.652 | n/a | 3 MiB | `NotApplicable` | `Passed` | `Passed` |
