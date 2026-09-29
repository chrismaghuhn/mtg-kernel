# MADS-02C Dynamic Engine Search Status

**Implementation status: review fixes implemented and focused verification passed on rebased main.**

`DynamicEngineSearchV1` owns a path-local engine tree and accepts a borrowed
authoritative `GameState`, its current engine `Decision`, and a deterministic
expansion budget. Construction clones the supplied state and re-runs
`advance_until_decision` on that clone. It admits the root only when the engine
returns the identical `Decision` and leaves the clone equal to the supplied
state. Reduced, stale, or non-quiescent root frames fail with
`INVALID_ENGINE_DECISION_FRAME`. The validated root clone is then owned by the
tree. `engine::step` runs only at expansion, and the caller's state is never
mutated. Budget zero creates no successor clone or transition. No state key,
transposition table, or hidden information abstraction is used.

The explicit admitted decision set is `CastSpellOrPass` (complete raw spell,
mana ability, land, activated ability, plot, and pass domain), empty
`DeclareAttackers`, and child `GameOver`. `Halted`, continuations, and other
decisions fail closed. Root coverage is bound to the engine-recomputed
decision. Certification compares every legal root action's lower bound to all
competitors' upper bounds. Results return both stable IDs and executable
`engine::Action` values. A root action certificate survives later unsupported
expansion; latest search status, action certification, and optional exact root
value are separate fields.

MADS-01's existing `FRONTIER_REBUILD_REFERENCE` contract remains unchanged:
its owner tie-break is `FixtureNodeId`. The dynamic tree has no fixture IDs, so
its separately versioned policy is `FRONTIER_REBUILD_DYNAMIC_PATH_V1`. It uses
the same critical-support task formation, role-rank, bound-width,
root-distance, cost-bucket, and stable action-order criteria, with semantic
action path as its owner tie-break. A regression fixture proves these owner
orders can differ and tests both contracts explicitly.

The bounded engine comparison synchronizes expansions using MADS-01's task
order and compares the complete critical-support task sets after every step
through frontier exhaustion (more than 64 expansions). This proves task-set
agreement for the tested path-local tree, not selected-order parity. Scheduler
parity on an actual engine DAG with multiple parents has not been proven; the
dynamic engine search does not merge states. All currently supported engine
actions use cost bucket `1`, matching the bounded reference fixture adapter; no
finer cost estimate is claimed. Cycles stop conservatively as unresolved,
without inferred repetition or GHI semantics.

Metrics include authoritative transitions, state clones, admitted/expanded
actions, bound updates, scheduler rebuilds, and root certification state.

## Verification

- `cargo check -p mtg-kernel --lib -j 3`: passed after rebase to current `main`.
- `cargo test -p mtg-kernel --lib mads02b_engine_probe_v1 -j 3`: passed (9 tests), including the reversed owner-ID/path-order regression and complete relevant-frontier traversal.
- `cargo test -p mtg-kernel --lib mads_v1 -j 3`: passed (16 tests), including the PR #6 adversarial differential test.
- `cargo test -p mtg-kernel --lib oracle_suite_v1 -j 3`: passed (4 tests).
- `cargo fmt --all -- --check`, Clippy with `-D warnings`, and `git diff --check`: passed.
- Full workspace and release tests: not run.

This API consumes authoritative engine state. It is not a player observation or
teacher-data surface; those require a separate information-set boundary.
