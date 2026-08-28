# Stab E2E Performance Report

- Tier: `soak`
- Source: `ef9866324640a744208e862a9a918f9fa16d4953`
- Architecture: `aarch64`
- CPU: `AI TOP ATOM (0xd85,0xd87)`
- Stim parity: `Passed`
- Stab self-regression: `Passed`
- Memory: `Passed`

| Case | Stab throughput | Stim ratio | Peak RSS | Parity | Self | Memory |
| --- | ---: | ---: | ---: | --- | --- | --- |
| `generate-surface.medium` | 6665516.593 | 0.805x | 4 MiB | `Passed` | `Passed` | `Passed` |
| `generate-surface.large` | 38977325.939 | 0.714x | 4 MiB | `Passed` | `Passed` | `Passed` |
| `convert-sparse.hits-narrow` | 327724.046 | 0.819x | 8 MiB | `Passed` | `Passed` | `Passed` |
| `convert-sparse.r8-wide` | 89672.431 | 0.801x | 24 MiB | `Passed` | `Passed` | `Passed` |
| `convert-dense.01-to-b8` | 1199748.803 | 0.629x | 24 MiB | `Passed` | `Passed` | `Passed` |
| `convert-dense.b8-to-01` | 695826.130 | 0.163x | 32 MiB | `Passed` | `Passed` | `Passed` |
| `convert-typed-dets.narrow` | 1533448.342 | 0.995x | 32 MiB | `Passed` | `Passed` | `Passed` |
| `convert-typed-dets.wide` | 532020.980 | 0.872x | 32 MiB | `Passed` | `Passed` | `Passed` |
| `sample-surface.small` | 2948323.717 | 0.873x | 32 MiB | `Passed` | `Passed` | `Passed` |
| `sample-surface.medium` | 2858450.854 | 0.898x | 32 MiB | `Passed` | `Passed` | `Passed` |
| `sample-surface.large` | 1039015.857 | 0.959x | 32 MiB | `Passed` | `Passed` | `Passed` |
| `sample-folded-ptb64.medium` | 2239406.557 | 0.964x | 32 MiB | `Passed` | `Passed` | `Passed` |
| `sample-folded-ptb64.large` | 1204147.724 | 1.120x | 32 MiB | `Passed` | `Passed` | `Passed` |
| `detect-observables.small` | 2658276.482 | 0.964x | 32 MiB | `Passed` | `Passed` | `Passed` |
| `detect-observables.medium` | 2404625.146 | 1.130x | 32 MiB | `Passed` | `Passed` | `Passed` |
| `detect-observables.large` | 915194.372 | 1.071x | 32 MiB | `Passed` | `Passed` | `Passed` |
| `m2d-packed-sweep.small` | 2737689.753 | 0.914x | 32 MiB | `Passed` | `Passed` | `Passed` |
| `m2d-packed-sweep.medium` | 3567186.068 | 1.094x | 32 MiB | `Passed` | `Passed` | `Passed` |
| `m2d-packed-sweep.large` | 2848923.585 | 1.095x | 32 MiB | `Passed` | `Passed` | `Passed` |
| `analyze-folded.small` | 2633256.742 | 0.863x | 32 MiB | `Passed` | `Passed` | `Passed` |
| `analyze-folded.medium` | 7952949.370 | 0.931x | 32 MiB | `Passed` | `Passed` | `Passed` |
| `analyze-folded.large` | 12347484.940 | 1.100x | 32 MiB | `Passed` | `Passed` | `Passed` |
| `sample-dem.small` | 2847355.074 | 0.888x | 32 MiB | `Passed` | `Passed` | `Passed` |
| `sample-dem.medium` | 832115.768 | 1.154x | 32 MiB | `Passed` | `Passed` | `Passed` |
| `sample-dem.large` | 320116.417 | 1.001x | 47 MiB | `Passed` | `Passed` | `Passed` |
| `qec-cli-pipeline.medium` | 781261.772 | 0.923x | 47 MiB | `Passed` | `Passed` | `Passed` |
| `qec-cli-pipeline.large` | 528932.689 | 1.005x | 47 MiB | `Passed` | `Passed` | `Passed` |
| `qec-rust-pipeline.medium` | 12901925.209 | n/a | 47 MiB | `NotApplicable` | `Passed` | `Passed` |
| `qec-rust-pipeline.large` | 13020462.238 | n/a | 47 MiB | `NotApplicable` | `Passed` | `Passed` |
