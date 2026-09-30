# MADS-03B5 – Authoritative Engine Binding V3

## Result

This increment adds the public `DynamicEngineSearchV3` adapter in
`mtg-kernel/src/mads03b5_engine_binding_v3.rs`. On the admitted deterministic
Lightning Bolt fixture, V3 reads its raw root and target domains from
quiescent Engine decisions, constructs a `VirtualPhysicalRootV3`, and binds
three complete physical responses to Engine-produced successor decisions:

```text
CastSpell(Bolt) → ChooseTarget(P0)
CastSpell(Bolt) → ChooseTarget(P1)
Pass
```

The target count and ordered targets are captured from the actual
`Decision::ChooseTargets`; P0/P1 describe this fixture only. Every response
has a distinct physical identity, its own V3 bound cell, and an authoritative
successor binding. All bounds remain `UNKNOWN`; no exact root value or root
certificate is produced.

```text
DYNAMIC_V3_ENGINE_INTEGRATION = TESTED_ON_ADMITTED_FIXTURE
ENGINE_FRAME_VALIDATED = YES_ON_ADMITTED_FIXTURE
ENGINE_TRANSITION_VALIDATED = YES_ON_ADMITTED_FIXTURE
PHYSICAL_RESPONSE_IDENTITY_VALIDATED = YES_ON_ADMITTED_FIXTURE
REAL_ENGINE_VALUE_SEARCH_PROVEN = NO
LIVE_TT = DISABLED
```

This is a privileged research adapter, not a player-facing search surface or
teacher interface. Frame and transition labels are path-local caller-side
provenance handles; they are not cryptographic Engine attestations, globally
stable state keys, or permission for state merging.

## Base and implementation

The branch is based on `origin/main` at `4df065390e74aa55afc206b3a22d7c0e41ddf70c`.
The adapter leaves the V1 and V2 result contracts unchanged and uses the
existing V3 virtual-root types. `DynamicEngineSearchV3::new` re-evaluates the
passed root decision on a clone, verifies quiescence and exact decision
equality, and admits only a priority frame whose full domain is exactly one
castable Lightning Bolt plus Pass, with no mana abilities, land drops,
activations, plot actions, or other castable spells. An out-of-scope root
domain is rejected as a whole. The existing Fireblast construction is a
negative fixture for this rule.

Frame and response identity are explicitly versioned typed projections:

- Root and target decision fields are serialized field-by-field in their
  current Engine order. `Debug` formatting is not used as a persistent key.
- The admitted action projection supports only `CastSpell(ObjectId)`,
  `ChooseTarget(Player/Object)`, and `Pass`, using length-framed
  `engine-action-projection.v3` identifiers. A round-trip test covers each
  admitted shape.
- Frame identities include a run-local session, ordinal, parent transition,
  actor, typed decision identity, and the ordered candidate identities. The
  frame privately owns its exact `GameState` clone and is re-evaluated on
  clones before admission or use.
- The run-local session makes raw frame/action IDs intentionally unsuitable
  for cross-run equality or TT reuse. Repeated runs are compared by their
  normalized typed response paths and measured work instead.

## Root ordering and prefix lifecycle

The raw root order is returned by V3 and validated as `[CastSpell(Bolt),
Pass]`. Its virtual root order reserves a `2^16` stride per raw-root item:
the Bolt target choices use raw-root order zero and their observed target
indices; Pass keeps raw-root order one at `65536`. This avoids collisions,
keeps target order as observed, and lets the raw priority order be recovered.
No candidate sorting or truncation is used.

Initially the V3 root contains one `UNKNOWN` unresolved Bolt prefix and one
known, unexpanded Pass alternative. The immutable root seal names those
original raw items and is linked to the validated root frame. After the
authoritative CastSpell transition, the prefix advances to the actual target
frame through separate transition provenance. The complete ordered target
domain is validated against that frame before it atomically replaces the
prefix with one candidate and one bound cell per Engine candidate. The old
root seal is never rewritten or treated as evidence for the new continuation;
the progressed prefix uses its transition attestation and the candidate
alternatives carry the exact target-frame domain evidence. Thus its old prefix
ID describes the original raw-root alternative only, not the current target
prefix or any admitted complete physical response.

