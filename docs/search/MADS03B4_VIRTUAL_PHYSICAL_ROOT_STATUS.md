# MADS-03B4 – Virtual Physical Root Status

## Outcome and scope

This increment adds the standalone, versioned `VirtualPhysicalRootV3` contract in `mtg-kernel/src/mads_virtual_physical_root_v3.rs`. It gives each complete physical response its own stable identity, bound cell, expansion state and root-critical support identity. An unresolved construction envelope has its own prefix identity and bound and is never emitted as a complete or certifiable action.

The module is deliberately **not wired into `DynamicEngineSearchV2`**. It does not execute Engine actions or infer Engine legality. Callers must supply exact response domains and successor bindings from an authoritative Engine transition or a bounded Oracle fixture. Dynamic V1/V2 and their scheduler identities are unchanged. No live State-Key, TT, Engine semantic or policy-observation changes were made.

```text
VIRTUAL_PHYSICAL_ROOT_LAYER = IMPLEMENTED_AND_TESTED
ROOT_CRITICAL_V3_FRONTIER = IMPLEMENTED_AND_TESTED_ON_V3_MODEL
DYNAMIC_V3_ENGINE_INTEGRATION = NOT_IMPLEMENTED
```

## Schema and identity contracts

- Schema: `mads.virtual-physical-root.v3`, version `3`.
- Frontier policy: `FRONTIER_ROOT_CRITICAL_PHYSICAL_V3`. This names the tested virtual-root frontier only; it does not replace or rename V1/V2 policies.
- `PhysicalRootActionIdentityV3` frames the physical owner decision, actor, stable root order, exact continuation identity, construction-stage path, and full ordered Engine-response path. The length-framed stable ID therefore differs for Bolt target P0 and target P1 and also includes continuation/stage identity.
- The original root action list and actor are separately sealed by `PhysicalRootDomainAttestationV3`, which must match every ordered complete alternative and unresolved prefix. Without that attestation, `root_action_domain_complete()` is false, no frontier or certificate is exposed, and `root_bounds()` retains an additional UNKNOWN envelope. Adding a new raw-root item invalidates the seal.
- Each `PhysicalRootAlternativeV3` owns a separate `BoundIntervalV1`, progress prefix, successor binding and optional current expansion slot. States distinguish a known full candidate not yet expanded, partial Construction, finalized physical response, and successor-expanded response.
- `UnresolvedConstructionEnvelopeV3` retains owner, actor, protocol, current response prefix, continuation, stage and pending domain-expansion slot. Advancing its prefix creates a new stage/path-specific prefix-support identity. Its bound is required to remain `UNKNOWN`; its support identity lives in a separate prefix namespace.
- `CompletePhysicalActionDomainV3` pairs an ordered candidate list with an explicit Engine-frame or Oracle-fixture evidence identity. Admission checks exact candidate-path equality, order uniqueness, action identity uniqueness, owner/actor/protocol/stage/continuation consistency, and prefix extension before mutating the graph. A rejected/incomplete domain leaves the unresolved envelope intact. The evidence value is a caller attestation; this module does not itself inspect Engine frames or independently prove the claim.
- `SuccessorBindingV3` records the exact response path, finalization boundary, successor node, next actor, provenance, and path-local graph ancestry. It rejects owner/response mismatches and repeated graph-node IDs. It is not a reusable state key or a transposition identity.

## Prefix resolution, bounds, and certification

The positive Oracle test starts with an attested raw root projection containing an unresolved Cast prefix plus known Pass and Other responses. Budget-free root state is `UNKNOWN`; the prefix remains a separate root-support item. Resolving Pass and Other to Oracle `DRAW` while the envelope remains open gives root bounds `[0,+1]`, no root certificate, and a frontier task retaining the current prefix identity.

After the complete ordered target domain is admitted, the prefix is removed atomically and the two target candidates become two distinct `PartiallyConstructed` alternatives. Both begin with `UNKNOWN` bounds and separate P0/P1 target slots. Together with Pass and Other, the virtual root now contains all four flat Oracle root responses. Completing a physical response only records finalization and the exact successor; its bound remains `UNKNOWN` until an Oracle/Engine successor bound is supplied.

The root backup uses MADS V0.4's conservative MAX interval backup across all virtual alternatives and unresolved envelopes. Root certification is restricted to successor-expanded full alternatives and compares its lower bound with every other alternative upper bound and every open envelope upper bound. The test ultimately gives the alternatives Oracle values WIN, LOSS, DRAW, so the root is exact WIN and only the P0-target response is certified. These values come from the synthetic Oracle graph, never from the real Engine fixture.

## Root-critical frontier

`rebuild_root_critical_frontier()` recomputes the virtual root incumbent and challenger from their own bound cells, follows their pending physical owner slots, deduplicates a shared slot, unions role masks, and retains separate `root_action_support_ids` and `unresolved_prefix_support_ids`. Its stable sort key is:

```text
(role_rank, -bound_width, min_root_distance, estimated_cost_bucket,
 stable_slot_identity, stable_semantic_tiebreak)
```

An independent simple test-only enumerator derives the expected root roles, deduplicated task set and support sets from the admitted virtual-root items and compares them with the production rebuild at the unresolved-prefix, complete-domain and changed-bound states. A separate shared-prefix test gives two distinct target alternatives the same initial Cast expansion slot: the slot appears once with `{INCUMBENT_LOWER, CHALLENGER_UPPER}` and support IDs for both physical alternatives. Their identities and bound cells remain separate.

