# MADS-03B3 – Root-Critical Physical Search Status

## Outcome

The production root-critical physical-action scheduler is **blocked by the current graph contract**. `DynamicEngineSearchV2` represents a Lightning Bolt cast as one root Engine-action slot (`CastSpell`) whose child is a typed Construction node with separate target-choice slots. The existing MADS root-bound and frontier code indexes root actions by the slots on the root `GameDecisionNode`; it therefore sees the announcement as one root action and cannot assign separate incumbent/challenger identities to the resulting complete target responses.

No production Dynamic V1/V2 graph or scheduler semantics were changed. This increment adds a focused Oracle fixture proving the required reference relationship: a nested Cast→Target construction graph and its equivalent flattened complete-physical-root-action graph have the same minimax value, while their root-action identities differ. The existing `MadsGraphV1` root-critical frontier is validated on the flattened physical-action oracle graph.

```text
ROOT_CRITICAL_PHYSICAL_SCHEDULER = BLOCKED_BY_GRAPH_CONTRACT
ORACLE_FRONTIER_REFERENCE = IMPLEMENTED_AND_TESTED
VALUE_DISCRIMINATION_ON_REAL_ENGINE = NOT_PROVEN
```

## Phase A – graph and value integration analysis

Source references below are relative to `mtg-kernel/src/`.

1. **Root slot identity.** `dynamic_engine_search_v1.rs::complete_cast_domain` creates one slot for each legal raw Engine action, including one `CastSpell(bolt)`. `DynamicEngineSearchV2::new` wraps that same V1 graph. Thus the V2 root `CastSpell` slot is an announcement, not a complete physical cast.
2. **Distinct complete answers.** `DynamicEngineSearchV2::collect_physical_actions_v2` descends through a `construction_context` and appends each ordered `ChooseTarget` response. Its result identities distinguish `CastSpell(bolt)+ChooseTarget(P0)` from `CastSpell(bolt)+ChooseTarget(P1)` and retain the final stack item and cast binding.
3. **First authoritative successor.** `expand_slot_v2` runs the selected target through `engine::step` and `engine::advance_until_decision`. For Lightning Bolt this clears PendingCast, finalizes the cast and yields an authoritative `CastSpellOrPass` frame for P0. That successor node is the child of the target slot in the path-local graph.
4. **Current bound path.** `DynamicEngineSearchV1::recompute` backs the Construction node up with the initiating actor's MAX role, then backs its single parent CastSpell slot up with that aggregate bound. The value of a complete-action row is read from the post-target child, but it is not represented as a root slot in that graph. Root rows/bounds and V1's scheduler continue to use the raw root slot identity.
5. **Construction ownership.** `admit_construction_v2` uses `DecisionConstructionV1.initiator` as the actor and assigns its MAX/MIN role from that actor. The target is a same-actor construction choice, not automatically a new opponent decision. `admit_v2` creates a new GameDecision node only after the authoritative classifier reports one; its role then comes from the returned decision actor relative to the root player.
6. **Unmaterialized alternatives.** Before CastSpell expansion, the root slot remains open with `UNKNOWN`. After the announcement, every known target slot remains open with `UNKNOWN`; `backup_bounds_v1` preserves the MAX unresolved-action upper envelope. `incomplete_root_frontier` separately lists target prefixes as `UNKNOWN`. This is conservative, but does not give each prefix a root-action support identity.
7. **Scheduler limitation.** V1's `build_frontier_v1` obtains incumbent/challenger from `root_action_bounds_v1`, whose rows enumerate `root.slots()`. `collect_dynamic_support` then records the root slot index in `root_action_support`. All Lightning Bolt target candidates therefore inherit support for the same CastSpell root index. A task can be critical to the grouped Cast value, but it cannot represent one target as incumbent and another as challenger.

### Required integration contract

A correct additive V3/reference graph needs a **virtual physical-root layer** above or alongside the existing engine graph:

