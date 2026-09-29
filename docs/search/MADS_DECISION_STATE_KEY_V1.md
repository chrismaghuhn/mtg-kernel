# MADS Decision State Key V1 Audit — MTG Kernel

**Status: NOT PROVEN. Real-engine transposition-table reuse is blocked.**

Audit baseline: `master` at `1265b62c1a0d22e6f3bcc853c4e355fbc696c90f`.
Rust 1.94.1. This audit does not import Manafold's identity contracts.

## 1. Identity questions are distinct

| Identity | Meaning | Current MTG Kernel support |
|---|---|---|
| Exact physical state | Every stored authoritative value, allocation identity and ordered collection agrees. | `GameState: PartialEq + Eq`; `state_hash()` is a 64-bit accelerator only. |
| Structural equality | A separately specified canonical representation agrees after approved normalization. | No MADS canonical encoder or approved normalization exists. |
| Future-semantic equivalence | All future legal decisions, transitions, terminals and search-visible information agree. | NOT PROVEN for any real-game normalization. |
| Player information-set equality | The same player knows the same permitted information, without access to hidden cards or future RNG. | Existing observations/keys are perspective-scoped, but no MADS information-set adapter exists. |

`GameState::state_hash()` is FNV-1a over Rust `Hash`; it is target/runtime oriented and can collide. `diagnostic_state_hash()` serializes the full state into a versioned JSON envelope, then reduces it to 64 bits; it also can collide and excludes decision/surface context. Neither is an equality proof or a complete DecisionStateKey.

## 2. Classification rule for this audit

Every state-bearing field below is classified `INCLUDE` for an exact bootstrap identity. This intentionally over-discriminates: there are **no** `NORMALIZE` or `EXCLUDE` decisions. A field documented as diagnostic or redundant remains included until a paired-state/bisimulation test proves that removing it preserves future semantics. `BENCHMARK_NAMESPACE` fields identify the whole fixture/search run, not an individual game state.

No production or test-facing real-kernel DecisionStateKey implementation is authorized by this audit. In particular, there is no key which currently compares the full GameState, PolicySurfaceV5, staged Decision, revisions, ordered candidates and RL/session context by exact equality. Therefore no real-engine TT is enabled.

## 3. Authoritative GameState inventory

| Field(s) | Classification | Reason / counterexample to any future omission |
|---|---|---|
| `objects` (`Arena<GameObject>`) | INCLUDE | Arena index is `ObjectId`; append order determines future IDs. Arena length, every object and object order matter. Zone changes retain IDs; tokens and spell copies append new objects. |
| `players` | INCLUDE | Both players' life, zones, mana, loss flags, turn counters and dungeon state affect rules and future choices. Each inner ordered zone remains ordered. |
| `turn`, `active_player`, `priority_player`, `starting_player`, `step` | INCLUDE | Affect turn structure, legal actions, draw-skip and which player controls the next decision. P1-start and P0-start states are not interchangeable. |
| `stack` | INCLUDE | Order, stack-item IDs, controller, targets, cast mode/cost metadata, inline effects and copy/trigger flags alter resolution and priority. |
| `exile`, `command`, `initiative` | INCLUDE | Zone contents/order, permissions and global Initiative affect later legality and triggers. |
| `library_knowledge`, `hand_knowledge` | INCLUDE | Observer/owner rows, positions, object IDs and zone-change generations govern what a player may know. Omniscient library order is not a player information key. |
| `randomness` | INCLUDE | Preserve representation/mode and exact future stream: legacy SplitMix64 cursor, or environment-v2 pair root and owner/purpose shuffle ordinals. Same board with a different future shuffle stream is not the same deterministic search state. |
| `engine` | INCLUDE | Includes priority bookkeeping, pending choices, continuations, event/trigger state and rule counters listed below. |

### Nested values are recursively included

| Type / fields | Classification | Reason |
|---|---|---|
| `GameObject`: `card_def`, `name`, `owner`, `controller`, `zone`, tap/sickness/damage/counters, attachments, `v4`, spell-copy origin, plotted turn, zone-change count | INCLUDE | Exact object incarnation, rules characteristics, links, permissions and zone history. `name` is documented debug-only, but is retained in the exact bootstrap key rather than normalized without a future-behavior proof. |
| `ObjectStateV4`: token/face/color/subtype state, chosen color, entry turn, ability-use counters, skip-untap, damage keywords, goad, attachments/exile links, ward, blocker override, landwalk, cast origin and finalized-cast binding | INCLUDE | These values affect current/future rules, target validation, costs, combat or incarnation identity. |
| `PlayerState`: life; ordered library/hand/battlefield/graveyard; mana pool; lost/draw/land/spell counters; dungeon state | INCLUDE | All values can change legality, triggers, termination or future information. |
| `StackItem` and `StackStateV4` | INCLUDE | Preserve item/source identity, order, targets, cast route, mode, X/cost bindings, kicked/madness/copy semantics and resolution program. |
| `ObjectId`, `StackItemId`, links and zone-change generations | INCLUDE | Object IDs are stable arena indices; StackItemId identifies one particular stack incarnation. A reused physical source is not the same stack object. |
| `LibraryKnowledgeEntry`, `HandKnowledgeEntry` | INCLUDE | Preserve observer, position/object association and incarnation generation. |

