# CARDS — Counterexample-guided Abstraction Refinement for Decision Search

## Research Concept V0.2.3 — Execution-Sufficiency, Symbolic-Coverage, and Soundness Hardening

**Status:** Formal-hardening research architecture; V0.2.2 semantic bridges tightened; certified implementation still blocked on three explicit proof gates and Manafold contract mapping  
**Target domain:** deterministic, perfect-information, adversarial trading-card-game decision search over an authoritative rules engine  
**Initial engine target:** Manafold  
**Relationship to MADS:** complementary research direction; CARDS studies adaptive representation while MADS studies adaptive computation  
**Primary V0.2.3 objective:** prove that structured construction replays exactly, that symbolic coverage can be certified without leaf enumeration, and that every abstract bound is a sound bound on its concrete semantic denotation

---

# 0. Why V0.2.3 Exists

V0.2.2 repaired the largest semantic contradiction in V0.2.1 by separating:

```text
materialized
symbolically covered
uncovered
```

instead of treating every non-materialized action as unknown. It also generalized the concrete kernel from `CompleteAction` to typed `DecisionResponse`, gave `Gamma/gamma` explicit roles, and separated checked semantic facts from derived fixed-point bounds.

A full architecture/soundness review of V0.2.2 found that the remaining risk has moved to two narrow bridges:

```text
ConstructionState
    -> finalized authoritative response
    -> Step
```

and:

```text
PrefixRegion
    -> canonical response denotation
    -> symbolic coverage / M-S-U accounting
```

The review also exposed a missing top-level theorem. CARDS had many transfer equations but did not state the central abstraction invariant as the primary proof obligation:

\[
\forall s\in\Gamma_k(C):
L_k(C)\le V_r(s)\le U_k(C)
\]

V0.2.3 therefore introduces three blocking lemmas.

## Gate A — Construction Replay Sufficiency

For every successfully finalized action construction `cs`:

\[
Step(base(cs),FinalizeConstruction(cs))
\equiv
CommitOutcome(cs)
\]

under exact authoritative decision-state identity.

No `CompleteActionKey` is canonical merely because its visible declaration tuple is equal. Its encoding must contain or reconstruct every authoritative commit-relevant fact, including any staging information needed to reproduce payment, target, mode, ordering, division, cost, or other declaration semantics.

## Gate B — Symbolic Canonical Coverage without enumeration

A symbolic region may move complete actions out of `UNKNOWN` only when a checker proves its denotation, canonical uniqueness/disjointness, and coverage without requiring exhaustive leaf materialization.

The preferred V0 mechanism is a canonical declaration normal form:

```text
one canonical CompleteActionKey
<->
one canonical declaration normal form
```

If a region cannot establish uniqueness/disjointness symbolically, CARDS must refine or fall back to exact materialization. It may not enumerate all leaves merely to claim that symbolic compression succeeded.

## Gate C — Concrete soundness of every abstract bound

Before fixed-point monotonicity matters, the following contracts must hold:

\[
SoundCell(C,L,U)
\iff
\forall s\in\Gamma_k(C): L\le V_r(s)\le U
\]

\[
SoundRegion(s,R,L,U)
\iff
\forall a\in\llbracket R\rrbracket_s:
L\le V_r(Step(s,a))\le U
\]

and analogous typed contracts for non-action response classes.

V0.2.3 also tightens closure. Closure is now producer-domain based, not merely incoming-edge based, because an unresolved response inside an abstract SCC can still create a previously undiscovered member of that same SCC.

Finally, this revision narrows the first structured-action slice:

> **Action construction must remain same-owner until `FinalizeConstruction`. Any protocol that yields an opponent-owned decision before completion is `UNSUPPORTED_ACTION_PROTOCOL` in the V0.2.3 certified slice.**

This avoids pretending that a suspended incomplete action declaration can already be executed through `Step`.

V0.2.3 adds no new search heuristic. Its purpose is to close the last places where a checker could either become unsound or secretly recover correctness by enumerating the exact space that CARDS is intended to avoid.

---

# 1. V0.2.3 Core Definition

> **CARDS V0.2.3 is a counterexample-guided adversarial search architecture over a finite terminating authoritative decision system. It may retain same-owner action-construction regions symbolically only when their canonical response denotation, replay semantics, coverage, and value envelopes are independently certified; every derived abstract interval must conservatively contain the exact root-relative value of every concrete state in its semantic cell; and one exact root response is returned only after all concrete and symbolic competitors are excluded.**

The semantic stack is:

```text
AuthoritativeState
      |
      v
DecisionBoundary(kind, owner)
      |
      +--> ActionDecision
      |       |
      |       v
      |   ConstructionState(base, prefix, sigma)
      |       |
      |       +--> same-owner continuation only
      |       |
      |       v
      |   FinalizeConstruction
      |       |
      |       v
      |   canonical DecisionResponse / CompleteActionKey
      |
      +--> ResolutionChoice
      +--> ReplacementChoice
      +--> OrderingChoice
      +--> other explicitly mapped DecisionKind
      |
      v
DecisionResponse
      |
      v
Step(state,response)
      |
      +--> next DecisionState
      +--> Terminal
```

For supported `ActionDecision`s:

```text
M = materialized canonical actions
S = non-materialized actions covered by certified symbolic regions
U = uncovered actions

A = M dot-union S dot-union U
UNKNOWN_ACTION = U only
```

The proof stack is:

```text
trusted authoritative semantics
        |
checked SemanticFactV2 certificates
        |
SoundRegion / SoundResponseClass lemmas
        |
SoundCell transfer lemmas
        |
least-lower / greatest-upper fixed points
        |
residual-aware exact root certificate
```

The three blocking invariants are:

```text
CONSTRUCTION_REPLAY_SUFFICIENT
SYMBOLIC_CANONICAL_COVERAGE_SOUND
CELL_BOUND_CONCRETELY_SOUND
```

If any is unavailable for a proof-relevant object, the reference implementation falls back to a conservative extreme, refinement, or exact materialization.

CARDS remains a research candidate. V0.2.3 explicitly permits the experiment to fail if symbolic checkers require near-total enumeration or if useful value envelopes are too rare.

---

# 2. What V0.2.3 Changes from V0.2.2

```text
V0.2.2 ConstructionState could contain sigma
    -> V0.2.3 FinalizeConstruction + replay-sufficiency theorem

V0.2.2 same-owner was an intended construction rule
    -> V0.2.3 same-owner-until-complete is a hard V0 admission invariant
       opponent-owned mid-construction choice => UNSUPPORTED_ACTION_PROTOCOL

V0.2.2 symbolic-region disjointness could rely on canonical leaf deduplication
    -> V0.2.3 CanonicalDeclarationNormalFormV1 + symbolic disjointness certificates
       exhaustive leaf generation is not an acceptable symbolic checker strategy

V0.2.2 cell transfer equations existed without one top-level concrete theorem
    -> V0.2.3 SoundCell / SoundRegion / SoundResponseClass are primary invariants

V0.2.2 closure reasoned primarily from incoming frontiers
    -> V0.2.3 ProducerDomainV1 + root-seeded producer closure over SCC condensation graph

V0.2.2 EXACT_SUCCESSOR_EQUIVALENCE allowed an undefined exact-equivalence escape hatch
    -> V0.2.3 EXACT_SUCCESSOR_EQUALITY means same exact DecisionStateKey only

V0.2.2 root owner was relabeled MAX
    -> V0.2.3 root-relative owner and utility transformations are explicit

V0.2.2 S was the union of symbolic regions
    -> V0.2.3 S = symbolic denotation minus M, with atomic residualization on materialization

V0.2.2 refinement invalidation was described operationally
    -> V0.2.3 refinement commits atomically as a new SemanticFactEpoch

V0.2.2 symbolic transfer sets had descriptive names
    -> V0.2.3 each proof-relevant symbolic set has explicit quantifier semantics

V0.2.2 payment/target/exact-successor families mixed proof strengths
    -> V0.2.3 proof types split into exact successor equality and direct value envelope;
       state-isomorphism is future work unless separately proven

V0.2.2 broad termination language
    -> V0.2.3 termination guaranteed only under finite deterministic budgets;
       unbounded fair-progress termination is not claimed

V0.2.2 compression denominators looked intrinsically measurable
    -> V0.2.3 cardinalities are EXACT_MEASURED / SYMBOLIC_EXACT / UNKNOWN

V0.2.2 timeout statistics risked successful-run selection
    -> V0.2.3 paired budget outcomes + certification rate + censoring-aware time summaries

V0.2.2 WorkVector measured authoritative work
    -> V0.2.3 adds ProofComputeVector for checker/region/predicate work

V0.2.2 synthetic neutral/stress suites
    -> V0.2.3 adds a NATURALISTIC suite from independently generated realistic Manafold positions

V0.2.2 oracle language sometimes fell back to 'actions'
    -> V0.2.3 ground truth is defined over typed root DecisionResponses
```

The revision deliberately does **not** introduce learned refinement, hidden-information search, chance nodes, general state isomorphism, suspended multi-owner action declarations, or parallel search.

---

# 3. Research Scope

V0.2.3 includes:

```text
deterministic perfect-information decision systems
finite root-reachable domain for oracle fixtures
terminating concrete decision graphs
MAX/MIN ownership relative to a root query actor
typed DecisionKind / DecisionResponse boundaries
same-owner ActionDecision construction until finalization
optional authoritative construction staging sigma
canonical CompleteActionKey identity
materialized / symbolically-covered / uncovered response accounting
query-local state cells with total predicates
producer-domain-based membership closure
abstract quotient cycles with extremal fixed-point semantics
counterexample-guided monotone refinement
exact concrete root-response certification
algorithm-neutral scenario generation
```

V0.2.3 explicitly excludes:

```text
opponent-owned decisions inside an unfinished action declaration
suspend/resume multi-owner construction protocols
hidden information
belief search
general chance nodes
general cycle/repetition game semantics
learned proof evidence
general state-isomorphism certificates
parallel reference search
unbounded termination guarantees
```

Admission invariant for the first structured-action slice:

```text
ACTION_CONSTRUCTION_SAME_OWNER_UNTIL_FINALIZE = true
```

If Manafold exposes a decision protocol violating this invariant:

```text
UNSUPPORTED_ACTION_PROTOCOL
```

is returned for the certified V0.2.3 slice. The architecture does not silently reinterpret the intervening opponent choice as a parameter of the current player's action.

---

# 4. General Concrete Decision System and Root-Relative Utility

Let the concrete authoritative decision system be:

\[
G=(S,Z,actor,kind,Responses,Step,u)
\]

where:

```text
S               authoritative decision-boundary states
Z               terminal states
actor(s)        authoritative player owning the decision
kind(s)         typed DecisionKind
Responses(s)    finite legal authoritative responses
Step(s,r)       deterministic authoritative step after one legal response
u(z,p)          terminal utility for player p, normalized to {-1,0,+1}
```

For a query rooted at exact state `r`, define:

```text
query_actor = actor(r)
```

and root-relative utility:

\[
u_r(z)=u(z,query\_actor)
\]

Root-relative ownership is:

\[
owner_r(s)=
\begin{cases}
MAX & actor(s)=query\_actor\\
MIN & actor(s)\ne query\_actor
\end{cases}
\]

for the two-player V0 domain.

The exact value used by every CARDS proof is therefore:

\[
V_r(s)=
\begin{cases}
 u_r(s) & s\in Z\\
 \max_{x\in Responses(s)}V_r(Step(s,x)) & owner_r(s)=MAX\\
 \min_{x\in Responses(s)}V_r(Step(s,x)) & owner_r(s)=MIN
\end{cases}
\]

The root is MAX **by definition of the query transformation**, not by relabeling ownership while leaving utility untouched.

Required concrete invariants:

```text
FINITE_OR_BUDGETED_REFERENCE_SCOPE
DETERMINISTIC_STEP_REQUIRED
NONTERMINAL_RESPONSE_NONEMPTY_REQUIRED
ROOT_UTILITY_PERSPECTIVE_FROZEN
ROOT_ACTOR_FROZEN
```

For every nonterminal state:

\[
|Responses(s)|\ge1
\]