- Each admitted complete response candidate has its own versioned semantic ID, initiating decision, ordered Engine response path and owner physical decision.
- An unexpanded CastSpell announcement remains a non-certifiable unresolved prefix/envelope. Once its typed construction context yields the complete legal target domain, that prefix is replaced by one root alternative per exact target response. No response is silently omitted.
- A response alternative points to its exact construction slots and, after authoritative execution/finalization, to the resulting GameDecision or Terminal node. Distinct target alternatives retain distinct bound cells and distinct `root_action_support` IDs even if they share a response prefix.
- The Construction node retains the initiator actor and continuation identity. Its candidates are not changed into opponent turns. The root virtual layer treats complete physical responses as the alternatives of the initiating physical decision; subsequent GameDecision roles come from the authoritative successor actor.
- A prefix not yet split into candidates retains `UNKNOWN` support in the root envelope. After candidate admission, each unexpanded response has `UNKNOWN`. Root bounds are backed up over complete physical alternatives plus any remaining prefix envelope. Certification requires `L(A) >= max U(other complete alternatives and unresolved envelopes)`.
- A task key must contain the exact owner construction/action slot and stable response-prefix identity, plus the union of root-role support IDs. The frontier is rebuilt after each expansion; no TT/DAG parent merging is implied.

The current `Node.slots` tree cannot satisfy that contract without a second logical root identity/bound layer or a new V3 graph representation. Reinterpreting a CastSpell slot's aggregate MAX bound as each individual target response would be unsound. V1 and V2 remain unchanged.

## Oracle frontier reference

Test: `mads_v1::tests::mads03b3_oracle_frontier_uses_complete_physical_root_actions`.

The test builds two bounded deterministic OracleSuite fixtures:

- A nested form with root actions `cast-lightning-bolt`, `pass`, `other`; the cast owns a P0 `DecisionConstruction` node with ordered P0/P1 target choices.
- A flattened reference form with four distinct physical root responses: `cast-bolt/target-P0`, `cast-bolt/target-P1`, `pass`, and `other`. Each points at the corresponding exact post-construction successor node.

The independent oracle values both roots at `WIN`. The physical responses evaluate to WIN, LOSS, DRAW and DRAW; Pass and Other are tied, and the only optimal physical response is `cast-bolt/target-P0`. The successors after the two target choices have different actors (P0 MAX and P1 MIN). The test confirms the nested MadsGraph root still has three raw action slots, whereas the physical reference graph has four complete root alternatives.

On the flattened graph, the existing `MadsGraphV1` frontier initially assigns incumbent-lower support to physical action order 0 and challenger-upper support to order 1. Across all 8 expansions, every action interval contains its independent Oracle value, root bounds contain the Oracle root value, any emitted certificate is Oracle-optimal, and repeated frontier rebuilds are deterministic. This validates the existing reference policy **when the root domain is already expressed as complete physical actions**. It does not implement that mapping for DynamicEngineSearch.

No A/B scheduler-efficiency comparison is claimed. The 8 fixture expansions are a correctness-test count, not an efficiency measurement. No reduction in irrelevant work, wall time, or cost has been measured.

## Real-engine result

The existing Lightning Bolt/Pass test was rerun from `mads02e_structured_decision_audit_v1`. It confirms that V2 still enumerates both target responses and Pass through public `run_v2()`, reports no root certificate, and keeps the real root value `UNKNOWN`. Its fixture-local metrics remain 4 authoritative transitions and 5 state clones. This fixture supplies no terminal Oracle values, so it cannot demonstrate Root-Critical value discrimination. No additional Engine protocol was admitted.

## Verification

All Cargo commands were serialized under the Windows host mutex `Global\\mtg-kernel-cargo-build.lock`, used `--locked -j 3`, and stayed within the per-command time limit.