Candidate admission uses the `DecisionConstructionV1` PendingCast target
classifier and the actual pending cast on the post-CastSpell state. It checks
the initiator, Lightning Bolt source, empty selected-target prefix, one
remaining mandatory target, target-stage actor, complete ordered legal
candidate list, and candidate uniqueness. The typed candidates are compared
with the re-evaluated Engine frame. The current V5 projection remains a
secondary projection exercised by the existing V2 test; it is not described
as an independent legality oracle.

The model frontier exposes actual root and construction `ExpansionSlot`
identities and separate support IDs. At budget zero the Cast announcement
prefix and Pass are schedulable separately. After complete candidate
admission, the two target candidates have different root support IDs and
target-stage slots, while Pass retains its own root slot. The execution order
used by `run_v3` is explicitly
`FRONTIER_PHYSICAL_ROOT_ENUMERATION_V3_CAST_THEN_PASS_THEN_TARGET_ORDER`;
it does **not** claim to choose work using incumbent/challenger values. The
separately versioned V3 model frontier is rebuilt and reported, but the
adapter does not run a subsequent root-critical value search.

Budget-one processing performs the Cast transition and atomically admits the
entire verified target domain. There is then no unresolved candidate-domain
envelope, but the target alternatives remain only
`PartiallyConstructed`—no physical response is prematurely complete. Budget
two completes only Pass. Subsequent work completes the two target responses.
If Engine candidate admission had failed, the prefix would not be replaced.

## Transition and successor binding

The initial CastSpell is applied to an owned root clone. For each complete
Bolt target answer, V3 independently replays CastSpell from a fresh clone of
the original root state, reclassifies the resulting PendingCast target frame,
and compares the replayed frame's exact state, decision, ordered candidates,
and path-local identity with the admitted target frame before applying that
target. The target response then runs through `engine::step` and
`advance_until_decision`. Completion is accepted only when PendingCast is
cleared, the source has an actual finalized cast binding, the stack contains
the Lightning Bolt spell controlled by P0, and its stack target equals the
selected response.

Pass runs from a separate root clone and is bound to the actual P1 priority
decision. Bolt target successors are actual P0 priority decisions. Each
successor binding records the full ordered typed response path, owner graph
node, source frame, transition label, physical response identity, successor
frame/node, actual actor, finalization evidence, and path-local ancestry.
The Engine has no transition-ID API, so the adapter derives a framed
run-local transition label from the validated source frame, typed response,
and path ordinal; the binding is created only after the corresponding
authoritative Engine transition succeeds and its resulting frame is
revalidated. This is structural in-process provenance, not authentication.

The V3 structural contract was strengthened for this adapter: authoritative
successor provenance carries a distinct successor-frame identity and an
explicit transition-response identity, and prefix progress carries source
frame, transition, response, and successor frame. Validation requires the
transition response to equal the last action in the exact physical response
path, and the complete physical response identity to match the alternative.
Focused tests reject wrong source frame, empty transition, wrong transition
response, wrong physical response, wrong successor frame/node, and actor
mismatch with no partial completion. Existing V3 tests also cover invalid
domains, prefix mismatches, and cyclic ancestry.

## Values, completion, and frontier limits

Physical completion, root-domain completeness, and root value are separate:

- `root_domain_complete` means all admitted raw-root and target candidate
  alternatives are represented in the V3 model; it does not say each
  alternative was transitioned or valued.
- `PhysicalAlternativeStateV3::Completed` means the physical response was
  authoritatively executed and bound to its actual successor.
- Every completed alternative retains `BoundIntervalV1::UNKNOWN` because this
  increment does not search the successor GameDecision graph.
- The root remains `UNKNOWN`, `exact_root_value` is `None`, and the certified
  action set is empty.
- With complete physical responses but no schedulable value-search work, the
  V3 snapshot reports `BlockedOnUnevaluatedSuccessor`, not an exhausted or
  solved root.

No synthetic MADS-03B4 Oracle values are injected into the Engine fixture.
No live state key, TT, state merging, POR, policy observation, Engine rule,
RNG, RL, or replay contract is changed.

## Regression evidence

