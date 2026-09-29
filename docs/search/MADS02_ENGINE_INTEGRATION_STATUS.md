# MADS-02 Engine Integration Status

Baseline: `edb91072c17d78769c7d72f4b757f8fa8a4c12f0` (`main`, MADS-01 merge).
Work branch: `feat/MADS-02-engine-integration`.

## Status matrix

| Gate | Status | Evidence |
|---|---|---|
| A. `REAL_PAUPER_DECISION_PROBE` | TESTED | Official runtime Burn mirror; deterministic Main1 `CastSpellOrPass`; all 3 raw actions independently enumerated, accepted on clones, and reproducible. |
| B. `BOUNDED_ENGINE_ORACLE` | TESTED | 295-node/331-edge synthetic Mountain game graph solved by `OracleSuiteV1`; MADS intervals contained oracle values and the certified root action matched. |
| MADS/Oracle agreement | CERTIFIED_ON_FIXTURE | Bounds and root result agree on the bounded synthetic fixture only. |
| C. Productive dynamic MADS search | NOT IMPLEMENTED | `TEST_ONLY_ENGINE_TREE_HARNESS` is a test harness and does not use the production root-critical scheduler. |
| Real MTG `DecisionStateKey` | NOT PROVEN | No real-state identity or TT reuse is implemented. |
| D. Fair hidden-information teacher | NOT PROVEN | Both engine fixtures are `ENGINE_ORACLE_ONLY`; no observation-to-teacher release path exists. |
| MADS training shards | NOT IMPLEMENTED | No shard writer or training output is implemented. |
| E. `MADS01_REGRESSION` | PASS | Existing MADS (15) and OracleSuite (3) focused tests plus Burn goldfish (2) pass unchanged. |

Neither probe produces training data. The whole-state engine oracle can see hidden hands, library order and RNG representation, so it is not a fair teacher for ordinary Pauper play.

## A. Real Pauper decision probe

The probe uses `runtime_deck_by_id("Burn")`, both seats, and the existing
`rl::build_deck_pair_state` builder. It consumes the catalog's 60 materialized
card IDs, validates the deck pair, deterministically shuffles with the fixed
seed, constructs the authoritative state, and deals the opening seven via the
existing propose/commit draw events.

```text
deck ID: Burn
source SHA-256: 4ebba6b42bb27a0ea55001cee133aada81f0dffd8661b46b012fc5026675aa32
runtime deck hash: 0x5fdb7b92986b6fc1
mainboard count: 60
seed: 0x000002b020260929
starting player: P0
engine commit: edb91072c17d78769c7d72f4b757f8fa8a4c12f0
```

After the builder's opening draw, the setup advances using only legal
`Action::Pass` actions until the first nontrivial Main1 `CastSpellOrPass`
decision. The exact post-advance `GameState` and `Decision` reproduce under a
second construction with the same seed. The actor is P0 at turn 1, Main1. The
ordered action domain is:

1. `PlayLand` for Mountain object 4 (stable V5 semantic includes owner,
   controller, hand zone and zone-change count).
2. `PlayLand` for Mountain object 6 (same semantic shape, distinct object).
3. `Pass` for P0.

The adapter independently enumerates this raw `CastSpellOrPass` domain from
the decision's cast, mana ability, land, activation, plot and pass fields. It
then compares the complete ordered `Action` vector to the V5 projection, checks
semantic and stable-ID uniqueness, and applies every action to two independent
clones. Each action is accepted by `engine::step`; the two resulting states
compare equal using `GameState` equality, and their next raw `Decision`s
compare equal. No 64-bit state hash is used as an equality proof. Six explicit
`engine::step` calls were accepted across the three actions and two clones.

