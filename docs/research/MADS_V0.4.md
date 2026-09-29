# MADS — Magic Adaptive Decision Search
## Research Concept V0.4 — Hardening Revision

**Status:** V0 research architecture; implementation blocked only on one explicit Manafold state-key audit  
**Target domain:** deterministic perfect-information Magic: The Gathering search on an authoritative rules engine such as Manafold  
**Repository baseline inspected for this revision:** `chrismaghuhn/Manafold` `master` at `6faf1b970def779adc2a8d8bd14ae8aff331dba4`  
**Positioning:** MADS is a bound-guided, transposition-aware adversarial decision-graph search with lazy action construction and conservative partial-order reduction. It is a research candidate, not a claimed replacement for MCTS or established best-first proof search.

---

# 0. Why V0.4 Exists

V0.3 closed the major algorithmic holes around:

```text
partial-expansion bounds
reverse-parent propagation
acyclic graph scope
typed construction nodes
certified vs anytime semantics
oracle validation
```

V0.4 does not add another layer of algorithmic ambition.

Instead it hardens five implementation-critical areas:

```text
1. DecisionStateKey is tied to Manafold's actual authoritative-state contracts.
2. DAG critical-bound scheduling becomes explicitly deterministic.
3. Critical frontier maintenance gets a correctness-first reference implementation.
4. Proof-number / conspiracy-number / proof-set search become mandatory literature/baseline checks.
5. The oracle suite receives explicit enumeration admission caps.
```

V0.4 also corrects one literature characterization:

> Graph-History Interaction is not "unsolved" in the sense that no solutions exist. It is a well-known path-dependence problem with published correctness-preserving solutions for several search families. MADS V0 excludes cycles because those solutions add complexity that is unnecessary for the first experiment.

---

# 1. V0.4 Core Definition

> **MADS V0 is an acyclic, transposition-aware adversarial decision-graph search that lazily constructs legal actions, preserves unresolved-action value envelopes, propagates minimax bounds through all reverse dependencies, optionally removes only engine-certified redundant orderings, and schedules work according to the bounds currently preventing resolution of the root decision.**

The core remains:

```text
Structured decision construction
        +
Lazy expansion
        +
Exact decision-state graph
        +
Safe POR
        +
Partial MAX/MIN bound envelopes
        +
Reverse-parent fixpoint propagation
        +
Root critical-bound scheduling
```

---

# 2. Research Scope

V0 includes:

```text
perfect information
deterministic benchmark positions
acyclic decision graphs
MAX/MIN nodes
exact authoritative transitions
exact decision-state transpositions
typed staged decisions
small explicit POR certificate set
certified and anytime modes
oracle-solvable validation subset
```

V0 excludes:

```text
general chance nodes
hidden-information adversarial play
belief search
cycles
general GHI handling
semantic state abstraction
learned search scheduling
resource/Pareto pruning
general dominance pruning
search distillation
parallel search
```

---

# 3. Value Domain

Utility is normalized:

```text
LOSS = -1
DRAW =  0
WIN  = +1
```

Every node carries:

```text
L(s)       certified lower bound
U(s)       certified upper bound
V_hat(s)   heuristic estimate
```

Invariant:

```text
-1 <= L(s) <= U(s) <= +1
```

Terminal nodes:

```text
loss = [-1, -1]
draw = [ 0,  0]
win  = [+1, +1]
```

`V_hat` never becomes a certified bound merely because it is calibrated well.

---

# 4. Search Node Types

```text
SearchNode =
    GameDecisionNode
  | DecisionConstructionNode
  | TerminalNode
```

## GameDecisionNode

A complete authoritative game state at a decision boundary.

```text
GameDecisionNode {
    key: DecisionStateKeyV1
    actor
    role: MAX | MIN

    L
    U
    V_hat

    decision_cursor
    expansion_status

    incoming_edges
    outgoing_edges
}
```

Eligible for exact transposition reuse.

## DecisionConstructionNode

A partial response inside one authoritative decision protocol.

Examples:

```text
spell chosen, mode unresolved
mode chosen, target unresolved
target chosen, payment unresolved
```

```text
DecisionConstructionNode {
    owner
    protocol_key
    partial_response
    continuation_cursor

    L
    U
    V_hat
}
```

It is not merged merely because the underlying physical game state has not yet changed.

## TerminalNode

```text
TerminalNode {
    value
}
```

with:

```text
L == U == value
```

---

