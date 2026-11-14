# Stab E2E Performance Report

- Tier: `soak`
- Source: `2c229ccd59b51657e0f68e349f4de98e157672ef`
- Architecture: `aarch64`
- CPU: `AI TOP ATOM (0xd85,0xd87)`
- Stim parity: `Passed`
- Stab self-regression: `Unseeded`
- Memory: `Passed`

| Case | Stab throughput | Stim ratio | Peak RSS | Parity | Self | Memory |
| --- | ---: | ---: | ---: | --- | --- | --- |
| `generate.surface-large` | 39979279.734 | 0.648x | 4 MiB | `Passed` | `Unseeded` | `Passed` |
| `generate.color-large` | 42793588.067 | 0.683x | 4 MiB | `Passed` | `Unseeded` | `Passed` |
| `convert.sparse-r8-wide` | 1580971.526 | 0.044x | 3 MiB | `Passed` | `Unseeded` | `Passed` |
| `convert.dense-b8-to-01` | 929680.223 | 0.122x | 3 MiB | `Passed` | `Unseeded` | `Passed` |
| `convert.typed-dets-wide` | 1978044.479 | 0.229x | 3 MiB | `Passed` | `Unseeded` | `Passed` |
| `sample-surface.medium` | 3065341.695 | 0.770x | 3 MiB | `Passed` | `Unseeded` | `Passed` |
| `sample-surface.large` | 1135967.487 | 0.809x | 3 MiB | `Passed` | `Unseeded` | `Passed` |
| `sample-surface.d25-b8` | 98956.603 | 0.922x | 6 MiB | `Passed` | `Unseeded` | `Passed` |
| `sample-folded-ptb64.large` | 1655572.927 | 0.766x | 3 MiB | `Passed` | `Unseeded` | `Passed` |
| `detect-observables.medium` | 2858263.354 | 0.866x | 3 MiB | `Passed` | `Unseeded` | `Passed` |
| `detect-observables.large` | 1103554.707 | 0.804x | 3 MiB | `Passed` | `Unseeded` | `Passed` |
| `detect-observables.d11-b8` | 1445143.300 | 0.805x | 3 MiB | `Passed` | `Unseeded` | `Passed` |
| `detect-observables.d11-sparse-hits` | 1047745.482 | 0.344x | 3 MiB | `Passed` | `Unseeded` | `Passed` |
| `detect-observables.color-d9-b8` | 4380512.980 | 0.806x | 3 MiB | `Passed` | `Unseeded` | `Passed` |
| `m2d.sweep-medium` | 3670740.986 | 0.885x | 3 MiB | `Passed` | `Unseeded` | `Passed` |
| `m2d.sweep-large` | 3497952.447 | 0.793x | 3 MiB | `Passed` | `Unseeded` | `Passed` |
| `m2d.surface-d11` | 1963115.931 | 0.486x | 3 MiB | `Passed` | `Unseeded` | `Passed` |
| `analyze-folded.large` | 14434449.809 | 0.846x | 5 MiB | `Passed` | `Unseeded` | `Passed` |
| `analyze-decompose.d15` | 564832.366 | 0.560x | 25 MiB | `Passed` | `Unseeded` | `Passed` |
| `analyze-decompose.d25` | 310054.565 | 0.598x | 105 MiB | `Passed` | `Unseeded` | `Passed` |
| `sample-dem.medium` | 919575.222 | 0.961x | 4 MiB | `Passed` | `Unseeded` | `Passed` |
| `sample-dem.large` | 332936.346 | 0.924x | 5 MiB | `Passed` | `Unseeded` | `Passed` |
| `sample-dem.surface-d11-side-outputs` | 144650.168 | 0.460x | 17 MiB | `Passed` | `Unseeded` | `Passed` |
| `qec-cli-pipeline.large` | 663518.356 | 0.717x | 4 MiB | `Passed` | `Unseeded` | `Passed` |
| `qec-rust-pipeline.large` | 16035609.349 | n/a | 3 MiB | `NotApplicable` | `Unseeded` | `Passed` |
