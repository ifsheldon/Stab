# Stab E2E Performance Report

- Tier: `soak`
- Source: `b4a758db169fd343c93f2b84a5b1d68558c9e6c3`
- Architecture: `aarch64`
- CPU: `AI TOP ATOM (0xd85,0xd87)`
- Stim parity: `Passed`
- Stab self-regression: `Passed`
- Memory: `Passed`

| Case | Stab throughput | Stim ratio | Peak RSS | Parity | Self | Memory |
| --- | ---: | ---: | ---: | --- | --- | --- |
| `generate-surface.medium` | 6893545.588 | 0.818x | 4 MiB | `Passed` | `Passed` | `Passed` |
| `generate-surface.large` | 39950087.177 | 0.712x | 4 MiB | `Passed` | `Passed` | `Passed` |
| `convert-sparse.hits-narrow` | 327281.960 | 0.822x | 8 MiB | `Passed` | `Passed` | `Passed` |
| `convert-sparse.r8-wide` | 88989.234 | 0.807x | 24 MiB | `Passed` | `Passed` | `Passed` |
| `convert-dense.01-to-b8` | 1209866.854 | 0.633x | 24 MiB | `Passed` | `Passed` | `Passed` |
| `convert-dense.b8-to-01` | 697420.243 | 0.161x | 32 MiB | `Passed` | `Passed` | `Passed` |
| `convert-typed-dets.narrow` | 1502078.284 | 1.027x | 32 MiB | `Passed` | `Passed` | `Passed` |
| `convert-typed-dets.wide` | 529524.515 | 0.879x | 32 MiB | `Passed` | `Passed` | `Passed` |
| `sample-surface.small` | 2626530.005 | 0.918x | 32 MiB | `Passed` | `Passed` | `Passed` |
| `sample-surface.medium` | 2789809.702 | 0.933x | 32 MiB | `Passed` | `Passed` | `Passed` |
| `sample-surface.large` | 1043539.229 | 0.968x | 32 MiB | `Passed` | `Passed` | `Passed` |
| `sample-folded-ptb64.medium` | 2310052.337 | 0.997x | 32 MiB | `Passed` | `Passed` | `Passed` |
| `sample-folded-ptb64.large` | 1187081.046 | 1.152x | 32 MiB | `Passed` | `Passed` | `Passed` |
| `detect-observables.small` | 2613338.233 | 1.012x | 32 MiB | `Passed` | `Passed` | `Passed` |
| `detect-observables.medium` | 2382635.843 | 1.163x | 32 MiB | `Passed` | `Passed` | `Passed` |
| `detect-observables.large` | 920402.901 | 1.077x | 32 MiB | `Passed` | `Passed` | `Passed` |
| `m2d-packed-sweep.small` | 2762267.001 | 0.944x | 32 MiB | `Passed` | `Passed` | `Passed` |
| `m2d-packed-sweep.medium` | 3490650.515 | 1.136x | 32 MiB | `Passed` | `Passed` | `Passed` |
| `m2d-packed-sweep.large` | 2804111.487 | 1.098x | 32 MiB | `Passed` | `Passed` | `Passed` |
| `analyze-folded.small` | 2564413.994 | 0.893x | 32 MiB | `Passed` | `Passed` | `Passed` |
| `analyze-folded.medium` | 7802574.053 | 0.956x | 32 MiB | `Passed` | `Passed` | `Passed` |
| `analyze-folded.large` | 12002105.633 | 1.104x | 32 MiB | `Passed` | `Passed` | `Passed` |
| `sample-dem.small` | 2853544.079 | 0.918x | 32 MiB | `Passed` | `Passed` | `Passed` |
| `sample-dem.medium` | 836761.105 | 1.158x | 32 MiB | `Passed` | `Passed` | `Passed` |
| `sample-dem.large` | 319673.883 | 1.026x | 49 MiB | `Passed` | `Passed` | `Passed` |
| `qec-cli-pipeline.medium` | 771551.306 | 0.953x | 49 MiB | `Passed` | `Passed` | `Passed` |
| `qec-cli-pipeline.large` | 529230.625 | 1.010x | 49 MiB | `Passed` | `Passed` | `Passed` |
| `qec-rust-pipeline.medium` | 12965800.170 | n/a | 49 MiB | `NotApplicable` | `Passed` | `Passed` |
| `qec-rust-pipeline.large` | 13038437.886 | n/a | 49 MiB | `NotApplicable` | `Passed` | `Passed` |