# 5. DecisionStateKey Is Now a Pre-Implementation Contract

The most dangerous part of a transposition search is not hashing performance.

It is answering:

> When are two reached positions truly interchangeable for all future search behavior?

V0.4 therefore no longer treats `DecisionStateKey` as an ordinary implementation detail.

Before the transposition graph is implemented, Manafold must complete:

```text
MADS_DECISION_STATE_KEY_V1_AUDIT
```

against the actual current `EngineState` and state-identity contracts.

Implementation of graph reuse is blocked until that audit is accepted.

---

# 6. What Manafold Already Provides

The inspected Manafold state architecture already has unusually strong ingredients for this job.

Current repository contracts distinguish:

```text
FullStateDigestV5
InformationStateDigestV2
ObservationDigest
CandidateSetDigest
checkpoint identity
execution identity
perspective-local opaque identity
retained player knowledge
continuation state
pending authoritative decisions
RNG state
```

The canonical full-state lineage explicitly includes authoritative state families such as:

```text
revision
core turn / priority state
zones and ordered zones
allocators
execution state
RNG state
knowledge
perspective identities
combat state
format state
```

and later state cuts add further rules-relevant state families.

This is important because MADS should build on Manafold's existing semantic closure work rather than invent an unrelated search-only model.

However:

> `FullStateDigest` and `DecisionStateKey` have different jobs.

The full-state digest answers:

```text
"Is this complete authoritative engine state exactly the same
under this versioned identity contract?"
```

MADS needs:

```text
"Can these two reached search states safely share all future
decision-search work?"
```

Those are related but not automatically identical questions.

---

# 7. Why FullStateDigest Cannot Be Blindly Used as the Final TT Key

Manafold's full authoritative digest intentionally contains fields that preserve exact execution identity.

Examples in the current lineage include:

```text
state revision
allocator state
pending decision identity
continuation identity
RNG state
perspective identity state
knowledge chronology
```

Some of these are obviously future-semantic.

Others may be execution/protocol identities that make two behaviorally equivalent search states hash differently.

If MADS blindly uses the complete full-state digest:

```text
false merges are unlikely
```

but:

```text
valid transpositions may disappear
```

because irrelevant path-local identities remain distinct.

If MADS aggressively strips fields:

```text
transposition rate may improve
```

but:

```text
false merges can make certified results unsound
```

Therefore V0.4 requires an explicit field classification.

---

# 8. DecisionStateKey Field Classification

Every current authoritative state field must be assigned exactly one classification:

```text
INCLUDE
NORMALIZE
EXCLUDE
BENCHMARK_NAMESPACE
```

## INCLUDE

The exact value must be part of the node identity because it can affect:

```text
future legality
future authoritative transitions
future terminal result
future player information
future decision construction
future RNG semantics
```

Examples are expected to include many:

```text
zone contents/order where semantically relevant
turn/phase/priority state
combat state
pending execution/continuation semantics
rule-relevant knowledge
current random state when V0 uses exact deterministic futures
format/rule state
future-relevant counters/effects/attachments/etc.
```

## NORMALIZE

The raw value differs, but a reviewed canonical quotient is proven to preserve future search semantics.

Examples might eventually include:

```text
request-local IDs
purely administrative revision numbers
allocator identities
```

but **none of these are assumed safe to normalize** until proven.

## EXCLUDE

The field has no future decision semantics and does not participate in any observable or authoritative future behavior relevant to the benchmark.

Exclusion requires evidence.

## BENCHMARK_NAMESPACE

The value is globally fixed for an entire benchmark run, so it does not need to be repeated in every node key.

Examples:

```text
engine build identity
rules snapshot
card/content contract identity
search algorithm version
DecisionStateKey schema version
RNG contract version
```

These values namespace the complete transposition table.

---

# 9. Required DecisionStateKey Audit Artifact

Before Phase 2 graph implementation, create:

```text
docs/search/MADS_DECISION_STATE_KEY_V1.md
```

with a table similar to:

| EngineState / protocol field | Classification | Reason | Counterexample test | Canonical encoding |
|---|---|---|---|---|
| active player | INCLUDE | changes legal actor | paired state | exact |
| turn position | INCLUDE | changes legality | paired state | exact |
| zone object state | INCLUDE | rules semantics | paired state | canonical |
| state revision | TBD | stale-response identity vs search semantics | required | TBD |
| global allocators | TBD | future IDs may affect semantics/order | required | TBD |
| pending request | INCLUDE/NORMALIZE by subfield | current legal decision | required | detached |
| continuation payload | INCLUDE | future staged action semantics | required | exact |
| RNG state | INCLUDE for deterministic V0 | changes future outcome | required | exact |
| retained knowledge | INCLUDE when actor decisions depend on it | epistemic semantics | required | canonical |
| event/replay-only provenance | TBD | must prove irrelevance | required | TBD |

