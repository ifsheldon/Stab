# Stab E2E Performance Report

- Tier: `full`
- Source: `b4a758db169fd343c93f2b84a5b1d68558c9e6c3`
- Architecture: `aarch64`
- CPU: `AI TOP ATOM (0xd85,0xd87)`
- Stim parity: `Passed`
- Stab self-regression: `Passed`
- Memory: `Passed`

| Case | Stab throughput | Stim ratio | Peak RSS | Parity | Self | Memory |
| --- | ---: | ---: | ---: | --- | --- | --- |
| `generate-surface.medium` | 6639774.888 | 0.831x | 4 MiB | `Passed` | `Passed` | `Passed` |
| `generate-surface.large` | 39648450.510 | 0.708x | 4 MiB | `Passed` | `Passed` | `Passed` |
| `convert-sparse.hits-narrow` | 327116.574 | 0.821x | 8 MiB | `Passed` | `Passed` | `Passed` |
| `convert-sparse.r8-wide` | 89690.295 | 0.799x | 24 MiB | `Passed` | `Passed` | `Passed` |
| `convert-dense.01-to-b8` | 1205366.530 | 0.635x | 24 MiB | `Passed` | `Passed` | `Passed` |
| `convert-dense.b8-to-01` | 725099.360 | 0.157x | 31 MiB | `Passed` | `Passed` | `Passed` |
| `convert-typed-dets.narrow` | 1545585.431 | 1.004x | 31 MiB | `Passed` | `Passed` | `Passed` |
| `convert-typed-dets.wide` | 531631.455 | 0.874x | 31 MiB | `Passed` | `Passed` | `Passed` |
| `sample-surface.small` | 2946726.551 | 0.925x | 31 MiB | `Passed` | `Passed` | `Passed` |
| `sample-surface.medium` | 2935879.769 | 0.918x | 31 MiB | `Passed` | `Passed` | `Passed` |
| `sample-surface.large` | 1048810.397 | 0.978x | 31 MiB | `Passed` | `Passed` | `Passed` |
| `sample-folded-ptb64.medium` | 2186931.376 | 1.004x | 31 MiB | `Passed` | `Passed` | `Passed` |
| `sample-folded-ptb64.large` | 1204717.044 | 1.129x | 31 MiB | `Passed` | `Passed` | `Passed` |
| `detect-observables.small` | 2661731.373 | 0.991x | 31 MiB | `Passed` | `Passed` | `Passed` |
| `detect-observables.medium` | 2441559.200 | 1.179x | 31 MiB | `Passed` | `Passed` | `Passed` |
| `detect-observables.large` | 919150.441 | 1.084x | 31 MiB | `Passed` | `Passed` | `Passed` |
| `m2d-packed-sweep.small` | 2887435.145 | 0.941x | 31 MiB | `Passed` | `Passed` | `Passed` |
| `m2d-packed-sweep.medium` | 3498987.826 | 1.103x | 31 MiB | `Passed` | `Passed` | `Passed` |
| `m2d-packed-sweep.large` | 2849248.099 | 1.106x | 31 MiB | `Passed` | `Passed` | `Passed` |
| `analyze-folded.small` | 2662641.679 | 0.893x | 31 MiB | `Passed` | `Passed` | `Passed` |
| `analyze-folded.medium` | 8018231.591 | 0.943x | 31 MiB | `Passed` | `Passed` | `Passed` |
| `analyze-folded.large` | 12251846.958 | 1.109x | 31 MiB | `Passed` | `Passed` | `Passed` |
| `sample-dem.small` | 2832453.724 | 0.911x | 31 MiB | `Passed` | `Passed` | `Passed` |
| `sample-dem.medium` | 836204.981 | 1.161x | 31 MiB | `Passed` | `Passed` | `Passed` |
| `sample-dem.large` | 320732.021 | 1.014x | 49 MiB | `Passed` | `Passed` | `Passed` |
| `qec-cli-pipeline.medium` | 795297.647 | 0.934x | 49 MiB | `Passed` | `Passed` | `Passed` |
| `qec-cli-pipeline.large` | 533071.136 | 1.021x | 49 MiB | `Passed` | `Passed` | `Passed` |
| `qec-rust-pipeline.medium` | 12929282.726 | n/a | 49 MiB | `NotApplicable` | `Passed` | `Passed` |
| `qec-rust-pipeline.large` | 13046588.810 | n/a | 49 MiB | `NotApplicable` | `Passed` | `Passed` |
