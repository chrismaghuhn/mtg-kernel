# MADS-02F-C Exhaustive Synthetic Oracle Stress Status

**Scope:** test-only validation of `OracleFixtureV1` and `MadsGraphV1` on
synthetic fixture DAGs. This does not exercise MTG engine transitions, add a
search algorithm, change the scheduler, or establish real-engine TT safety.

## Inventory and interfaces

The tests use existing public `OracleFixtureV1::solve_oracle_v1()`,
`MadsGraphV1::new`, `nodes`, `root_bounds`, `root_action_bounds_v1`,
`result_v1`, `rebuild_frontier_v1`, `expand_next_v1`, and `run_v1` APIs. No
private MADS hook or production source change was needed. They are registered
as a `#[cfg(test)]` library module.

Oracle admission requires a present unique root ID, unique node IDs,
nonempty game/construction action domains, unique nonempty action IDs and
orders sorted in semantic order, existing child IDs, well-owned construction
nodes with the owner actor preserved, every node reachable from the root,
acyclicity, and maximum path depth at most 32. Per-game-node actions cap at
64; construction continuations cap at 32; game nodes cap at 100,000; total
edges cap at 500,000; instrumented authoritative transitions cap at
2,000,000. Fixture terminals are exactly Loss/Draw/Win (-1/0/+1). `Halted`
is not a fixture terminal type.

The MADS graph admits the complete root action domain immediately, retaining
UNKNOWN intervals for unexpanded action slots. `expand_next_v1` selects one
task from a rebuilt frontier, creates or reuses the exact fixture child, adds
the reverse-parent edge, and propagates changed bounds through parents.
Oracle fixture IDs are exact only within that fixture. `run_v1` stops at
first certification or budget exhaustion; its `anytime_action` is separate
from `chosen_action` and certificates.

## Generated domains

### Bounded exhaustive domain

Every member of this explicitly finite family is generated and checked:

- 1, 2, or 3 ordered root actions (MAX/P0).
- Each root branch is either one terminal or a MIN/P1 node with 1 or 2
  ordered terminal actions.
- Each leaf independently takes all three outcomes: -1, 0, or +1.
- All ordered leaf label assignments are enumerated. Identical terminal
  outcomes are interned to create exact shared DAG successors.

That gives 15 branch programs and `15 + 15^2 + 15^3 = 3,615` fixtures.
This is exhaustive for the defined shallow family, not for all possible DAGs.
It covers one- and two-edge paths, all required root widths, root ties,
unequal root values, unfavorable outcome/action orders, shared terminal
successors and multiple parents.

### Deterministic sampled deeper family

An explicitly **sampled** family of 128 DAGs uses SplitMix64 with root seed
`0x02f0_cafe_0000_0001`. Each graph has a MAX/MIN/MAX decision path, 1-3
actions at MAX nodes, a shared MAX node referenced by multiple parents, and
interned terminal outcomes. Sample fixture IDs include the generated seed.
This is deterministic sampling, not exhaustive enumeration.

## Differential and budget checks

For each generated fixture, the independent oracle is solved first. The
tests inspect the initial graph, then expand one task at a time and recheck
every admitted MADS node and every root action after each expansion:

`lower <= oracle node value <= upper`.

They also check the complete ordered root action set, UNKNOWN on unexpanded
root actions, root interval containment, legal anytime actions, and that
every certified action/chosen action is oracle-optimal. An unresolved result
must have no chosen action even though it has an anytime action. Whenever all
root action intervals become exact, the certified optimal set must equal the
oracle tie set. A separate assertion confirms that root-action certification
can precede exact root-value resolution.

Each exhaustive fixture also runs budgets 0, 1, 2, the first-certification
budget found by single-expansion continuation, and the fixture edge-count
budget. Budget 0 must expand zero edges. Every result is checked against the
oracle, and a partial-budget result cannot certify a nonoptimal action. The
stepwise differential passes already drain every task the reference scheduler
exposes, checking bounds after each such expansion.

## Transposition and scheduler checks

The dedicated shared-child DAG routes two root actions through separate MIN
parents to one shared MAX node. The public scheduler tests verify one stored
node for that fixture ID, one valid fixture-local reuse hit, no duplicated
frontier `(owner, action_order)` tasks, deterministic frontier rebuilds,
reverse-parent bound propagation, and conservative root bounds after each
expansion.

The test-only reference ordering independently reconstructs the public
scheduler key for these game-decision-only cases: BOTH-role rank, descending
bound width, ascending root distance, estimated cost bucket, owner ID, then
semantic action order. Tests cover equal-width/equal-distance tasks and a
three-way equal root-action challenger tie; the earliest semantic challenger
is selected stably. Construction-node semantic ordering is covered by the
existing `mads_v1` regression, not by this generated family.

The existing internal test
`shared_descendant_propagates_to_every_parent_and_roles_merge_once` directly
prepares a shared critical-DAG state and verifies `BOTH`-role support union.
The new public-scheduler DAG did not naturally expose a BOTH task in its
chosen expansion sequence: lower-bound critical tasks were selected before
the challenger path reached the shared node. This is recorded as a boundary
of the public scheduler test, not as a scheduler mismatch; the existing
prepared-state regression still checks role-mask union. No `mads_v1.rs` hook
was added while Agent A may be editing that file.

## Invalid fixture cases

Negative tests use existing errors and validators:

| Case | Observed contract |
|---|---|
| Cycle | `CYCLE_DETECTED` |
| Unreachable node | `INVALID_ORACLE_FIXTURE` |
| Missing child | `INVALID_ORACLE_FIXTURE` |
| Duplicate action identity | `INVALID_ORACLE_FIXTURE` |
| Terminal root / wrong root actor | `INVALID_ORACLE_FIXTURE`; MADS root validation also returns its existing `InvalidRoot` |
| Construction owner actor mismatch | `INVALID_ORACLE_FIXTURE` |
| More than 64 game actions | `ORACLE_FIXTURE_TOO_LARGE` |
| More than 32 construction continuations | `ORACLE_FIXTURE_TOO_LARGE` |
| Path deeper than 32 | `ORACLE_FIXTURE_TOO_LARGE` |
| Authoritative transition count above 2,000,000 | `ORACLE_FIXTURE_TOO_LARGE` |

The 100,000-game-node and 500,000-total-edge caps are not stress-allocated in
this standard test module; doing so would require a much larger fixture than
these correctness cases need. Their checks remain in the oracle admission
implementation. A fixture cannot encode `Halted` as a terminal because the
fixture outcome enum deliberately has no such variant.

## Results

On the MADS-02F-C branch:

- Bounded exhaustive fixtures: 3,615.
- Deterministic sampled deeper fixtures: 128 (seed root above).
- Stepwise expansion checks: 22,356 exhaustive + 1,423 sampled = 23,779.
- Bound violations: 0.
- Certification violations: 0.
- Scheduler ordering mismatches: 0 for covered public game-decision task
  shapes.
- Shared DAG reuse/reverse-parent propagation: passed; one valid reused node
  in the focused shared-child fixture.
- New tests: 7 passed.
- Existing `mads_v1` tests: 16 passed, including BOTH-role union regression.
- `cargo fmt --all -- --check`: passed.
- Package `cargo clippy -p mtg-kernel --lib --tests -j 3 -- -D warnings`: passed.
- Full workspace suite and large admission-cap allocations: not run.

No reproducible MADS correctness failure was found. This result applies only
to the enumerated/sampled synthetic `OracleFixtureV1` families. It is not a
performance measurement, engine differential, DynamicEngineSearchV1 result,
or proof that real-engine state transpositions are safe.