No `TBD` may remain when graph reuse is enabled.

---

# 10. Paired-State Equivalence Tests

Every `NORMALIZE` or `EXCLUDE` decision requires paired-state tests.

For proposed equivalent states:

```text
S1
S2
```

the harness must verify under the complete V0 domain:

```text
same legal structured decision space
same terminal classification
same successor-key multiset after corresponding actions
same actor
same rule-relevant observations
same deterministic random semantics
```

where applicable.

The strongest practical contract is a bounded bisimulation-style test:

```text
DecisionEquivalent(S1, S2, depth = k)
```

for oracle-sized fixtures.

If a proposed normalization has a counterexample, it cannot enter `DecisionStateKeyV1`.

---

# 11. Safe Bootstrap Key

If the full DecisionStateKey audit is not yet finished, MADS may still be implemented in a no-risk bootstrap mode:

```text
DecisionStateKeyBootstrap =
    current canonical FullStateDigest
```

This is deliberately over-discriminating.

It may miss transpositions.

It must not be used to conclude:

```text
"Magic has few transpositions"
```

because identity noise may be hiding them.

Therefore benchmark reports must distinguish:

```text
BOOTSTRAP_EXACT_STATE_KEY
DECISION_STATE_KEY_V1
```

---

# 12. State-Key Versioning

The transposition identity is versioned independently:

```text
mads.decision-state-key.v1
```

Changing any:

```text
included field
normalized field
canonicalization rule
protocol-equivalence rule
```

requires a new identity version.

Search traces record:

```text
decision_state_key_schema
full_state_digest_schema
engine identity
content identity
```

---

# 13. Partial Expansion Bounds

Lazy action generation remains governed by the unresolved-action envelope.

## Partial MAX

With at least one expanded child:

```text
L(s) = max L(expanded_children)
```

If any legal action remains unexpanded:

```text
U(s) = +1
```

otherwise:

```text
U(s) = max U(all_children)
```

Zero generated children:

```text
[-1, +1]
```

## Partial MIN

With at least one expanded child:

```text
U(s) = min U(expanded_children)
```

If any legal action remains unexpanded:

```text
L(s) = -1
```

otherwise:

```text
L(s) = min L(all_children)
```

Zero generated children:

```text
[-1, +1]
```

---

# 14. Fully Expanded Backup

MAX:

```text
L = max child L
U = max child U
```

MIN:

```text
L = min child L
U = min child U
```

This is minimax interval propagation.

It is independent of scheduler quality.

A bad scheduler may be slow.

It must not make the certified bounds wrong.

---

# 15. Reverse-Parent Fixpoint Propagation

Transpositions create shared descendants:

```text
       A
      / \
     X   Y
      \ /
       Z
```

A change at `Z` may change:

```text
X
Y
A
```

Therefore every game node stores all reverse dependencies.

Reference propagation:

```text
worklist = parents(changed_node)

while worklist not empty:
    p = pop_stable(worklist)

    old = bounds(p)
    recompute_bounds(p)

    if bounds(p) changed:
        enqueue all parents(p)
```

Continue until no ancestor interval changes.

For V0:

```text
graph must be acyclic
```

so termination is straightforward.

---

# 16. Critical DAG Scheduling — Problem Statement

V0.3 described scheduler traversal largely as if each bound had one parent-child support path.

That is insufficient for a transposition DAG.

A shared node may influence:

```text
incumbent lower bound
challenger upper bound
multiple root actions
```

simultaneously.

Therefore MADS must not represent search relevance as:

```text
node -> one parent path
```

Instead it represents:

```text
expansion task -> set of current root-bound roles
```

---

# 17. Root Roles

At each scheduling epoch:

```text
incumbent =
    root action with maximal L

challenger =
    non-incumbent root action with maximal U
```

Stable semantic root-action order resolves ties.

An unresolved expansion opportunity may carry:

```text
INCUMBENT_LOWER
CHALLENGER_UPPER
OTHER_ROOT_LOWER
OTHER_ROOT_UPPER
```

