# MADS-03B2 – Dynamic Typed Construction Status

## Result

This increment adds an additive `DynamicEngineSearchV2` surface for the narrowly admitted Lightning Bolt cast path. The existing V1 search API and its one-engine-action meaning remain unchanged. V2 records a completed physical response as the initiating decision, its ordered engine responses, and an authoritative finalization boundary. A `CastSpell` announcement alone is never returned as a complete cast or certified action.

The implementation is **tested on the admitted Lightning Bolt fixture**, not a general proof of Magic decision construction or game-value correctness. The root remains `UNKNOWN` in that fixture because it is not a fully solved authoritative game graph.

## Implementation and admission boundary

Source: `mtg-kernel/src/dynamic_engine_search_v1.rs`.

- Additive public types: `DynamicEngineSearchV2` (`API_VERSION = 2`), `DynamicSearchResultV2`, `CompletePhysicalActionV2`, `CompletePhysicalActionIdentityV2`, `IncompletePhysicalActionFrontierV2`, and `PhysicalActionFinalizationV2`.
- The V2 identity is schema-versioned and contains the initiating engine `Decision`, ordered authoritative `Action` responses, and a finalization boundary. A finalized cast additionally captures its source, authoritative stack item and `FinalizedCastBindingV1`. Stable response IDs are framed from the ordered stable action-path IDs.
- Dynamic V2 expands through the existing path-local scheduler and performs `engine::step` plus `engine::advance_until_decision` on owned state clones. It classifies the resulting frame with `classify_after_transition_v1`; a typed PendingCast target context is retained on a construction node. No TT, state-key normalization or state merging is introduced.
- V1's public result contract is unchanged. V2 does not reinterpret an existing V1 `chosen_engine_action` as a physical action.
- The production V2 root admission scope is exactly a priority domain containing one castable Lightning Bolt and Pass, with no mana abilities, land drops, activations or plot actions. Any other root action domain is rejected as unsupported at construction time; V2 does not inherit V1's broader root domain silently.
- Within that root, V2 admits **Lightning Bolt PendingCast target selection only**. Target candidates come from the authoritative `ChooseTargets` frame and are checked against the existing V5 candidate projection bound to the post-CastSpell state; the selected target is then applied by the authoritative engine transition. Finalization requires the engine to have cleared PendingCast and exposes the actual finalized cast binding and stack object.
- The V2 public scheduler enumerates all admitted root actions and their construction candidates, then stops at the physical-action boundary. It does not search subsequent game decisions; their outcome values stay `UNKNOWN`. This bounded behavior is intentional until later decision types are admitted with their own contracts.
- Root-domain enumeration can be complete while root value remains unknown. Open construction prefixes are listed as incomplete and retain `UNKNOWN` bounds. Only complete response paths participate in V2 certification, and any incomplete competitor retains its upper bound. A larger game result is not fabricated.

## Tested fixture and evidence

The deterministic real-engine fixture is defined in `mads02e_structured_decision_audit_v1.rs`, test `dynamic_v2_tracks_complete_lightning_bolt_root_responses_without_certifying_cast_start`.

- The authoritative P0 Main1 decision has exactly `CastSpell(Lightning Bolt)` and `Pass`; the test independently compares the raw domain with the V5 action projection.
- Budget zero returns `UnresolvedWithinBudget`, `UNKNOWN`, no exact value, and no certificate.
- The public scheduler's first expansion announces the Bolt and opens the target construction stage; `CastSpell` alone is absent from complete actions and has no certificate. Its next expansion admits the independent Pass response while both target choices remain listed as incomplete prefixes.
- The target stage actor, spell, cardinality, candidate order and full domain are compared against the engine frame and V5 projection using the actual post-CastSpell state. Each target is replayed from a fresh root clone; the engine finalizes the cast, sets the finalized binding and creates a stack item with that target.
- Both target answers retain distinct ordered response identities (`CastSpell`, `ChooseTarget`). The independently completed `Pass` response is distinct and reaches P1 priority.
- The public `run_v2()` scheduler completes the root action domain containing both Bolt target responses and Pass, with no open prefix. It stops at that physical-action boundary. Root value remains `UNKNOWN`, and there are no certified optimal actions.
- The fixture measures 4 authoritative transitions and 5 state clones (including the owned initial clone) at completion. These are fixture-local search metrics, not a performance claim.
- The caller's original state remains unchanged.

Negative test `dynamic_v2_fails_closed_for_out_of_scope_fireblast_construction` confirms that a root domain containing a legal Fireblast announcement is rejected by V2 admission because its mode/payment construction is outside this scope. The entire root domain is rejected; no legal candidate is silently omitted.

Existing `typed_pending_cast_target_adapter_preserves_domain_and_finalizes_only_after_pick` and structured audit tests remain covered by the regression command below.

