# MADS-01 Status and Evidence

Baseline: `master` at `1265b62c1a0d22e6f3bcc853c4e355fbc696c90f`.

## Implemented

| Component | Status | Scope |
|---|---|---|
| DecisionStateKey audit | IMPLEMENTED; gate result NOT_PROVEN | All `GameState`/`EngineState` fields and decision/surface/session context are conservatively `INCLUDE`; no normalization/exclusion approved. Real-engine TT is blocked. See `MADS_DECISION_STATE_KEY_V1.md`. |
| OracleSuiteV1 | IMPLEMENTED | Independent full minimax over explicit deterministic fixtures; root action set, all legal fixture decisions, all reachable values and terminal classifications. Fixture caps are enforced. |
| MADS interval graph | IMPLEMENTED | Typed game-decision, construction and terminal nodes; exact fixture-ID interning; MAX/MIN conservative partial and full interval backup; reverse-parent fixpoint worklist. |
| Root-critical scheduler | IMPLEMENTED | `FRONTIER_REBUILD_REFERENCE`: critical support rebuild, task deduplication, role-mask union and stable tuple sort. |
| Certified and anytime results | IMPLEMENTED | Certified output is separate from heuristic fallback; unknown remains unresolved. Heuristic values never update bounds. |
| MTG Engine adapter / TT | NOT IMPLEMENTED | State identity is not proven; ordinary game states contain hidden hand/library/RNG information. No privileged state is exposed to an agent. |
| CARDS, Q, POR, incremental frontier, classic search baselines | NOT IMPLEMENTED | Outside MADS-01 scope. |

## Fixture identity and expansion boundary

`FixtureNodeId` is an exact fixture-local identity asserted by the fixture author. Repeated IDs share one fixture node; distinct IDs never merge. The MADS interner uses `HashMap` equality after hashing, and the forced-collision test confirms distinct IDs stay distinct. This is not an MTG state key or a measured real-engine TT hit.

Fixture action/continuation lists are complete and eagerly present in the fixture. MADS creates successor graph nodes one edge at a time, so it exercises partial-expansion bounds and scheduling but **does not save legal-action construction**. The mode is labelled `EAGER_ACTION_ADMISSION`.

`DecisionConstructionNodeV1` stores its owning game decision, the same authoritative actor/role, a protocol key, an exact partial-response prefix and a continuation cursor. It does not automatically switch MAX/MIN roles. Fixture root edges must represent stable complete root-action identities; no mapping from live `Decision`/`PolicyActionV5` to complete physical actions has been proven.

## Test evidence

Focused debug and optimized Release results on Rust 1.94.1:

```text
cargo test -p mtg-kernel --lib oracle_suite_v1 -- --nocapture                 => 3 passed
cargo test -p mtg-kernel --lib mads_v1 -- --nocapture                         => 13 passed
cargo test -p mtg-kernel --lib --release oracle_suite_v1                      => 3 passed
cargo test -p mtg-kernel --lib --release mads_v1                              => 13 passed
```

The tests cover MAX/MIN, last-action counterexamples, bound containment, shared-parent propagation, shared critical-task roles, deterministic ordering, staged-context separation, constant-hash collisions, heuristic/bound separation, cycle rejection, action-cap rejection, tiny-budget unknown, and root tie handling. No failures remain in these focused groups.

## Isolated comparison smoke

Machine: Ryzen 7 5800X, WSL2 Linux x86_64. Commit above. Rust 1.94.1. Build: optimized Release, `opt-level=3`, LTO disabled, 16 codegen units (chosen to make the isolated test build practical; repository default is Thin LTO / one codegen unit).

Fixture `mads-test`: 2 unique game-decision nodes, 1 terminal node, 3 edges, a shared successor, exact root value DRAW. MADS created 3 graph nodes, expanded 3 edges and recorded 1 exact fixture-ID reuse. Authoritative engine transitions and GameState clones were both 0.

Repeated final-source Release smoke invocations reported 3.3–39.4 µs around the oracle and 9.7–76.5 µs around MADS construction/search. The spread at this scale shows timer/setup noise dominates the fixture's actual work. `/usr/bin/time -v` on the final isolated test process rounded user/system CPU and elapsed time to 0.00 s and reported 7,968 KiB process peak RSS. This is process RSS, not graph-only memory. These are tiny harness costs and make **no** search-performance claim. Per-algorithm CPU time and per-graph peak memory remain NOT MEASURED.

## Live integration stop gate

The existing `GameState` holds omniscient library order, opponent cards in hand, RNG state and perspective knowledge. `diagnostic_state_hash()` is 64-bit and lacks current decision, policy-surface scan, revision and complete ordered-action context; `PolicySurfaceV5` has no exact structural equality contract. No full exact key or safe normalization exists. The existing RL/search surfaces are staged and may carry policy-only continuation state. MADS-01 therefore stops at synthetic fixture DAGs: there is no real Pauper/Engine fixture certified as a deterministic, fair perfect-information, fully enumerable V0 position.

## Evidence labels

| Claim | Status |
|---|---|
| Core builds and focused tests pass | TESTED |
| Oracle values and MADS bounds agree on covered synthetic DAGs | CERTIFIED_ON_FIXTURE |
| Any real MTG state-key merge is safe | NOT_PROVEN |
| Search performance wins over classical or production searchers | NOT_PROVEN |
| Novelty relative to PNS, B*, CN/df-pn or proof-set search | NOT_PROVEN; literature checks incomplete |