For an `ActionDecision`, `Responses(s)` contains finalized complete action responses. Other `DecisionKind`s use their own typed responses; they are not forced into `CompleteAction` terminology.

---

# 5. Structured Action-Decision Model and Construction Replay Sufficiency

Only:

```text
kind(s) == ActionDecision
```

uses the structured declaration protocol.

The construction state is:

```text
ConstructionState {
    base_state_key
    actor
    protocol_schema
    prefix
    sigma
    construction_generation
}
```

`prefix` records canonical declaration choices already made. `sigma` is optional authoritative staging state required by the Manafold adapter when legality or final commit depends on staged protocol information.

## Same-owner V0 contract

Every continuation from the initial action declaration until finalization must be owned by the same authoritative actor:

\[
owner(next\_construction\_choice)=actor(base(cs))
\]

until `FinalizeConstruction` succeeds.

If the authoritative rules engine exposes an opponent-owned choice before completion:

```text
UNSUPPORTED_ACTION_PROTOCOL
```

for the V0.2.3 certified slice.

There is no implicit transition:

```text
incomplete ConstructionState -> opponent DecisionState
```

because an incomplete construction is not a legal `DecisionResponse` and cannot be passed to `Step`.

## Well-founded construction

The adapter must provide either:

```text
PREFIX_PROTOCOL_ACYCLIC
```

or a versioned well-founded rank such that every legal continuation strictly progresses under that rank.

## Finalization

V0.2.3 replaces a bare `Complete(cs)` function with:

```text
FinalizeConstruction(cs) -> DecisionResponse
```

For the first action slice the result contains a canonical `CompleteActionKey` plus all authoritative declaration payload required for replay from `base(cs)`.

The key contract is not merely syntactic equality. It is **replay sufficiency**.

### Construction Replay Sufficiency Lemma

Let:

```text
resp = FinalizeConstruction(cs)
```

Then the authoritative adapter must prove:

\[
DecisionStateKey(
    Step(base(cs),resp)
)
=
DecisionStateKey(
    CommitOutcome(cs)
)
\]

or the corresponding terminal identity if finalization ends the game.

`CommitOutcome(cs)` means the outcome of committing the authoritative staged construction exactly as represented by `cs`, including `sigma`.

If two construction states finalize to the same canonical complete-action identity, they must be commit-equivalent:

\[
CompleteActionKey(cs_1)=CompleteActionKey(cs_2)
\Rightarrow
CommitOutcome(cs_1)\equiv CommitOutcome(cs_2)
\]

under exact authoritative successor identity.

This means payment-source selection, targets, modes, ordering, division, alternative costs, and any other future-relevant commit semantics must either be encoded by the canonical response or proven irrelevant by an exact contract.

## Prefix denotation

For construction state `cs`:

\[
Completions(cs)=
\{a\in Responses(base(cs))\mid a\text{ is produced by a legal same-owner path extending }cs\}
\]

A prefix is never itself executable through `Step`.

## Canonical declaration normal form

The preferred V0.2.3 contract is:

```text
CanonicalDeclarationNormalFormV1
```

with:

```text
normalize_construction_path(path) -> CanonicalDeclaration
canonical_response(CanonicalDeclaration) -> CompleteActionKey
```

and required uniqueness:

\[
canonical\_response(d_1)=canonical\_response(d_2)
\iff
 d_1,d_2\text{ denote the same authoritative complete response}
\]

for the admitted slice.

This normal form is the structural basis for symbolic disjointness. If the adapter cannot provide or verify it without exhaustive leaf enumeration, the affected symbolic region is not admitted as certified symbolic coverage.

---

# 6. Exact Identity, Exact TT, and Abstraction Are Separate

Every concrete authoritative decision state retains an exact search identity:

```text
DecisionStateKeyV1-or-later
```

CARDS additionally uses:

```text
AbstractCellKey
ConstructionStateKey
CompleteActionKey
DecisionResponseKey
```

Their semantics are distinct.

Invariant:

```text
AbstractCellKey MUST NEVER be used as an exact TT key.
```

V0.2.3 introduces a conservative exact-TT rule:

> `ExactTTV1` may contain only entries keyed by exact authoritative decision identity whose stored value/bound is independent of CARDS cell membership, partition epoch, query-local symbolic region identity, or closure certificate scope.

Therefore the following may not be stored as reusable exact-TT facts:

```text
cell-local bounds
partition-epoch-derived bounds
Gamma(C)-scoped witness bounds
root-region-local residual bounds
bounds depending on query-local SeparatorSpec state
```

unless they are separately re-proved as exact state facts.

Exact TT reuse may sit underneath CARDS. It does not inherit CARDS abstraction claims.

---

# 7. Total Query-Local Cell Semantics and the Primary Soundness Contract

At refinement epoch `k`, CARDS defines every state cell through a total deterministic predicate over root reachability.

For every cell `C` and exact state `s` in `Reach(r)`:

\[
\phi_k(C,s)\in\{true,false\}
\]

with total partition property:

\[
\forall s\in Reach(r):\sum_C[\phi_k(C,s)]=1
\]

Define:

\[
\Gamma_k(C)=\{s\in Reach(r)\mid\phi_k(C,s)\}
\]

and:

\[
\gamma_k(C)=\Gamma_k(C)\cap S_{represented,k}
\]

The fundamental semantic contract of CARDS is:

\[
SoundCell_k(C,L,U)
\iff
\forall s\in\Gamma_k(C):
L\le V_r(s)\le U
\]

Every proof-visible cell interval must satisfy `SoundCell`.

For a symbolic region `R` at exact state `s`:

\[
SoundRegion_k(s,R,L,U)
\iff
\forall a\in\llbracket R\rrbracket_s:
L\le V_r(Step(s,a))\le U
\]

For a typed non-action response class `q`:

\[
SoundResponseClass_k(s,q,L,U)
\iff
\forall x\in\llbracket q\rrbracket_s:
L\le V_r(Step(s,x))\le U
\]

These are stronger and more important than mere monotonicity of the fixed-point transfer operator.

The cell data structure retains:

```text
AbstractCell {
    cell_key
    cell_predicate_schema
    state_partition_epoch
    semantic_fact_epoch
    membership_generation
    represented_member_digest
    role
    terminal_value_if_any
    membership_status: OPEN | CLOSED
    optional_closure_certificate
    L
    U
}
```

Refinement replaces a predicate by total disjoint child predicates. No later-discovered root-reachable state may be unclassifiable under the active partition epoch.

---

# 8. Structural Floor and Separator Compatibility

Before discretionary abstraction, the total cell predicates must separate incompatible proof semantics.

Mandatory structural dimensions include:

```text
MAX vs MIN owner
terminal vs nonterminal
different terminal utility
different supported DecisionKind
incompatible authoritative protocol family
incompatible benchmark namespace
```

The Manafold integration must explicitly decide whether additional dimensions are mandatory in the first slice, including:

```text
turn/phase/priority class
combat decision family
pending execution family
replacement/order-choice family
rules/format state family
```

A structural-floor violation is an implementation bug, not an ordinary counterexample.

Every later refinement is represented by a `SeparatorSpecV1` compatible with this floor.

---

# 9. Membership Freshness vs Semantic Membership

`membership_generation` continues to track the represented member set:

```text
membership_generation
represented_member_digest
```

If a newly discovered exact state satisfies `phi_k(C,s)`:

```text
membership_generation += 1
```

and represented-member-universal certificates tied to the previous generation become stale.

But freshness and completeness remain distinct:

```text
freshness:
    is evidence about the current gamma_k(C)?

completeness:
    has gamma_k(C) been proved equal to Gamma_k(C)?
```

Only a valid membership-closure proof establishes:

\[
\gamma_k(C)=\Gamma_k(C)
\]

for a proof epoch.

A certificate can be fresh but incomplete.

A complete certificate can become stale after a partition or engine/content change.

Both conditions must hold whenever a proof requires universal quantification over `Gamma_k(C)`.

---

# 10. Producer-Domain Membership Closure and SCC Grounding

`CLOSED` is a theorem-backed status. V0.2.3 does not define closure from a list of already observed incoming edges.

The primitive is:

```text
ProducerDomainV1
```

A producer domain denotes a root-reachable source domain whose legal response/protocol space may create an exact successor satisfying one or more target cell predicates.

Examples include:

```text
exact DecisionState + materialized response class
exact DecisionState + certified symbolic response region
closed ConstructionState region
non-action DecisionResponse class
```

A producer domain is `CLOSED` only when all of its outputs relevant to the target predicate have been:

```text
materialized and classified
or
symbolically covered by a sound total certificate
or
proved impossible
```

An unresolved response inside an abstract SCC therefore remains an open producer even when its source cell is already represented inside that same SCC.

## Grounded cell closure

For a non-cyclic dependency component, closure proves:

```text
all root-seeded producer domains capable of generating phi_k(C,.) states are closed
and
all produced matching states are present in gamma_k(C)
```

which establishes:

\[
\gamma_k(C)=\Gamma_k(C)
\]

## SCC closure

Abstract quotient cycles are closed as one component.

```text
MembershipClosureSccCertificateV2 {
    scc_id
    member_cell_keys[]
    partition_epoch
    semantic_fact_epoch
    producer_domain_ids[]
    external_producer_frontier[]
    internal_producer_domains[]
    represented_member_digests[]
    producer_coverage_digest
    unresolved_producer_frontier_empty: true
    root_seed_provenance
    engine_identity
    content_identity
}
```

The checker must prove:

```text
1. every root-seeded producer domain that can reach the SCC predicate is accounted for;
2. every external producer is closed or proved unable to enter the SCC;
3. every internal producer domain is closed over its response space;
4. no uncovered response/prefix remainder can create a new SCC member;
5. every generated exact successor is classified by the total cell predicates;
6. gamma_k(C) = Gamma_k(C) for every member cell.
```

The proof is rooted in concrete root reachability and the condensation DAG of producer components, not in mutual certificate references.

Forbidden:

```text
Cert(C1) because Cert(C2)
Cert(C2) because Cert(C1)
```

without an independently grounded closed producer frontier.

If producer closure cannot be established without unacceptable cost:

```text
cell/SCC remains OPEN
```

and proof sides requiring universal `Gamma` scope remain conservative.

---

# 11. Three-Way Complete-Action Semantics with Atomic Materialization

For an exact `ActionDecision` state `s`:

\[
A(s)=Responses(s)
\]

At epoch `k`:

\[
A(s)=M_k(s)\;\dot\cup\;S_k(s)\;\dot\cup\;U_k(s)
\]

where:

```text
M_k(s) = materialized canonical CompleteActionKeys
D_k(s) = union of accepted symbolic-region denotations
S_k(s) = D_k(s) \ M_k(s)
U_k(s) = A(s) \ (M_k(s) union S_k(s))
```

The explicit subtraction matters. A response cannot remain symbolically covered after it has moved to the materialized set.

Required invariants:

```text
M intersect S = empty
M intersect U = empty
S intersect U = empty
M union S union U = A
UNKNOWN_ACTION = U
```

## Atomic materialization

When a response `a` currently represented by symbolic region `R` is materialized, the semantic update is one transaction:

```text
prepare:
    materialize canonical response a
    validate legality and identity
    construct residual region Difference(R, Singleton(a))
    validate new coverage/disjointness facts

commit new SemanticFactEpoch atomically:
    a enters M
    a is removed from symbolic denotation contributing to S
    residual symbolic coverage replaces prior coverage
    dependent certificates are either refreshed or stale
```

No fixed-point or root certificate may observe an intermediate epoch in which `a` belongs to both `M` and `S`, or to neither while an old coverage certificate remains active.

---

# 12. Symbolic Canonical Coverage and Non-Enumerative Closure

A certified `SymbolicActionRegion` has denotation:

\[
\llbracket R\rrbracket_s\subseteq A(s)
\]

over canonical complete-action identities.

A region may contribute to symbolic coverage only with a checked:

```text
SymbolicCoverageCertificateV1
```

that proves:

```text
DENOTATION_TOTAL_FOR_REGION
CANONICAL_RESPONSE_MAPPING_VALID
NO_OVERLAP_WITH_MATERIALIZED_AFTER_RESIDUALIZATION
PAIRWISE_SYMBOLIC_DISJOINTNESS
VALUE_CERTIFICATE_SCOPE_MATCHES_DENOTATION
```