For V0's critical scheduler, the first two are primary.

A shared descendant can carry:

```text
{INCUMBENT_LOWER, CHALLENGER_UPPER}
```

at the same time.

This removes the ambiguity noted in V0.3.

---

# 18. Critical Support DAG

For each root objective, derive a support subgraph.

## Incumbent-lower support

Follow the bound dependencies currently determining:

```text
L(incumbent)
```

At MAX nodes:

```text
children whose L == parent L
```

At MIN nodes:

```text
if unexpanded actions exist:
    unresolved-action envelope is critical
else:
    children whose L == parent L
```

## Challenger-upper support

Follow dependencies currently determining:

```text
U(challenger)
```

At MIN nodes:

```text
children whose U == parent U
```

At MAX nodes:

```text
if unexpanded actions exist:
    unresolved-action envelope is critical
else:
    children whose U == parent U
```

All ties are retained.

This intentionally produces a **support DAG**, not a single principal path.

---

# 19. ExpansionTask

Scheduler unit:

```text
ExpansionTask {
    owner_node_key
    construction_key_or_next_action_slot
    role_mask
    root_action_support_set

    bound_width
    min_root_distance
    estimated_cost

    stable_semantic_tiebreak
}
```

A physical unresolved expansion opportunity appears at most once per scheduling epoch.

If it supports multiple root roles, the role mask is unioned.

---

# 20. Deterministic V0 Scheduler

V0 uses a deliberately simple, reproducible ordering.

Choose the lexicographically minimal tuple:

```text
(
    role_rank,
    -bound_width,
    min_root_distance,
    estimated_cost_bucket,
    owner_node_key,
    construction_key_or_action_order
)
```

where:

```text
role_rank = 0  supports BOTH incumbent-L and challenger-U
role_rank = 1  supports incumbent-L only
role_rank = 2  supports challenger-U only
role_rank = 3  other currently root-relevant support
```

If a benchmark wants symmetry between incumbent and challenger, a separately versioned scheduler may alternate ranks 1/2.

V0 chooses the fixed ordering above because:

```text
it is deterministic
it is easy to test
it is not disguised as a learned value-of-computation model
```

The scheduler is a **performance policy**, not part of bound correctness.

---

# 21. Root Resolution

One optimal action is certified when:

```text
L(A) >= max U(other_actions)
```

Unique best:

```text
L(A) > max U(other_actions)
```

Decision gap:

```text
gap = U(challenger) - L(incumbent)
```

If:

```text
gap <= 0
```

at least one optimal root action is certified.

---

# 22. CriticalBoundFrontier — Reference Implementation

Efficient incremental frontier maintenance is non-trivial.

V0.4 makes a deliberate engineering choice:

> The first correct implementation does **not** require a clever incrementally maintained frontier.

Reference mode:

```text
after every graph mutation or bound fixpoint:
    rebuild current critical support DAG
    derive all ExpansionTasks
    deduplicate tasks
    sort by deterministic scheduler tuple
    choose first
```

Call this:

```text
FRONTIER_REBUILD_REFERENCE
```

This may be slower.

That is acceptable for the correctness prototype.

---

# 23. Incremental Frontier Optimization

Only after the reference implementation passes oracle validation may MADS implement:

```text
FRONTIER_INCREMENTAL_V1
```

Possible mechanisms:

```text
generation counters
dirty-node queues
version-stamped heap entries
root-role dependency masks
lazy stale-entry removal
```

The optimized implementation must be tested against the reference frontier.

For every oracle search step:

```text
selected_task_incremental
==
selected_task_reference
```

under the same graph state.

Any mismatch is a correctness failure of the optimized scheduler implementation.

---

# 24. Why This Separation Matters

Without a reference scheduler, debugging becomes ambiguous:

```text
wrong root result
```

could be caused by:

```text
bounds
propagation
TT merge
POR
frontier invalidation
heap staleness
scheduler logic
```

With a rebuild oracle:

```text
graph semantics
```

and:

```text
frontier optimization
```

are independently testable.

---

# 25. Acyclic V0 Contract

V0 benchmark graph:

```text
ACYCLIC_REQUIRED
```

If successor key occurs in the active ancestor set:

```text
CYCLE_DETECTED
```

The fixture is unsupported by V0.

No approximation is permitted.

---

# 26. Graph-History Interaction

GHI means that a nominally identical position can have different semantics depending on the path used to reach it.