| Command | Result |
|---|---|
| `cargo test --locked -p mtg-kernel --lib mads03b3_oracle_frontier_uses_complete_physical_root_actions -j 3 -- --nocapture` | PASS: 1 passed; oracle fixture reported 8 expansions, root value 1, 4 physical root actions |
| `cargo test --locked -p mtg-kernel --lib mads_v1 -j 3 -- --nocapture` | PASS: 18 passed |
| `cargo test --locked -p mtg-kernel --lib mads02e_structured_decision_audit_v1 -j 3 -- --nocapture` | PASS: 8 passed |
| `cargo test --locked -p mtg-kernel --lib mads02b_engine_probe_v1 -j 3 -- --nocapture` | PASS: 9 passed |
| `cargo fmt --all -- --check`, `git diff --check` | PASS |
| `cargo clippy --locked -p mtg-kernel --lib -j 3 -- -D warnings` | PASS |
| `cargo clippy --locked -p mtg-kernel --lib --tests -j 3 -- -D warnings` | FAIL: two existing `clippy::type_complexity` findings in `dynamic_engine_search_v1.rs` test-only helpers at lines 246 and 277; these are outside this increment and were not changed. |

Full workspace, Release/Thin-LTO, CUDA, full MADS-wide runs, and broad benchmarks were not run.

## Correctness matrix and gates

| Property | Result | Scope |
|---|---|---|
| Complete physical action identity | Preserved | Existing V2 Bolt fixture; ordered CastSpell/Target responses remain distinct. |
| Construction owner mapping | Tested | P0 owns both Bolt target stages; successor actors are determined by the Oracle/Engine decision frames. General continuations remain open. |
| Oracle bounds | Sound in executed tests | Existing MadsGraph frontier on the flattened synthetic physical-root fixture; each interval contains the Oracle value. |
| Root-critical tasks | Oracle-validated on reference graph | Incumbent/challenger IDs and support are validated only when complete physical actions are explicit root edges. |
| Dynamic physical-root critical scheduler | Blocked by graph contract | Current Dynamic root slots still identify raw Engine actions; no separate physical-root bound/support table exists. |
| Real Engine value discrimination | `NOT_PROVEN` | Lightning Bolt root actions remain `UNKNOWN`; no full game Oracle. |
| Wrong certificates / unsound bounds / missed legal actions | 0 observed in executed synthetic reference and real fixture checks | Fixture-scoped only; not a universal proof. |
| Productive TT / live key | Disabled | Not changed. |

```text
ROOT_CRITICAL_PHYSICAL_SCHEDULER = BLOCKED_BY_GRAPH_CONTRACT
ORACLE_FRONTIER_REFERENCE = IMPLEMENTED_AND_TESTED
COMPLETE_ACTION_IDENTITY = PRESERVED
CONSTRUCTION_OWNER_MAPPING = VALIDATED_ON_BOLT_AND_ORACLE_FIXTURE
ROOT_BOUNDS = SOUND_ON_EXECUTED_ORACLE_FIXTURES
ROOT_CRITICAL_TASKS = ORACLE_VALIDATED_ON_FLATTENED_REFERENCE
ROOT_CERTIFICATION = ORACLE_ONLY_ON_SYNTHETIC_PHYSICAL_ROOT_FIXTURE
VALUE_DISCRIMINATION_ON_REAL_ENGINE = NOT_PROVEN
```

## Open proof obligations and next increment

1. Introduce an additive V3 virtual-root alternative record keyed by complete physical response identity, with exact links to construction slots and authoritative successor GameDecision nodes.
2. Preserve unresolved Cast prefixes as conservative envelopes while candidate domains are incomplete; prove replacement by all and only the legal complete alternatives when construction candidates are admitted.
3. Rebuild critical support over physical-root alternatives, unioning role masks when one exact task supports multiple root alternatives. Validate scheduler ordering and all bounds against OracleSuiteV1.
4. Differential-test the new policy against a FIFO physical-response enumerator on identical complete-action Oracle fixtures, including ties, UNKNOWN envelopes, certificates, bounded budgets and starvation cases.
5. Only after these fixtures pass, connect the V3 policy to Dynamic Engine post-construction successor nodes. Real Engine values remain UNKNOWN until a fully enumerated authoritative game Oracle is available.

No Engine rules, GameState semantics, V1/V2 scheduler/API contracts, live key, TT, POR, RL/replay contracts, cards, training, or hidden-information surfaces were changed.