## Non-enumeration requirement

A certificate does **not** count as symbolic coverage if its checker obtains correctness by enumerating every canonical completion in the region.

Every checker declares its method:

```text
SYMBOLIC_STRUCTURAL
SYMBOLIC_CARDINALITY
EXACT_ENUMERATION_FALLBACK
```

Only the first two demonstrate pre-materialization symbolic savings. `EXACT_ENUMERATION_FALLBACK` is allowed for correctness but is reported as exact fallback and does not count toward symbolic-compression claims.

## Canonical uniqueness / disjointness

Preferred proof form:

```text
CanonicalDeclarationNormalFormV1
+
SymbolicDisjointnessCertificateV1
```

The certificate proves, without leaf enumeration, that two regions describe disjoint sets of canonical declarations or that their overlap is explicitly residualized.

If symbolic injectivity/disjointness cannot be proved:

```text
refine region
or
materialize exact leaves
```

The checker may not assume two syntactically different prefixes denote different complete actions.

Define active symbolic denotation:

\[
D_k(s)=\bigcup_{R\in SR_k(s)}\llbracket R\rrbracket_s
\]

and:

\[
S_k(s)=D_k(s)\setminus M_k(s)
\]

Then:

\[
U_k(s)=A(s)\setminus(M_k(s)\cup S_k(s))
\]

`ACTION_SPACE_CLOSED(s)` holds exactly when:

\[
U_k(s)=\varnothing
\]

without requiring every element of `S_k(s)` to be individually materialized.

---

# 13. UNKNOWN Means Uncovered

`UNKNOWN_ACTION` denotes exactly `U_k(s)`.

It does **not** denote all actions that lack allocated complete-action objects.

Therefore an action represented by a certified symbolic prefix region is no longer unknown merely because it is non-materialized.

Reference semantics:

```text
UNKNOWN_ACTION {
    denotation = U_k(s)
    may_exist = true iff U_k(s) may be nonempty
    L = -1
    U = +1
}
```

`UNKNOWN_RESPONSE` is the corresponding typed remainder for other `DecisionKind`s.

At a construction prefix, CARDS likewise distinguishes:

```text
materialized continuation responses
symbolically covered continuation responses
uncovered continuation responses
```

so a certified symbolic child region does not remain in `UNKNOWN_CONTINUATION`.

The fundamental invariant is:

```text
UNKNOWN == UNCOVERED
UNKNOWN != UNMATERIALIZED
```

This repair is necessary for prefix abstraction to tighten bounds before complete-action leaf materialization.

---

# 14. May Sets and Symbolic May Regions

For materialized complete actions in a state cell `C`, `MayMaterialized_k(C)` contains action classes with at least one represented concrete pair.

For symbolically covered complete actions, `MaySymbolic_k(C)` contains accepted symbolic regions with non-empty denotation for at least one state in the proof scope.

The possible-action side of a cell is therefore not represented by one flat class set. It is the typed union:

```text
materialized May classes
symbolic May regions
uncovered remainder
```

No member of the uncovered remainder is silently assumed to belong to a known class.

For proof transfer, every materialized class or symbolic region must carry its own denotation and bound contract.

The old notion:

```text
May(C) over every legal action whether known or not
```

is removed from the certified semantics.

---

# 15. GuaranteedActionCover — Quantified Choice over Covered Domains

A `GuaranteedActionCover` is a proof of available choice, not a single action.

For a closed semantic cell `Gamma_k(C)`, let `Q` denote a set of accepted materialized classes and/or symbolic action regions.

The cover contract is:

\[
\forall s\in\Gamma_k(C),\;\exists a\in A(s):a\in\llbracket Q\rrbracket_s
\]

where `[[Q]]_s` is the union of the canonical complete-action denotations of the cover members.

Write:

\[
Q\in G_k(C)
\]

only when:

```text
cell membership closure is current
cover denotations are current
no cover member relies on uncovered action space
canonical CompleteActionKey deduplication has been applied
```

A cover may include a symbolic region without enumerating each complete action in that region.

A cover alone proves existence. To tighten a state bound through a concrete selected action, CARDS still needs either:

```text
ActionWitnessPolicy
```

or a separately accepted quantified symbolic value theorem whose quantifier direction matches the required bound.

---

# 16. Refinement-Safe Cover Lifting

Suppose an accepted cover member `q` is refined into child regions/classes `children(q)`.

Define:

\[
Lift(Q)=\bigcup_{q\in Q}children(q)
\]

Cover lifting is valid only when a checker establishes:

```text
1. child denotations are pairwise disjoint after canonical action normalization;
2. union child denotations equals the parent denotation;
3. cell Gamma scope is unchanged or the cover is revalidated;
4. no action moves into the uncovered remainder because of the refinement;
5. all certificate epochs are current.
```

Under those conditions the existential statement:

\[
\forall s\exists a\in\llbracket Q\rrbracket_s
\]

is preserved by replacing `Q` with `Lift(Q)`.

If any condition is unknown, the old cover is invalidated rather than guessed forward.

---

# 17. ActionWitnessPolicy Uses Canonical Complete Actions Only

For a closed cell and accepted cover `Q`, an `ActionWitnessPolicy` maps each exact state in the certified scope to one canonical legal complete action:

```text
ActionWitnessPolicy {
    cell_key
    closure_certificate_id
    cover_id
    mapping:
        DecisionStateKey -> CompleteActionKey
}
```

with:

\[
\forall s\in\Gamma_k(C):\pi_Q(s)\in A(s)
\]

and:

\[
\pi_Q(s)\in\llbracket Q\rrbracket_s
\]

A `DecisionPrefix`, `ConstructionState`, `RootRegion`, or unresolved response is never a witness value.

If the same complete action is reachable through multiple construction paths, all paths canonicalize to the same `CompleteActionKey` before witness validation.

The policy may be partial during exploration. It tightens certified bounds only when complete over the claimed `Gamma_k(C)` scope.

---

# 18. Typed Successor Semantics

The authoritative transition relation is the general:

\[
Step(s,r)
\]

for legal `DecisionResponse r` at state `s`.

For an `ActionDecision`, a complete action `a` is one subtype of response and:

\[
Step(s,a)
\]

is defined only for canonical legal `CompleteActionKey`s.

A non-complete construction object has no authoritative successor through `Step`.

For a materialized response class `q`, define its covered successor relation:

\[
Post_M(C,q)=\{\alpha^S(Step(s,r))\mid (s,r)\text{ is in the certified pair domain of }q\}
\]

For a symbolic region `R`, CARDS does not need to materialize every successor if a semantic certificate directly provides a sound value envelope over all denoted responses.

If no such certificate exists, the region remains too weak for tightening and must either refine/materialize or retain `[-1,+1]`.

No prefix object is ever passed to `Step` merely because it denotes complete actions.

---

# 19. Symbolic Value Certificate Proof Types

V0.2.3 separates **proof type** from domain-specific certificate generator.

The reference proof types are:

```text
EXACT_SUCCESSOR_EQUALITY
DIRECT_VALUE_ENVELOPE
```

`STATE_ISOMORPHISM` is reserved for future work and is not accepted in certified V0.2.3 unless a separate congruence/bisimulation theorem is frozen.

## EXACT_SUCCESSOR_EQUALITY

A region certificate is valid only if the checker proves that every canonical completion in scope reaches the same exact authoritative successor identity:

\[
\forall a\in\llbracket R\rrbracket_s:
DecisionStateKey(Step(s,a))=K
\]

for one exact `DecisionStateKey K`, or the same exact terminal result.

There is no `accepted exact decision-equivalence key` escape hatch.

If exact keys differ, this proof type fails.

## DIRECT_VALUE_ENVELOPE

A checker may instead prove directly:

\[
\forall a\in\llbracket R\rrbracket_s:
L_R\le V_r(Step(s,a))\le U_R
\]

from a separately reviewed semantic argument whose premises are checked against authoritative state.

This proof type does not assert successor equality.

## Domain-specific generators

Names such as:

```text
PAYMENT_EQUIVALENCE
TARGET_SYMMETRY
```

are generator/checker strategies, not proof strengths.

A payment checker might discharge `EXACT_SUCCESSOR_EQUALITY` when alternative declarations lead to the same exact successor. If it cannot, it must discharge a `DIRECT_VALUE_ENVELOPE` theorem or return `UNKNOWN`.

A target-symmetry checker may not claim exact equality when object permutation produces distinct exact states. General symmetry/isomorphism reasoning is outside the V0.2.3 certified core unless separately formalized.

Every family reports:

```text
proof_type
denotation contract
checker method
whether leaf enumeration occurred
semantic premises
result: VALID_CERTIFICATE | CONCRETE_COUNTEREXAMPLE | UNKNOWN
```

If no accepted proof applies:

```text
region interval = [-1,+1]
```

until refinement/materialization provides stronger evidence.

---

# 20. Concrete Soundness Lemmas, Semantic Facts, and Derived Fixed Points

Fixed-point monotonicity is not itself a soundness proof. V0.2.3 places concrete-denotation lemmas before the fixed-point theorem.

## Semantic facts

`SemanticFactV2` contains only independently checked facts such as:

```text
terminal root-relative utility
exact Step result
exact DecisionStateKey
Construction Replay Sufficiency certificate
cell predicate membership
producer-domain closure
symbolic region denotation
canonical disjointness
response-space closure
EXACT_SUCCESSOR_EQUALITY certificate
DIRECT_VALUE_ENVELOPE certificate
GuaranteedActionCover witness legality
```

These facts are immutable inside one `SemanticFactEpoch`.

## Primitive soundness predicates

\[
SoundCell(C,L,U)
\iff
\forall s\in\Gamma(C):L\le V_r(s)\le U
\]

\[
SoundRegion(s,R,L,U)
\iff
\forall a\in\llbracket R\rrbracket_s:
L\le V_r(Step(s,a))\le U
\]

\[
SoundResponseClass(s,q,L,U)
\iff
\forall x\in\llbracket q\rrbracket_s:
L\le V_r(Step(s,x))\le U
\]

Every transfer rule used in certified mode needs a lemma of the form:

```text
sound semantic premises
+
sound child intervals
=>
sound parent interval
```

## Fixed-point layer

Only after the transfer lemmas are established are abstract SCCs solved.

For a frozen `SemanticFactEpoch`:

```text
lower:
    initialize unresolved L to -1
    iterate monotone F_L upward
    choose the least fixed point

upper:
    initialize unresolved U to +1
    iterate monotone F_U downward
    choose the greatest fixed point
```

Derived intervals are never promoted to semantic axioms for the same epoch.

Required theorems:

```text
TRANSFER_PRESERVES_SOUND_CELL
F_L_MONOTONE
F_U_MONOTONE
LEAST_LOWER_FIXED_POINT_SOUND
GREATEST_UPPER_FIXED_POINT_SOUND
```

Mandatory invariant:

```text
L <= U
```

Catastrophic gates:

```text
BOUND_INTERVAL_INVERSION = 0
DERIVED_BOUND_USED_AS_OWN_SEMANTIC_PREMISE = 0
UNSOUND_CELL_INTERVAL = 0
```

The fixed-point solver is therefore a mechanism for combining already-sound constraints, not a source of semantic truth.

---

# 21. Complete Response Accounting and Explicit Symbolic Quantifiers

For each exact state or closed cell, proof-relevant response space is represented by:

```text
materialized response classes
certified symbolic regions
uncovered remainder
```

For action decisions this is `M/S/U`.

V0.2.3 replaces descriptive symbolic set names with typed proof sets.

## SymbolicUniversalEnvelope

A region belongs here when it proves:

\[
\forall s\in\Gamma(C),
\forall a\in\llbracket R\rrbracket_s:
L_R\le V_r(Step(s,a))\le U_R
\]

This is suitable for all-response upper/lower accounting.

## SymbolicExistentialLowerWitness — MAX

A region belongs here only when it proves:

\[
\forall s\in\Gamma(C),
\exists a\in\llbracket R\rrbracket_s:
V_r(Step(s,a))\ge L_R
\]

This may tighten a MAX lower bound.