Published game-search work provides path-sensitive techniques and correctness-preserving solutions for several algorithms.

MADS V0 nevertheless excludes cycles because handling:

```text
repetition semantics
path-dependent legality
path-sensitive transpositions
cycle proof semantics
```

would confound the first experiment.

Future work:

```text
MADS V1+
    GHI-safe state identity
    path signatures or verified proof reuse
    cycle semantics
    repetition-aware TT entries
```

---

# 27. Proof-Number Search Is Now a Mandatory Comparison

The literature relation is stronger than V0.3 acknowledged.

Proof-Number Search (PNS) is directly relevant because it:

```text
targets proof of a root result
selects a most-proving node
uses proof/disproof quantities
works especially naturally on non-uniform branching structures
```

There is also explicit historical work on:

```text
Proof-Number Search + transpositions
acyclic graph forms
cyclic graph forms
depth-first proof-number search
```

Therefore MADS cannot make a credible novelty or efficiency claim without confronting this family.

---

# 28. Conspiracy-Number Search

Conspiracy-number search is also relevant because it asks, in effect:

```text
how much search evidence must change
before the root minimax conclusion changes?
```

That idea is close to MADS's:

```text
which unresolved work can change the root decision?
```

MADS should therefore discuss:

```text
Conspiracy Number Search
Applied CN search
alpha-beta CN variants
```

in the formal literature review.

---

# 29. Proof-Set Search Warning for DAGs

A particularly important result for MADS is that proof/disproof-number arithmetic that is correct on a tree can become misleading on a DAG containing shared descendants.

A shared node may be counted multiple times along multiple paths.

Therefore:

> MADS must never naively import tree proof-number sums into the transposition DAG.

If future MADS schedulers use:

```text
proof counts
disproof counts
expected number of leaves
conspiracy counts
```

they require a DAG-aware formulation.

Proof-set-style reasoning is one relevant literature direction.

For V0:

```text
certified minimax interval backup
```

remains separate from:

```text
heuristic scheduler work estimates
```

This separation avoids accidental proof-count unsoundness.

---

# 30. Baseline / Relative Matrix

Minimum V0 comparison set:

```text
A. Minimax / Alpha-Beta + TT
B. Best-First Minimax
C. B*
D. probability-based B* if practical
E. Proof-Number Search family implementation
F. MCTS + TT
G. MADS V0
```

Optional but valuable:

```text
df-pn
conspiracy-number search
proof-set search
```

Not every historical algorithm must be production-optimized.

But the literature and mechanism comparison must be explicit.

---

# 31. What MADS Is Not Claiming as Novel

The following are established ideas:

```text
best-first adversarial search
root proof via bounds
transposition tables
PNS
conspiracy numbers
reverse dependency propagation
partial-order reduction
lazy search
```

Potential MADS contribution, if evidence supports it, lies in the specific integration of:

```text
authoritative Magic rules engine
versioned decision-state equivalence
typed structured decision continuations
lazy action construction
POR before full response materialization
exact state-DAG reuse
safe partial-action bound envelopes
root-critical adversarial scheduling
counterfactual search telemetry
```

Novelty remains an open research question.

---

# 32. Partial-Order Reduction

POR remains certificate-only.

Default:

```text
NO CERTIFICATE
=> DO NOT REDUCE
```

V0 begins with a tiny closed certificate family rather than a universal dependency analyzer.

Each certificate requires:

```text
formal preconditions
positive fixtures
near-miss negative fixtures
order-swap equivalence evidence
decision-space equivalence evidence
```

---

# 33. POR and DecisionStateKey Are Coupled

POR safety and transposition safety cannot be designed independently.

For a proposed commuting pair:

```text
A then B
B then A
```

the final states must not merely have similar physical boards.

They must produce equal:

```text
DecisionStateKeyV1
```

and preserve all relevant intermediate semantics required by the POR certificate.

Thus:

```text
DecisionStateKey audit
```

is a prerequisite for strong POR claims.

---

# 34. RNG Epistemic Firewall

Authoritative match RNG remains separate from planner knowledge.

V0 deterministic fixtures may freeze random outcomes internally, but search must not gain information that the acting player could not know.

Architecture:

```text
authoritative match RNG
        |
        X  no future leak
        |
search model / search RNG
```

When general chance search arrives later, it receives its own versioned sampling semantics.

---

# 35. Certified and Anytime Modes

## CERTIFIED

Return an action only if the bound proof succeeds.

Otherwise:

```text
UNRESOLVED_WITHIN_BUDGET
```

## ANYTIME

If no proof is reached by budget end:

```text
1. highest V_hat
2. then highest L
3. then stable semantic action order
```

The fallback is frozen before experiments.

---

# 36. Oracle Suite Must Be Explicitly Bounded

"Small enough to fully enumerate" is not a sufficient benchmark contract.

V0.4 defines an admission profile.

Initial:

```text
OracleSuiteV1
```

uses frozen hard caps.

Recommended initial caps:

```text
max_complete_legal_actions_per_game_node = 64
max_continuations_per_construction_node   = 32
max_search_depth                         = 32
max_unique_game_nodes                    = 100_000
max_total_edges                          = 500_000
max_authoritative_transitions            = 2_000_000
```

These numbers are engineering admission limits, not theoretical claims.

If any cap is exceeded:

```text
ORACLE_FIXTURE_TOO_LARGE
```

The position is excluded from `OracleSuiteV1`.

---

# 37. Why Both Per-Node and Global Caps Exist

A total transition cap alone can hide pathological branching.

A per-node action cap ensures oracle fixtures do not contain a single enormous complete-response enumeration.

A graph-size cap prevents:

```text
moderate branching
x
deep horizon
```

from exhausting memory.

Construction-node caps separately constrain:

```text
mode
target
payment
ordering
```

combinatorics.

---

# 38. Oracle Enumeration Uses Structured Decisions Too

The oracle must not bypass the same semantic decision protocol.

It exhaustively traverses:

```text
DecisionConstructionNode
```

until all complete legal responses are generated.

The difference is:

```text
MADS:
    selective / lazy

Oracle:
    exhaustive subject to suite caps
```

This makes legal-action completeness comparable.

---

# 39. Oracle Ground Truth

For every accepted oracle fixture compute:

```text
true root value
true optimal root action set
all reachable unique DecisionStateKeyV1 nodes
all legal complete responses
all graph edges
true transposition count
true terminal leaves
```

Then validate:

```text
MADS selected action
MADS certified intervals
MADS TT reuse
MADS POR reductions
```

against ground truth.

---

# 40. Catastrophic Gate

Required:

```text
CERTIFIED_WRONG_ROOT_ACTION = 0
```

Also:

```text
UNSOUND_BOUND_INTERVAL = 0
UNSAFE_POR_SKIP = 0
INVALID_TT_MERGE = 0
MISSED_LEGAL_ACTION_IN_ORACLE = 0
```

Any nonzero result blocks further performance claims.

---

# 41. Scheduler Reproducibility Tests

Given identical:

```text
graph
bounds
incumbent
challenger
decision cursors
cost buckets
```

the scheduler must produce the same ordered task list.

Test cases include:

```text
one critical path
multiple tied children
shared descendant
node supporting both root objectives
multiple root actions sharing subtree
construction node and game node tied
equal stable keys
```

No pointer address, hash-map iteration order, thread timing, or insertion accident may affect selection.

---

# 42. Shared-Descendant Scheduler Test

Construct:

```text
        ROOT
       /    \
      A      B
       \    /
        X
```

where:

```text
A = incumbent
B = challenger
```

and unresolved work at `X` affects both.

Expected:

```text
X expansion task role_mask =
    INCUMBENT_LOWER | CHALLENGER_UPPER
```

and the physical expansion is scheduled once.

This is a mandatory regression test.

---

# 43. Frontier Optimization Equivalence Test

For every step in an oracle fixture:

```text
Reference frontier rebuild
```

and:

```text
Incremental frontier
```

must agree on:

```text
incumbent
challenger
critical role masks
ordered expansion task
```

If not:

```text
FRONTIER_INCREMENTAL_MISMATCH
```

The optimized mode is disabled.

---

# 44. State-Key Audit Tests

Mandatory paired fixtures include at least:

```text
same board / different revision
same board / different allocator state
same board / different pending request identity
same visible state / different retained knowledge
same physical state / different continuation payload
same physical state / different RNG cursor
same physical state / different candidate ordering
same physical state / different rule-relevant history
same physical state / irrelevant replay provenance only
```

For each pair, V0.4 requires an explicit expected result:

```text
MERGE
or
DO_NOT_MERGE
```

with rationale.

---

# 45. FullStateDigest vs DecisionStateKey Telemetry

Record both:

```text
full_state_digest
decision_state_key
```

for graph nodes during research builds.

