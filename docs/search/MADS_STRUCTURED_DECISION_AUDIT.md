# MADS Structured Decision Audit

Audit baseline: `ec46cd068b7a408259e39ad1fc64fded36cf4658` (`origin/main`,
updated for PR #11 integration). This baseline includes the prior MADS-02B,
state-key-contract, and MADS-02F-C Oracle-stress changes. Audit branch:
`feat/MADS-02E-structured-decisions-audit`.

This document maps the current engine protocol to the MADS V0.4 distinction
between `GameDecisionNode`, `DecisionConstructionNode`, and `TerminalNode`.
It does not implement that adapter, a state key, or POR.

## Findings at a glance

- `engine::advance_until_decision` returns authoritative action-answer points,
  but a `Decision` is not consistently a completed physical game action.
- `Action::CastSpell` and `Action::ActivateAbility` begin staged operations.
  `begin_cast` even moves a placeholder spell onto the stack before mode,
  target, and cost selections are complete. Only `finalize_cast` commits the
  completed cast and resumes priority.
- Cast and activation continuations ordinarily retain their initiating actor.
  The engine validates and rejects an unrelated action while one is pending.
- Some choices are real decisions by a different player inside an unresolved
  effect, notably Chain Lightning's affected-player copy payment/retarget
  sequence. APNAP trigger placement can also expose one controller's order
  decision and then another controller's order decision before priority.
  These actor changes must not be hidden inside a same-owner construction node.
- `HarnessSurfaceV2` and `PolicySurfaceV5` create policy microsteps for combat,
  discard, and optional-cost prompts. Those steps are not raw Engine decisions
  and are not complete physical decisions by themselves.
- No Engine Decision variant asks the player to choose a replacement effect.
  Implemented replacement effects are applied by the event replacement pass.
- The real Engine `DecisionStateKey` remains `NOT_PROVEN`; the separately
  merged state-key contract does not prove a complete live Engine identity.
  All POR candidates below remain disabled.

## Construction-node contract required by MADS V0.4

V0.4 defines a `GameDecisionNode` as a complete authoritative decision
boundary and a `DecisionConstructionNode` as a partial response within an
owned protocol. Its examples include a chosen spell with mode unresolved, a
chosen mode with targets unresolved, and chosen targets with payment
unresolved. Construction choices retain their owner/role; they do not switch
MAX/MIN merely because a protocol has another prompt. The Oracle has separate
caps for complete game actions, construction continuations, depth, nodes,
edges, and authoritative transitions.

The current `DecisionConstructionNodeV1` shape in `oracle_suite_v1.rs` has an
owner game node, actor, role, `protocol_key`, exact `partial_response`, and
`continuation_cursor`. That is useful fixture vocabulary, but it is not bound
to any live Engine continuation. A live key/payload would need, at minimum:

- the exact owning decision and Engine actor;
- a typed protocol/stage identifier, not only a `Decision` variant or display
  text;
- exact source `ObjectId`, zone-change generation, controller, stack-item
  identity, and cast/activation route where applicable;
- every selected target and its target contract in order;
- the exact object-cost/discard selection prefix, remaining cardinality, and
  legal candidates;
- mode, kicker, X, optional-cost, cast-mode, and completion/finish flags;
- the `PendingDiscard::resume`, `EffectContinuation` frame/choice/guard, or
  `PendingSpellCopy` stage which will resume after the answer;
- the `HarnessSurfaceV2`/`PolicySurfaceV5` scan prefix and cursor if the
  selected interface is a policy microstep;
- the complete stable semantic identity and order of all legal choices.

Hash equality, a public observation, or the unchanged physical board is not a
construction identity proof. The exact decision/session context is outside
`GameState`, and the current Engine state-key audit conservatively includes
all state-bearing fields.

## Decision and continuation mapping

“Engine successor” below means the state reached after the answer and the
subsequent `advance_until_decision` walk. `engine::step` can stage a partial
answer; it does not promise that the physical action has committed.

| Raw `Decision` / action family | Full physical decision or construction step? | Owner | Continuation and commit point | Identity that must remain distinct | Candidate/adapter status |
|---|---|---|---|---|---|
| `CastSpellOrPass` → `Pass` | Complete priority response | `priority_player` | Pass flag and priority advance in `step`; actual resolution waits for both passes | Priority actor/round, stack and pending effects | V5 emits `Pass`; exact for this action |
| `CastSpellOrPass` → `PlayLand` | Usually complete land play; partial if the land has an as-enters color choice | `priority_player` | Ordinary `play_land` commits immediately. A color-choice land creates `PendingLandPlay`; `ChooseEffectOption` commits the land | Land object/incarnation, chosen color, controller, excluded color, land-play count | V5 maps ordinary play; color continuation is state-contextual |
| `CastSpellOrPass` → `ActivateManaAbility` / choice / cost target | Complete mana-ability action, not a pending spell payment stage | `priority_player` | Cost/effect and mana addition occur in `activate_mana_ability_for`; no stack item. Priority-pass flags reset; caller can then cast, act, or pass | Source, selected color, extra cost target, tapped/sacrificed incarnation, resulting mana pool, activation counters, priority round | V5 enumerates available colors and any cost targets; do not silently bundle several activations into one cast construction |
| `CastSpellOrPass` → `ActivateAbility` | Starts an activation construction | `priority_player` / activated ability controller | `begin_activation` creates `PendingActivation`; final cost/stack item only after target, object-cost, and discard continuations | Source incarnation, ability index, controller, target prefix/contracts, object-cost prefix, discard resume | V5 emits the initiating action; successor is not the completed activation |
| `CastSpellOrPass` → `CastSpell` | Starts a cast construction, not a completed cast | `priority_player` / caster | `begin_cast` moves the card to the stack and pushes a placeholder immediately; mode/target/cost decisions follow. `finalize_cast` pays costs and binds final metadata; caster then retains priority | Source contract, origin zone and cast route (hand/Flashback/Escape/Plot/Madness), modes, ordered targets/contracts, cast mode, kicker, X, optional/additional/object costs | V5 emits the initiating action. Root label must say “begin cast” if this edge is exposed; it is not a complete physical cast |
| `CastSpellOrPass` → `PlotSpell` | Complete special action | `priority_player` | Pays plot cost, moves card to exile, stamps plot turn, keeps priority | Exact exiled object and plotted incarnation/turn | V5 emits a separate `PlotSpell`, distinct from cast |
| `ChooseKicker` | Construction step | Pending caster | Stores the pay/decline answer in `PendingCast`; then the cast walk continues | Owner, source contract, kicker choice, available mana state | V5 emits false/true; no role switch |
| `ChooseSpellMode` | Construction step | Pending caster | Stores printed mode index, then target selection continues | Printed mode count/index, viable-mode set, owner, source, target spec | V5 emits each `legal_modes` index; mode identity/order matters |
| `ChooseTargets` | Usually a per-target construction step; source may be cast, activation, spell copy, or trigger | Producer’s controller/target chooser | Appends target and `StackTargetContractV4`; pending producer finalizes only when its target protocol is complete and `advance_until_decision` resumes it | Producer kind, source/stack ID, target spec, ordered prefix/contracts, remaining/min/max, finish marker | V5 enumerates `legal_targets` plus Finish when allowed. Same raw variant can belong to different protocols |
| `ChooseCastMode` (Normal/Alternative) | Construction step | Pending caster | Stores payment mode; object-cost selection/automatic payment/finalization follow | Cast route, viable modes, target prefix, source and cost components | V5 enumerates offered modes. This is an alternative cost choice, not a mana-source choice |
| `ChooseCostTargets` | One-object-at-a-time construction step | Pending cast/activation controller | Appends a selected object; actual sacrifice/exile/tap/return payment occurs at finalization | `CostKind`, source/ability, remaining count, ordered chosen IDs, candidate list and incarnations | V5 enumerates supplied candidates. Cost-family and prefix are part of protocol identity |
| `ChooseEffectOption` | Context-dependent construction or resolving-effect step | Actor named by the matching continuation | Can choose a land color, cast optional cost, chosen-creature cost zone, X, or an effect branch. Land play/cast/payment/effect completion happens in the associated continuation | Typed purpose/path, source, option order/count, `PendingCast`/land/effect continuation stage | V5 specializes colors and otherwise enumerates indices; a raw variant alone is not a protocol key |
| `ChooseEffectTargets` | Per-target continuation step | Actor in pending cast/activation/effect choice | Appends a target; `FinishEffectSelection` or reaching max resumes the appropriate cast/effect | Pending protocol, source, selected ordered prefix, min/max and `ordered` flag in `PendingEffectChoice` | V5 maps candidates plus optional finish; not a completed effect resolution |
| `ChooseEffectBoolean` | Resolving-effect continuation step | Actor in `PendingEffectChoice::ChooseBoolean` | Records the Boolean and resumes the exact effect frame | Source stack item, frame path, default, purpose, answered-choice guard | V5 emits false/true; keep continuation context |
| `ChooseOptionalCost` | A resolution decision; H2 may expose two policy microsteps | Actor in `PendingOptionalCost` | Decline resumes resolution. Discard creates `PendingDiscard::FinishOptionalCost`; sacrifice creates `PendingOptionalCostSacrifice`; the effect resumes after payment | Source, payable branches, chosen branch, `then`, deferred spell movement/resume | Raw engine has three choices. V5 only accepts H2 sentinel combinations and presents a binary Use/Which reshape |
| `Discard` | Complete discard choice at the Engine level; incomplete physical protocol when it resumes a cast, activation, spell resolution, or optional cost | `PendingDiscard.player` | Moves the chosen group; `DiscardResume` selects cleanup, `FinishCast`, `FinishActivation`, `FinishSpellResolution`, or `FinishOptionalCost` | Exact chosen vector/order, count, eligible choices, and full `DiscardResume` binding | V5 rejects raw `count != 1`. H2 reshapes multi-card choices to successive single-card microsteps and commits at the final pick |
| `DeclareAttackers` / `DeclareBlockers` | Aggregate declaration actions are complete combat declarations. V5’s inclusion steps are construction microsteps | Active player for attackers; defending player for blockers | Engine commits the aggregate `Vec` in one step. H2/V5 scans collect Boolean inclusions first and commit the aggregate only after the scan | Eligible/candidate order, selected prefix, current cursor, goad/minimum-blocker constraints; blocker assignment order | Raw attacker subset generator is capped at 12 objects. Raw V5 rejects aggregate combat semantics; H2 auto-resolves empty attackers and reshapes nonempty combat. Raw `DeclareBlockers` is explicitly rejected by V5 |
| `OrderTriggers` | Complete ordering choice for one controller’s APNAP group; not necessarily the whole pending trigger batch | Controller of the first pending group | `apply_order_triggers` marks/reorders that group and attempts stack placement. A later controller group can still require its own order decision before priority | Trigger group boundary, source/ability identity, exact permutation, APNAP position, placement progress | V5 enumerates permutations up to 7 triggers; larger groups fail closed. Oracle’s 64-edge node cap is smaller than 5! |
| `ChooseSpellCopyPayment` | Real decision during an unresolved Chain Lightning resolution; not the caster’s pending cast | `PendingSpellCopy.player` (affected player; can be opponent of source controller) | Pay creates a fresh copy and advances to Retarget; decline resumes the resolving source | Resolving stack item/source contract, affected player, inherited target and contract, copy identity/stage | V5 emits pay/decline. Actor may legitimately differ from original spell’s controller |
| `ChooseSpellCopyRetarget` | Copy-resolution construction step, owned by affected player | `PendingSpellCopy.player` | Keep inherited target or move to `Target`; copy remains a distinct stack incarnation | Parent/copy stack IDs, source, target contract, stage | V5 emits change/keep; if target is chosen next, it remains the same affected player |
| `ChooseMadnessCast` | Choice on a resolved Madness offer; accepting starts a new cast construction | Player named by validated Madness offer (the discarder) | Decline completes the offer. Accept moves exact card to its Madness cast route and calls `begin_cast_ex`; normal cast continuations follow | Offer stack/source incarnation, exile card incarnation, Madness route/cost, cast continuation | V5 emits cast/decline. The offer can only appear after ordinary stack priority has been passed |
| `GameOver` | Terminal | None | Exact terminal result | Winner/draw relative to root | Terminal mapping only |
| `Halted` | Unsupported/error, never a value | None | No answer; engine remains halted | Mechanic/source for diagnosis | Fail closed; never map to LOSS/DRAW/WIN |

There is no raw `Decision::ChooseReplacement` or `Decision::ChooseManaPayment`.
The current `event::apply_replacements` automatically applies the implemented
`PreventNextDamage` and `PreventDamageFromColorUntilEndOfTurn` replacements in
active-replacement vector order; it does not ask a player. If a supported
state ever requires an affected-player replacement choice that this kernel
does not represent, an MADS adapter must mark that protocol unsupported rather
than inventing or merging a choice. `finalize_cast` chooses a deterministic
`mana::can_pay` plan and `pay_plan` executes it synchronously. Ordinary mana
abilities are separate priority actions before a cast, not an internal payment
prompt.

## Authoritative path sketches

### Spell cast

```text
GameDecision(CastSpellOrPass, caster)
  -- Action::CastSpell -->
Construction(PendingCast: placeholder already on stack)
  -> kicker / optional additional cost
  -> spell mode
  -> ordered target picks / finish
  -> Normal vs Alternative cast mode
  -> object-cost picks / optional-cost picks / X / mandatory discard
  -> finalize_cast: pay and bind final stack item
  -- advance_until_decision -->
GameDecision(CastSpellOrPass, caster; caster retains priority)
  -- caster passes -->
GameDecision(CastSpellOrPass, opponent; unresolved stack item)
```

The precise live order is the order in `drain_pending_cast_or_decide`, not an
assumed generic “mode, target, payment” order: kicker and optional additional
cost precede printed spell mode; targets precede the Normal/Alternative
payment-mode choice; cost targets and Collect Evidence/Bargain/chosen-creature
selections follow; X and mandatory additional discard precede finalization.
Some stages auto-resolve if only one choice is legal.

`engine::step` rejects `Pass` and unrelated action families while
`pending_cast` or `pending_activation` is live. A chosen target only extends
the continuation prefix; `advance_until_decision` resumes the protocol and
commits the cast when all stages finish. The opponent does not get priority
between a normal cast’s mode/target/cost microsteps.

### Trigger placement and resolution

```text
committed event batch -> collect_and_process -> APNAP pending groups
  -> OrderTriggers(P0) -> place P0 group
  -> OrderTriggers(P1) -> place P1 group
  -> targeted-trigger choices, if any
  -> priority
```

Each controller makes an actual rules choice over that controller’s trigger
group. Actor ownership legitimately changes between these Engine decisions
while the shared trigger-placement checkpoint remains unfinished. Model these
as separate `GameDecisionNode`s with exact continuation state (or an explicit
multi-owner batch protocol); do not extend P0’s ConstructionNode across P1’s
choice.

Chain Lightning gives a second actor-switch case: its resolving spell’s
affected player may pay to create a copy and then decide whether to retarget
it. That player may be the opponent of the original caster. This is a new
authoritative game decision inside resolution, with the parent stack item and
copy continuation retained. It is not a continuation owned by the caster.
Generic resumable effects also store the chooser in `PendingEffectChoice.player`
and may name an affected player rather than the resolving stack controller
(for example a counter-unless-pays choice). The Engine actor field is
authoritative for that node; source/stack controller is not a substitute.

## Candidate-domain audit

The current policy candidate code is an adapter, not a universal completeness
proof. `legal_action_candidates_v5` calls the policy-surface generator after
`PolicySurfaceV5` transformations. `core_surface_action_candidates_v1` maps
most surfaced Decision variants; `core_policy_action_candidates_v5` creates
binary combat inclusion candidates. The following boundaries are concrete:

| Domain | What the current code does | Audit result |
|---|---|---|
| `CastSpellOrPass` | Emits every `castable_spells`, mana ability/color/cost-target combination, land drop, activation, plot action, then Pass | Complete for a valid raw payload when compared to the Decision fields; MADS-02B independently checked one Burn state. This does not prove all cost/choice variants universally |
| Fixed `ChooseTargets` | Emits each supplied legal target, plus Finish only when `can_finish` | Complete for that surfaced target prefix if the exact pending producer is validated |
| `ChooseCostTargets` | Emits every supplied object candidate | Complete per current pick; the next pick is another construction step |
| Cast mode/kicker/spell mode | Emits all `options`, false/true, or legal printed mode indices | Complete per current stage; each remains inside `PendingCast` |
| `ChooseEffectOption` / Boolean | Enumerates all option indices or both Boolean values; color branches use exact legal colors | Complete only after matching the raw option to its pending continuation purpose/source |
| `ChooseEffectTargets` | Emits each legal target plus Finish when permitted | Complete for the surfaced prefix; `selected_count`, bounds, order, and producer matter |
| `ChooseOptionalCost` | Supports only the H2 Use sentinel `(false,false)` or Which sentinel `(true,true)` | Raw one-shot 3-way Decision is reshaped; other flag combinations fail closed |
| `Discard` | Requires `count == 1`, emits one candidate per offered card | Does not support raw multi-card domain directly; H2 reshapes count>1 into per-card microsteps |
| `DeclareAttackers` | Enumerates subsets up to `MAX_SUBSET_OBJECTS = 12`, filters through Engine validation | Raw V1 generator can enumerate the supplied aggregate set, but V5 forbids aggregate semantics; V5 uses ordered Boolean inclusion scans when eligible is nonempty. Empty attackers are auto-declared by H2 |
| `DeclareBlockers` | Raw generator returns an error: “raw DeclareBlockers is not a HarnessSurfaceV2 decision” | Unsupported on raw V5 path. H2 transforms to ordered per-attacker sets, skips attackers with no remaining blocker, and V5 scans each set as binary blocker inclusion |
| `OrderTriggers` | Enumerates all permutations for up to 7 triggers | Complete within that local cap. At 5 triggers, 120 legal orders already exceed Oracle’s 64 complete-action cap; those need construction enumeration or fail-closed size rejection |
| Terminals | GameOver/Halted yield no candidates | GameOver is terminal; Halted is an error, not an outcome |

Candidate generation caps are not interchangeable with Oracle admission caps.
`subsets()` can enumerate through 12 candidates (up to 4,096 subsets), but an
Oracle game node admits at most 64 complete actions. Trigger ordering supports
up to 7 items (up to 5,040 permutations), while a Construction node admits at
most 32 continuations. If any relevant domain exceeds its applicable cap, the
adapter must return `ORACLE_FIXTURE_TOO_LARGE`/`UNSUPPORTED_DECISION`; it must
not keep an action prefix.

Action semantic IDs are JSON-derived 64-bit strings. The same decision’s V5
generator rejects duplicate full semantics and duplicate IDs, but an ID/hash
is not an equality proof across Engine states or construction contexts. A
construction edge needs the full semantic plus a typed protocol key and
ordered prefix. Surface-generated combat actions are policy microsteps, not
aggregate physical declarations.

## Specific Engine facts that affect construction

- `PendingCast` contains controller/source contract, cast route, mode, kicker,
  optional cost, target prefix/contracts, object-cost choices, X, and discard
  continuation data. `begin_cast` puts a placeholder on the stack at
  announcement; `finalize_cast` is the cost/final-binding commit point.
- `PendingActivation` similarly captures source incarnation, controller,
  ability index, target prefix/contracts, object costs, and discard state;
  `finalize_activation` is its commit point.
- `PendingLandPlay` holds source incarnation/controller/origin/excluded color;
  the land does not enter until the as-enters color answer calls `play_land`.
- `PendingDiscard.resume` distinguishes cleanup, cast cost, activation cost,
  resolving-spell discard, and resolution optional cost. The same `Discard`
  Decision shape therefore does not identify the protocol.
- `EffectContinuation` retains the resolving `StackItem`, execution context,
  frames, typed `PendingEffectChoice`, and answered-choice guard. Its choice
  path and frame stack are part of construction identity.
- `PendingSpellCopy` retains resolving source/stack incarnation, affected
  player, inherited target and contract, copy stack identity, and Payment /
  Retarget / Target stage.
- `pending_triggers` is already APNAP grouped. A controller ordering choice
  may be followed by targeted-trigger choices or a different controller’s
  ordering choice before normal priority.
- Replacement state is an ordered list with replacement IDs and mutable
  prevention counters. `event::apply_replacements` repeatedly applies the
  first currently applicable, not-yet-touched replacement. No player chooser
  Decision exists in this kernel increment.
- `GameState` clone/equality includes full Engine continuation state, but a
  live construction identity also needs Decision plus surface/session context.
  The state-key audit remains `NOT PROVEN`; no TT reuse is supported.

## Information boundary

The executable tests inspect privileged authoritative states only to verify
Engine protocol transitions. The Burn cast probe starts from the official
runtime Burn mirror and observes both players' full state internally; the
Fireblast/APNAP fixtures are synthetic protocol checks. All are
`ENGINE_ORACLE_ONLY`. They are not fair perfect-information states and do not
emit MADS teacher labels or training data.

## Safe POR candidates — analysis only, none enabled

`NO CERTIFICATE => NO REDUCTION.` Utilities below are theoretical branching
upper bounds, not measured savings.

| Candidate / equivalence hypothesis | Preconditions and potential positive case | Negative counterexample / falsification | Required key/proof | Possible utility |
|---|---|---|---|---|
| Reorder activations of independent mana sources | Same actor, same produced color/value, no target/cost side effects, no intervening pass/response, both sequences reach the same exact mana pool and source state. Two ordinary Mountains are a candidate pair | One activation is itself a separate priority action; a player can stop after the first and cast/pass. Flexible colors, sacrifice sources, ability-use counters, replacement events, and distinct tapped ObjectIds can change enabled actions. The MADS-02B Burn probe’s two Mountain land/action IDs are intentionally distinct | Exact source incarnations, priority flags/round, mana pool, activation count, stack/replacement/trigger state; compare every interleaving and actor-visible decision, not only summed mana | At most `k!` order variants in a forced batch of k proven-independent sources; no batch/forced-payment protocol currently exists |
| Reorder active replacement effects | For a proposal matched by at most one effect, moving disjoint effects could preserve that proposal’s rewrite | A color-prevention replacement before a one-shot target shield can prevent the event without decrementing the shield; reversing order can consume the shield. The future replacement state differs | Exact ordered active-replacement vector, IDs, counters, touched-by set and all event proposals; prove rewrite plus replacement bookkeeping commute | Potentially factorial in the number of disjoint active replacements; no player choice or reduction certificate exists |
| Reorder Escape graveyard object-cost picks for the same set | `finalize_owned_cast` explicitly canonicalizes Escape’s selected ObjectIds by graveyard-vector order before payment/stack provenance. Same exact set, cast mode/targets/source, no intermediate effect; a promising order-only case | Different selected sets with the same count/value exile different card identities and change future graveyard castability. Any selection-prefix observation or candidate change invalidates a merge. The canonicalization alone is not a full future-bisimulation proof | PendingCast source/route, exact selected set and remaining candidates, graveyard order/zone generations, paid-cost refs, effect and future decisions | Up to `r!` order paths for r selected cards in the same set; zero reduction until paired successor/observation proof |
| Reorder target picks for a multi-target spell/effect | Only if the operation is symmetric in target indices and the chosen set is identical. An effect that consumes a target set as an unordered batch is a candidate to prove | `ExecCtx.targets`, `TargetRef::Target(i)`, target contracts, and ordered effect programs can assign different effects to positions; Winding Way explicitly has an ordered selection. Different targets are always different decisions | Exact producer/mode/spec, indexed targets/contracts, frame path, all follow-up stack/event behavior and observations | Up to `k!` permutations of k same-set target picks; unmeasured |
| Reorder attacker subset declarations | Only permutations of the same legal subset, with no ordering-sensitive trigger/policy continuation and equivalent subsequent blocker protocol | Kernel stores the ordered attacker vector; H2/V5 scan order and blocker prompts derive from it, and power-tie order is retained. Goad/must-attack/requirements also constrain the set | Exact combat state, scan order/cursor, trigger order, blockers and damage assignment; test swapped declarations through combat to terminal | At most `k!` orderings for k selected attackers; no reduction certified |
| Reorder blocker assignments | A trivial single-blocker/single-attacker case has one order, so no nontrivial reduction | `CombatState.blocked_by` preserves blocker order and combat damage assigns lethal from first to last. Swapping two gang blockers can change damage allocation and which blocker survives | Exact attacker/blocker tuple sequence, damage assignment, keywords, replacement/damage triggers, and successor state | Potential factorial reduction in larger gangs, but a known rules-visible counterexample rejects general sorting |
| Reorder a controller’s trigger group | Identical, independent, untargeted effects might commute in a particular group | Trigger order controls LIFO stack order; effects can alter targets/resources or create different follow-up triggers. APNAP groups belong to different actors and cannot be merged across owners | Exact trigger identity/effect/controller, group boundary, chosen permutation, stack order and all resolutions | Up to `n!` orderings; generator caps n at 7, but no safe certificate for current general groups |
| Reorder discard selections with the same card set | A set of ordinary cards with no discard triggers/effects might appear equivalent | Discard order feeds event/zone order and Madness offers. Two Madness cards can create ordered offers whose stack/resolution sequence differs. Graveyard order is stateful | Exact resume kind, selected order, Madness/trigger queue, graveyard order, event history and future legal actions | Up to `r!` permutations for r cards; raw V5 does not currently enumerate multi-card choices directly |
| Collapse duplicate-looking land/cost objects | Two copies with the same definition and no special text are a candidate to investigate | Their ObjectIds and zone-change generations are distinct; selected land/cost choice changes which object remains/taps/sacrifices, and source/target contracts can refer to the exact incarnation. MADS-02B explicitly tests two legal Burn `PlayLand` actions remain distinct | Full arena/zones, object IDs/incarnations, attachments, ability counters, event and observation changes | Could reduce repeated identical-copy choices; no normalization is approved by the DecisionStateKey audit |

No positive general POR certificate is available in this audit. The only
positive signal is Escape’s explicit canonicalization of an identical
selected set; that is a candidate for a future paired-state proof, not
authorization to merge now.

## Executable audit tests

The test-only module `mads02e_structured_decision_audit_v1.rs` adds:

1. An official Burn sequence using the existing runtime deck builder and
   `engine::step`: cast Lightning Bolt, show P0 owns target selection and that
   `Pass` is rejected while `PendingCast` is active, advance to the finalized
   stack item, then show P1 receives priority only after P0 passes. The
   deterministic opening uses seed `0x2` and the cataloged 60-card Burn
   mirror (`source_sha256=4ebba6b42bb27a0ea55001cee133aada81f0dffd8661b46b012fc5026675aa32`,
   runtime hash `0x5fdb7b92986b6fc1`), with P0 starting.
2. A protocol-only Fireblast fixture with six existing Mountains: both
   Normal and Alternative cast modes are legal; the two modes lead to distinct
   committed successors, and selecting different Mountain ObjectIds for the
   Alternative cost also produces distinct exact successor states.
3. A simultaneous-leave event batch for four existing Clockwork Percussionist
   definitions. The authoritative trigger collector produces P0’s APNAP group
   first; after P0 orders it, P1 receives its own `OrderTriggers` decision
   before priority. This is a synthetic protocol fixture, not a claim that a
   normal deck line creates those exact four deaths.
4. A multi-card discard continuation reshaped by H2 into two P0-owned
   microsteps; the hand is unchanged after the first pick and the Engine
   discards the complete set only on the final pick. V5 keeps the three
   distinct card ObjectIds as distinct first-pick actions.
5. Negative V5 checks showing raw aggregate attackers, raw `DeclareBlockers`,
   and raw multi-card `Discard` are refused; their supported paths require the
   existing H2/V5 surface transformations.

Existing regressions also cover H2 per-attacker blocker aggregation and V5
binary attacker/blocker scans in `surface_v2.rs` and `policy_surface_v5.rs`.

## Validation

Post-integration validation was run on the merge of the audit branch with
`origin/main` at `ec46cd068b7a408259e39ad1fc64fded36cf4658`. Every Cargo command
used `/tmp/mtg-kernel-cargo-build.lock`, the same clone-local `target/` cache,
and serial execution. No Release/Thin-LTO or workspace build was run.

```text
cargo fmt --all -- --check                                               => passed
cargo check -p mtg-kernel --lib -j 3                                     => passed
cargo test -p mtg-kernel --lib -j 3 mads02e_structured_decision_audit_v1 => 5 passed
cargo test -p mtg-kernel --lib -j 3 mads_v1                             => 16 passed
cargo test -p mtg-kernel --lib -j 3 mads02f_c_oracle_stress_v1           => 7 passed
cargo test -p mtg-kernel --lib -j 3 policy_surface_v5                   => 13 passed
cargo test -p mtg-kernel --lib -j 3 surface_v2                          => 20 passed
cargo test -p mtg-kernel --test burn_goldfish -j 3                      => 2 passed
cargo clippy -p mtg-kernel --lib --tests -j 3 -- -D warnings            => passed
```

The audit test module is restricted by `#[cfg(test)]`; it adds no production
search or Engine API. The 16 MADS tests and seven MADS-02F-C Oracle stress
tests are the MADS/Oracle regressions run on this integrated commit. The full
workspace suite is intentionally not scheduled for this analysis-only branch.

## Supported scope and blockers

Potential first ConstructionNode protocols, after typed modeling and review:

- same-actor `PendingCast` stages through finalize;
- same-actor `PendingActivation` stages through finalize;
- `PendingLandPlay` as-enters choice;
- same-actor `PendingDiscard` resumptions and H2 discard reshape;
- same-actor effect frames/choices while one stack item resolves;
- one actor-owned combat scan prefix (with aggregate declaration only at
  completion).

They are **not implemented as MADS Engine ConstructionNodes**. The normal
MADS-02E integration remains blocked on: a typed protocol descriptor and
continuation key; an independent complete action-domain contract per stage;
correct actor ownership across copy/trigger cases; role-safe transition
classification; complete fixture/oracle admission under action caps; and an
exact information/session key. Raw blockers, raw `Discard(count > 1)`, and
large subset/permutation spaces must fail closed or be lazily constructed,
never truncated.

The full workspace suite was not run; this branch is limited to the focused
protocol/MADS/Oracle/Surface checks above. No Release build was run, per the
shared-lock Debug/check workflow.

## Integration plan for MADS-02E

1. Define typed Engine protocol IDs and owner extraction for every reachable
   continuation, including explicit “actor switch/new GameDecision” edges.
2. Add an adapter result that classifies each successor as complete
   `GameDecision`, same-owner `DecisionConstruction`, terminal, or
   `UNSUPPORTED_DECISION`; never infer completion from `Action` variant alone.
3. Build complete per-stage legal domains independently of V5 serialization;
   enforce Oracle caps before returning an admitted fixture, with no prefix
   truncation.
4. Add differential traces for cast, activation, Fireblast payment, discard
   resumes, combat scans, trigger APNAP groups, Chain Lightning copy/retarget,
   and generic effect frames.
5. Keep all state merges and POR disabled. Require exact state/continuation
   identity and paired future-decision/terminal comparisons before proposing
   any normalization or reduction.
6. Only after adapter/oracle correctness is reviewed, connect the typed nodes
   to the production dynamic MADS root-critical scheduler. No teacher labels
   until a separate hidden-information contract passes.

No Engine, RNG, RL, replay, MADS graph, scheduler, or production candidate code
is modified by this audit.
