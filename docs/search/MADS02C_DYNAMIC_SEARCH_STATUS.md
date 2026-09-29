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

The dynamic path shares MADS-01's conservative bound backup,
Root-Critical-Support predicate, and frontier sort-key implementation. It
collects and deduplicates the full current support task set, merges role masks
and root-action support, then sorts by role rank, descending bound width, root
distance, estimated cost bucket, semantic action path, and stable action or
construction order under `FRONTIER_REBUILD_REFERENCE`. The bounded engine
fixture comparison checks the selected task and every sort-key field after
each expansion. All currently supported engine actions use cost bucket `1`,
matching the reference fixture adapter; no unsupported cost estimate is
claimed. Cycles stop conservatively as unresolved, without inferred repetition
or GHI semantics.

Metrics include authoritative transitions, state clones, admitted/expanded
actions, bound updates, scheduler rebuilds, and root certification state.

## Verification

- `cargo check -p mtg-kernel --lib -j 3`: passed after rebase to current `main`.
- `cargo test -p mtg-kernel --lib mads02b_engine_probe_v1 -j 3`: passed (8 tests), including a per-expansion scheduler parity check and incomplete Burn decision rejection.
- `cargo test -p mtg-kernel --lib mads_v1 -j 3`: passed (16 tests), including the PR #6 adversarial differential test.
- `cargo test -p mtg-kernel --lib oracle_suite_v1 -j 3`: passed (4 tests).
- `cargo fmt --all -- --check`, Clippy with `-D warnings`, and `git diff --check`: passed.
- Full workspace and release tests: not run.

This API consumes authoritative engine state. It is not a player observation or
teacher-data surface; those require a separate information-set boundary.