This is a decision probe, not a full physical-action label: it follows the
engine decision protocol one step at a time. The two `PlayLand` actions have
different `ObjectId`s and different full V5 semantic values; neither is
deduplicated. The original probe state remains equal to its pre-branch clone.
V5 is used only for this raw
decision type. This does not establish completeness for `Discard`, raw
`DeclareBlockers`, or any other engine decision. Those remain unsupported by
this probe; raw `DeclareBlockers` is rejected by the existing V5 path.

No hidden-state observation or feature vector enters the adapter. The action
semantics identify the acting player's own legal land choices, but the
underlying authoritative state still contains information the actor does not
know. Classification: `ENGINE_ORACLE_ONLY`; fair MADS teacher: `NOT_PROVEN`.

## B. Bounded engine oracle

The separate synthetic fixture uses only the existing `Mountain` definition.
It starts with two Mountains in P0's library and three in P1's library, draws
two cards for each player through propose/commit draw events, and follows
legal engine priority actions to a P1 Main2 decision. Its restricted enumerator
supports complete `CastSpellOrPass` domains and the one-element empty
`DeclareAttackers` domain. Every other nonterminal decision fails closed as
`UNSUPPORTED_DECISION`; `Halted` is rejected and never assigned an outcome.

Every search path receives a distinct fixture node. No state hash, equality
merge, or transposition reuse is used. For each admitted decision, every
candidate is sent through `engine::step`, then `advance_until_decision` creates
the next node. Exact `(GameState, Decision)` equality is used only to reject a
cycle on the current ancestry path. Fixture-local action IDs encode the raw
action variant and its object/color fields explicitly; IDs do not depend on
`Debug` output or hashes. The fixture validated as acyclic and within
the OracleSuite depth, action, node, edge and transition caps.

```text
fixture: mads02b-engine-oracle-only-two-mountains-v1
root actor: P1
root decision: CastSpellOrPass
ordered root actions: [Pass]
oracle root value: WIN (+1) relative to P1
reachable game-decision nodes: 295
edges / accepted engine::step transitions: 331 / 331
construction nodes: 0
terminal nodes: 37 (37 WIN, 0 DRAW, 0 LOSS)
```

The single-action root is forced, so this fixture does not test competing
engine root actions. Existing MADS-01 unit tests continue to cover tied and
competing root actions. The OracleSuite is independent of the engine and
reports `authoritative_transitions = 0`; 331 is the adapter's separately
counted number of accepted `engine::step` calls. Automatic rule processing
inside `advance_until_decision` is not included in that count.

MADS was expanded one scheduled edge at a time over this complete oracle
fixture. After each expansion, every created MADS node's interval contained
its exact oracle value. MADS certified `Pass`, matching the oracle optimal set
`[Pass]`; the returned chosen action was checked to belong to that set. This
proves the selected action for this forced-action fixture. Separately, the
final MADS root interval was exactly `[+1,+1]`, matching Oracle root value
`+1`; this exact-value proof is not implied by action certification alone. At
the certification point MADS had created 260 nodes and expanded 259 actions;
it did not need to expand every fixture edge to certify the sole root action.
The root contains only `Pass`, so this test does not show MADS selecting
between competing Engine actions. No wrong root action, unsound bound, missed
admitted root action, Halted-to-outcome mapping, or invalid TT merge was
observed.

Classification: `ENGINE_ORACLE_ONLY`. The synthetic game includes
authoritative hidden information and is not training data.

## C. Dynamic search boundary

`TEST_ONLY_ENGINE_TREE_HARNESS` keeps a private state clone per tree node, eagerly
admits the complete legal action domain at each visited node, and materializes
only one successor per expansion. It has no TT; exact state and decision
comparisons reject path cycles. Its supported decisions are limited to
`CastSpellOrPass` and empty `DeclareAttackers`; other decisions, including
`Halted`, fail closed. It uses conservative MAX/MIN partial intervals and
checks the root after each expansion against the full fixture oracle. Full
expansion produced 332 path-specific tree nodes and 331 accepted transitions;
all intervals contained the oracle values and the certified root action was
`Pass`. Budget zero performed no transition and retained UNKNOWN bounds; a
single expansion retained UNKNOWN at the root. The caller's initial
`GameState` remained unchanged. A negative test confirms `Halted` is rejected
before a node or outcome is created.

