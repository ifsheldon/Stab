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
