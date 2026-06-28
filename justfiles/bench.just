# Validate benchmark contracts without running long benchmark workloads.
[working-directory: '..']
smoke *args:
    cargo run -q -p stab-bench -- smoke {{args}}

# Print planned benchmark contracts grouped by milestone and threshold class.
[working-directory: '..']
list *args:
    cargo run -q -p stab-bench -- list {{args}}

# Record pinned C++ Stim baseline benchmark results.
[working-directory: '..']
baseline *args:
    cargo run -q -p stab-bench -- baseline {{args}}

# Report planned Stab-vs-Stim benchmark comparisons.
[working-directory: '..']
compare *args:
    cargo run -q -p stab-bench --profile release -- compare {{args}}

# Report Stab-vs-Stim comparisons with Stab-side allocation tracking enabled.
[working-directory: '..']
compare-allocations *args:
    cargo run -q -p stab-bench --features count-allocations --profile release -- compare --track-allocations {{args}}

# Check source-owned M12 primary timing-regression thresholds.
[working-directory: '..']
primary-regression *args:
    cargo run -q -p stab-bench --profile release -- compare --primary --warmup --measurement-runs 3 --thresholds benchmarks/m12-primary-thresholds.json --require-profiler-notes --profiler-notes-dir benchmarks/profiler-notes/m12 {{args}}

# Check the M12 primary beta timing gate with source-owned contract-only waivers.
[working-directory: '..']
primary-beta *args:
    cargo run -q -p stab-bench --profile release -- compare --primary --warmup --measurement-runs 3 --require-beta-gate --beta-waivers benchmarks/m12-primary-beta-waivers.json --require-profiler-notes --profiler-notes-dir benchmarks/profiler-notes/m12 --report target/benchmarks/m12-primary-beta {{args}}

# Check source-owned M12 primary memory-regression baseline.
[working-directory: '..']
primary-memory-regression *args:
    cargo run -q -p stab-bench --features count-allocations --profile release -- compare --track-allocations --primary --require-memory-gate --memory-baseline benchmarks/m12-primary-memory-baseline.json --require-profiler-notes --profiler-notes-dir benchmarks/profiler-notes/m12 --report target/benchmarks/m12-primary-memory-regression {{args}}
