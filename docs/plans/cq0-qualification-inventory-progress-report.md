# CQ0 Qualification Inventory Progress Report

## Status

CQ0 implementation, milestone audit, and independent-review closure are complete in the working tree and pending clean committed-revision verification.

Compatibility target: Stim v1.16.0 at commit `e2fc1eca7fd21684d433aa5f10f4504ea4860d07`.

Frozen manifest digest: `7b8703977838aa796f3da6bf5e6321de4491a73dfc5af2a2a07785337918abb0`.

Pinned isolated Python AST version: 3.14.6.

This milestone freezes a finite source and API inventory; it does not claim that the 3,495 planned CQ2 through CQ5 evidence owners already pass.

## Inventory

| Inventory | Count | Notes |
| --- | ---: | --- |
| C++ test files read | 103 | Read from pinned `vendor/stim/file_lists/test_files`; 102 files contain selected extractor declarations. |
| Python test files read | 91 | Listed from the pinned Stim Git tree without importing test modules. |
| Direct C++ cases | 1,877 | Includes explicit 64-bit, 128-bit, and 256-bit expansion of every `TEST_EACH_WORD_SIZE_W` declaration. |
| Direct Python semantic records | 844 | Includes 727 unparameterized cases, 94 statically expanded parameter subcases, and 23 dynamic parameter families. |
| Exact blocker-ledger subcases | 165 | References source-owned blocker ids without copying selector payloads. |
| Total upstream records | 2,886 | One exact source record can be relevant to multiple CQ domains. |
| Multi-domain relevance records | 686 | Primarily command plus engine behavior and mixed semantic methods; relevance does not itself confer passing evidence. |
| Dynamic parameter families in executable scope | 0 | All 23 dynamic families are content-addressed, visible, and non-executable. |
| Default-feature public API items | 1,922 | Includes re-exports, variants, enum payload fields, public struct fields, inherent methods, trait methods, and explicit non-synthetic, non-blanket trait implementations. |
| Evidence owners | 4,099 | 2,678 upstream semantic owners, 804 public Rust API owners, 438 oracle fixtures, 165 blocker cases, thirteen planned qualification resource owners, and one hostile-path regression. |

### Upstream Dispositions

| Disposition | Count |
| --- | ---: |
| `ported-rust` | 165 |
| `semantic-mining` | 2,069 |
| `deferred-product` | 640 |
| `not-applicable` | 12 |
| `exact-oracle` | 0 |
| `superseded` | 0 |

The 640 deferred records name one of twelve typed products: Crumble 5, deprecated detector hypergraph 1, diagrams 88, `explain_errors` 2, interactive simulators 75, Python bindings 68, QASM 10, Quirk 4, sinter 118, stimcirq 117, stimflow 113, and ZX or lattice-surgery integrations 39.
Of those deferred records, 163 remain relevant to at least one CQ domain summary, but no deferred record owns executable evidence or contributes a passing case.

### Evidence Status

| Status | Count |
| --- | ---: |
| `implemented` | 587 |
| `evidence-close` | 17 |
| `planned` | 3,495 |
| `deferred` | 0 |

The 604 implemented or evidence-close owners establish that every CQ domain has at least one exact primary case; they do not close the remaining planned owners.

### Comparator Inventory

| Comparator | Count |
| --- | ---: |
| `canonical` | 211 |
| `error-class` | 19 |
| `exact-bytes` | 459 |
| `exact-value` | 97 |
| `property` | 1,316 |
| `resource` | 14 |
| `semantic-invariant` | 432 |
| `state-equivalence` | 629 |
| `statistical` | 560 |
| `structural` | 362 |

