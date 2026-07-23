# OpenAI Build Week Submission Plan

This document records the positioning, evidence, and action plan for the OpenAI Build Week Challenge submission.
Key facts: submissions are due July 21 at 5:00 PM PT; the track is Developer tools; judging criteria are Technological Implementation (skillful Codex use), Design (complete product experience), Potential Impact (real problem, real audience), and Quality of the Idea.
Submission components: a working project built with Codex and GPT-5.6, a project description, a public YouTube demo video under three minutes with audio covering how Codex and GPT-5.6 were used, a public repository URL with setup instructions and sample data, a way for judges to test without rebuilding from scratch, and the Codex `/feedback` session ID recorded on the submission form.

## Positioning

Thesis: Stab is agent-native infrastructure for quantum error correction research, a safe-Rust codebase that QEC researchers and their AI agents can safely modify and extend, starting from selected implemented surfaces with pinned Stim v1.16.0 compatibility evidence.

Focused audience: the QEC researcher who works with an LLM agent and whose next result needs simulator behavior that Stim does not have, such as new noise models, exotic feedback, custom decoders, or tight tool integration.
Today that researcher must fork or reimplement a hand-tuned C++ simulator, and an agent editing manual-memory, architecture-specific vectorized C++ can introduce silent corruption that produces wrong science rather than crashes.

Supporting pillars, in priority order:

1. Strict compiler guardrails for agent modification. Rust's ownership, lifetime, and exhaustiveness checking converts human review burden into machine checking; the agent absorbs the borrow checker and the scientist reviews code that already passed strict checks. This repository is the exhibit: Codex built Stab inside a compiler, oracle, and qualification-receipt loop.
2. The guardrails ship with the repository. The compatibility matrix, oracle fixture corpus, and correctness and performance qualification harnesses are committed here, so anyone who forks or modifies Stab can re-run the same evidence checks on their own changes.
3. Performance-portable vectorization. `std::simd` portable kernels give one safe, readable vectorized source that targets x86-64 and AArch64 alike, instead of per-architecture hand-tuned C++ SIMD template paths that only specialists can safely touch. Per-kernel performance against pinned Stim is measured and published by the qualification harness, including the rows that still need work.
4. Compatibility first, composable components next. Byte-level Stim parity is the trust bootstrap, not the ceiling. The declared direction is modular Rust components for QEC tooling (bit kernels, stabilizer algebra, circuit and detector-error-model formats, samplers) that researchers can reuse in their own tools.

## Message Discipline

Claims to make, because they are verified or documented:

- `stab gen` produces byte-identical circuits to `stim gen` for the same arguments, including the comment header.
- The implemented CLI surface includes `gen`, `convert`, `sample`, `detect`, `m2d`, `analyze_errors`, and `sample_dem`, and accepts the selected `.stim`, `.dem`, `01`, `b8`, `r8`, `hits`, `dets`, and `ptb64` format surfaces recorded in the checklist.
- The exact implemented scope and the deliberate deferrals (Python bindings, WASM, diagrams, `explain_errors`) are recorded in `docs/stab-feature-checklist.md`.
- Selected compatibility evidence is machine-checkable and committed, with cryptographic provenance on qualification reports; reopened result-format claims are identified explicitly and are not presented as qualification-complete.

Claims to avoid, because a judge can falsify them:

- Do not say Stim contains hand-written assembly. It is hand-tuned C++ with width-specialized SIMD template paths. The danger is modification, not use; Stim is famously well tested.
- Do not claim `analyze_errors` output is byte-identical to Stim's. Detector error model probabilities differ at about the seventeenth significant digit from floating-point combination order; the outputs are semantically equivalent.
- Do not claim blanket performance parity or superiority. Cite only ratios from committed qualification reports, and present the rows above 1.25x as published known gaps.
- Do not imply the component ecosystem already exists. `stab-core` is one crate today; modularization is the declared next direction.
- Do not demonstrate `stab sample` or `stab detect` at large distances. Informal local measurement (not qualification evidence): 100 shots took 0.07s at distance 5, 2.5s at distance 9, 8.2s at distance 11, and over a minute at distance 15, where pinned Stim finishes 1k shots in 12ms. Keep circuit-level sampling at distance 3 to 7 in the demo, and use the `analyze_errors` plus `sample_dem` path for the realistic-scale beat; `sample_dem` handles distance 15 instantly and enforces the documented 64-million sampled-error bound by design.

## Demo Video Plan (target 2:45)

- 0:00 to 0:25. Hook: a QEC researcher asks their agent for a simulator feature Stim does not have; today that means editing hand-tuned C++; Stab exists so the agent works in safe Rust with guardrails.
- 0:25 to 0:45. What Stab is: `stab --help` on screen; a Rust implementation targeting Stim v1.16.0 CLI and format compatibility as milestone one, with selected implemented surfaces already backed by pinned evidence.
- 0:45 to 1:35. Live demo: `stab gen` a rotated surface code circuit, then `diff` against `stim gen` output to show byte-identical circuits; then the quickstart pipeline at distance 3 (`sample`, `detect`, `m2d`, `analyze_errors`, `sample_dem`, `convert`).
- 1:35 to 2:05. The differentiator: the committed compatibility matrix, `just oracle::list`, and one qualification report directory with its digest-bound receipts. Codex did not just write the simulator; it wrote the machinery that proves the simulator matches, and that machinery ships with the repo.
- 2:05 to 2:20. Scale beat: `analyze_errors` at distance 15 plus instant `sample_dem`.
- 2:20 to 2:45. How Codex and GPT-5.6 were used (required audio): plan-first workflow from `docs/plans/`, agent operating rules in `AGENTS.md`, key decisions (portable-SIMD kernels, streaming CLI architecture, evidence-first qualification), and the `/feedback` session ID on screen.

