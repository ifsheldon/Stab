# Stab E2E Performance Report

- Tier: `full`
- Source: `a8b56db319410f1d52bc64bfb7ee6a63c01c490f`
- Architecture: `aarch64`
- CPU: `AI TOP ATOM (0xd85,0xd87)`
- Stim parity: `Passed`
- Stab self-regression: `Unseeded`
- Memory: `Passed`

| Case | Stab throughput | Stim ratio | Peak RSS | Parity | Self | Memory |
| --- | ---: | ---: | ---: | --- | --- | --- |
| `generate-surface.medium` | 5351545.803 | 0.865x | 4 MiB | `Passed` | `Unseeded` | `Passed` |
| `generate-surface.large` | 34091336.918 | 0.733x | 4 MiB | `Passed` | `Unseeded` | `Passed` |
| `convert-sparse.hits-narrow` | 314706.066 | 0.824x | 8 MiB | `Passed` | `Unseeded` | `Passed` |
| `convert-sparse.r8-wide` | 86554.994 | 0.813x | 24 MiB | `Passed` | `Unseeded` | `Passed` |
| `convert-dense.01-to-b8` | 1072037.182 | 0.673x | 24 MiB | `Passed` | `Unseeded` | `Passed` |
| `convert-dense.b8-to-01` | 529053.101 | 0.210x | 31 MiB | `Passed` | `Unseeded` | `Passed` |
| `convert-typed-dets.narrow` | 1396554.472 | 1.014x | 31 MiB | `Passed` | `Unseeded` | `Passed` |
| `convert-typed-dets.wide` | 521217.901 | 0.877x | 31 MiB | `Passed` | `Unseeded` | `Passed` |
| `sample-surface.small` | 2685889.311 | 0.880x | 31 MiB | `Passed` | `Unseeded` | `Passed` |
| `sample-surface.medium` | 2617536.729 | 0.929x | 31 MiB | `Passed` | `Unseeded` | `Passed` |
| `sample-surface.large` | 947191.748 | 0.946x | 31 MiB | `Passed` | `Unseeded` | `Passed` |
| `sample-folded-ptb64.medium` | 2229268.510 | 0.915x | 31 MiB | `Passed` | `Unseeded` | `Passed` |
| `sample-folded-ptb64.large` | 1149388.034 | 1.081x | 31 MiB | `Passed` | `Unseeded` | `Passed` |
| `detect-observables.small` | 2220123.993 | 1.097x | 31 MiB | `Passed` | `Unseeded` | `Passed` |
| `detect-observables.medium` | 2121605.386 | 1.118x | 31 MiB | `Passed` | `Unseeded` | `Passed` |
| `detect-observables.large` | 819764.982 | 1.018x | 31 MiB | `Passed` | `Unseeded` | `Passed` |
| `m2d-packed-sweep.small` | 2185384.759 | 0.948x | 31 MiB | `Passed` | `Unseeded` | `Passed` |
| `m2d-packed-sweep.medium` | 3190423.122 | 1.120x | 31 MiB | `Passed` | `Unseeded` | `Passed` |
| `m2d-packed-sweep.large` | 2603852.125 | 1.064x | 31 MiB | `Passed` | `Unseeded` | `Passed` |
| `analyze-folded.small` | 2264755.903 | 0.885x | 31 MiB | `Passed` | `Unseeded` | `Passed` |
| `analyze-folded.medium` | 7259207.313 | 0.947x | 31 MiB | `Passed` | `Unseeded` | `Passed` |
| `analyze-folded.large` | 10911871.585 | 1.065x | 31 MiB | `Passed` | `Unseeded` | `Passed` |
| `sample-dem.small` | 2465595.433 | 0.913x | 31 MiB | `Passed` | `Unseeded` | `Passed` |
| `sample-dem.medium` | 780993.561 | 1.138x | 31 MiB | `Passed` | `Unseeded` | `Passed` |
| `sample-dem.large` | 289951.961 | 1.063x | 49 MiB | `Passed` | `Unseeded` | `Passed` |
| `qec-cli-pipeline.medium` | 757336.933 | 0.924x | 49 MiB | `Passed` | `Unseeded` | `Passed` |
| `qec-cli-pipeline.large` | 521785.168 | 0.998x | 49 MiB | `Passed` | `Unseeded` | `Passed` |
| `qec-rust-pipeline.medium` | 12839340.542 | n/a | 49 MiB | `NotApplicable` | `Unseeded` | `Passed` |
| `qec-rust-pipeline.large` | 13021400.689 | n/a | 49 MiB | `NotApplicable` | `Unseeded` | `Passed` |
