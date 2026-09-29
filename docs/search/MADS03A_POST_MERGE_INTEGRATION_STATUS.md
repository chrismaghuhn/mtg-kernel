# MADS-03A Post-Merge Integration Status

**Inventory HEAD:** `5f6eab937c90266df4cfc7aaa120f30380f0bf0b` (`origin/main`, fetched 2026-09-30)
**Branch:** `feat/MADS-03A-postmerge-integration`
**Toolchain/host:** Rust 1.94.1; Ubuntu 24.04 under WSL2; Linux x86_64; AMD Ryzen 7 5800X (16 logical CPUs); NVIDIA RTX 4060 Ti visible to WSL (8 GiB), but no CUDA feature lane was built or tested.
**Scope:** validation, inventory, and documentation only. No production implementation changes.

## Baseline and resources

The user-supplied initial commit `5f6eab937c90266df4cfc7aaa120f30380f0bf0b` still equals fetched `origin/main`; no intervening main change was found. The original checkout was left on `feat/MADS-02-engine-integration`; this work uses a separate worktree based on that fetched commit.

Before Cargo work the host reported 15 GiB RAM, 4 GiB swap, 893 GiB free on the filesystem, and a 17 GiB existing target cache. Builds used `CARGO_TARGET_DIR=/home/chris/src/mtg-kernel/target` and were serialized by `flock /tmp/mtg-kernel-cargo-build.lock`; no `cargo clean` was run. Peak RSS for the workspace test process was not reliably sampled; observed resident memory was approximately 2.2 GiB while running. Build profile was Debug for executed Cargo checks/tests. Repository Release profile is Thin LTO with one codegen unit (`Cargo.toml:18-20`).

## Phase A: post-merge status matrix

| Area | Status | Current evidence and boundary |
|---|---|---|
| OracleSuiteV1 and MADS-01 | IMPLEMENTED; TESTED; CERTIFIED_ON_FIXTURE | `mtg-kernel/src/oracle_suite_v1.rs`, `mtg-kernel/src/mads_v1.rs`; focused run: 4 Oracle tests and 16 MADS tests passed. Values/bounds and certificates cover test fixtures only. |
| Real Engine-Decision probes | IMPLEMENTED (test adapter); TESTED | `mtg-kernel/src/mads02b_engine_probe_v1.rs`; 9 tests passed. Burn `CastSpellOrPass` domain was independently enumerated and cloned transitions replayed. This is a bounded probe, not universal candidate completeness, a complete physical action, or a fair-information teacher. |
| DynamicEngineSearchV1 | IMPLEMENTED; TESTED; NOT_PROVEN generally | Public module and entry points in `mtg-kernel/src/dynamic_engine_search_v1.rs`; its oracle/budget probe passed within the 9-test MADS-02B module. No non-test production caller was found in `mtg-kernel/src` or `mtg-kernel/examples`. It is not currently invoked by a bot/evaluation lane. |
| Synthetic adversarial Oracle regression | IMPLEMENTED; TESTED; CERTIFIED_ON_FIXTURE | `mads_v1::tests::adversarial_control_differential_checks_every_interval_and_root_certificate`; differential coverage is synthetic, not arbitrary game graphs. |
| MADS-02F-C Oracle stress | IMPLEMENTED; TESTED; CERTIFIED_ON_FIXTURE | `mtg-kernel/src/mads02f_c_oracle_stress_v1.rs`; current output confirmed 3,615 exhaustive shallow fixtures, 128 deterministic deeper samples from `0x02f0cafe00000001`, and 22,356 + 1,423 = 23,779 stepwise expansion checks; zero bound/certification violations in this fixture domain. |
| DecisionStateKeyContractV1 | IMPLEMENTED; TESTED; NOT_PROVEN | `mtg-kernel/src/mads_decision_state_key_v1.rs`; three contract unit tests pass for structural snapshots and sensitivity. The audit at `docs/search/MADS_DECISION_STATE_KEY_V1.md` keeps real semantic equivalence and normalization unproven. |
| Trusted Live-Key Preflight | IMPLEMENTED; TESTED; NOT_PROVEN as reusable TT key | `rl_session.rs` tests `live_key_preflight_rejects_stale_incomplete_and_unsupported_capture_contexts` and `exact_key_contract_accepts_live_engine_surface_and_session_values` both passed. Preflight admits a complete supported capture context; it does not prove future-semantic equivalence or authorize TT reuse. |
| Structured-Decision-Audit | IMPLEMENTED; TESTED; PARTIALLY_PROVEN | `mtg-kernel/src/mads02e_structured_decision_audit_v1.rs`; five audit tests pass for selected continuation/actor-switch and raw-domain cases. It records explicit support boundaries; it is not a complete decision/protocol proof for all engine decisions. |

### DynamicEngineSearchV1 integration details

