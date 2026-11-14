# Stab E2E Performance Report

- Tier: `full`
- Source: `2c229ccd59b51657e0f68e349f4de98e157672ef`
- Architecture: `aarch64`
- CPU: `AI TOP ATOM (0xd85,0xd87)`
- Stim parity: `Passed`
- Stab self-regression: `Unseeded`
- Memory: `Passed`

| Case | Stab throughput | Stim ratio | Peak RSS | Parity | Self | Memory |
| --- | ---: | ---: | ---: | --- | --- | --- |
| `generate.surface-large` | 36589277.030 | 0.640x | 4 MiB | `Passed` | `Unseeded` | `Passed` |
| `generate.color-large` | 40857846.508 | 0.670x | 4 MiB | `Passed` | `Unseeded` | `Passed` |
| `convert.sparse-r8-wide` | 1573946.875 | 0.045x | 3 MiB | `Passed` | `Unseeded` | `Passed` |
| `convert.dense-b8-to-01` | 830799.287 | 0.135x | 3 MiB | `Passed` | `Unseeded` | `Passed` |
| `convert.typed-dets-wide` | 1916955.934 | 0.233x | 3 MiB | `Passed` | `Unseeded` | `Passed` |
| `sample-surface.medium` | 2971760.656 | 0.756x | 3 MiB | `Passed` | `Unseeded` | `Passed` |
| `sample-surface.large` | 1098744.277 | 0.789x | 3 MiB | `Passed` | `Unseeded` | `Passed` |
| `sample-surface.d25-b8` | 97585.796 | 0.921x | 6 MiB | `Passed` | `Unseeded` | `Passed` |
| `sample-folded-ptb64.large` | 1620832.766 | 0.768x | 3 MiB | `Passed` | `Unseeded` | `Passed` |
| `detect-observables.medium` | 2727198.579 | 0.838x | 3 MiB | `Passed` | `Unseeded` | `Passed` |
| `detect-observables.large` | 1074491.145 | 0.803x | 3 MiB | `Passed` | `Unseeded` | `Passed` |
| `detect-observables.d11-b8` | 1441086.287 | 0.808x | 3 MiB | `Passed` | `Unseeded` | `Passed` |
| `detect-observables.d11-sparse-hits` | 1054851.386 | 0.343x | 3 MiB | `Passed` | `Unseeded` | `Passed` |
| `detect-observables.color-d9-b8` | 4393935.318 | 0.803x | 3 MiB | `Passed` | `Unseeded` | `Passed` |
| `m2d.sweep-medium` | 3818981.578 | 0.911x | 3 MiB | `Passed` | `Unseeded` | `Passed` |
| `m2d.sweep-large` | 3552309.294 | 0.804x | 3 MiB | `Passed` | `Unseeded` | `Passed` |
| `m2d.surface-d11` | 1964178.967 | 0.483x | 3 MiB | `Passed` | `Unseeded` | `Passed` |
| `analyze-folded.large` | 14903326.644 | 0.847x | 5 MiB | `Passed` | `Unseeded` | `Passed` |
| `analyze-decompose.d15` | 619160.076 | 0.521x | 25 MiB | `Passed` | `Unseeded` | `Passed` |
| `analyze-decompose.d25` | 331873.763 | 0.577x | 105 MiB | `Passed` | `Unseeded` | `Passed` |
| `sample-dem.medium` | 942039.177 | 0.971x | 4 MiB | `Passed` | `Unseeded` | `Passed` |
| `sample-dem.large` | 343121.540 | 0.914x | 5 MiB | `Passed` | `Unseeded` | `Passed` |
| `sample-dem.surface-d11-side-outputs` | 146788.541 | 0.463x | 17 MiB | `Passed` | `Unseeded` | `Passed` |
| `qec-cli-pipeline.large` | 680143.335 | 0.739x | 4 MiB | `Passed` | `Unseeded` | `Passed` |
| `qec-rust-pipeline.large` | 16085832.012 | n/a | 3 MiB | `NotApplicable` | `Unseeded` | `Passed` |