## SymbolicExistentialUpperWitness — MIN

A region belongs here only when it proves:

\[
\forall s\in\Gamma(C),
\exists a\in\llbracket R\rrbracket_s:
V_r(Step(s,a))\le U_R
\]

This may tighten a MIN upper bound.

The type itself encodes the quantifier direction. A universal envelope and an existential witness are not interchangeable.

For uncovered remainder:

```text
[-1,+1]
```

is retained.

This section defines the proof domains consumed by the MAX/MIN transfer rules; there are no untyped placeholders such as `GuaranteedSymbolicLowerRegions` whose quantifier semantics live only in prose.

---

# 22. Guaranteed-Cover Witness Bounds

For a closed MAX cell with complete `ActionWitnessPolicy pi_Q`:

\[
L^{MAX}_{witness}(C,Q,\pi_Q)=
\min_{s\in\Gamma_k(C)}
L(Step(s,\pi_Q(s)))
\]

For a closed MIN cell:

\[
U^{MIN}_{witness}(C,Q,\pi_Q)=
\max_{s\in\Gamma_k(C)}
U(Step(s,\pi_Q(s)))
\]

The witness policy exists only for `ActionDecision` covers in the first slice.

Other `DecisionKind`s may use exact materialized responses until their own quantified coverage theorem is specified.

The quantifier order remains:

\[
\forall state\;\exists legal\ representative
\]

and must never be weakened into a universal statement about every response inside a cover.

---

# 23. Optional Fully Enumerated Cover Bounds

When every canonical representative in a cover is materialized, CARDS may compute stronger state-conditioned bounds.

For MAX:

\[
L^{MAX}_{cover}(C,Q)=
\min_{s\in\Gamma_k(C)}
\max_{a\in A_Q(s)}L(Step(s,a))
\]

For MIN:

\[
U^{MIN}_{cover}(C,Q)=
\max_{s\in\Gamma_k(C)}
\min_{a\in A_Q(s)}U(Step(s,a))
\]

These bounds are optional in V0.2.3.

They are controls for comparing fully enumerated action abstraction against symbolic-prefix CARDS; they are not the intended source of pre-materialization savings.

---

# 24. Conservative MAX Transfer

For a closed MAX cell `C`, sound lower evidence may come from concrete witness policies or symbolic existential-lower witnesses:

\[
F_L(C)=
\max\left(
-1,
\max_{(Q,\pi_Q)} L^{MAX}_{witness}(C,Q,\pi_Q),
\max_{R\in SELW(C)} L_R
\right)
\]

where `SELW(C)` is the set of checked `SymbolicExistentialLowerWitness` facts.

The MAX upper bound must cover **every** possible response:

\[
F_U(C)=
\max\left(
\{U_q:q\in MaterializedCovered(C)\},
\{U_R:R\in UniversalSymbolic(C)\},
UnknownUpper(C)
\right)
\]

where:

```text
UnknownUpper(C) = +1 iff an uncovered response may exist
```

and every symbolic/member response included in the domain is represented exactly once after canonical residualization.

The transfer is admissible in certified mode only after its `SoundCell` preservation lemma is accepted.

---

# 25. Conservative MIN Transfer

For a closed MIN cell, the lower bound must cover every possible response:

\[
F_L(C)=
\min\left(
\{L_q:q\in MaterializedCovered(C)\},
\{L_R:R\in UniversalSymbolic(C)\},
UnknownLower(C)
\right)
\]

with:

```text
UnknownLower(C) = -1 iff an uncovered response may exist
```

The MIN upper bound may use grounded existential-upper witnesses:

\[
F_U(C)=
\min\left(
+1,
\min_{(Q,\pi_Q)}U^{MIN}_{witness}(C,Q,\pi_Q),
\min_{R\in SEUW(C)}U_R
\right)
\]

where `SEUW(C)` contains checked `SymbolicExistentialUpperWitness` facts.

The root query itself is MAX-normalized through `u_r` and `owner_r`; MIN transfer remains necessary below the root.

As with MAX, certified use is blocked until the concrete `SoundCell` preservation lemma is accepted.

---

# 26. No Witness Means No Tight Guaranteed Side

If a MAX cell has no complete current witness policy for any GuaranteedActionCover:

```text
L(C) = -1
```

unless a separate valid proof provides a stronger lower bound.

If a MIN cell has no complete current witness policy:

```text
U(C) = +1
```

unless a separate valid proof provides a stronger upper bound.

CARDS is allowed to be useless before it is allowed to be unsound.

---

# 27. Refinement Order and Transactional Semantic Epochs

State and symbolic-response refinement is monotone in precision:

\[
P_{k+1}\preceq P_k
\]

for state partitions, with corresponding refinement relations for response/region partitions.

But V0.2.3 forbids piecemeal visible refinement.

Reference transaction:

```text
prepare refinement
    derive child predicates / child regions
    prove totality and disjointness
    construct lifted/residualized coverage
    validate required replay/denotation facts
    identify stale dependents

validate transaction
    no response lost
    no response duplicated
    no state unclassified
    semantic fact dependencies closed

atomic commit
    SemanticFactEpoch e -> e+1
    install new partition/regions/facts
    invalidate all non-refreshed dependent certificates

recompute derived fixed points for epoch e+1
```

No root certificate or fixed-point computation may observe the preparation state.

Counterexample-driven refinement must make one of the following strict changes:

```text
state predicate split
response/region split
uncovered domain reduction by newly checked coverage
certificate-strength increase from UNKNOWN to checked fact
exact materialization fallback
```

A no-op refinement is invalid.

---

# 28. Precision Monotonicity Goal

For unchanged concrete membership and unchanged semantic proof scope, pure precision refinement should not worsen certified information:

\[
L_{k+1}(x)\ge L_k(x)
\]

\[
U_{k+1}(x)\le U_k(x)
\]

for proof-relevant objects after recomputation to the extremal fixed points of §20.

Legitimate exceptions require a semantic-domain change, such as:

```text
new concrete state enters a cell
previous closure proof becomes stale
previously hidden incoming domain becomes reachable
engine/content namespace changes
```

Telemetry distinguishes:

```text
REFINEMENT_PRECISION_CHANGE
MEMBERSHIP_DOMAIN_CHANGE
CLOSURE_INVALIDATION
```

A bound regression caused solely by relabeling/splitting the same semantic domain is an invariant failure.

---

# 29. Cover Lifting under Action Refinement

For every old action class \(q\), the refinement relation records:

\[
children(q)=\{q_1,\ldots,q_m\}
\]

For a cover \(Q\):

\[
Lift(Q)=\bigcup_{q\in Q} children(q)
\]

A concrete witness action remains the same authoritative action.

Only its abstract class label changes.

Therefore an `ActionWitnessPolicy` may be relabeled without changing its concrete semantics.

This is the primary mechanism preventing the V0.1 refinement-information loss.

---

# 30. State-Partition Refinement

When a cell \(C\) splits:

\[
C\rightarrow \{C_1,\ldots,C_m\}
\]

then:

```text
gamma(C_i) are pairwise disjoint
union gamma(C_i) = gamma(C)
```

A valid witness policy on \(C\) can be restricted to each child cell.

Therefore, absent membership change, a state split cannot invalidate the concrete witness itself.

Successor-cell references and derived envelopes are rebuilt in the reference implementation.

---

# 31. SeparatorSpecV1 — Deterministic Total Refinement

A separator is not a local test that merely distinguishes two currently observed witnesses.

```text
SeparatorSpecV1 {
    separator_id
    version
    domain_type
    parent_predicate_or_region
    child_ids[]
    total_child_selector(object) -> exactly one child
    applicability_predicate
    canonical_encoding
    checker
}
```

Required theorem over the parent semantic domain:

\[
Parent = \dot\bigcup_i Child_i
\]

The selector must classify future unseen members deterministically.

For state refinement, the theorem is over the parent `Gamma` predicate domain.
For action/region refinement, it is over canonical response denotation.

A separator that requires exhaustive enumeration of the parent merely to decide child membership may be used as exact fallback but is not counted as symbolic refinement savings.

---

# 32. Counterexamples and Obligation Validation Outcomes

A counterexample is a concrete witness that falsifies a current abstraction claim.

Every proof-relevant validation procedure returns exactly one of:

```text
VALID_CERTIFICATE(certificate)
CONCRETE_COUNTEREXAMPLE(witness)
UNKNOWN(reason)
```

`UNKNOWN` is first-class. Failure to prove does not imply a counterexample, and failure to find a counterexample does not imply validity.

Typed counterexamples include at least:

```text
CELL_MEMBERSHIP_COUNTEREXAMPLE
CLOSURE_FRONTIER_COUNTEREXAMPLE
ACTION_LEGALITY_COUNTEREXAMPLE
ACTION_COVER_COUNTEREXAMPLE
SYMBOLIC_REGION_OVERLAP_COUNTEREXAMPLE
PREFIX_CONTINUATION_COUNTEREXAMPLE
PREFIX_TERMINATION_COUNTEREXAMPLE
CANONICAL_ACTION_DUPLICATION_COUNTEREXAMPLE
SUCCESSOR_COVERAGE_COUNTEREXAMPLE
VALUE_ENVELOPE_COUNTEREXAMPLE
ROOT_REGION_COUNTEREXAMPLE
SEPARATOR_TOTALITY_COUNTEREXAMPLE
```

Every witness records exact replayable identities sufficient for the independent checker to reproduce the failure.

---

# 33. Obligation Validation, Counterexamples, UNKNOWN, and Progress

Every proof-relevant obligation is handled through:

```text
ObligationValidatorV1.validate(obligation)
    -> VALID_CERTIFICATE
     | CONCRETE_COUNTEREXAMPLE
     | UNKNOWN
```

`VALID_CERTIFICATE` installs a checked semantic fact in the next atomic `SemanticFactEpoch`.

`CONCRETE_COUNTEREXAMPLE` must include enough authoritative witness data to falsify the exact claim being refined.

`UNKNOWN` means the checker could neither prove nor refute the obligation within its deterministic primitive budget. It is not a counterexample and does not justify a split by itself.

V0.2.3 does **not** claim unbounded termination merely from finite vocabularies because validators may repeatedly return `UNKNOWN`.

Guaranteed termination claim:

> **A reference run terminates under any finite `ReferenceDeterministicBudgetV3`, because primitive authoritative and proof-compute operations are metered before execution and the run halts when no admissible operation fits the remaining budget.**

For unlimited-budget semantics, fairness/progress termination is an open property unless a separate well-founded global measure is proven.

Repeated `UNKNOWN` outcomes are tracked explicitly:

```text
obligation_id
attempt_count
budget_consumed
last_unknown_reason
```

A deterministic retry policy must prevent zero-cost busy loops.

---

# 34. RegionExprV1 — Symbolic Region Algebra over Canonical Responses

```text
RegionExprV1 :=
    PrefixRegion(prefix_region_id)
  | ResponseClass(class_id)
  | Singleton(DecisionResponseKey)
  | Difference(RegionExprV1, RegionExprV1)
  | Union(disjoint RegionExprV1[])
  | Empty
```

For state `s`:

\[
\llbracket R\rrbracket_s\subseteq Responses(s)
\]

For `ActionDecision`, denotation is over canonical `CompleteActionKey`s produced by the declaration normal form.

Required symbolic operations:

```text
canonical_normalize(R)
membership(R,response)
prove_disjoint(R1,R2)
semantic_empty(R)
denotation_digest(R)
cardinality(R) -> EXACT_MEASURED | SYMBOLIC_EXACT(n) | UNKNOWN
```

`Difference(R,Singleton(a))` is the primitive residualization operation used when a symbolic response becomes materialized or when a concrete root witness is extracted.

An operation returning `prove_disjoint=true` must carry a `SymbolicDisjointnessCertificateV1`; syntactic inequality of prefixes is insufficient.

If disjointness/emptiness cannot be proved symbolically, the result is `UNKNOWN`, not an optimistic Boolean.

---

# 35. Root Response-Space Coverage

Because each query is normalized to `ROOT_OWNER = MAX`, root certification is expressed once from the maximizing perspective.

Before a certified result, CARDS requires a region partition:

\[
Responses(r)=\biguplus_i\llbracket R_i\rrbracket_r
\]

with current evidence for:

```text
total coverage
pairwise disjointness
canonical response identity
semantic emptiness of uncovered root remainder
engine/content identity
region-expression schema
```

For an `ActionDecision` root, `Responses(r)=A(r)` and coverage may be built from closed structured prefix regions plus materialized complete actions.

No action needs to be materialized merely to prove it belongs to a certified symbolic region.

But every root response must be either:

```text
materialized
symbolically covered
or uncovered
```

and certification requires the uncovered root remainder to be empty.

---

# 36. Root Region Bounds

Every root region `R` has a conservative interval:

\[
L(R)\le V(Step(r,a))\le U(R)
\quad\forall a\in\llbracket R\rrbracket_r
\]

The interval must come from checked semantic facts and derived fixed-point bounds.

A candidate concrete witness `a*` selected from region `R*` induces:

\[
R^- = Difference(R^*,Singleton(a^*))
\]

and:

\[
\llbracket R^*\rrbracket_r
=
\{a^*\}\;\dot\cup\;\llbracket R^-\rrbracket_r
\]

`R^-` remains a competitor until:

```text
its upper bound is sufficiently low
or it is certified empty
or a valid value-equivalence theorem discharges it
```

Region subtraction is therefore part of the checked `RegionExprV1` algebra rather than informal notation.

---

# 37. Concrete Root Certification — MAX-Normalized Query

Let `a*` be an exact legal root `DecisionResponse` represented by `Singleton(a*)` and selected from `R*`.

Because the query is normalized to root MAX, `a*` is certified optimal only if:

```text
ROOT_OWNER = MAX
root response space closed
root region partition total and disjoint
all proof-relevant semantic certificates current
candidate response concretely legal
```

and:

\[
L(a^*)\ge
\max\left(
\max_{R_i\neq R^*}U(R_i),
U(R^-)
\right)
\]

The residual term may be removed only with one of:

```text
RESIDUAL_REGION_EMPTY
ROOT_REGION_VALUE_EQUIVALENCE
```

`ROOT_REGION_VALUE_EQUIVALENCE` must prove that no residual member can have value greater than the candidate's certified value. Equal lower bounds are insufficient.

For `ActionDecision`, the returned response is a canonical complete legal action.

For another supported root `DecisionKind`, the returned object is that kind's exact legal response.

V0.2.3 does not need a separate MIN-root theorem because perspective normalization is an explicit game-model invariant.

---

# 38. Unique Root Best

A unique best response is certified only if:

\[
L(a^*)>
\max\left(
\max_{R_i\neq R^*}U(R_i),
U(R^-)
\right)
\]

or the residual is certified empty and the strict inequality holds against every external region.

A value-equivalence certificate for distinct residual responses establishes optimal ties, not uniqueness, unless it additionally proves the residual denotation is empty.

CARDS never infers uniqueness from abstraction coarseness.

---

# 39. Certified Result Type

`CARDS_CERTIFIED_V0_2_3` returns only:

```text
CERTIFIED_OPTIMAL_RESPONSE(
    exact_response,
    CertificateManifestV1
)
```

or:

```text
UNRESOLVED_WITHIN_DETERMINISTIC_BUDGET
UNSUPPORTED_DECISION_KIND
EXTERNALLY_CENSORED
```

`EXTERNALLY_CENSORED` is a benchmark-harness observation, not a deterministic CARDS proof result.

Any heuristic or learned fallback is outside the Certified result type.

---

# 40. Reference Search / Refinement Loop

Reference algorithm:

```text
INPUT:
    exact root state r
    ReferenceDeterministicBudgetV2 DB
    CellPredicateSchema CPS
    SeparatorVocabulary SV
    CertificateFamilyManifest CF

normalize root perspective so owner(r) = MAX
initialize exact identities
initialize total cell predicates
initialize typed decision-kind adapter
initialize materialized/symbolic/uncovered response accounting

while scheduler has an admissible next obligation:

    freeze SemanticFactEpoch

    rebuild represented indexes
    validate materialized/symbolic/uncovered partitions
    validate membership closure / SCC closure where required
    rebuild current covers and witness policies
    rebuild checked symbolic-region envelopes
    derive transfer constraints from frozen semantic facts

    lower = least_fixed_point(F_L)
    upper = greatest_fixed_point(F_U)

    assert no bound interval inversion

    rebuild canonical RegionExpr root partition
    compute residual-aware root certificate conditions

    if concrete root certificate verifies:
        return CERTIFIED_OPTIMAL_RESPONSE

    obligation = select deterministic RefinementObligation
    result = ObligationValidatorV1.run(
        obligation,
        primitive_budget_meter = DB
    )

    if result == VALID_CERTIFICATE:
        add checked semantic fact for next epoch

    elif result == CONCRETE_COUNTEREXAMPLE:
        separator = first applicable checked SeparatorSpecV1
        if separator exists:
            refine total predicate/region partition
        else:
            exact_fallback(affected_scope)

    else if result == UNKNOWN:
        apply frozen unresolved policy:
            refine elsewhere / materialize / exact fallback / terminate

return UNRESOLVED_WITHIN_DETERMINISTIC_BUDGET
```

The deterministic budget is charged by the primitive authoritative operation wrapper, not once per high-level obligation.

Wall clock is never consulted by this reference loop.

---

# 41. Refinement Obligation

The scheduler unit is:

```text
RefinementObligation {
    obligation_id
    type
    cell_key
    optional_action_class
    optional_cover_id
    optional_prefix_key
    root_support_set
    proof_role
    bound_width
    root_distance
    estimated_authoritative_cost_bucket
    stable_semantic_tiebreak
}
```

This remains intentionally different from MADS `ExpansionTask`.

CARDS schedules missing representation evidence.

MADS schedules exact search work.

---

# 42. Deterministic Reference Scheduler

Choose the lexicographically minimal tuple:

```text
(
    proof_blocking_rank,
    root_support_rank,
    -bound_width,
    root_distance,
    estimated_authoritative_cost_bucket,
    cell_key,
    obligation_type,
    stable_semantic_tiebreak
)
```

The exact rank table is versioned before experiments.

Scheduler quality affects performance only.

It must not affect certified correctness.

---

# 43. State Abstraction Is Initially Post-Materialization Compression

V0.2.3 retains the narrowed state-abstraction claim.

If exact states `s17` and `s29` have already been produced by authoritative transitions before CARDS groups them into one cell, their creation cost has already been paid.

Further, a strong `GuaranteedActionCover` witness over a closed multi-state cell initially requires:

```text
know every concrete member in the certified scope
have a complete-action witness for every member
have a MembershipClosureCertificate proving the scope cannot grow
```

Therefore reference state abstraction is initially best described as:

> **subtree compression after state discovery**

rather than:

> symbolic avoidance of state discovery.

It may save:

```text
future subtree expansion
future structured-action exploration
repeated downstream analysis
some graph/search bookkeeping
```

It does not claim to avoid the authoritative transitions that first discover the members.

Benchmarks must report state-discovery cost separately from descendant work saved.

---

# 44. Prefix-First Experimental Order

The strongest plausible pre-materialization saving comes from structured decisions, not from grouping already-created complete actions.

Therefore V0.2.3 evaluates:

```text
A. PREFIX/ACTION-ONLY CARDS
   singleton state cells
   prefix + continuation abstraction enabled
   PREFIX_VALUE_ENVELOPE enabled
   complete-action abstraction enabled

B. COMPLETE-ACTION-ONLY CONTROL
   singleton state cells
   prefix abstraction disabled
   measures post-materialization compression only

C. STATE-ONLY CARDS
   exact complete-action construction
   state cells enabled

D. COMBINED CARDS
```

The contrast between A and B tests whether CARDS actually avoids constructing:

```text
targets x modes x payments x orderings
```

rather than merely compressing actions after construction.

A decisive negative result is explicit:

> If useful closed `PREFIX_VALUE_ENVELOPE` certificates cannot be obtained without enumerating nearly every completion, the principal action-abstraction hypothesis fails for that scenario family.

---

# 45. Common Instrumentation: AuthoritativeWorkVector and ProofComputeVector

Every algorithm is instrumented below its own implementation through the same authoritative adapter event stream.

```text
AuthoritativeOperationEventV1
```

records primitive operations such as:

```text
STEP_TRANSITION
LEGALITY_CHECK
RESPONSE_ENUMERATION_STEP
PREFIX_EXPANSION_STEP
STATE_MATERIALIZATION
ROOT_CONCRETIZATION
EXACT_STATE_KEY_BUILD
```

From this shared stream derive:

```text
AuthoritativeWorkVectorV1 {
    step_transitions
    legality_checks
    response_enumeration_steps
    prefix_expansion_steps
    state_materializations
    root_concretizations
    exact_key_builds
}
```

CARDS also performs potentially substantial deterministic proof computation outside the engine. V0.2.3 therefore adds:

```text
ProofComputeEventV1
```

with at least:

```text
REGION_MEMBERSHIP_CHECK
REGION_NORMALIZATION
DISJOINTNESS_CHECK
CELL_PREDICATE_EVAL
CERTIFICATE_CHECK_STEP
HASH_BYTES
FIXED_POINT_TRANSFER
PRODUCER_DOMAIN_CHECK
```

and derives:

```text
ProofComputeVectorV1
```

No weighted scalar is treated as authoritative. Reports publish both vectors plus:

```text
CPU time
wall time
peak RSS
```

This prevents a symbolic checker from appearing cheap merely because its work did not invoke `Step`.

---

# 46. Baseline Certification Must Be Comparable

V0.2 distinguishes:

```text
SEARCH-PERFORMANCE BASELINES
CERTIFIED-ABSTRACTION BASELINES
```

Search-performance baselines include:

```text
exhaustive minimax
alpha-beta + exact TT
MADS or equivalent exact lazy search
MCTS for non-certified strength/cost comparison
```

Certified-abstraction baselines include:

```text
static state abstraction
static action abstraction
```

but they must use the same:

```text
total coverage contracts
conservative envelopes
root-space closure
root concretization requirements
```

with:

```text
refinement = disabled
```

This isolates the value of counterexample-guided refinement from abstraction itself.

---

# 47. Oracle Correctness Ground Truth

For oracle-admitted scenarios the exhaustive ground truth is defined over typed `DecisionResponse`s.

Record:

```text
exact root-relative value
optimal concrete root response set
all legal root responses
all reachable exact DecisionStateKeys
all exact decision edges
all terminal outcomes
all supported structured-decision construction paths
canonical CompleteActionKey equivalence classes when root kind is ActionDecision
```

For an `ActionDecision`, additional action-specific ground truth may include all canonical complete root actions and construction-prefix statistics.

For `ResolutionChoice`, `ReplacementChoice`, `OrderingChoice`, or other kinds, the oracle does not misuse `CompleteAction` terminology.

Oracle validation checks CARDS certificates against exact ground truth wherever the suite is small enough to enumerate.

---

# 48. Oracle Correctness Suite vs Scale Benchmark Suite

Two evaluation domains remain separate.

## ORACLE_CORRECTNESS_SUITE

Small enough for exhaustive ground truth. Used for:

```text
soundness
regression tests
certificate validation
optimal root-response checking
exact compression cardinalities
```

Admission may use hard graph/response caps because this suite exists specifically for exhaustive verification.

## SCALE_BENCHMARK_SUITE

May exceed exhaustive oracle capability. Used for:

```text
resource scaling
certification rate under fixed budgets
wall-time/RSS behavior
symbolic coverage rates
fallback frequency
```

Scale admission may depend only on properties that are:

```text
CONSTRUCTIVELY_GUARANTEED_PROPERTY
```

by the generator or cheaply observable without exhaustive search.

Properties requiring full exploration are marked:

```text
POST_HOC_MEASUREMENT
```

and may not determine scale-suite admission.

No optimality claim is made for an uncertified scale result without independent ground truth.

---

# 49. Scenario/Game Factory — Motivation

V0.2.3 retains a separate research component:

```text
CARDS Scenario Factory
```

It is not part of CARDS correctness.

Its purpose is to generate reproducible legal decision problems on which many algorithms can be compared under identical conditions.