`pub mod dynamic_engine_search_v1` is exported from `mtg-kernel/src/lib.rs:60`. Public constructors/results are `DynamicEngineSearchV1::new(root_state, root_decision)` and `run_v1(compute_budget)` (`dynamic_engine_search_v1.rs:267,292`). Status/error/result/metrics types are public. The search owns cloned authoritative `GameState`s, revalidates the root through `engine::advance_until_decision`, and calls `engine::step` on expansion; it therefore requires privileged engine state, not a player observation. Metrics count transitions, clones, admitted and expanded actions, bound updates, scheduler rebuilds, and root certification (`:94-102, 267-341`).

The advertised supported kinds are `CastSpellOrPass`, empty `DeclareAttackers`, and child `GameOver` (`:174-176`). Unsupported decisions return `UnsupportedDecision`/`UNSUPPORTED_DECISION`, including `Halted`, nonempty/other combat protocols, staged continuations and any decision whose complete raw domain is not admitted (`:52-79, 600-750`). Cycles end unresolved. No state-key merge/TT is used. The module has test coverage through MADS-02B, but source search found no production bot/evaluation dispatch to it. This is a callable public API, not an integrated evaluated opponent.

`FRONTIER_REBUILD_REFERENCE` in MADS-01 and `FRONTIER_REBUILD_DYNAMIC_PATH_V1` in Dynamic Engine Search are separately versioned. The former ties owners by fixture node ID; the latter uses semantic action path. Tests explicitly prove those can differ (`mads02b_engine_probe_v1`). The synchronized dynamic-frontier test compares critical task sets after synchronized expansions through frontier exhaustion; it does not prove selected-order parity, particularly for real engine DAGs. No selection-order parity is claimed.

### Focused Debug regression commands

All commands below ran serially under the common lock with `--locked -j 3` (the first command caused a one-time Debug test-binary compile, about 70 seconds). Results:

| Command filter | Result |
|---|---:|
| `cargo test --locked -p mtg-kernel --lib oracle_suite_v1 -j 3 -- --nocapture` | 4 passed |
| `cargo test --locked -p mtg-kernel --lib mads_v1 -j 3 -- --nocapture` | 16 passed |
| `cargo test --locked -p mtg-kernel --lib mads02b_engine_probe_v1 -j 3 -- --nocapture` | 9 passed |
| `cargo test --locked -p mtg-kernel --lib mads02f_c_oracle_stress_v1 -j 3 -- --nocapture` | 7 passed |
| `cargo test --locked -p mtg-kernel --lib mads02e_structured_decision_audit_v1 -j 3 -- --nocapture` | 5 passed |
| `cargo test --locked -p mtg-kernel --lib mads_decision_state_key_v1 -j 3 -- --nocapture` | 3 passed |
| `cargo test --locked -p mtg-kernel --lib live_key_preflight_rejects_stale_incomplete_and_unsupported_capture_contexts -j 3 -- --nocapture` | 1 passed |
| `cargo test --locked -p mtg-kernel --lib exact_key_contract_accepts_live_engine_surface_and_session_values -j 3 -- --nocapture` | 1 passed |

Total focused MADS/key/audit evidence: **46 passed, 0 failed**. Test output confirmed the above enumerated fixture counts. Passing tests establish only the tested contracts, not universal correctness.

### Workspace-level checks and open Debug failures

- `cargo fmt --all -- --check`: PASS.
- `cargo check --locked --workspace --all-targets -j 3`: PASS.
- `cargo clippy --locked --workspace --all-targets -j 3 -- -D warnings`: FAIL, two `clippy::type_complexity` errors in test-only debug accessors in `mtg-kernel/src/dynamic_engine_search_v1.rs:189,220`. Per scope, no search code was changed.
- `cargo test --locked --workspace --all-targets -j 3`: started 1,762 library tests. During the long full-suite run, the following two tests reported FAIL in `native_checkpoint_shadow_stdio_v1`; no completed aggregate summary was available before the user changed scope and the run was interrupted after roughly 50 minutes:
  - `model_guided_search_replay_is_bit_identical_apart_from_wall_time_v1`
  - `the_chosen_action_is_independent_of_the_measured_latency_v1`

  The retained failure output contained only the Rust harness lines ending in `...model_guided_search_replay_is_bit_identical_apart_from_wall_time_v1 ... FAILED` and `...the_chosen_action_is_independent_of_the_measured_latency_v1 ... FAILED`; no panic/assertion message was retained. Each test was then run individually under the common lock and passed (46.06 s and 60.24 s respectively). This does not erase the full-suite failure: both remain **OPEN / INCONCLUSIVE**, with a possible interaction with parallel/full-suite load not proven. Per the revised scope, no further failure-detail rerun is planned here. Full Debug suite status: `INCONCLUSIVE_INTERRUPTED_AFTER_FAILURES`; not a PASS. No raw workspace-test logfile was redirected to disk; the retained failure names, focused counts, tool outputs and interruption status are recorded in this report.