The public V3 regression in
`mtg-kernel/src/mads02e_structured_decision_audit_v1.rs` starts from the
deterministic fixture with seed `0x03b5_0001`. It checks the stale-root
negative case, budget 0/1/2 and completion, exact typed V2/V3 response-domain
equality, all three distinct V3 IDs, separate target supports and bound
cells, all three successor links and actors, Engine finalization targets,
UNKNOWN bounds, zero certificates, and unchanged caller state. A second
independent V3 run has the same normalized paths and work counts; its raw
session-local identities differ as intended. A separate Fireblast fixture
confirms that V3 rejects an unsupported root domain without mutating state.

Measured on that single fixture through the public V3 adapter:

| Metric | Result |
|---|---:|
| Authoritative Engine transitions (`step`) | 6 |
| State clones (adapter and classifier validation) | 13 |
| Candidate entries captured across validated frames/replays | 11 |
| Complete physical responses | 3 |
| Open construction envelopes after completion | 0 |
| Successor bindings | 3 |
| Frontier rebuilds across the test's five snapshots | 5 |
| UNKNOWN root alternatives | 3 |
| Root certificates | 0 |

The six transitions comprise the initial CastSpell for domain admission, Pass,
two independent CastSpell branch replays, and two target responses. The 13
clones include root-frame validation/retention and `classify_after_transition`
checks. These counts describe only this execution and are not performance
claims. V2's previously reported four transitions/five clones are a different
shared-prefix implementation; V3's extra replay work is not a speedup.

## Tests and build

Cargo builds/tests were serialized with the Windows host mutex
`Global\\mtg-kernel-cargo-build.lock`, used `--locked -j 3`, and reused
`C:\\Dev\\mtg-kernel-target`. No command exceeded ten minutes. Results:

| Command | Result |
|---|---|
| `cargo check --locked -p mtg-kernel --lib -j 3` | PASS |
| `cargo test --locked -p mtg-kernel --lib mads03b5 -j 3` | PASS: 3 passed, 0 failed |
| `cargo test --locked -p mtg-kernel --lib mads_virtual_physical_root_v3 -j 3` | PASS: 7 passed, 0 failed |
| `cargo test --locked -p mtg-kernel --lib mads02e_structured_decision_audit_v1 -j 3` | PASS: 10 passed, 0 failed |
| `cargo test --locked -p mtg-kernel --lib mads02b_engine_probe_v1 -j 3` | PASS: 9 passed, 0 failed |
| `cargo test --locked -p mtg-kernel --lib mads_v1 -j 3` | PASS: 18 passed, 0 failed |
| `cargo fmt --all -- --check` | PASS |
| `cargo clippy --locked -p mtg-kernel --lib --tests -j 3 -- -D warnings -A clippy::type_complexity` | PASS; only the known pre-existing Dynamic Search helper lint is allow-listed |

The first focused test iterations exposed and corrected a root-to-construction
protocol identity mismatch, the Lightning Bolt pending target-contract
cardinality (the Engine stores selected contracts, not the required count),
and test normalization of typed action paths. The Fireblast probe initially
reached generic construction rejection before root-domain rejection; root
preflight now checks stale-frame equality first and then rejects the whole
unsupported domain explicitly. Final test results above are after these
corrections.

Not run: full workspace tests/checks, release/Thin-LTO, CUDA, `verify_all.sh`,
training, shards, and performance benchmarks. This increment provides Engine
construction and provenance evidence only; no Magic win-rate or search-value
claim was measured.

## Remaining proof obligations

1. The frame and transition provenance is generated by this adapter, but there
   is no authenticated Engine-owned revision/token or global state identity.
   Do not use these identities across runs or for exact TT reuse.
2. Candidate completeness is established only for this quiescent Bolt+Pass
   root and the actual one-target Bolt continuation represented by the
   fixture. No general Lightning Bolt, spell, or Magic decision-domain proof
   follows.
3. The target decision is replayed and compared on full `GameState`, decision,
   candidates, and run-local frame identity for each response. This does not
   prove all continuation identity classes or actor-switch protocols.
4. Successor GameDecision values are unresolved. Root-critical value search,
   root certificates, exact root values, productive TT reuse, and universal
   V3 engine integration remain unproven or disabled.
5. A later search must explicitly schedule and value supported successor
   GameDecision tasks and propagate only certified minimax bounds. Empty or
   blocked frontier states must remain unresolved.