### EngineState inventory

All `EngineState` fields are `INCLUDE`, including fields described as diagnostics or transient. No field-level omission proof exists, and continuations can bind to history indices or exact object incarnations.

```text
next_stack_item_id; priority_passes; priority_round; stack_len_at_round_open;
pending_cast; pending_activation; pending_discard; pending_optional_cost;
pending_optional_cost_sacrifice; pending_spell_copy; pending_effect;
event_log; event_history; active_replacements; next_replacement_id;
linked_exile_records; pending_triggers; combat; until_end_of_turn;
mana_ability_activations; mana_ability_count_at_round_open;
pending_kicked_source; exile_play_permissions; next_effect_timestamp;
halted; last_mana_ability_activator; pending_land_play; initiative_source;
until_next_turn_keywords.
```

Nested `PendingCast`, `PendingActivation`, `PendingDiscard`, optional-cost/copy continuations, `EffectContinuation` (resolving item, execution context, frames, pending choice and answer guard), replacement records, pending triggers, combat assignments, permissions, and duration effects are recursively `INCLUDE`. For example, omitting `event_history` is not approved: `EffectContinuation` guards and linked-exile records can refer to history/event identity, while event order is retained as an engine contract.

## 4. Decision and construction identity

`Decision` is not part of `GameState`. Its variant, actor, payload, ordered legal candidates and construction progress must be included at a game-decision/construction boundary. Examples include:

```text
CastSpellOrPass candidate lists;
target/cost-target candidates and remaining counts;
cast mode, kicker, spell mode and optional-cost choices;
effect option/target/boolean continuation;
discard choices;
attacker/blocker scan prefix, cursor and selected objects;
trigger ordering; terminal or halted classification.
```

`PolicySurfaceV5` has state beyond GameState. Every field below is `INCLUDE`:

```text
PolicySurfaceV5.inner and scan;
CombatScanV5 variant, player, attacker where applicable, ordered_candidates,
cursor, selected prefix and exact environment binding;
HarnessSurfaceV2 suppression_audit_mode, suppression_counts, suppressions,
debug_surface_walk, blockers (remaining/accumulated/current attacker),
combat_priority_spent, combat_priority_round_seen,
combat_priority_stack_len_seen, combat_priority_mana_count_seen,
combat_round_opening_mana_count, round_opening_stack_len,
stack_len_round_seen, last_seen_stack_len, mana_count_at_last_stack_change,
madness_cast_reprompt_exemption, discard reshape, optional-cost reshape.
```

Some surface fields are diagnostic or audit-only in a particular constructor, but there is no proof that all constructors, future transitions and observation bindings treat them as irrelevant. The fact that `PolicySurfaceV5` is Clone-only, and does not expose full structural equality, is a blocker to a proven session key.

For `RlEpisodeSessionV1`, exact bootstrap identity would additionally include every stored field:

```text
deck_ids, deck_hashes, episode_id, max_physical_decisions,
max_policy_steps, GameState, PolicySurfaceV5, environment_revision,
policy_step_count, physical_decision_count, current, terminal;
CurrentDecisionV1 actor, physical_decision_id, substep_index/substep_count,
ObservationV5, ordered candidates, environment_revision,
bound_policy_step_count and bound_physical_decision_count.
```

For `FastActorSessionV1`, include:

```text
deck_ids, deck_hashes, episode_id, limits, GameState, PolicySurfaceV5,
environment_revision, policy_step_count, physical_decision_count, current,
flat_action_contract_mode, both spare caches and terminal;
FastActorCurrentDecisionV1 actor, decision_kind, origin_decision,
physical_decision_id, substep_index/substep_count, ordered core candidates,
environment_revision, bound counters, V1/V2 caches and cached errors.
```

`environment_revision`, `policy_step_count`, `physical_decision_id` and substep context are not normalized: they bind stale-action validation and can alter future policy protocol behavior. Stable action IDs, observations and caches are also retained until their relation to future MADS search is proven.