Architecture:

```text
                    Manafold
              authoritative engine
                     |
                     v
            Scenario / Game Factory
                     |
        +------------+------------+
        |            |            |
        v            v            v
   Positions      Games      Decision Tasks
        |            |            |
        +------------+------------+
                     |
                     v
              Frozen datasets
                     |
       +-------------+-------------+
       |             |             |
       v             v             v
      MADS          CARDS        Alpha-Beta
       |             |             |
       +-------------+-------------+
                     |
                     v
            common evaluation
```

The factory should support both:

```text
algorithm benchmarking
future ML/RL data generation
```

without conflating them.

---

# 50. ScenarioDescriptorV2

A scenario is generated from a versioned descriptor and seed.

```text
ScenarioDescriptorV2 {
    scenario_family
    generator_version
    seed
    engine_identity
    rules_snapshot
    allowed_content_pool

    constructive_constraints[]
    optional_post_hoc_measurements[]

    suite_role
    split_role_if_learning
}
```

Each property declaration is typed:

```text
ScenarioProperty {
    property_id
    provenance:
        CONSTRUCTIVELY_GUARANTEED
      | CHEAP_MEASUREMENT
      | POST_HOC_MEASUREMENT
    requested_range_if_admission_relevant
}
```

Examples suitable for constructive generation may include controlled counts of explicitly created target candidates, payment alternatives, or staged continuation choices when the generator itself guarantees them.

Examples such as:

```text
exact transposition density
exact reachable depth
exact unique-state count
```

are post-hoc unless the generator has a proof-producing construction that guarantees them without exhaustive search.

---

# 51. Scenario Families — Synthetic and Naturalistic

Synthetic mechanism-controlled families remain useful:

```text
TARGET_SYMMETRY
PAYMENT_SYMMETRY
MODE_COMBINATORICS
ORDERING_COMBINATORICS
HIGH_BRANCHING
HIGH_EXACT_TRANSPOSITION
LOW_EXACT_TRANSPOSITION
TRIGGER_SENSITIVE
REPLACEMENT_SENSITIVE
STACK_INTERACTION
COMBAT_HEAVY
```

But they are not sufficient to support claims about Magic broadly because their design may mirror CARDS certificate families.

V0.2.3 therefore requires a separate:

```text
NATURALISTIC_SUITE
```

whose positions are generated independently of CARDS-specific mechanism labels, for example from:

```text
realistic locked decks
neutral authoritative rollouts
fixed policy mixtures
recorded engine states from ordinary games when provenance permits
```

Selection into the naturalistic suite must not depend on whether a CARDS certificate fires, whether MADS expands few nodes, or whether a specific algorithm solves the position cheaply.

Synthetic suites answer:

> Does the proposed mechanism work where its preconditions are intentionally present?

The naturalistic suite asks:

> How often do useful preconditions arise in realistic Manafold play?

---

# 52. Avoiding Benchmark Overfitting and Designer Bias

Before headline experiments freeze:

```text
generator schema
suite manifests
algorithm versions
certificate families
budget profiles
statistical protocol
```

Synthetic family parameters are frozen before comparison runs.

However, preregistration alone does not remove designer bias. Therefore results must be reported separately for:

```text
MECHANISM_CONTROLLED_SYNTHETIC
NATURALISTIC
OOD / ADVERSARIAL
```

A CARDS-positive result confined to `TARGET_SYMMETRY` or `PAYMENT_SYMMETRY` is reported as a mechanism result, not evidence of broad Magic effectiveness.

Naturalistic suite generation must be algorithm-blind and reproducible from manifests/seeds/provenance.

---

# 53. Suite Taxonomy

Maintain at least:

```text
ORACLE_CORRECTNESS_SUITE
SCALE_BENCHMARK_SUITE
STRESS_SUITE
```

`ORACLE_CORRECTNESS_SUITE`:

```text
small
exhaustively solved
used for soundness and exact quality checks
not the sole scaling benchmark
```

`SCALE_BENCHMARK_SUITE`:

```text
algorithm-independent generation/admission
frozen before headline runs
may lack exhaustive ground truth
used for cost/certification/scaling comparisons
```

`STRESS_SUITE`:

```text
may deliberately target known failure modes
used for robustness and falsification
never substituted for neutral headline reporting
```

Later TRAIN/VALIDATION/TEST/OOD pools remain separate from these benchmark-suite roles.

---

# 54. ScenarioManifestV1

Every generated scenario records:

```text
ScenarioManifestV1 {
    scenario_id
    descriptor
    generator_version
    generator_seed

    engine_identity
    rules_snapshot
    content_snapshot
    RNG_contract

    exact_root_state_digest
    exact_root_decision_state_key

    suite_role
    structural_measurements

    oracle_profile_if_applicable
    oracle_status_if_applicable
    ground_truth_digest_if_available
}
```

Re-running the same accepted manifest must reconstruct the same scenario.

A scale scenario does not need to pretend that exhaustive oracle truth exists.

---

# 55. Generated Games vs Generated Decision Positions

The factory may produce:

```text
complete games
mid-game positions
decision-boundary tasks
structured-decision-prefix tasks
```

For search research, a decision task is often more efficient than replaying an entire game.

For later ML/RL research, complete trajectories may be valuable.

These artifacts should share provenance but remain distinct schemas.

---

# 56. Benchmarking Search Algorithms on Generated Scenarios

Search algorithms are not necessarily "trained" on generated scenarios.

Classical algorithms are evaluated:

```text
Alpha-Beta
PNS
df-pn
MADS
CARDS
MCTS
other search procedures
```

Possible learned components may later be trained:

```text
MADS scheduler model
CARDS refinement-policy model
policy network
value network
search-cost predictor
scenario difficulty predictor
```

V0.2.3 keeps benchmark evaluation and learned training conceptually separate.

---

# 57. Dataset Split Discipline

If generated scenarios are later used for learning, maintain disjoint pools:

```text
TRAIN
VALIDATION
TEST
OOD_TEST
```

`TRAIN`:

```text
large generated corpus
may contain search traces and oracle labels
```

`VALIDATION`:

```text
unseen frozen seeds
used for model selection
```

`TEST`:

```text
frozen before final evaluation
not used for tuning
```

`OOD_TEST`:

```text
unseen card/mechanic/scenario families where feasible
used to test generalization
```

Generator version and split assignment are part of immutable provenance.

---

# 58. SearchTraceV2

A common trace schema supports fair post-hoc analysis:

```text
SearchTraceV2 {
    scenario_id
    suite_role
    algorithm_id
    algorithm_version
    configuration_id
    stochastic_seed_if_any

    root_response_if_any
    certified
    correctness_label_if_ground_truth_available
    termination_reason

    deterministic_budget_manifest_if_applicable
    external_limit_manifest
    operation_event_stream_digest
    AuthoritativeWorkVectorV1
    ProofComputeVectorV1

    wall_time
    CPU_time
    peak_RSS

    algorithm_specific_telemetry
}
```

The common authoritative-operation event layer supplies comparable engine work counts.

Timeout, memory kill, deterministic-budget exhaustion, unsupported decision kind, unresolved proof, and successful certification are distinct outcomes.

---

# 59. CARDS-Specific Telemetry

Record at least:

```text
state_partition_epoch
action_partition_epoch
cell_count
action_class_count
membership_generation_changes
open_cells
closed_cells
May class count
GuaranteedActionCover count
witness_policy count
UNKNOWN_ACTION occurrences
UNKNOWN_SUCCESSOR occurrences
state refinements
action refinements
prefix refinements
exact fallbacks
counterexamples by type
stale certificate invalidations
root-region count
validation obligations created
validation obligations discharged
```

---

# 60. Compression Metrics without Forced Enumeration

Compression counts carry a measurement status.

```text
CardinalityMeasurement :=
    EXACT_MEASURED(n)
  | SYMBOLIC_EXACT(n, certificate_id)
  | UNKNOWN
```

Examples:

```text
concrete complete-action cardinality
continuation-prefix cardinality
symbolic-region cardinality
exact state cardinality
```

A denominator may be used in an exact compression ratio only when it is `EXACT_MEASURED` or `SYMBOLIC_EXACT`.

CARDS must not enumerate a symbolic region merely to compute a prettier compression metric.

If the exact denominator is unknown, report structural metrics instead:

```text
materialized response count
symbolic region count
uncovered region count
certificate coverage count
exact-fallback count
```

and mark the compression ratio `UNKNOWN`.

This preserves the experiment's intended cost model.

---

# 61. Refinement Efficiency Metrics

Measure:

```text
root interval reduction per validation work
counterexamples per certified root
partition splits per certified root
exact fallback rate
surviving coarse-cell fraction
cover survival across action refinement
witness-policy reuse across state splits
```

A useful CARDS workload should tend toward:

```text
local refinements
persistent coarse regions
small validation overhead
large avoided structured-action construction
```

---

# 62. BenchmarkStatisticsV2 — Frozen Statistical Protocol

The benchmark protocol separates **outcome at budget** from **time conditional on eventual success**.

Required per-scenario paired outcomes include:

```text
certified / unresolved / timeout / unsupported
root response if returned
AuthoritativeWorkVector
ProofComputeVector
CPU time
wall time
peak RSS
```

For deterministic certifying algorithms, report at each frozen budget:

```text
certification rate
paired delta in authoritative work
paired delta in proof-compute work
paired delta in capped wall cost
```

Wall-time experiments require:

```text
fixed hardware/software environment
warmup policy
repetition count
median and selected quantiles
confidence interval method
predeclared outlier policy
```

Timeout is right-censored and is never treated as a successful timing sample.

To avoid survivorship bias, headline time summaries do not compare only the intersection of successfully certified runs. Use one or both of:

```text
capped cost at the external timeout
restricted mean time-to-certification (RMST) over a frozen horizon
```

alongside certification rate.

For stochastic algorithms such as MCTS:

```text
seed set frozen
seed aggregation rule frozen
oracle-ground-truth action quality reported where available
```

MCTS results remain in a non-certified evaluation family; no 'certification rate' is invented for it.

---

# 63. Primitive Deterministic Budgeting vs External Limits

The algorithmic budget is deterministic and metered at primitive operations.

```text
ReferenceDeterministicBudgetV3 {
    max_step_transitions
    max_legality_checks
    max_response_enumeration_steps
    max_prefix_expansion_steps
    max_certificate_check_steps
    max_region_operations
    max_predicate_evaluations
    max_fixed_point_transfers
    optional max_materializations
}
```

Before each primitive authoritative or proof-compute operation, the shared meter checks whether the operation credit remains.

A composite call such as:

```text
validate_or_expand(obligation)
```

cannot bypass budgeting merely because its internal cost was unknown at call entry; each primitive sub-operation consumes credit independently.

External experiment limits are separate:

```text
BenchmarkExternalLimit {
    wall_clock_timeout
    RSS_cap
    CPU_allocation
}
```

An external timeout may censor a benchmark run. It is not part of CARDS deterministic semantics.

Termination guarantee in V0.2.3 is explicitly limited to finite deterministic budget profiles.

---

# 64. Required Baseline Matrix and Evaluation Families

Certified/search-proof family:

```text
A. exhaustive minimax on oracle fixtures
B. alpha-beta + exact TT
C. MADS / exact lazy certified search
D. static certified state abstraction
E. static certified action abstraction
F. CARDS action/prefix-only
G. CARDS state-only
H. CARDS combined
```

Where practical add:

```text
PNS / df-pn
best-first minimax
B*
```

Non-certified strength/cost family:

```text
MCTS + exact engine
future learned policy/value search
```

On oracle fixtures, non-certified methods may be evaluated against exact optimal root actions/values.

On scale fixtures without ground truth, their outcome axis is **not** called certification rate or solve rate. Report separately, for example:

```text
chosen action
self-consistency / evaluation score where valid
resource cost
stochastic variance
```

unless independent ground truth becomes available.

Certified and non-certified outcome axes must never be merged into one headline percentage.

---

# 65. CARDS Ablations

Required V0.2.3 ablations:

```text
CARDS-action-only
CARDS-state-only
CARDS-combined

CARDS - exact TT reuse
CARDS - witness-policy strengthening
CARDS - structured-prefix abstraction
CARDS - refinement prioritization
CARDS - coarse semantic classes
CARDS exact-fallback-only control

static abstraction with same coverage contracts
```