This yields four useful cases:

```text
same full digest / same decision key
different full digest / same decision key
same full digest / different decision key   # suspicious
different full digest / different key
```

The second case measures successful safe normalization.

The third case should normally trigger an invariant investigation.

---

# 46. Transposition Quality Metrics

Do not report only:

```text
TT hit rate
```

Also report:

```text
exact_full_digest_hits
normalized_decision_key_hits
paths_merged
descendant_expansions_saved
state_key_build_time
paired-equivalence validation coverage
```

This distinguishes true search reuse from identity engineering.

---

# 47. Performance Metrics

Measure:

```text
authoritative transitions
wall time
CPU time
peak RSS

decision-construction time
state-key time
TT lookup time
fixpoint propagation time
frontier rebuild time
frontier incremental maintenance time
heuristic time
POR certificate time
```

A search that saves transitions but spends five times more CPU is not automatically better.

---

# 48. Resource Regimes

Run:

```text
transition-limited
wall-clock-limited
memory-capped
```

separately.

Do not aggregate them into one headline score.

---

# 49. V0 Ablations

Required:

```text
MADS-full

MADS - TT reuse
MADS - DecisionStateKey normalization
MADS - POR
MADS - lazy construction
MADS - critical-bound scheduling
MADS - frontier incremental optimization
```

The final one verifies that the optimization changes speed, not semantics.

---

# 50. Stop Conditions

Reconsider the research direction if:

```text
certified correctness fails

or

DecisionStateKey cannot safely normalize enough identity noise
to produce useful transpositions

or

POR certification is too narrow to matter

or

lazy action construction does not save meaningful work

or

PNS/B*/best-first minimax consistently match or beat MADS
with lower complexity

or

TT/key/frontier overhead erases transition savings

or

oracle fixtures show the scheduler repeatedly expands
large irrelevant support regions
```

---

# 51. Revised Implementation Order

## Phase 0 — Literature and benchmark freeze

```text
B*
probability-based B*
PNS
PNS + transpositions
df-pn
conspiracy-number search
proof-set search
GHI literature
```

Deliverable:

```text
MADS_SEARCH_PRIOR_ART_MATRIX.md
```

## Phase 1 — Manafold state-key audit

```text
MADS_DECISION_STATE_KEY_V1.md
paired-state fixtures
INCLUDE/NORMALIZE/EXCLUDE/NAMESPACE table
canonical encoder
```

This is now a blocking phase.

## Phase 2 — Oracle harness

```text
OracleSuiteV1 limits
full structured-decision enumeration
ground-truth graph
```

## Phase 3 — Classical baselines

```text
alpha-beta/minimax + TT
best-first minimax
B*
PNS-family baseline
MCTS + TT
```

## Phase 4 — MADS graph core

```text
typed nodes
TT
reverse parents
partial-expansion bounds
fixpoint propagation
```

## Phase 5 — Reference scheduler

```text
critical support DAG
ExpansionTask
FRONTIER_REBUILD_REFERENCE
```

## Phase 6 — Lazy decision construction

```text
DecisionConstructionNode
structured continuation traversal
```

## Phase 7 — Safe POR

```text
small certificate set
```

## Phase 8 — Incremental frontier

Only after reference equivalence is proven.

## Phase 9 — Benchmark + ablation

Only then consider V1 research.

---

# 52. Literature Position After V0.4

The current closest conceptual neighborhood is not one algorithm but an intersection:

```text
B* / probability-based B*
        |
        | root bound proof
        v
      MADS
     /    \
    /      \
 PNS/CN    TT/DAG search
    \      /
     \    /
   POR + structured TCG decisions
```

The strongest established challenge to novelty is now:

```text
PNS / CN / proof-set style root-proof scheduling
```

combined with:

```text
transposition-aware game graphs
```

The strongest potentially distinctive MADS component remains:

```text
authoritative TCG decision protocol
+ lazy partial action construction
+ decision-state equivalence contract
+ POR during action construction
+ interval-safe adversarial search
```

---

# 53. Important Literature Correction

Do not state:

> GHI is an unsolved problem.

Prefer:

> GHI is a long-standing path-dependence problem in graph game search. Published methods address it for several search settings, but correct handling adds state/path semantics and implementation complexity that MADS V0 intentionally avoids through its acyclic benchmark contract.

Likewise, do not state:

> PNS is tree-only.

Prefer:

> Classical PNS was formulated as tree search, but transposition-aware and graph variants have been studied; shared descendants introduce additional accounting issues that matter directly to MADS.

---

# 54. Research Hypothesis

> **In deterministic perfect-information Magic-like positions, a search architecture combining a sound decision-state transposition contract, structured lazy action construction, conservative POR, interval-safe partial expansion, and root-critical adversarial scheduling can reduce the work required to choose an optimal or strong root action in at least some position classes compared with established search baselines.**

This remains a hypothesis.

---

# 55. What Would Be a Strong Positive Result?

A convincing result would look like:

```text
Oracle correctness:
    zero false certifications

Sequencing-heavy positions:
    materially fewer complete actions generated

High-transposition positions:
    materially fewer unique expansions

Same wall-clock:
    equal or better optimal-action hit rate than strong baselines

Ablations:
    identifiable benefit from at least one MADS-specific component
```

Not:

```text
MADS beats deliberately weak vanilla MCTS on one toy position
```

---

# 56. What Would Be a Useful Negative Result?

Examples:

```text
FullState identity noise prevents practical transpositions.

Safe DecisionState normalization is too difficult.

PNS/B* already dominates the proposed scheduler.

POR opportunities are rare in real Magic.

Action construction is not the bottleneck we expected.

State-key and graph bookkeeping cost more than duplicated search.
```

Any of those would materially improve our understanding of Magic search.

---

# 57. Implementation Readiness Gate

Current status after V0.4:

```text
Research hypothesis                    PASS
Partial-expansion semantics            PASS
MAX/MIN backup                         PASS
Reverse-DAG propagation                PASS
Cycle scope                            PASS
Certified/anytime semantics            PASS
Scheduler determinism design           PASS
Frontier reference design              PASS
Oracle admission policy                PASS
Literature-family coverage             PASS

DecisionStateKey field audit           REQUIRED BEFORE TT IMPLEMENTATION
Scientific novelty                     OPEN
```

Therefore:

```text
ALGORITHM DESIGN       = READY
BENCHMARK HARNESS      = READY
TT IMPLEMENTATION      = BLOCKED ON STATE-KEY AUDIT
```

This is a much narrower blocker than "the algorithm is not specified."

---

# 58. Questions for the Next Reviewer

The next review should attack these exact points:

1. Is the `INCLUDE / NORMALIZE / EXCLUDE / BENCHMARK_NAMESPACE` classification sufficient to define a safe transposition key?
2. Which current Manafold fields are clearly unsafe to normalize?
3. Is `FullStateDigest` an acceptable bootstrap key, provided we do not draw conclusions about transposition scarcity?
4. Does the critical-support-DAG construction preserve every expansion that can currently affect incumbent-L or challenger-U?
5. Is the deterministic task priority merely suboptimal, or can it create starvation?
6. Should incumbent-only and challenger-only tasks alternate instead of using a fixed rank?
7. Is rebuilding the frontier every step a good correctness oracle?
8. What is the cleanest incremental frontier structure after the reference mode works?
9. Which PNS/df-pn/proof-set mechanism is the strongest direct baseline for an acyclic transposition graph?
10. Does shared-descendant accounting expose any problem in MADS beyond scheduler efficiency?
11. Are the proposed OracleSuiteV1 caps reasonable for a first implementation?
12. Should oracle admission be based on complete-action count, construction-node count, or both?
13. What is the smallest Manafold state pair that would falsify an incorrect `DecisionStateKey` normalization?
14. What is the smallest shared-descendant graph that would falsify the scheduler?
15. Is anything still missing before coding the benchmark harness and state-key audit?

---

# 59. One-Sentence Definition

> **MADS is an adversarial search over a versioned Magic decision-state DAG that lazily constructs legal responses, safely retains uncertainty from unseen actions, reuses only proven-equivalent states, optionally removes certified redundant orderings, propagates bounds through all shared descendants, and expands the unresolved work most directly blocking resolution of the root choice.**

---

# 60. Immediate Next Artifact

Before implementation, produce:

```text
MADS_DECISION_STATE_KEY_V1.md
```

directly from the current Manafold `EngineState`, decision protocol, information model, RNG contract, and state digest inputs.

That document should contain no abstract `f(...)`.

It should enumerate every field that can reach the current authoritative state identity and say exactly:

```text
INCLUDE
NORMALIZE
EXCLUDE
BENCHMARK_NAMESPACE
```

with a counterexample test for every normalization or exclusion.

That is now the highest-value next step.
