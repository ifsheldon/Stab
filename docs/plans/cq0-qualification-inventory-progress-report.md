# CQ0 Qualification Inventory Progress Report

## Status

CQ0 is complete after CQ1-driven exact-selector and canonical-owner corrections plus the completed selected CQ2 `.stim`, `.dem`, and result-format ownership slices.

Original source-owned evidence revision: `02c93c19566bdc465ad9c795f35e956e1ff85440` with `local_modifications=false`.

Corrected inventory execution evidence revision: `e7ba513822c26859a2b5c70c94d406e1c6adb6b6` with `local_modifications=false`.

Compatibility target: Stim v1.16.0 at commit `e2fc1eca7fd21684d433aa5f10f4504ea4860d07`.

Current corrected schema-version-3 manifest digest: `adcc3d19605e4fc7bd9e1b3f3373ddf38d81301430d891f062baaab0c6fdc8f6`.

Pinned isolated Python AST version: 3.14.6.

This milestone freezes a finite source and API inventory; it does not claim that the 3,223 planned CQ2 through CQ5 evidence owners already pass.

## Inventory

| Inventory | Count | Notes |
| --- | ---: | --- |
| C++ test files read | 103 | Read from pinned `vendor/stim/file_lists/test_files`; 102 files contain selected extractor declarations. |
| Python test files read | 91 | Listed from the pinned Stim Git tree without importing test modules. |
| Direct C++ cases | 1,877 | Includes explicit 64-bit, 128-bit, and 256-bit expansion of every `TEST_EACH_WORD_SIZE_W` declaration. |
| Direct Python semantic records | 844 | Includes 727 unparameterized cases, 94 statically expanded parameter subcases, and 23 dynamic parameter families. |
| Exact blocker-ledger subcases | 165 | References source-owned blocker ids without copying selector payloads. |
| Total upstream records | 2,886 | One exact source record can be relevant to multiple CQ domains. |
| Multi-domain relevance records | 651 | Primarily command plus engine behavior and mixed semantic methods; relevance does not itself confer passing evidence. |
| Dynamic parameter families in executable scope | 0 | All 23 dynamic families are content-addressed, visible, and non-executable. |
| Default-feature public API items | 1,922 | Includes re-exports, variants, enum payload fields, public struct fields, inherent methods, trait methods, and explicit non-synthetic, non-blanket trait implementations. |
| Evidence owners | 3,716 | 2,383 upstream semantic owners, 685 public Rust API owners, 389 oracle-fixture owners, 165 blocker cases, 93 qualification-plan owners, and one hostile-path regression; 51 oracle fixtures are retained as supporting provenance on canonical blocker or qualification parents instead of duplicating terminal selectors. |

### Upstream Dispositions

| Disposition | Count |
| --- | ---: |
| `ported-rust` | 165 |
| `semantic-mining` | 2,051 |
| `deferred-product` | 659 |
| `not-applicable` | 11 |
| `exact-oracle` | 0 |
| `superseded` | 0 |

The 659 deferred records name one of twelve typed products: Crumble 5, deprecated detector hypergraph 1, diagrams 88, `explain_errors` 2, interactive simulators 75, Python bindings 87, QASM 10, Quirk 4, sinter 118, stimcirq 117, stimflow 113, and ZX or lattice-surgery integrations 39.
Of those deferred records, 182 remain relevant to at least one CQ domain summary, but no deferred record owns executable evidence or contributes a passing case.

### Evidence Status

| Status | Count |
| --- | ---: |
| `implemented` | 476 |
| `evidence-close` | 17 |
| `planned` | 3,223 |
| `deferred` | 0 |

The 493 implemented or evidence-close owners establish that every CQ domain has at least one exact primary case; they do not close the remaining planned owners.
All 440 implemented fixture-manifest rows are represented: 247 own exact existing oracle-fixture primary cases, 142 retain broad inherited Cargo filters only as supporting evidence behind planned atomic oracle-fixture selectors, and 51 are supporting provenance on canonical blocker-ledger or qualification parents.

### Comparator Inventory