- Full Release/Thin-LTO suite: `NOT_RUN_DEFERRED_BY_SCOPE` per the user's MADS-03A scope change. A targeted optimized build on benchmark hardware is a separate gate.
- `scripts/verify_all.sh` was inspected and not run: it additionally pins `uv 0.11.29`, syncs Python dependencies, runs Release Clippy/tests, builds `kernel_rl_env`, verifies Pauper manifests and runs Python tests. It is not an appropriate blind shortcut here.
- CUDA feature/backend tests: NOT RUN. Visible RTX 4060 Ti under WSL does not mean the opt-in CUDA Cargo features/qualified training lane were exercised. No prior hardware/training evidence is relabeled.

## Reproducibility and contract limits

The MADS-02F-C regression confirmed the documented seed and exact counts above. Oracle generation is exhaustive only for its defined bounded shallow family; the 128 deeper DAGs are deterministic samples, not exhaustive coverage. `UNKNOWN` at budget zero, interval envelopes, root action certification, exact root-value resolution, reverse-parent propagation and fixture-local ID reuse are tested in the MADS/Oracle test modules. Those graph-ID reuses are not real engine transpositions.

`FRONTIER_REBUILD_REFERENCE` and `FRONTIER_REBUILD_DYNAMIC_PATH_V1` have different owner tie-breaking contracts. The dynamic test compares task sets at synchronized expansions, not actual chosen scheduler order. Unsupported decisions fail closed; no unsupported outcome is assigned a minimax value.

## Scientific correctness matrix

| Area | Status | Evidence | Open proof obligation |
|---|---|---|---|
| Synthetic minimax bounds | CERTIFIED_ON_FIXTURE | 3,615 exhaustive + 128 sampled synthetic fixtures; per-step bounds | Extend beyond admitted fixture families; no general proof for arbitrary engine graph adapters |
| Root certificates | CERTIFIED_ON_FIXTURE | competing/tied/last-best and stress regression tests | Map all supported live root actions to complete physical decisions |
| Dynamic Engine Search | TESTED / NOT_PROVEN | public API and bounded engine-oracle probe | Production bot/eval wiring, broader complete domains and engine-state proof |
| Scheduler contract | PARTIALLY_PROVEN | distinct version strings, tests for ordering and synchronized task sets | Do not claim choice-order parity; real multiple-parent engine DAG behavior remains unproven |
| Engine-decision provenance | PARTIALLY_PROVEN | Burn raw `CastSpellOrPass` probe and structured audit | Universal candidate completeness and full-physical-action mapping |
| Typed construction | NOT_PROVEN | audit explains actors/protocol boundaries; MADS synthetic construction nodes | Live continuation adapter with exact owner/protocol/prefix/cursor proof |
| Exact State-Key | NOT_PROVEN | conservative identity audit; key contract tests | Field-complete canonical encoding and paired-state/bisimulation evidence |
| Transposition table | NOT_IMPLEMENTED | MADS fixture interner only; Dynamic Engine Search has no TT | Exact reusable live key and path/GHI semantics |
| Reverse-DAG propagation | CERTIFIED_ON_FIXTURE | shared-child/reverse-parent stress tests | Real engine DAG integration; no live TT graph exists |
| POR | NOT_IMPLEMENTED | no activation in current MADS paths | Engine-certified commutation certificate set and soundness evidence |
| Hidden-information fairness | NOT_PROVEN | existing probe is engine-oracle-only and GameState is omniscient | Observation-set teacher contract and leakage audit |
| Search baselines | PARTIALLY_PROVEN inventory only | existing kernel/model-guided modules and harness inventory | Fair common-position/budget adapters; classical baseline implementations |
| Teacher shards | NOT_IMPLEMENTED | no MADS shard writer; no MADS teacher release path | Fair labels, exact contracts, and separate authorization |

## Gates

```text
POST_MERGE_INTEGRATION_GATE = INCONCLUSIVE
DYNAMIC_MADS_CURRENT_SCOPE = TESTED / NOT_PROVEN
MADS_V0_4_COMPLETE = NO
BENCHMARK_PROTOCOL_READY = YES
PERFORMANCE_BASELINE_MEASURED = NO
EXACT_TT_GATE = OPEN
FAIR_TEACHER_GATE = OPEN
SHARD_PRODUCTION_AUTHORIZED = NO
```

The integration gate is inconclusive because focused MADS tests pass, but workspace Clippy fails and the complete Debug workspace suite was interrupted after two failures (both pass in isolation). The V0.4 gate remains open because typed live construction, an approved DecisionStateKey and exact TT are absent. No performance claim is made.

## Next increment

First isolate the two full-suite replay failures under controlled serial test execution and record the exact panic/assertion output; do not change the production search implementation in MADS-03A. Separately address Clippy's test-helper type complexity if approved. Then run the designated optimized benchmark build and establish a common supported-position/budget adapter before measuring any search comparison.
