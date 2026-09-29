# MADS-02F Oracle Ground Truth

## Fixtures available on this branch

`mads02b-engine-oracle-only-two-mountains-v1` remains the only fully
enumerated engine graph. It is rebuilt deterministically from
`TINY_ORACLE_SEED = 0x02b0_0001` in `mads02b_engine_probe_v1.rs`; the fixture
contains 295 game nodes, 331 edges, 37 terminals, and the sole root action
`Pass`. All 37 terminals are wins for the root player. The engine's
`advance_until_decision` and `step` are the authority for its rules and
transitions; `OracleSuiteV1` only solves the resulting captured graph.

The new `mads02f-synthetic-adversarial-control-v1` graph is explicitly a
synthetic control, not a Magic position. Its ground truth is four decision
nodes, six terminal nodes, nine edges, root value DRAW (0), root actions
`safe-a`, `safe-b`, and `risky`, with `safe-a` and `safe-b` optimal. Its leaves
include one LOSS, three DRAWs, and two WINs. P0 maximizes at root, then P1
minimizes at every child, so the fixture exercises MAX/MIN alternation,
unequal action values, and a root tie. The expected facts are asserted in the
test rather than supplied to or derived from MADS.

The existing oracle and MADS unit regressions also cover over-cap rejection as
`ORACLE_FIXTURE_TOO_LARGE`, complete root action admission, unexpanded-action
bounds, stable tie ordering, unsupported decisions, and `Halted` rejection.
No `Halted` decision is represented as a terminal outcome.

## Real-engine coverage limitation

The available engine oracle adapter in `mads02b_engine_probe_v1.rs` explicitly
supports only `CastSpellOrPass` (with an independently constructed full action
domain) and empty `DeclareAttackers`. Other decisions fail closed. The
reproducible tiny Pauper setup reaches a Main2 position with only `Pass`; the
Burn probe reaches a Main1 decision with multiple actions and proves its
ordered action projection, but it does not fully enumerate successor play to
terminal. Therefore this branch does **not** claim an engine-backed fixture
with multiple root actions, distinct root values, or mixed terminal outcomes.
Constructing one reliably requires a small reachable position whose complete
continuation remains within supported decisions and oracle caps; the current
card pool/setup utilities have not established such a position.

## Dynamic search differential attachment

The independent oracle API is the existing
`OracleFixtureV1::solve_oracle_v1()`. A future adapter should capture a
complete engine decision graph using only `engine::advance_until_decision`
and `engine::step`, reject unsupported decisions and caps, translate the
captured graph into `OracleFixtureV1`, and compare DynamicEngineSearchV1's
certified root action set, chosen action, root interval, and every exposed
certification-relevant bound against the oracle's `node_values`. It must also
compare the complete ordered legal root action domain. No Dynamic MADS bounds
or certification result may participate in constructing the graph or its
values. Agent A's API is intentionally not referenced here; integration is a
follow-up once its public contract exists.