| Comparator | Count |
| --- | ---: |
| `canonical` | 34 |
| `error-class` | 19 |
| `exact-bytes` | 283 |
| `exact-value` | 97 |
| `property` | 1,319 |
| `resource` | 14 |
| `semantic-invariant` | 432 |
| `state-equivalence` | 631 |
| `statistical` | 560 |
| `structural` | 327 |

All 560 statistical rows have typed plan references: 14 reference blocker-ledger plans, 18 reference oracle-fixture plans, and 528 planned rows reference their future qualification-case owner.
The cross-cutting qualification rows comprise one implemented symlink resource regression, one implemented registered property-worker contract, and thirteen independently planned boundary families for parser admission, checked arithmetic, result records, materialization, streaming buffers, writer and visitor failures, replay and side inputs, traversal, search, allocation, typed paths, and output lifecycle.
Atomic semantic cases do not inherit feature-wide resource or negative claims.

## Extraction Contract

The bounded C++ extractor recognizes `TEST`, `TEST_F`, `TYPED_TEST`, and `TEST_EACH_WORD_SIZE_W` tokens only at identifier boundaries after masking comments, quoted literals, character literals, and raw strings.
It requires identifier suite and test names, records declaration lines, rejects malformed selected macros, and expands the Stim word-size macro into `W=64`, `W=128`, and `W=256` records independently of the host C++ preprocessing target.

The Python extractor passes at most 16 MiB across at most 512 files to `uv run --no-project --python 3.14.6 python -I` and uses only Python's standard-library AST.
The child enforces 2,048 bytes per output field and a one-MiB cumulative encoded-record budget before retaining each record, bounds parameter names before expansion, and yields Cartesian products lazily, preventing bounded source text from amplifying into unbounded repeated symbols or parameter ids.
It records top-level and class-owned synchronous or asynchronous `test_*` functions, skips nested helper functions by construction, and never imports or executes a pinned test module.
Literal collections, literal `range` calls, supported dictionary keys, `itertools.product`, and stacked `pytest.mark.parametrize` decorators expand into bounded deterministic subcases; unsupported expressions produce one content-addressed `dynamic-family` record that validation rejects from executable scope.

The public API extractor runs default-feature rustdoc JSON for `stab-core` and `stab-cli` into fresh securely created operating-system temporary target directories for an explicitly validated host target, caps each JSON artifact at 32 MiB, traverses named and glob re-exports with cycle and traversal-work guards, and rejects duplicate canonical identities.
Trait implementation identities hash canonical trait and implementing-type structure after removing rustdoc-local numeric ids.
Compiler-generated synthetic, blanket, negative, and doc-hidden items are excluded; enum tuple and named payload fields are included under their enum parent contract.
Every API item is classified from an explicit source and canonical path rule, an unknown item is a regeneration error instead of falling into `CQ-RESOURCE`, and known `ops-contracts` exports appearing in default-feature rustdoc are rejected before hidden-item filtering.

## Manifest Contract

The checked manifest separates domain relevance from executable ownership and separates behavioral surface from evidence provenance.
Typed case ids, API paths, repository-relative source paths, fixed-size lowercase semantic digests, bounded strings, bounded arrays, and capped validation diagnostics reject malformed or oversized checked input before semantic digest cloning or selector execution.
Every existing Cargo primary selector must name one concrete libtest case with `--exact`, imported oracle and blocker ids must remain valid, every implemented fixture must have primary or supporting ownership, shared terminal selectors are rejected, and deterministic regeneration must match the checked bytes and frozen digest.
Broad inherited Cargo filters remain useful as supporting evidence, but they cannot close a qualification owner or contribute a pass.

## Exact-Selector Correction

CQ1 execution exposed that the original CQ0 validator checked whether a Cargo filter resolved but did not prove that it selected exactly one test.
The original inventory therefore counted 164 umbrella fixture filters as implemented atomic evidence.
The correction resolved and promoted 80 fixture selectors to exact concrete libtest names, retained 164 broad filters only as supporting evidence behind planned oracle-fixture primaries, and made all eight affected blocker-ledger Cargo selectors exact.
CQ1 review then found 33 exact terminal Cargo selectors owned once by blocker evidence and again by imported oracle evidence, plus two implemented fixture rows that were not represented in the generated evidence graph.
The second correction made each blocker case the sole primary owner, retained its colliding oracle fixture as a typed supporting selector, classified `coverage-util-bot-twiddle` under `CQ-BIT-KERNELS`, and attached the broad `pf5-detecting-regions-clifford-rust` fixture as supporting-only provenance under `CQ-FLOW-UTILS`.
The dependent performance inventory was regenerated because these confirmed inventory defects changed the frozen correctness binding without changing performance dispositions.

