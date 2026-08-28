# Stab E2E Performance Report

- Tier: `soak`
- Source: `a8b56db319410f1d52bc64bfb7ee6a63c01c490f`
- Architecture: `aarch64`
- CPU: `AI TOP ATOM (0xd85,0xd87)`
- Stim parity: `Passed`
- Stab self-regression: `Unseeded`
- Memory: `Passed`

| Case | Stab throughput | Stim ratio | Peak RSS | Parity | Self | Memory |
| --- | ---: | ---: | ---: | --- | --- | --- |
| `generate-surface.medium` | 6132887.199 | 0.827x | 4 MiB | `Passed` | `Unseeded` | `Passed` |
| `generate-surface.large` | 38022360.188 | 0.710x | 4 MiB | `Passed` | `Unseeded` | `Passed` |
| `convert-sparse.hits-narrow` | 323495.112 | 0.823x | 8 MiB | `Passed` | `Unseeded` | `Passed` |
| `convert-sparse.r8-wide` | 89095.053 | 0.808x | 24 MiB | `Passed` | `Unseeded` | `Passed` |
| `convert-dense.01-to-b8` | 1196162.468 | 0.637x | 24 MiB | `Passed` | `Unseeded` | `Passed` |
| `convert-dense.b8-to-01` | 644293.309 | 0.176x | 32 MiB | `Passed` | `Unseeded` | `Passed` |
| `convert-typed-dets.narrow` | 1495167.171 | 1.017x | 32 MiB | `Passed` | `Unseeded` | `Passed` |
| `convert-typed-dets.wide` | 524228.213 | 0.882x | 32 MiB | `Passed` | `Unseeded` | `Passed` |
| `sample-surface.small` | 2713995.497 | 0.914x | 32 MiB | `Passed` | `Unseeded` | `Passed` |
| `sample-surface.medium` | 2796258.502 | 0.914x | 32 MiB | `Passed` | `Unseeded` | `Passed` |
| `sample-surface.large` | 991422.837 | 0.971x | 32 MiB | `Passed` | `Unseeded` | `Passed` |
| `sample-folded-ptb64.medium` | 2211054.191 | 0.977x | 32 MiB | `Passed` | `Unseeded` | `Passed` |
| `sample-folded-ptb64.large` | 1175650.712 | 1.124x | 32 MiB | `Passed` | `Unseeded` | `Passed` |
| `detect-observables.small` | 2505802.622 | 0.981x | 32 MiB | `Passed` | `Unseeded` | `Passed` |
| `detect-observables.medium` | 2282161.144 | 1.171x | 32 MiB | `Passed` | `Unseeded` | `Passed` |
| `detect-observables.large` | 908782.106 | 1.082x | 32 MiB | `Passed` | `Unseeded` | `Passed` |
| `m2d-packed-sweep.small` | 2699144.261 | 0.930x | 32 MiB | `Passed` | `Unseeded` | `Passed` |
| `m2d-packed-sweep.medium` | 3517901.489 | 1.112x | 32 MiB | `Passed` | `Unseeded` | `Passed` |
| `m2d-packed-sweep.large` | 2817710.923 | 1.101x | 32 MiB | `Passed` | `Unseeded` | `Passed` |
| `analyze-folded.small` | 2720588.204 | 0.879x | 32 MiB | `Passed` | `Unseeded` | `Passed` |
| `analyze-folded.medium` | 7961213.196 | 0.934x | 32 MiB | `Passed` | `Unseeded` | `Passed` |
| `analyze-folded.large` | 12364049.088 | 1.098x | 32 MiB | `Passed` | `Unseeded` | `Passed` |
| `sample-dem.small` | 3046970.660 | 0.907x | 32 MiB | `Passed` | `Unseeded` | `Passed` |
| `sample-dem.medium` | 837527.360 | 1.153x | 32 MiB | `Passed` | `Unseeded` | `Passed` |
| `sample-dem.large` | 317899.430 | 1.020x | 47 MiB | `Passed` | `Unseeded` | `Passed` |
| `qec-cli-pipeline.medium` | 788763.509 | 0.938x | 47 MiB | `Passed` | `Unseeded` | `Passed` |
| `qec-cli-pipeline.large` | 518723.237 | 1.011x | 47 MiB | `Passed` | `Unseeded` | `Passed` |
| `qec-rust-pipeline.medium` | 12927997.273 | n/a | 47 MiB | `NotApplicable` | `Unseeded` | `Passed` |
| `qec-rust-pipeline.large` | 12994226.892 | n/a | 47 MiB | `NotApplicable` | `Unseeded` | `Passed` |