All 560 statistical rows have typed plan references: 14 reference blocker-ledger plans, 18 reference oracle-fixture plans, and 528 planned rows reference their future qualification-case owner.
The resource comparator rows comprise one implemented symlink regression and thirteen independently planned boundary families for parser admission, checked arithmetic, result records, materialization, streaming buffers, writer and visitor failures, replay and side inputs, traversal, search, allocation, typed paths, and output lifecycle.
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
Every implemented Cargo primary selector must resolve to exactly one test, imported oracle and blocker ids must remain valid, shared selectors are rejected, and deterministic regeneration must match the checked bytes and frozen digest.

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
| Classify source relevance and executable ownership | Satisfied | 2,886 records, 686 multi-domain relevance records, zero executable dynamic families, and twelve typed deferred products. |
| Import existing evidence by stable id | Satisfied | 438 oracle rows, 165 blocker rows, and one Cargo regression are referenced without copied fixture or ledger payloads. |
| Freeze cross-cutting resource owners | Satisfied | One implemented symlink case and thirteen exact planned boundary families are required by manifest validation. |
| Reject stale, duplicate, unsafe, shared, oversized, or unknown state | Satisfied | Adversarial schema and mutation tests plus exact selector resolution in `correctness-check`. |
| Freeze semantic digest and deterministic bytes | Satisfied | `correctness-regenerate --check` compares a fresh pinned-source and rustdoc build with the checked manifest. |
| Give every CQ domain executable or evidence-close ownership | Satisfied | `correctness-check` reports an implemented-or-closed count greater than zero for all sixteen domains. |

Implementation and review revealed three genuine CQ0 specification gaps: parameterized pytest identity; the separation of domain relevance, evidence ownership, statistical-plan staging, and dedicated resource claims; and a finite cross-cutting resource-owner inventory.
All three gaps are resolved in `docs/plans/comprehensive-correctness-qualification-plan.md` and recorded in `docs/plans/milestone-spec-gaps.md`.

## Full Review

The first independent architecture pass found six confirmed issues: file-level single-domain ownership, omitted enum payload fields, unknown APIs falling into `CQ-RESOURCE`, incomplete trait-implementation identities, string-prefix ownership with untyped ids or paths, and premature completion documentation.
Those findings were fixed with multi-domain relevance and exact ownership records, enum field traversal, fail-closed API classification, canonical trait-and-self identities, typed boundaries, delimiter-aware ownership, and this source-owned report.

The second independent security and correctness-contract passes found eleven additional classes of issues: stale or externally redirected rustdoc artifacts, cyclic glob recursion, absent statistical-plan ownership, late hostile-input bounds, missing secondary simulator and analyzer relevance, collapsed pytest parameter cases, conflated behavioral surface and provenance, unbounded literal-range and C++ line scanning work, environment-dependent Python AST ids, phantom trait-implementation identities, and an empty planned resource ledger.
Those findings were fixed with fresh symlink-checked explicit-target rustdoc output, traversal guards, typed statistical-plan references, bounded deserialization and diagnostics, exact source-domain rules, pinned and bounded static or dynamic pytest records with stacked-decorator tests, linear C++ line tracking with a global case budget, implementing-type trait identities, independent surface and provenance fields, and fourteen exact resource owners.

The post-fix security pass then found cumulative Python record-byte amplification, a residual workspace-parent race in temporary rustdoc target creation, and remove-before-rename behavior in the non-Unix atomic-write fallback.
Those findings were fixed with per-field and cumulative child-output budgets, fresh operating-system temporary rustdoc targets, Windows atomic replacement through `tempfile::persist`, and a fail-closed no-clobber fallback on other non-Unix platforms.

The final independent contract audit reported no remaining blocker, and the final security verifier confirmed closure after the last parameter-amplification fix.
The CQ0 milestone audit is complete with every task and acceptance criterion satisfied and no open specification gap.

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

The latest focused qualification run executed 46 tests with no failures.
The workspace test run passed with only the repository's pre-existing documented long-running parser fuzz smoke ignored.
`just maintenance::pre-commit` and clean committed-revision verification remain to be recorded after final review closure.

## Remaining Program Work

- PQ0 must freeze performance dispositions against these CQ ids.
- CQ1 must implement executable comparator, tier, timeout, statistical-budget, and report machinery.
- CQ2 through CQ5 must replace all 3,495 planned evidence owners with direct cases or justified non-executable dispositions.
- CQ6 must run and publish the final comprehensive correctness qualification.

These are later milestones and do not weaken CQ0's finite inventory contract.