For `DecisionConstructionNode`, the typed original decision, controller, owner game decision, already-selected prefix, continuation payload and candidate order form the construction identity. A target choice or mode choice is not automatically a new physical MAX/MIN turn; its chooser's role must be derived from the authoritative actor relative to the MADS root player.

## 5. Benchmark namespace

These identities must namespace an entire MADS table/fixture. They are not per-node omissions:

| Namespace | Classification | Status |
|---|---|---|
| MTG Kernel source/build and engine/rules contract | BENCHMARK_NAMESPACE | Exact commit must be recorded. |
| Card database/content and deck fixture identity | BENCHMARK_NAMESPACE | Exact content identity required for a future engine adapter. |
| Action/decision and policy-surface schema versions | BENCHMARK_NAMESPACE | Must match the live decoder and construction protocol. |
| RNG/environment-randomization contract | BENCHMARK_NAMESPACE | Exact version plus per-state RNG cursor/root/ordinals still included in the node identity. |
| MADS key and scheduler versions | BENCHMARK_NAMESPACE | `mads.decision-state-key.v1` is reserved, not implemented for MTG states. |

## 6. Exclusions and normalization

| Candidate | Classification | Decision |
|---|---|---|
| GameState `name` display string | INCLUDE | Explicitly debug-only in `GameObject`, but retained for over-discriminating equality. No canonical equality tests or state-key implementation. |
| `event_history`, transient event log | INCLUDE | No proof that all effect guards, trigger provenance and future observables are independent of them. |
| allocator/counter IDs and revisions | INCLUDE | Arena IDs allocate future identities; revisions bind transactions; no equivalence proof. |
| derived observation/candidate/cache material | INCLUDE | Derived does not mean semantically irrelevant; cache equality/invalidation and candidate order are unverified. |
| deck/engine/schema/version data | BENCHMARK_NAMESPACE | Fixed for a complete run; no cross-namespace merging. |

Thus: **zero approved NORMALIZE fields and zero approved EXCLUDE fields**.

## 7. Hashing and collision contract

`state_hash()` covers the GameState's Rust `Hash` value; the manual implementation preserves historical sequences and randomness-mode identity. `diagnostic_state_hash()` covers the serialized GameState envelope. Both return 64 bits and neither includes `PolicySurfaceV5`, the current `Decision`, session counters or the complete ordered policy candidate binding. Hash equality is never a merge proof. Any future table must verify exact key equality after hash/bucket lookup.

The only safe bootstrap idea would be an over-discriminating key that owns and compares the full exact GameState plus all decision/session context above, with `Eq` after hash lookup. The current public types do not provide that full exact session equality, and a 64-bit privileged environment hash is not a substitute. Therefore even this bootstrap key is **NOT IMPLEMENTED** and a real-engine TT remains disabled.

## 8. Hidden-information boundary

`GameState` contains omniscient ordered libraries, opponent hand contents, and both observer knowledge rows. An internal exact state key is not permission to supply that state to a player. Existing policy observations project only actor-authorized information; information-set identities are perspective-local. MADS-01 does not construct a live agent root from an ordinary Pauper session, does not put hidden card identities into agent keys, and does not claim a fair perfect-information adapter.

## 9. Required future counterexample tests

No normalization/exclusion is approved, so there are no affirmative merge tests in this milestone. Any proposal to change that requires paired-state tests for at least:

```text
different library order or RNG state; same board/different arena allocation;
same board/different stack incarnation or target binding; different priority;
different pending mode/target/cost/effect continuation;
different combat assignment/order; different retained player knowledge;
different policy scan prefix/candidate order/revision; different rule history.
```

For every proposed equivalent pair, compare the full legal decision protocol, terminal classification, actor-authorized observation, corresponding successor identities and deterministic randomness through the oracle's bounded depth. Any counterexample rejects that normalization.

## 10. Gate result

| Item | Status |
|---|---|
| Full GameState and EngineState field inventory | AUDITED; all recursively included |
| Decision/policy-surface/session context | AUDITED; exact identity required, not presently comparable as a single Eq key |
| NORMALIZE / EXCLUDE field with proof | NONE |
| Full exact bootstrap MTG key | NOT PROVEN / NOT IMPLEMENTED |
| Real-engine transposition table | BLOCKED |
| Isolated OracleFixtureId identity | ALLOWED only within a self-contained fixture; fixture IDs are exact semantic identity by construction |
| Fair perfect-information MTG search adapter | NOT IMPLEMENTED; hidden-information firewall applies |