A later optimization ablation may compare:

```text
REFERENCE_REBUILD
vs
INCREMENTAL_V1
```

only after semantic equivalence is established.

---

# 66. Catastrophic Correctness Gates

All must remain zero:

```text
CERTIFIED_WRONG_ROOT_RESPONSE
UNSOUND_CELL_INTERVAL
UNSOUND_REGION_INTERVAL
BOUND_INTERVAL_INVERSION
UNCOVERED_RESPONSE_OMITTED
SYMBOLIC_RESPONSE_DOUBLE_COUNTED
MATERIALIZED_AND_SYMBOLIC_OVERLAP
CANONICAL_RESPONSE_COLLISION_WITH_DIFFERENT_COMMIT_OUTCOME
CONSTRUCTION_REPLAY_MISMATCH
INCOMPLETE_CONSTRUCTION_PASSED_TO_STEP
OPPONENT_CHOICE_SWALLOWED_INSIDE_ACTION_CONSTRUCTION
UNSUPPORTED_PROTOCOL_CERTIFIED
INVALID_PRODUCER_DOMAIN_CLOSURE
CIRCULAR_CLOSURE_WITHOUT_GROUNDING
UNKNOWN_TREATED_AS_COVERED
EXACT_SUCCESSOR_CERT_WITH_DIFFERENT_DECISION_STATE_KEYS
DERIVED_BOUND_USED_AS_OWN_SEMANTIC_PREMISE
RESIDUAL_ROOT_COMPETITOR_IGNORED
INVALID_ROOT_CONCRETIZATION
NON_TOTAL_SEPARATOR_COMMITTED
PARTIAL_REFINEMENT_EPOCH_OBSERVED
```

Any nonzero count blocks performance/novelty claims.

---

# 67. Mandatory Regression: Disjunctive Removal Cover

Construct:

```text
C = {s1, s2}

s1:
    legal representative = DESTROY_CREATURE

s2:
    legal representative = EXILE_CREATURE
```

Initially:

```text
Q0 = {REMOVE_CREATURE}
Q0 in G(C)
```

Refine:

```text
REMOVE_CREATURE
 -> DESTROY_CREATURE
 -> EXILE_CREATURE
```

Expected lifted cover:

```text
Q1 = {DESTROY_CREATURE, EXILE_CREATURE}
Q1 in G(C)
```

The existing concrete witness mapping remains valid after relabeling.

No certified lower-bound information may be lost solely because of the action-class split.

---

# 68. Mandatory Regression: Cell Membership Growth

Construct:

```text
C generation 1 = {s1, s2}
```

with a valid cover certificate.

Then discover:

```text
s3 -> same current abstract cell
```

Expected:

```text
membership_generation 1 -> 2
old cover certificate = STALE
old witness policy = STALE until extended
proof-relevant bound tightening from stale certificate removed
```

After extending the certificate to s3, the bound may tighten again.

---

# 69. Mandatory Regression: Symbolic Coverage Removes Unknown

At an exact MAX `ActionDecision`, construct a complete action space with three canonical actions:

```text
a1 materialized and losing
{a2,a3} not materialized but covered by one accepted PrefixRegion R
```

Require:

```text
M = {a1}
S = {a2,a3}
U = empty
ACTION_SPACE_CLOSED = true
UNKNOWN_ACTION absent
```

If `R` has certified upper bound `0`, the MAX upper transfer may use:

```text
max(U(a1), U(R))
```

and must **not** inject `+1` merely because `a2/a3` have no materialized action objects.

Negative control:

Remove coverage for `a3`.

Then:

```text
U = {a3}
UNKNOWN_ACTION present
MAX upper includes +1
```

This regression directly protects the V0.2.3 three-way semantics.

---

# 70. Mandatory Regression: Root Region Residual Competitor

Construct:

```text
R* = {a*, b}
R2 = {c}

V(a*) = 0
V(b)  = +1
U(R2) = -1
```

Expected:

```text
selecting a* from R* creates logical residual region R- = {b}
U(R-) = +1
0 >= max(-1,+1) is false
CERTIFIED_OPTIMAL_ACTION(a*) forbidden
```

Then test two legal repair paths.

### Residual bounded

If later:

```text
U(R-) <= 0
```

then `a*` may certify under non-strict optimality.

### Exact region equivalence

If an accepted certificate proves every action in `R*` reaches the same exact `DecisionStateKey` as `a*`, the residual may be discharged without materializing every member.

This regression directly protects `CERTIFIED_WRONG_ROOT_ACTION = 0`.

---

# 71. Mandatory Regressions: Decision Responses, Construction Replay, and Symbolic Canonicality

At minimum:

## A. Construction replay sufficiency

Two construction paths that finalize to the same `CompleteActionKey` must produce the same exact commit outcome from the same base state.

Near miss:

```text
same visible spell/target
but different payment source changes authoritative successor
```

Expected:

```text
different canonical response identity
or explicit proof that payment source is exactly irrelevant
```

## B. Same-owner until finalize

Fixture containing an opponent-owned choice before an action is finalized.

Expected:

```text
UNSUPPORTED_ACTION_PROTOCOL
```

not a silently continued MAX construction.

## C. Canonical duplicate path

Two syntactically distinct declaration paths denote the same authoritative complete action.

Expected:

```text
same canonical CompleteActionKey
one semantic response
no double symbolic coverage
```

## D. Symbolic disjointness without leaf enumeration

Two large symbolic regions with a structural proof of disjoint canonical declaration domains.

Expected:

```text
proof succeeds
zero exact leaf enumeration attributable to disjointness checker
```

## E. Symbolic overlap counterexample

Two regions syntactically differ but share one canonical action.

Expected:

```text
disjointness certificate rejected
concrete counterexample or UNKNOWN
```

## F. Cell soundness oracle check

For every oracle cell after every semantic epoch:

\[
\min_{s\in\Gamma(C)}V_r(s)\ge L(C)
\]

and:

\[
\max_{s\in\Gamma(C)}V_r(s)\le U(C)
\]

Any violation is catastrophic.

## G. Internal-SCC producer remainder

A represented state inside an abstract SCC has an uncovered response that reaches a previously undiscovered member of the same SCC.

Expected:

```text
SCC closure rejected until producer remainder is closed
```

## H. Root-relative utility

Same exact game state queried once from player A's root decision and once from player B's root decision when both fixtures are meaningful.

Expected terminal sign and MAX/MIN ownership transform consistently with query actor.

---

# 72. Determinism

Given identical:

```text
scenario manifest
engine/content identity
CARDS version
DecisionKind adapter versions
CellPredicate schema
SeparatorSpec vocabulary
CertificateFamily manifest
ReferenceDeterministicBudgetV2
stable semantic ordering
```

reference CARDS must reproduce:

```text
root perspective normalization
cell predicates
represented membership generations
M/S/U response partitions
closure/SCC certificates
validation outcomes
counterexample sequence
separator choices
partition refinements
semantic-fact epochs
least lower fixed points
greatest upper fixed points
RegionExpr normalization
residual regions
final deterministic result
AuthoritativeOperationEventV1 sequence
```

No deterministic result depends on:

```text
wall clock
pointer address
hash-map iteration order
thread race
allocation layout
```

External timeout may truncate observation but never redefines the internal sequence.

---

# 73. Reference Before Incremental Optimization

V0.2.3 reference mode rebuilds after every proof-relevant mutation:

```text
state-cell indexes
action-pair partition
GuaranteedActionCovers
witness-policy validity
abstract successor relation
value envelopes
root regions
```

Only after oracle equivalence may V1-style optimization introduce:

```text
dirty cells
generation-stamped cache entries
incremental relation maintenance
lazy stale-certificate removal
incremental root-region bounds
```

Optimized mode must be differential-tested against reference mode step by step.

---

# 74. Revised Implementation Phases

## Phase 0 — Formal gate freeze

Prove/specify before certified search code:

```text
Construction Replay Sufficiency
Symbolic Canonical Coverage / Deduplication
SoundCell / SoundRegion / SoundResponseClass
```

## Phase 1 — Manafold decision-response mapping

```text
DecisionKind mapping
Responses(s)
Step(s,response)
root-relative utility u_r
root-relative owner_r
unsupported-kind behavior
```

## Phase 2 — Structured action model

```text
ConstructionState(base,prefix,sigma)
same-owner-until-finalize audit
FinalizeConstruction
CanonicalDeclarationNormalFormV1
CompleteActionKey replay sufficiency fixtures
```

## Phase 3 — Symbolic coverage kernel

```text
RegionExpr
SymbolicCoverageCertificate
SymbolicDisjointnessCertificate
M/S/U accounting
atomic materialization residualization
```

## Phase 4 — Cell semantics and producer closure

```text
phi / Gamma / gamma
ProducerDomainV1
root-seeded closure
SCC producer closure
```

## Phase 5 — Value certificate types

```text
EXACT_SUCCESSOR_EQUALITY
DIRECT_VALUE_ENVELOPE
quantified symbolic witness types
```

## Phase 6 — Abstract transfer proof

```text
SoundCell preservation
F_L/F_U monotonicity
least-lower / greatest-upper theorem
```

## Phase 7 — Root certificate

```text
root response partition
residual competitor
concrete root witness
```

## Phase 8 — Obligation validation + deterministic reference loop

## Phase 9 — Oracle correctness suite

## Phase 10 — Action/prefix-only experiment

Stop if symbolic checking requires near-total leaf enumeration or useful envelopes are absent.

## Phase 11 — State abstraction experiment

Only if Phase 10 justifies further complexity.

## Phase 12 — Scale + naturalistic benchmarks

Incremental optimizations and learning remain later work.

---

# 75. Immediate Pre-Implementation Artifacts and Trust Boundary

Required in order:

```text
1. CARDS_CONSTRUCTION_REPLAY_SUFFICIENCY_V1.md
2. CARDS_SYMBOLIC_CANONICAL_COVERAGE_V1.md
3. CARDS_ABSTRACT_SOUNDNESS_V1.md
4. CARDS_DECISION_RESPONSE_MODEL_V1.md
5. CARDS_STRUCTURED_DECISION_MODEL_V1.md
6. CARDS_CELL_PREDICATE_AND_PRODUCER_CLOSURE_V1.md
7. CARDS_OBLIGATION_VALIDATION_V1.md
8. CARDS_ROOT_CERTIFICATE_V3.md
9. CARDS_BENCHMARK_STATISTICS_V2.md
10. CARDS_CERTIFICATE_MANIFEST_V1.md
```

The trusted computing base must be explicit.

Potentially trusted:

```text
Manafold authoritative Step semantics
exact DecisionStateKey contract
root-relative utility adapter
FinalizeConstruction adapter
canonical response encoder
cell/region predicate evaluators
small independent certificate checkers
primitive instrumentation layer
```

Not trusted merely because it is convenient:

```text
heuristic abstraction proposals
scheduler ordering
learned predictions
cached derived fixed-point bounds
algorithm-local work counters
syntactic prefix inequality
```

The preferred architecture is generator/checker separation: complex code may propose a certificate, but a small deterministic checker decides whether it becomes a `SemanticFactV2`.

---

# 76. Implementation Readiness Gate

Current V0.2.3 design status:

```text
Research hypothesis                           PASS
DecisionResponse kernel                       PASS DESIGN
Root-relative utility/ownership                PASS DESIGN
M/S/U semantic partition                      PASS DESIGN
UNKNOWN = uncovered                           PASS
Residual root competitor                      PASS DESIGN
Concrete cell-soundness invariant             PASS DEFINITION
Producer-domain closure                       PASS DESIGN
Transactional semantic epochs                 PASS DESIGN
Common authoritative instrumentation          PASS DESIGN
Proof-compute instrumentation                  PASS DESIGN
Naturalistic benchmark requirement            PASS DESIGN

Construction Replay Sufficiency proof         REQUIRED / BLOCKER
Canonical symbolic coverage proof              REQUIRED / BLOCKER
Transfer -> SoundCell proof                    REQUIRED / BLOCKER
Manafold same-owner construction audit         REQUIRED / BLOCKER
Prefix certificate checker implementation      REQUIRED
Producer-domain closure checker                REQUIRED
Root certificate proof                         REQUIRED
Benchmark statistics implementation            REQUIRED BEFORE HEADLINE RESULTS
Scientific novelty                             OPEN
```