## Devpost Description Draft

> Stab is agent-native infrastructure for quantum error correction (QEC) research: a safe-Rust codebase that researchers and their AI agents can safely modify and extend, starting from selected implemented surfaces with pinned compatibility evidence against Stim v1.16.0, the simulator the field uses to design quantum error-correcting codes.
>
> QEC researchers increasingly work with LLM agents, but the field's standard simulator is hand-tuned C++ with architecture-specific vectorized paths. An agent editing that code can introduce silent memory corruption that produces wrong science, not crashes. Stab reimplements a growing selected Stim surface in safe Rust with strict compiler guardrails, including `.stim` and `.dem` handling, core CLI workflows, and standard result formats. The checked feature inventory distinguishes implementation from completed pinned qualification instead of presenting the unfinished surface as universal parity.
>
> Compatibility is proven, not promised, and the proof ships with the repo: a committed compatibility matrix against the pinned Stim v1.16.0 sources, an oracle fixture corpus that executes both implementations, and machine-checkable correctness and performance qualification reports with cryptographic provenance. When a researcher's agent modifies Stab, the same harness re-validates the fork. Performance-portable `std::simd` kernels replace per-architecture hand-tuning, with per-kernel ratios measured and published, including known gaps.
>
> The entire implementation was built with OpenAI Codex, and the test, oracle, and benchmark qualification harnesses were built with GPT-5.6, working plan-first from committed milestone documents (`docs/plans/`) and agent operating rules (`AGENTS.md`). The Codex `/feedback` session ID is provided in this submission form.
>
> Try it in two minutes: prebuilt binaries are attached to the repository's Releases, the README quickstart runs the full pipeline (`gen`, `sample`, `detect`, `analyze_errors`, `sample_dem`, `convert`) against the sample data in `examples/`, and `docs/stab-feature-checklist.md` records the exact implemented scope. Qualified on Linux x86-64 and AArch64.

## Repository State And Commit Plan

The working tree contains a large interrupted change set from the Codex session that stopped on credit outage: 53 files changed (about 3900 insertions, 2728 deletions) plus new files under `ops/bench/`, covering PQ2 Clifford qualification hardening.
Assessment on 2026-07-20: `cargo fmt --check` and `cargo clippy --workspace --all-targets` are clean, and the workspace test suite passes except one new test, `qualification::runtime::completion::tests::publication::completion_actions_use_retained_root_during_swap_and_restore`.

Root cause of the failure: the new descriptor-root repository view builds paths of the form `/proc/<pid>/fd/<n>`, but `GitView` reads `.git` files through the hardened component-wise opener in `ops/bench/src/source_file.rs`, which opens every component with `O_NOFOLLOW` and therefore fails with `ELOOP` on the `/proc/<pid>/fd/<n>` magic symlink.
The fix is real qualification-harness work (dirfd-relative Git access from the retained repository descriptor) and should not be rushed before the submission.

Commit plan (presentation and interrupted work stay separate):

```sh
# 1. Commit the presentation changes on main.
git add README.md examples/ docs/build-week-submission.md
git commit -m "docs: add Build Week quickstart, sample data, and submission plan"

# 2. Preserve the interrupted work on a local branch, uncommitted tests included.
git checkout -b wip/pq2-clifford-qualification
git add -A
git commit -m "wip(bench): park interrupted PQ2 Clifford qualification hardening

Interrupted agent session. Known failure: the descriptor-root
/proc/<pid>/fd/<n> repository view is unreadable by the O_NOFOLLOW
component-wise source opener (ELOOP), which breaks
completion_actions_use_retained_root_during_swap_and_restore.
Resume here; do not publish."
git checkout main
```

Publish only `main`. The `wip/pq2-clifford-qualification` branch may stay local or be pushed marked as work in progress; judges evaluate `main`, where the full test suite must be green.

Before recording the demo, rebuild the release binary from the published commit and regenerate `examples/` with it, so the video, the binaries attached to Releases, and the published source are exactly the same code.

## Action Checklist

- [x] Verify clean-HEAD workspace tests are green (verified 2026-07-20: every workspace test target passes at da7c787d).
- [ ] Commit presentation changes on `main`; park interrupted work on `wip/pq2-clifford-qualification`.
- [ ] Rebuild release binary from the published commit; regenerate `examples/` with it.
- [ ] Make the repository public; confirm `vendor/stim` submodule URL resolves anonymously.
- [ ] Create a GitHub Release with the prebuilt `stab` binary for Linux AArch64 (decided: AArch64 only for this submission; other platforms build from source with `cargo install`).
- [ ] Record the video per the shot list; upload public YouTube.
- [ ] Submit the Devpost form: description above, Developer tools track, repository URL, video URL, Codex `/feedback` session ID, testing instructions pointing to the README quickstart and the Release binaries.
- [ ] Keep the repository and video public through the judging period ending August 7.
- [ ] After the deadline: update `docs/plans/rust-stim-drop-in-rewrite.md` with the composable-components direction, and resume `wip/pq2-clifford-qualification` with the root-cause note above.