## Test runs

All Cargo commands ran serially under the Windows host mutex `Global\\mtg-kernel-cargo-build.lock`, with `--locked` and `-j 3`. No full workspace, release/Thin-LTO, CUDA, or broad benchmark run was started.

| Command | Result |
|---|---|
| `cargo test --locked -p mtg-kernel --lib dynamic_v2_ -j 3 -- --nocapture` | PASS: 2 passed, 0 failed (after review corrections) |
| `cargo test --locked -p mtg-kernel --lib mads02e_structured_decision_audit_v1 -j 3 -- --nocapture` | PASS: 8 passed, 0 failed |
| `cargo test --locked -p mtg-kernel --lib mads02b_engine_probe_v1 -j 3 -- --nocapture` | PASS: 9 passed, 0 failed |
| `cargo test --locked -p mtg-kernel --lib mads_v1 -j 3 -- --nocapture` | PASS: 17 passed, 0 failed |
| `cargo fmt --all -- --check` | PASS |
| `git diff --check` | PASS |
| `cargo clippy --locked -p mtg-kernel --lib -j 3 -- -D warnings` | PASS |

The first public-scheduler regression attempt showed that the debug-driven test had not established that `run_v2()` itself completed the root action domain. V2 now schedules only the admitted root actions and target-construction candidates, then stops at the physical-action boundary. The positive test drives full root-domain enumeration through public `run_v2()`. The suite was rerun after that correction. The full workspace suite was not run by design.

## Supported and unsupported behavior

**Admitted:** Only the tested authoritative root domain with exactly one Lightning Bolt cast and Pass; the Bolt's one mandatory legal target pick and engine finalization; and Pass reaching the opposing priority actor. This is an engine/privileged-state research lane. It does not export hidden state to an observation or policy interface.

**Fail-closed / not admitted:** Fireblast modes and sacrifice payments; other spell constructions; PendingActivation; optional target completion; multi-target and cost stages; APNAP, Chain Lightning, discard, combat, other priority/actor switches, and terminal transitions not explicitly admitted by the current adapter. No generic “new Decision means next game node” fallback is used for construction. This increment does not integrate typed construction into V1 callers or claim complete Magic physical-action coverage.

## Correctness status

| Property | Status | Evidence / remaining obligation |
|---|---|---|
| Complete physical identity | `PROVEN_ON_ADMITTED_SCOPE` | Distinct Bolt targets have distinct ordered responses and authoritative finalization payload in the fixture. This is limited to this schema and admitted path. |
| Root action domain | `TESTED_ON_ADMITTED_SCOPE` | Engine/V5 root and target domain comparisons plus complete root enumeration for the fixture. Not a universal independent proof of all engine domains. |
| Construction-node bounds | `TESTED_CONSERVATIVE_ON_FIXTURE` | Open candidate frontier is `UNKNOWN`; after enumeration both physical Bolt responses and Pass remain `UNKNOWN`. No terminal oracle is supplied for this position. |
| Dynamic construction integration | `TESTED` | Additive V2 path-local search expansion is exercised on Bolt target construction. Other continuation protocols are unsupported. |
| Root certification / exact root value | `NOT_PROVEN` | The admitted engine position is not exhaustively solved; V2 returns no certificate and `UNKNOWN`. |
| Wrong certified root actions / missed admitted actions / unsound intervals | `0 observed in executed fixture tests` | These counts describe only the tests listed here; they are not universal guarantees. |
| Exact live key / productive TT | `DISABLED` | No live key or TT reuse is added. |

## Open proof obligations

- Independently prove complete and correctly ordered candidate domains for every construction stage before admitting it.
- Prove exact continuation ownership/provenance across all continuation stages and actor changes.
- Extend root identity and certification to every physical response type without collapsing policy microsteps into engine decisions.
- Provide bounded complete Oracle graphs before claiming value-bound or root-certificate correctness for a real cast position.
- Keep unsupported transitions unresolved; do not infer loss, actor ownership, or completion from an enum change.

No engine rules, `GameState` semantics, RL/replay contracts, trained policy, TT, state-key contract, scheduler contract, POR, cards, training, or shard production were changed.

## Gate

```text
CONSTRUCTION_ADAPTER = PARTIAL
COMPLETE_PHYSICAL_ACTION_IDENTITY = PROVEN_ON_ADMITTED_SCOPE
DYNAMIC_CONSTRUCTION_INTEGRATION = TESTED_ON_LIGHTNING_BOLT_FIXTURE
ROOT_ACTION_DOMAIN = TESTED_ON_ADMITTED_SCOPE
ROOT_CERTIFICATION = NOT_PROVEN (fixture result UNKNOWN)
LIVE_TT = DISABLED
```
