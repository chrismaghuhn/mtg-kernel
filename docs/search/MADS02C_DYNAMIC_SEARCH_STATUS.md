# MADS-02C Dynamic Engine Search Status

**Implementation status: initial production API implemented; focused oracle verification pending.**

`DynamicEngineSearchV1` owns a path-local engine tree and accepts a borrowed
authoritative `GameState`, its current engine `Decision`, and a deterministic
expansion budget. It clones the root once and clones each admitted parent only
when that action is expanded. `engine::step` runs only at expansion; the
caller's state is not mutated. The root clone is counted even with budget zero,
while no successor clone or transition is created. No state key, transposition table, or hidden
information abstraction is used.

The explicit admitted decision set is `CastSpellOrPass` (including the complete
raw spell, mana ability, land, activated ability, plot, and pass domain), empty
`DeclareAttackers`, and `GameOver`. `Halted`, continuations, and every other
decision fail closed as `UNSUPPORTED_DECISION`. Mana ability choices and cost
targets are enumerated from the engine's authoritative helpers. Unexpanded
actions retain UNKNOWN intervals. Root action coverage is admitted before
search; certification compares every legal root action's lower bound to all
competitors' upper bounds. Results expose certification, exact root value,
anytime action, unresolved budget status, and unsupported status separately.

The dynamic path shares MADS-01's conservative bound backup and
Root-Critical-Support predicates. It rebuilds its deterministic frontier after
each expansion under `FRONTIER_REBUILD_REFERENCE`, using root incumbent lower
and challenger upper support. Cycles are conservatively stopped as unresolved;
no repetition or GHI value is inferred.

Metrics include authoritative transitions, state clones, admitted/expanded
actions, bound updates, scheduler rebuilds, and root certification state.

## Verification

- `cargo check -p mtg-kernel --lib -j 3`: passed.
- `cargo test -p mtg-kernel --lib mads02b_engine_probe_v1 -j 3`: passed (6 tests).
- `cargo test -p mtg-kernel --lib mads_v1 -j 3`: passed (15 tests).
- `cargo check -p mtg-kernel --lib -j 3`: passed.
- `cargo clippy -p mtg-kernel --lib -- -D warnings`: passed.
- Full workspace and release tests: not run.

The API is not a perfect-information policy surface. It consumes the
authoritative engine state and must not be exposed as player observation or
teacher data without a separate information-set boundary.
