# MADS-03B Typed Decision Construction Status

**Base:** `0d7703c3958db0f90d657b45d31db328b775febd` (`origin/main` at task start)
**Branch:** `feat/MADS-03B-typed-construction`
**Scope outcome:** isolated, versioned adapter implemented for one real `PendingCast` target-pick stage; not integrated into `DynamicEngineSearchV1`.

## Implemented adapter

`mtg-kernel/src/mads_decision_construction_v1.rs` adds `DECISION_CONSTRUCTION_ADAPTER_VERSION_V1` and `classify_after_transition_v1`. It first recomputes the supplied `Decision` on a cloned state and requires both the decision and quiescent state to match. It classifies exact `GameOver` as terminal, rejects `Halted`, recognizes a fresh priority `CastSpellOrPass`, distinguishes a changed priority actor, and classifies APNAP trigger-group changes only when the pending engine trigger prefix matches the decision.

The supported construction shape is deliberately narrow: `PendingCast` plus raw `Decision::ChooseTargets` with a fixed target count (`can_finish == false`). At a priority `GameDecision`, the adapter separately lists raw CastSpell, ActivateAbility and PlayLand candidates as possible construction starters; the decision-point node therefore does not imply that those one-step answers are completed physical actions. The typed context preserves the complete cloned `PendingCast` (source contract/incarnation, controller, origin, chosen modes and costs, ordered targets/contracts and pending selection values), exact `Decision`, ordered target prefix, remaining cardinality, and the full ordered `Action::ChooseTarget` candidate domain. It verifies the actor, source object, top stack source/controller, target-contract/prefix lengths, nonempty unique candidates, and previous actor ownership. Unsupported PendingCast stages, activation construction and unrelated resolution choices fail closed.

This covers the concrete Lightning Bolt flow: cast announcement creates a placeholder and remains a construction node; selecting a target on an isolated state clone is followed by authoritative `advance_until_decision`, which finalizes the cast and returns priority to P0. The test compares the ordered construction candidates with the existing V5 projection and independently applies every candidate through `engine::step`. A stale foreign-actor Decision is rejected. The same fixed seed reproduces the starting engine state and root decision domain. The APNAP audit test now additionally verifies the P0-to-P1 trigger-order actor switch classification.

`Halted` never becomes a value. Construction candidates are not minimax outcomes. The adapter does not emit any certified action or teacher label.

## Dynamic integration gate

`DynamicEngineSearchV1` and its root result/action interface are unchanged. It still models one `engine::Action` per edge; it does not carry a complete cast response through `PendingCast` stages. A root `Action::CastSpell` therefore cannot be treated as a completed physical decision by this adapter. Wiring only the target node into the current scheduler would still leave root action identity and complete construction-path ownership unresolved. The adapter has no caller outside the focused tests.

Other `PendingCast` stages, `PendingActivation`, Chain Lightning copy retarget transitions and effect/APNAP combinations have not been integrated into Dynamic MADS. The classifier has a fail-closed identity branch for a Chain Lightning copy actor switch, but this increment does not claim that path's end-to-end validation. APNAP and priority actor switches have focused evidence; other actor-switch protocols remain unsupported.

```text
CONSTRUCTION_ADAPTER = PARTIAL
DYNAMIC_INTEGRATION = BLOCKED
```

## Verification

The Windows/C-drive target cache was initially absent; targeted Debug builds used `CARGO_TARGET_DIR=C:\dev\mtg-kernel-target`, `-j 3`, and a Windows named Cargo mutex to serialize this host's Cargo work. Commands were bounded below ten minutes. No workspace suite, Release suite, or WSL build was run.

- `cargo fmt --all -- --check` — passed.
- `cargo test --locked -p mtg-kernel --lib typed_pending_cast_target_adapter_preserves_domain_and_finalizes_only_after_pick -j 3 -- --nocapture` — 1 passed.
- `cargo test --locked -p mtg-kernel --lib mads02e_structured_decision_audit_v1 -j 3 -- --nocapture` — 6 passed, including the typed PendingCast flow and APNAP actor switch.
- `cargo clippy --locked -p mtg-kernel --lib -j 3 -- -D warnings` — passed after boxing the adapter enum's larger construction case, using the copied target value, explicitly versioning the construction payload, and marking the crate-private module as intentionally unwired.
- No full MADS/Oracle or workspace regression suite was repeated; earlier merged baseline evidence remains in the MADS-03A report.

## Remaining proof obligations

1. Give DynamicEngineSearch a root-action representation for a complete typed construction response (target/mode/cost and authoritative finalize boundary) before it can certify casts.
2. Prove full-domain candidate adapters for every admitted cast stage, including optional target selection, payment-mode and interactive object-cost stages.
3. Validate Chain Lightning affected-player choices and all other actor-changing resolution continuations with positive and negative engine fixtures.
4. Decide and prove the supported `PendingActivation`, discard-resume, APNAP continuation, effect-choice and policy-microstep boundaries before widening support.
5. Keep the trusted live state-key and TT gates separate; this adapter's cloned continuation context is not a reusable state key.

A later 03B follow-up may widen one protocol at a time, but the current Dynamic MADS integration remains blocked until complete physical root-action identity and certificates are end-to-end.
