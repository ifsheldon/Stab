# Validate benchmark contracts without running long benchmark workloads.
[working-directory: '..']
smoke *args:
    cargo run -q -p stab-bench -- smoke {{args}}

# Print planned benchmark contracts grouped by milestone and threshold class.
[working-directory: '..']
list *args:
    cargo run -q -p stab-bench -- list {{args}}

# Validate the comprehensive performance qualification ledger.
[working-directory: '..']
qualification-check *args:
    cargo run -q -p stab-bench -- qualification-check {{args}}

# List comprehensive performance qualification coverage.
[working-directory: '..']
qualification-list *args:
    cargo run -q -p stab-bench -- qualification-list {{args}}

# Regenerate the comprehensive performance qualification ledger.
[working-directory: '..']
qualification-regenerate *args:
    cargo run -q -p stab-bench -- qualification-regenerate {{args}}

# Reproduce one bounded paired-runner or product-adapter qualification probe.
[working-directory: '..']
qualification-probe *args:
    cargo run -q -p stab-bench --profile release -- qualification-probe {{args}}

# Execute one source-owned paired qualification group and publish validated evidence.
[working-directory: '..']
qualification-run *args:
    cargo run -q -p stab-bench --profile release -- qualification-run {{args}}

# Validate a paired qualification report and regenerate its derived artifacts.
[working-directory: '..']
qualification-report *args:
    cargo run -q -p stab-bench --profile release -- qualification-report {{args}}

# Check promotable qualification evidence against source-owned thresholds.
[working-directory: '..']
qualification-regression *args:
    cargo run -q -p stab-bench --profile release -- qualification-regression {{args}}

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
    cargo run -q -p stab-bench --profile release -- compare --primary --warmup --measurement-runs 3 --thresholds benchmarks/m12-primary-thresholds.json --regression-waivers benchmarks/m12-primary-regression-waivers.json --require-profiler-notes --profiler-notes-dir benchmarks/profiler-notes/m12 {{args}}

# Check the M12 primary beta timing gate with source-owned contract-only waivers.
[working-directory: '..']
primary-beta *args:
    cargo run -q -p stab-bench --profile release -- compare --primary --warmup --measurement-runs 3 --require-beta-gate --beta-waivers benchmarks/m12-primary-beta-waivers.json --require-profiler-notes --profiler-notes-dir benchmarks/profiler-notes/m12 --report target/benchmarks/m12-primary-beta {{args}}

# Check source-owned M12 primary memory-regression baseline.
[working-directory: '..']
primary-memory-regression *args:
    cargo run -q -p stab-bench --features count-allocations --profile release -- compare --track-allocations --primary --require-memory-gate --memory-baseline benchmarks/m12-primary-memory-baseline.json --require-profiler-notes --profiler-notes-dir benchmarks/profiler-notes/m12 --report target/benchmarks/m12-primary-memory-regression {{args}}