## CQ2 Exact-Parent Mapping Refresh

CQ2 implementation revealed that the frozen inventory had no source-owned mechanism for replacing several exact planned upstream or exported-API owners with one reviewed independently selectable parent test.
`oracle/qualification-cases.json` now owns that mapping contract, while deterministic regeneration rejects missing owners, duplicate claims, cross-feature or comparator mismatches, non-exact selectors, and reused terminal primaries.
The completed selected `.stim`-format slice has 32 implemented evidence parents and zero planned owners: 24 exact qualification parents map 44 upstream owners and nine exported-API owners, while eight independently selected oracle-fixture parents remain direct evidence.
The completed selected `.dem`-format slice has 28 implemented evidence parents and zero planned owners: 17 qualification parents, eight direct oracle-fixture parents, and three blocker-ledger parents close all selected exact upstream and exported Rust API contracts while retaining broad imported fixtures as supporting-only provenance.
The completed selected result-format slice has 39 implemented evidence parents and zero planned owners: 36 exact qualification parents and three direct oracle-fixture parents close all 156 selected upstream records and all 97 selected exported Rust API items while retaining seven broad imported fixtures as supporting-only provenance.
Two additional instruction-semantic parents map two exact upstream cases and four exact Rust methods without claiming untested derived traits or Stim-incompatible circuit measurement-count overflow behavior.
Semantic review classifies mixed `circuit.test.cc`, gate-target equality, instruction value/count, and Python-only constructor ownership by exact symbol; it also found and fixed inverted Pauli-target admission for `CORRELATED_ERROR` and `ELSE_CORRELATED_ERROR`.
One initially proposed mapping was rejected because its Cargo selector was already the canonical primary of an implemented oracle fixture, and the `MeasureRecordOffset` mapping received a new focused positive-and-boundary test instead of claiming broad mixed evidence.

## Operational Surface

The thin recipes are:

```sh
just qualification::correctness-list
just qualification::correctness-list --feature CQ-RESULT-FORMATS
just qualification::correctness-check
just qualification::correctness-regenerate --check
```

All source discovery, bounded reads, rustdoc and AST subprocesses, stable-id generation, validation, selector resolution, deterministic rendering, and atomic writes live in the Rust `stab-oracle` binary.

## Audit

### Completion Matrix

| CQ0 requirement | Status | Evidence |
| --- | --- | --- |
| Parse all 103 C++ and 91 Python files | Satisfied | Frozen counts above; deterministic regeneration reads and count-checks both source sets. |
| Support every selected C++ and pytest form | Satisfied | Extractor tests cover four C++ macros, masked fake declarations, module, class, async, static-parameter, stacked-parameter, dynamic-family, and nested-helper cases. |
| Freeze default-feature public API inventory | Satisfied | 1,922 typed rows; tests cover re-exports, methods, variants, enum fields, implementing-type trait identities, exclusions, and cyclic glob re-exports. |
| Classify source relevance and executable ownership | Satisfied | 2,886 records, 651 multi-domain relevance records, zero executable dynamic families, and twelve typed deferred products. |
| Import existing evidence by stable id | Satisfied | All 440 implemented oracle rows are represented without copied fixture payloads; 247 own exact oracle-fixture primaries, 142 retain planned atomic primaries, and 51 are supporting selectors on canonical blocker or qualification parents. |
| Freeze cross-cutting resource owners | Satisfied | One implemented symlink case, one implemented property-worker case, and thirteen exact planned boundary families are required by manifest validation. |
| Reject stale, duplicate, unsafe, shared, oversized, or unknown state | Satisfied | Adversarial schema and mutation tests plus exact selector resolution in `correctness-check`. |
| Freeze semantic digest and deterministic bytes | Satisfied | `correctness-regenerate --check` compares a fresh pinned-source and rustdoc build with the checked manifest. |
| Give every CQ domain executable or evidence-close ownership | Satisfied | `correctness-check` reports an implemented-or-closed count greater than zero for all sixteen domains from 493 canonical exact owners. |

