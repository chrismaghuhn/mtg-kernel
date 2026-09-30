# MADS-03D Synthetic Fixture-DAG Differential Status

**Base:** `0d7703c3958db0f90d657b45d31db328b775febd` (`origin/main` at task start)
**Branch:** `feat/MADS-03D-exact-tt`
**Scope outcome:** test-only OracleFixture/MadsGraph differential; no live or productive TT.

## Key-gate decision

MADS-03C remains `TRUSTED_LIVE_KEY = NOT_PROVEN`, with `PRODUCTIVE_TT_REUSE = DISABLED`. Accordingly this increment does not connect a GameState key to `MadsGraphV1`, does not use diagnostic hashes or observations, and does not alter the live Engine searcher. The comparison exercises the existing fixture-local interner keyed by exact author-supplied `FixtureNodeId`; it is not evidence that two real Engine states may merge.

## Added differential fixture

`mads_v1::tests::path_local_tree_and_fixture_id_dag_match_oracle_with_exact_tt_hits` constructs two equivalent complete synthetic decision trees:

- A compact DAG where two MIN parents reference the same MAX child and exact draw terminal.
- An unrolled path-local fixture with a distinct clone of each corresponding descendant.

The independent `OracleSuiteV1` solves both. After every scheduled expansion, the test checks each created node's lower/upper envelope against that fixture's exact oracle node value. At completion it compares the exact root value, the complete ordered root action domain, and certified actions against the oracle-optimal set. It intentionally does not require identical certified-action subsets or selected expansion order across the two scheduler traversals. The shared node has two reverse parents; the path-local version has none shared.

The successful test output reported:

```text
fixture root value: DRAW (0)
shared DAG search nodes: 6
path-local search nodes: 8
fixture-ID duplicate nodes avoided: 2
expanded action slots: 7 shared / 7 path-local
fixture-ID TT lookups/hits: 7 / 2
path-local lookups/hits: 7 / 0
status: fixture_only
```

These are fixture graph counters, not Magic engine transitions or performance results. A first draft over-constrained the test by requiring identical certified root-action subsets. The run showed the DAG traversal certified `left` and `right`, while the path-local traversal had certified only `left`; both were Oracle-optimal. The assertion was corrected to require each returned certificate to be in the Oracle-optimal set, matching the instruction that scheduler-order differences are not correctness failures.

The existing `mads_v1` group also passed its tests for forced hash collisions, exact construction-context separation, multi-parent reverse propagation/role union, bound envelopes and adversarial root certificates. These use fixture identities only.

## Gate and limits

```text
PRODUCTIVE_TT_REUSE = DISABLED
MADS03D_FIXTURE_DAG_DIFFERENTIAL = TESTED
EXACT_TT_GATE = OPEN
```

No exact live StateKey is available, and the fixture interner has no implementation path from `GameState`. The current test proves only that the existing synthetic interner and reverse-parent propagation match the oracle on the described DAG and unrolled tree. It does not prove real-state equivalence, cyclic/GHI handling, construction-key safety, a general TT, or a Magic search speedup.

## Verification

- `cargo fmt --all -- --check` — passed; `git diff --cached --check` also passed.
- `cargo test --locked -p mtg-kernel --lib path_local_tree_and_fixture_id_dag_match_oracle_with_exact_tt_hits -j 3 -- --nocapture` — passed.
- `cargo test --locked -p mtg-kernel --lib mads_v1 -j 3 -- --nocapture` — 17 passed, including forced-hash-collision and construction/reverse-parent regressions.
- No workspace or Release suite was run. No performance baseline is claimed.

## Remaining obligations

A future productive TT requires MADS-03C to close the trusted live-key gate, a reviewed DynamicEngineSearch graph integration using exact key equality after bucket lookup, and differential tests over corresponding graph states with all reverse-parent and construction contexts preserved. Until then, 03D is confined to the isolated Oracle fixture graph.