Therefore:

```text
ALGORITHM RESEARCH DESIGN       = HARDENED BUT NOT FROZEN
CERTIFIED REFERENCE CODE        = BLOCKED
SCENARIO FACTORY / ORACLE WORK  = CAN PROCEED INDEPENDENTLY
```

No certified implementation claim should be made until the three blocking lemmas have executable fixtures/checkers against Manafold.

---

# 77. Research Hypotheses

## H1 — Prefix-level semantic compression exists

> In at least some Magic-like structured decisions, large sets of target/mode/payment/ordering completions can remain symbolically represented long enough to avoid materializing many complete actions before the root decision is certified.

This is now the primary hypothesis.

## H2 — Complete-action abstraction alone is weaker

> If prefix abstraction is disabled, grouping already materialized complete actions will save substantially less authoritative work than prefix/action CARDS on structured-combinatorial fixtures.

## H3 — State abstraction is initially downstream compression

> After concrete state discovery, query-local state cells can still reduce descendant work enough to offset closure/witness bookkeeping on at least some position classes.

V0.2.3 does not claim symbolic avoidance of arbitrary state discovery.

## H4 — Refinement remains local

> Counterexamples usually split a limited abstract region rather than forcing global exact fallback.

## H5 — Quantified action coverage survives useful refinement

> `GuaranteedActionCover` plus cover lifting preserves useful `forall state exists representative` information across many action/prefix splits.

## H6 — Generated suites separate mechanisms

> Frozen scenario families can distinguish prefix-compression, state-compression, transposition, and search-scheduling effects without selecting scenarios based on which algorithm wins.

---

# 78. Strong Positive Result

A convincing V0.2.3 result would require all of:

```text
Oracle correctness:
    zero catastrophic gate failures

Root certification:
    residual-region regression passes
    no intra-region competitor is ignored

Structured action fixtures:
    prefix/action CARDS materializes materially fewer complete actions
    than complete-action-only control at comparable certification rate

State fixtures:
    descendant work saved exceeds membership/witness closure overhead
    on at least one clear position family

Abstract-cycle fixtures:
    least-lower / greatest-upper reference propagation agrees with oracle truth

Scale suite:
    cost-vs-certification curves remain competitive beyond oracle-small cases

Statistics:
    paired scenarios, timing repetitions, stochastic seed variance,
    and censoring are reported according to the frozen protocol
```

A reduction in one raw counter without equivalent solve/certification quality is not sufficient.

---

# 79. Useful Negative Results

Examples include:

```text
prefix closure requires nearly full complete-action enumeration
prefix denotation bookkeeping costs more than avoided materialization
root residual regions stay too wide to certify without flattening
abstract fixed points remain [-1,+1] until near-exact refinement
membership closure makes state abstraction too expensive
state abstraction saves only bookkeeping, not meaningful engine work
GuaranteedActionCovers rarely survive realistic Magic refinements
CARDS frequently falls back to exact representation
MADS / exact lazy search dominates on most neutral scenarios
scale-suite certification rate collapses
```

These remain scientifically useful.

They would identify whether the bottleneck is representation, proof closure, or simply the intrinsic semantics of Magic.

---

# 80. Novelty Position

V0.2.3 preserves the conservative prior-art position.

CARDS does not claim to invent:

```text
abstract interpretation
CEGAR
abstraction refinement for games
state abstraction
action abstraction
counterexample-guided planning
certified abstraction
```

The possible research contribution remains the specific integration of:

```text
authoritative TCG structured-decision protocol
+
query-local state/action/prefix abstraction
+
quantified GuaranteedActionCovers
+
state-conditioned concrete witnesses
+
conservative adversarial envelopes
+
counterexample-guided monotone refinement
+
symbolic root-region coverage
+
exact concrete root certification
+
reproducible scenario/game factory
```

Whether this combination is novel enough for publication remains open until the V0.2.3 prior-art matrix is completed.

---

# 81. CARDS vs MADS After V0.2.3

```text
MADS
    representation:
        exact
    adaptation target:
        computation
    key question:
        Which exact unresolved work blocks the root proof?

CARDS
    representation:
        intentionally variable
    semantic authority:
        exact engine
    adaptation target:
        representation precision
    key question:
        Which concrete distinctions are required by the root proof?
```

A later hybrid may be investigated:

```text
CARDS chooses representation resolution
MADS chooses exact expansion order inside the represented region
```

But no hybrid should be attempted before independent baselines exist.

---

# 82. Scenario Factory Relationship to MADS and CARDS

The factory creates a neutral experimental substrate:

```text
Scenario Factory
      |
      +--> Alpha-Beta
      +--> PNS
      +--> MCTS
      +--> MADS
      +--> CARDS
      +--> future learned methods
```

The same scenario manifests permit mechanism-specific evaluation:

```text
high branching
high structured-action fanout
high exact transposition
low exact transposition
high payment symmetry
high target symmetry
trigger-sensitive distinctions
replacement-sensitive distinctions
```

The factory is therefore a research-platform feature of Manafold, not a CARDS-only advantage.

---

# 83. Future Learning Layer

After a large corpus of solved scenarios exists, training data may include:

```text
exact state
legal structured decision space
optimal concrete root response set
minimax value
exact game graph statistics
MADS expansion trace
CARDS refinement trace
CARDS counterexample types
CARDS successful separators
AuthoritativeWorkVectorV1 + ProofComputeVectorV1 + measured runtime cost
```

Possible supervised targets include:

```text
best next MADS ExpansionTask
best next CARDS RefinementObligation
likely useful CARDS separator
position difficulty
search-cost estimate
policy/value targets
```

No learned prediction becomes proof evidence merely because it was trained on exact data.

The proof layer remains separate.

---

# 84. Questions for the Next Reviewer

The next review should focus on proof closure, not feature ideation:

1. Is `FinalizeConstruction` sufficient to replay every admitted Manafold action from the original base state?
2. Can two construction states share one `CompleteActionKey` while differing in any authoritative commit effect?
3. Does the first Manafold slice truly satisfy same-owner-until-finalize for every supported action protocol?
4. Is `CanonicalDeclarationNormalFormV1` complete and injective over canonical authoritative responses?
5. Can symbolic region disjointness be checked without enumerating all completions?
6. Does any current checker hide leaf enumeration behind a convenience API?
7. Are `M`, `S`, and `U` pairwise disjoint after every atomic materialization/refinement commit?
8. Is `S = D \ M` sufficient when several overlapping pre-normalization region proposals exist?
9. Are `SoundCell`, `SoundRegion`, and `SoundResponseClass` the right concrete contracts?
10. Does every MAX/MIN transfer have a proved soundness lemma under those contracts?
11. Does `EXACT_SUCCESSOR_EQUALITY` use only exact `DecisionStateKey`, with no semantic-equivalence escape hatch?
12. Are any useful payment regions exact-successor equal in real Manafold states?
13. Which first regions need `DIRECT_VALUE_ENVELOPE` instead?
14. Should state isomorphism remain entirely outside V0?
15. Does ProducerDomainV1 capture unresolved responses originating inside the same abstract SCC?
16. Is producer closure root-seeded and non-circular in every fixture?
17. Can producer-domain closure itself be checked without near-total game enumeration?
18. Does root-relative `u_r` correctly handle draws and both players' terminal outcomes?
19. Can an incomplete construction ever reach an opponent-owned choice in the planned slice?
20. Are SemanticFactEpoch commits truly atomic with respect to root certification and fixed-point recomputation?
21. Are symbolic existential witness types using the intended `forall state, exists response` quantifier order?
22. Can a universal symbolic envelope be mistaken for an existential witness, or vice versa?
23. Does every region cardinality report its measurement status rather than forcing enumeration?
24. Does `ProofComputeVector` capture the dominant checker costs in practice?
25. Is the naturalistic suite generated independently enough to expose designer bias?
26. Are scale-suite admission properties constructive rather than post-hoc exhaustive?
27. Does the censoring-aware benchmark protocol avoid successful-run survivorship bias?
28. On the first action-only fixture, how many leaves are avoided **after counting all checker work**?
29. If symbolic coverage is sound but expensive, what exact stop threshold ends the CARDS direction?
30. After these gates, is any path left by which a wrong root response can be certified?

---

# 85. One-Sentence Definition

> **CARDS V0.2.3 is a counterexample-guided adversarial search architecture that represents parts of a typed authoritative response space symbolically only when construction replay, canonical denotation, coverage, and value bounds are independently checkable; maintains concrete-sound intervals over total query-local state cells; and returns one exact root response only after all covered, uncovered, and residual competitors are conservatively excluded.**

---

# 86. Immediate Next Artifacts

The next artifact is no longer another broad architecture revision.

First:

```text
CARDS_CONSTRUCTION_REPLAY_SUFFICIENCY_V1.md
```

It must enumerate the actual Manafold action-construction fields and prove/test:

\[
Step(base(cs),FinalizeConstruction(cs))\equiv CommitOutcome(cs)
\]

for every admitted action family.

Second:

```text
CARDS_SYMBOLIC_CANONICAL_COVERAGE_V1.md
```

It must define:

```text
CanonicalDeclarationNormalFormV1
canonical CompleteActionKey
SymbolicCoverageCertificateV1
SymbolicDisjointnessCertificateV1
non-enumeration accounting
atomic S -> M residualization
```

Third:

```text
CARDS_ABSTRACT_SOUNDNESS_V1.md
```

It must prove the transfer rules against:

\[
\forall s\in\Gamma(C):L(C)\le V_r(s)\le U(C)
\]

Only after these three are accepted should:

```text
CARDS_CELL_PREDICATE_AND_PRODUCER_CLOSURE_V1.md
CARDS_OBLIGATION_VALIDATION_V1.md
CARDS_ROOT_CERTIFICATE_V3.md
```

be frozen.

That ordering is intentional: closure and fixed-point machinery are useful only after the execution and symbolic-denotation bridges are sound.

---

# 87. Conclusion

V0.2.3 narrows CARDS to the question that actually matters.

The architecture is not useful merely because it can name a prefix region. It is useful only if that region can replace exact leaves **without lying and without secretly generating those leaves in the checker**.

The first hard bridge is authoritative execution. An incomplete construction is not an action. A finalized construction is trustworthy only when its canonical response is replay-sufficient from the original authoritative base state. V0.2.3 therefore makes `FinalizeConstruction` and Construction Replay Sufficiency a blocking theorem.

The second bridge is symbolic denotation. Prefixes are not automatically disjoint just because their syntax differs. CARDS now requires a canonical declaration normal form or an equivalent symbolic proof that coverage and disjointness hold over canonical responses. Exhaustive deduplication remains a correctness fallback, but it is not counted as symbolic savings.

The third bridge is abstract-value soundness. Least/greatest fixed points and monotone operators are secondary. The primary requirement is concrete:

\[
\forall s\in\Gamma(C):
L(C)\le V_r(s)\le U(C)
\]

Every transfer must preserve that statement.

Membership closure is likewise grounded in producer domains rather than observed edges. An unresolved response inside an SCC is still capable of producing a missing cell member; closure must account for that producer before claiming `gamma = Gamma`.

The experimental interpretation is consequently stricter:

```text
symbolic coverage succeeds
only if
    coverage is sound
    canonicality is sound
    bounds are sound
    checker does not hide equivalent exhaustive work
```

The first decisive experiment is therefore not "does CARDS beat alpha-beta?" It is:

> **On a realistic Manafold ActionDecision, can CARDS certify a useful symbolic region, account for its canonical complete-action denotation, derive a nontrivial sound value envelope, and prove the root response while materializing substantially fewer complete actions even after all authoritative and proof-compute work is counted?**

If yes, CARDS has demonstrated the core adaptive-representation mechanism.

If no, the negative result is equally precise: semantic correctness may force the algorithm to reconstruct the very complete-action space it hoped to avoid.

That is the right falsifiable boundary for V0.2.3.

---