This is a correctness harness, not the production dynamic MADS integration.
`MadsGraphV1` currently borrows a fully materialized `OracleFixtureV1`, and
its scheduler expands only fixture edges already present. The test-only
dynamic harness has separate minimal scheduling/backup code and does not reuse
the production MADS scheduler. No general API, persistent result/status type,
physical-action boundary, or production metrics contract was added. Therefore
productive dynamic search is `NOT IMPLEMENTED`.

## Unsupported scope and information gates

- V5 candidate completeness is proven here only for the reached
  `CastSpellOrPass` type, against an independent raw-field enumeration.
- The bounded fixture's raw action adapter additionally handles empty
  `DeclareAttackers`; nonempty combat subsets, blockers, discard reshaping,
  casting/target/cost continuations, and other raw decisions return
  `UNSUPPORTED_DECISION`.
- The Engine-Decision step is not equated with a complete physical game action
  or a Policy-Microstep. Cast, mode, target and cost continuation ownership
  still needs an explicit construction-node adapter; roles cannot switch just
  because a continuation yielded another engine decision.
- MADS-01's real Engine state-key audit remains `NOT PROVEN`. These tests use
  path-local tree identities and perform no TT merge.
- Both scenarios contain hidden-information state. `PERFECT_INFORMATION_PROVEN`
  is not claimed. Teacher release remains blocked until the information-set
  boundary and labels are proven safe.

## Tests run

```text
cargo fmt --all -- --check                                               => passed
cargo check -p mtg-kernel --tests                                       => passed (Debug profile)
cargo test -p mtg-kernel --lib mads02b_engine_probe_v1                  => 5 passed
cargo test -p mtg-kernel --lib mads_v1 -- --nocapture                  => 15 passed
cargo test -p mtg-kernel --lib oracle_suite_v1 -- --nocapture           => 3 passed
cargo test -p mtg-kernel --test burn_goldfish -- --nocapture            => 2 passed
cargo clippy -p mtg-kernel --lib --tests -- -D warnings               => passed
cargo test -p mtg-kernel --lib --release mads02b_engine_probe_v1        => 5 passed (6m 43s build)
cargo test --workspace --locked                                         => NOT RUN TO COMPLETION (interrupted after 35m)
cargo test -p mtg-kernel --lib native_checkpoint_runner_v1::tests::wide_checkpoint_runs_a_genuine_evaluation_game_end_to_end -- --exact --nocapture
                                                                         => 1 passed (isolated rerun)
```

Additional negative checks reject `Halted`, reject unsupported `DeclareBlockers`,
and verify that Oracle/MADS reject a graph with a missing reachable terminal
and a fixture over the per-node action cap. Those are included in the five
MADS-02B adapter tests.

The full Workspace run started all 1,734 library tests and ran for about 35
minutes before it was stopped. It had reported one failure in the existing,
untouched wide checkpoint evaluation test; an isolated exact rerun passed 1/1
in 54.8 seconds. Because the workspace run did not finish, no aggregate pass or
failure count is claimed. The focused MADS-02B, MADS-01, OracleSuite, Burn and
lint gates all passed on the final source changes.

No performance or novelty claim is made.

## Smallest next step

Implement a production `DynamicEngineSearchV1` boundary that owns exact cloned
engine nodes, admits only explicitly supported complete raw action domains,
uses the existing MADS bounds/scheduler rather than the test-only duplicate,
and returns `UNRESOLVED_WITHIN_BUDGET` or a certificate. Validate it against
the bounded fixture before widening supported decisions. Keep teacher output
disabled until a separate information-set contract proves which labels may be
released.
