# MADS-03C Trusted Live DecisionStateKey Status

**Base:** `0d7703c3958db0f90d657b45d31db328b775febd` (`origin/main` at task start)
**Branch:** `feat/MADS-03C-trusted-live-key`
**Decision:** `BLOCKED_BY_PROOF`; no trusted live key or productive TT is enabled.

## Existing capture path examined

`FastActorSessionV1` owns private `GameState`, `PolicySurfaceV5`, current surfaced decision, ordered candidate snapshot, revision/counters and internal caches (`mtg-kernel/src/rl_session.rs`, `FastActorSessionV1`). Its crate-private `trusted_live_key_capture_preflight_v1(&self)` accepts no caller-supplied state, `Decision`, or candidate vector; it performs no Engine `step`/`advance`. It rejects terminal/no-current sessions, stale revision/counter bindings, policy-only surfaces, terminals, and empty/mismatched reconstructed candidate lists. The reconstruction compares the complete ordered V5 core candidate list with the saved one.

On a clean build the preflight then calls `authenticated_namespace_gate_v1()`, which authenticates build HEAD/tree cleanliness and the generated card-database hash but unconditionally returns `MissingSchedulerContract`. Thus the actual function returns only a rejection; it emits no state, key, hash or digest. No change in this increment weakens that gate.

## Required proof obligations and present evidence

| Obligation | Evidence on this HEAD | Result |
|---|---|---|
| T8: current Decision provenance | FastActor has private session-owned state/current fields and no public API taking an arbitrary Decision. Revision/counter ownership is checked. The preflight does not independently prove that every accepted stored Decision is the unique authoritative Engine decision for the current GameState, and no pure Engine validator provides that proof. | NOT_PROVEN |
| Candidate completeness | Preflight reconstructs V5 core candidates and checks exact order/equality, and rejects policy-only attacker/blocker contexts. That proves snapshot consistency for admitted forms, not universal completeness against all raw Engine actions, reshaped protocols, or future action variants. | PARTIALLY_PROVEN |
| Continuation identity | `GameState`, `PolicySurfaceV5`, current decision and session structural snapshots preserve stored continuation fields; the exact key audit classifies every field as INCLUDE. No live DecisionConstruction adapter covers all cast/activation/effect/discard/APNAP paths. MADS-03B's separate open PR covers only a test-validated PendingCast target stage and is not included in this branch. | NOT_PROVEN |
| Namespace | Build-derived HEAD, tracked tree/clean identity and card DB hash are checked. No independently accepted live Dynamic MADS scheduler contract exists in this main baseline; no standalone raw Decision/rules semantic version closes the namespace. Clean builds still receive `MissingSchedulerContract`. | INCOMPLETE |
| Exact comparison | `GameState` and `PolicySurfaceV5` are structurally comparable; `Decision`, `PolicyDecisionV5`, `PolicyActionV5`, and the envelope expose `PartialEq` only. Full comparison after a future bucket lookup would still not prove future-semantic equivalence/bisimulation. No `Eq`/`Hash` or normalization was added. | NOT_PROVEN |
| Hidden-information firewall | The owned GameState contains both players' hidden zones and future RNG. The audit requires those fields in a privileged exact key, and forbids projecting them into observations/teacher labels. No information-set key or teacher adapter exists. | TT key is privileged only; fair teacher NOT_PROVEN |

## Added negative-pair sensitivity coverage

The test-only `exact_contract_sensitivity_covers_hidden_rng_history_and_continuation_stage` confirms the generic structural contract distinguishes paired values with:

- Different hidden opponent library order while other fixture fields begin equal.
- Different deterministic RNG seeds.
- Different committed event history.
- Different typed construction protocol stage.

Existing tests in `mads_decision_state_key_v1.rs` also distinguish namespace source/rules/card/schema/surface/RNG/scheduler/key versions, raw Decision actor, ordered candidate reordering, construction prefix, and session revision. Existing FastActor preflight tests reject a stale revision, an incomplete candidate snapshot, policy-only contexts, and the missing namespace contract. These are structural/admission negatives; they do not establish safe transposition reuse or T8.

## Verification

- `cargo test --locked -p mtg-kernel --lib mads_decision_state_key_v1 -j 3 -- --nocapture` — 4 passed on the modified test module.
- The main-branch FastActor preflight regressions were not rerun because production preflight code was not changed; their previous 03A evidence is recorded in `MADS03A_POST_MERGE_INTEGRATION_STATUS.md`.
- No Engine, FastActor production code, key issuance, hash table, or namespace gate was changed. No workspace or Release suite was run.

## Gate decision

```text
TRUSTED_LIVE_KEY = NOT_PROVEN
PRODUCTIVE_TT_REUSE = DISABLED
EXACT_TT_GATE = OPEN
```

The precise blockers are T8's missing proof that the owned stored Decision is the current unique authoritative decision across every session mutation/restore path, incomplete universal candidate coverage, incomplete typed continuation coverage, and the absent checked-in live Dynamic MADS scheduler namespace. The current preflight's unconditional `MissingSchedulerContract` is the concrete fail-closed behavior. Do not turn the generic structural envelope or a diagnostic hash into a reusable key.

## Next proof-focused increment

Before live-key issuance, establish a closed supported Engine decision set with a proof that current decision provenance follows only authoritative owned transitions; validate complete ordered raw candidate domains for each admitted decision; close typed construction identity for those protocols; then check in and authenticate the actual production scheduler/rules/schema/RNG namespace. Only afterward may an opaque privileged key capture be considered. Until those steps close, table reuse remains disabled.