Implementation and review revealed three genuine CQ0 specification gaps: parameterized pytest identity; the separation of domain relevance, evidence ownership, statistical-plan staging, and dedicated resource claims; and a finite cross-cutting resource-owner inventory.
All three gaps are resolved in `docs/plans/comprehensive-correctness-qualification-plan.md` and recorded in `docs/plans/milestone-spec-gaps.md`.

## Full Review

The first independent architecture pass found six confirmed issues: file-level single-domain ownership, omitted enum payload fields, unknown APIs falling into `CQ-RESOURCE`, incomplete trait-implementation identities, string-prefix ownership with untyped ids or paths, and premature completion documentation.
Those findings were fixed with multi-domain relevance and exact ownership records, enum field traversal, fail-closed API classification, canonical trait-and-self identities, typed boundaries, delimiter-aware ownership, and this source-owned report.

The second independent security and correctness-contract passes found eleven additional classes of issues: stale or externally redirected rustdoc artifacts, cyclic glob recursion, absent statistical-plan ownership, late hostile-input bounds, missing secondary simulator and analyzer relevance, collapsed pytest parameter cases, conflated behavioral surface and provenance, unbounded literal-range and C++ line scanning work, environment-dependent Python AST ids, phantom trait-implementation identities, and an empty planned resource ledger.
Those findings were fixed with fresh symlink-checked explicit-target rustdoc output, traversal guards, typed statistical-plan references, bounded deserialization and diagnostics, exact source-domain rules, pinned and bounded static or dynamic pytest records with stacked-decorator tests, linear C++ line tracking with a global case budget, implementing-type trait identities, independent surface and provenance fields, and fourteen exact resource owners.

The post-fix security pass then found cumulative Python record-byte amplification, a residual workspace-parent race in temporary rustdoc target creation, and remove-before-rename behavior in the non-Unix atomic-write fallback.
Those findings were fixed with per-field and cumulative child-output budgets, fresh operating-system temporary rustdoc targets, Windows atomic replacement through `tempfile::persist`, and a fail-closed no-clobber fallback on other non-Unix platforms.

The original independent contract audit reported no remaining blocker, and the final security verifier confirmed closure after the last parameter-amplification fix.
CQ1 later revealed and corrected the non-exact-selector validation loophole, duplicate terminal-selector ownership, and two omitted implemented-fixture classifications without weakening the finite CQ0 inventory or adding a deferral.

## Verification

Passing working-tree commands:

```text
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --quiet
cargo test -p stab-oracle qualification --quiet
just qualification::correctness-regenerate --check
just qualification::correctness-check
```

The corrected focused qualification run executes the CQ0 inventory tests plus the CQ1 selector and property-plan validation tests with no failures.
The workspace test run passed with only the repository's pre-existing documented long-running parser fuzz smoke ignored.
`just maintenance::pre-commit` passed before the implementation commit.
The original `just qualification::correctness-check` passed from committed revision `02c93c19566bdc465ad9c795f35e956e1ff85440` with a clean worktree. The previous corrected digest then passed clean CQ1 PR, full, and soak execution from revision `e7ba513822c26859a2b5c70c94d406e1c6adb6b6`; the full and soak tiers each passed all 410 then-current implemented or evidence-close owners. The completed selected `.stim` and `.dem` slices raised that count to 441 and 457. The completed result-format slice now raises it to 493, and clean global current-digest execution from revision `7d58bc8b3d70be7fe9188202c9611c7e076a3a8c` passes PR 331/331, full 493/493, and soak 493/493 with offline regeneration and exact full/soak parent preflight.

## Remaining Program Work

- PQ0 is complete and has been regenerated against the corrected CQ digest without changing its performance dispositions.
- CQ1 is complete with clean committed-revision audit, review, PR, full, soak, report, and preflight evidence in `docs/plans/cq1-correctness-harness-progress-report.md`.
- CQ2 through CQ5 must replace all 3,223 planned evidence owners with direct cases or justified non-executable dispositions.
- CQ6 must run and publish the final comprehensive correctness qualification.

These are later milestones and do not weaken CQ0's finite inventory contract.