This frontier is implemented and oracle-tested over the V3 model. It is not yet connected to DynamicEngineSearch's path-local Engine graph; it therefore does not claim production search prioritization or performance.

## Oracle differential tests

Test module: `mads_virtual_physical_root_v3::tests`.

- The existing MADS-03B3 nested Oracle fixture and an equivalent flat complete-physical-root fixture both solve to `WIN`.
- Physical successor values are Bolt→P0 `WIN`, Bolt→P1 `LOSS`, Pass `DRAW`; the flat MadsGraph reference certifies only Bolt→P0.
- Virtual alternative bounds are checked against the exact Oracle node values as each successor value is admitted. The virtual root reaches exact `WIN` and certifies only the same physical response.
- Prefix admission is checked before/after domain completion, including the open-prefix `[0,+1]` root envelope and no premature Pass certificate.
- Negative tests reject missing candidate attestations, duplicate physical response paths, mismatched actor, stale owner revision, wrong Construction stage, wrong continuation identity, invalid owner/response successor bindings, and cyclic graph ancestry. Rejections preserve the envelope or the pre-transition alternative state.
- Root frontier tasks, role masks, task deduplication, and support IDs are compared against the independent simple enumeration at each relevant tested state.

These are synthetic structural/Oracle tests. They prove the standalone V3 data contract only on the executed fixture scope.

## Real Engine integration

No V3 object is constructed from a live Lightning Bolt Engine transition in this increment. The existing V2 real-engine tests remain in their previous module and continue to report real values as `UNKNOWN`; they do not validate the new V3 successor-binding adapter. No candidate-domain completeness or dynamic-engine binding claim is made here.

```text
STRUCTURAL_IDENTITY_TESTED = YES (synthetic Bolt/Target paths)
BOUNDS_ORACLE_VALIDATED = YES (synthetic flat and nested Oracle fixture)
ROOT_CRITICAL_FRONTIER_TESTED = YES (V3 model and independent task-set reference)
REAL_ENGINE_CONSTRUCTION_TESTED = NO (V3 not integrated)
REAL_ENGINE_VALUE_SEARCH_PROVEN = NO
```

## Verification

Cargo commands were serialized by `Global\\mtg-kernel-cargo-build.lock`, used `--locked -j 3`, and were kept within the per-command time limit.

| Command | Result |
|---|---|
| `cargo test --locked -p mtg-kernel --lib mads_virtual_physical_root_v3 -j 3 -- --nocapture` | PASS: 4 passed |
| `cargo test --locked -p mtg-kernel --lib mads_v1 -j 3 -- --nocapture` | PASS: 18 passed |
| `cargo test --locked -p mtg-kernel --lib mads02e_structured_decision_audit_v1 -j 3 -- --nocapture` | PASS: 8 passed |
| `cargo test --locked -p mtg-kernel --lib mads02b_engine_probe_v1 -j 3 -- --nocapture` | PASS: 9 passed |
| `cargo fmt --all -- --check`, `git diff --check` | PASS |
| `cargo clippy --locked -p mtg-kernel --lib -j 3 -- -D warnings` | PASS |
| `cargo clippy --locked -p mtg-kernel --lib --tests -j 3 -- -D warnings -A clippy::type_complexity` | PASS; only the two pre-existing DynamicSearch test-helper `type_complexity` findings were allow-listed for this check. |

The repository has two known `clippy::type_complexity` findings in existing `dynamic_engine_search_v1.rs` debug helpers; that file was not modified. Test-target Clippy was run with only that known lint allow-listed. Full Workspace, Release/Thin-LTO, CUDA, and broad benchmark suites were not run.

## Gate and open obligations

```text
VIRTUAL_PHYSICAL_ROOT_LAYER = IMPLEMENTED_AND_TESTED
PHYSICAL_ALTERNATIVE_IDENTITY = DISTINCT_ON_EXECUTED_FIXTURES
UNRESOLVED_PREFIX_ENVELOPES = UNKNOWN_AND_NON_CERTIFIABLE
SUCCESSOR_BINDINGS = STRUCTURALLY_VALIDATED_ON_ORACLE_FIXTURE
ROOT_BOUNDS = ORACLE_VALIDATED_ON_SYNTHETIC_SCOPE
ROOT_CRITICAL_V3_FRONTIER = TESTED_ON_V3_MODEL
DYNAMIC_V3_ENGINE_INTEGRATION = NOT_IMPLEMENTED
ROOT_CERTIFICATION = SYNTHETIC_ORACLE_ONLY
```

Open proof obligations:

1. Build complete candidate attestations from an authoritative Engine frame and bind them to its exact decision revision and continuation context.
2. Wire each V3 alternative's Construction slots and successor binding to the Dynamic Engine path-local graph without changing V1/V2 semantics.
3. Prove prefix-envelope replacement is sound for actual PendingCast stages and preserves every legal ordered candidate.
4. Propagate successor GameDecision bounds back to the exact physical alternative through supported continuation search; UNKNOWN remains unresolved on unsupported successors.
5. Run real Lightning Bolt+Pass construction through V3 with separate target support IDs and UNKNOWN game values. Only then consider a wider dynamic root-critical policy gate.

No Engine rules, RNG, RL/replay contracts, observations, checkpoints, state-key normalization, TT, POR, CARDS, Q, training, shards, cards, or decks were changed.
