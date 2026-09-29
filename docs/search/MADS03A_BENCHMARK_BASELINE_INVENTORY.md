# MADS-03A Benchmark and Baseline Inventory

**Inventory commit:** `5f6eab937c90266df4cfc7aaa120f30380f0bf0b` (`origin/main`, fetched 2026-09-30)
**Branch:** `feat/MADS-03A-postmerge-integration`
**Purpose:** freeze what exists and define a fair future comparison. This document reports no new benchmark run and no measured search performance.

## Inventory method and result

The inventory inspected `mtg-kernel/examples/bench_kernel.rs`, search/evaluation modules, checkpoint loaders and tests, tracked model artifacts, repository-local model/checkpoint extensions, evidence directories, and Pauper catalogs. No checkpoints or shards were downloaded.

### Existing search/evaluation baselines

| Candidate | Status | What exists | Current limitation for MADS comparison |
|---|---|---|---|
| Existing policy, no extra search | IMPLEMENTED; artifact availability split below | `native_checkpoint_runner_v1` evaluates checkpoint policies (`run_native_checkpoint_v1`, `run_native_checkpoint_wide_v1`); checkpoint inference has strict manifest/payload loaders and end-to-end tests. | No trained checkpoint file was found locally that can be pinned and loaded as a benchmark input. A fixed initialization snapshot is not silently promoted to a trained policy. |
| Kernel-native Search Opponent | IMPLEMENTED; test/evaluation path exists, but not a matched MADS baseline | `kernel_native_search_opponent_v1.rs` exposes a deterministic search opponent and authority; rollout code has a search-opponent lane. `run_native_checkpoint_with_search_opponent_eval_v1` in `native_checkpoint_runner_v1.rs` is `#[cfg(test)]`, and the whole calibration runner is test-gated in `lib.rs:71-80`. | Different algorithm/objective and evaluation harness; no frozen common MADS position protocol or measured head-to-head result. It uses engine/redeterminization context and must be compared under identical permitted information. |
| Model-guided search | IMPLEMENTED; callable production runner, conditionally usable | Public `run_checkpoint_shadow_stdio_with_model_guided_search_v1` in `native_checkpoint_shadow_stdio_v1.rs:4939`; it checks `model.search_capable_v1()` and fails closed otherwise. `ModelGuidedSearchCoreV1` is in `model_guided_search_core_v1.rs`; real leaf forward is pinned through the native evaluator path. | Requires a compatible search-capable checkpoint and an adapter that drives the same benchmark starts. The tracked snapshots below were not shown to be compatible search-trained checkpoints. Existing deterministic/replay tests do not constitute a MADS matchup. |
| DynamicEngineSearchV1 | IMPLEMENTED; TESTED; not integrated into a production bot/eval lane | Public API in `dynamic_engine_search_v1.rs`; bounded engine oracle and budget test in the MADS-02B test module. | No non-test call site found outside its own test adapter; privileged `GameState` input; only the explicitly supported raw decision families; no benchmark runner or transposition table. |
| Future MADS, complete construction protocol | NOT_IMPLEMENTED | No complete live construction-node adapter. | Construction ownership, protocol/prefix/cursor and complete physical root actions remain proof gates. |
| Future MADS with exact TT | NOT_IMPLEMENTED | `DecisionStateKeyContractV1` audit and structural capture preflight exist, but no safe reusable live decision key/TT. | Exact future-semantic equivalence and approved encoding/normalization are not proven. |
| Alpha-Beta / Best-First / Proof-Number (PNS, df-pn) / conspiracy or proof-set search | NOT_FOUND as comparable implementation | Literature/baseline inventory is referenced in `docs/search/MADS_SEARCH_PRIOR_ART_MATRIX.md`; no matched kernel implementation was found in the inspected search/eval sources. | Candidates to assess for an apples-to-apples follow-up, not runnable baseline claims. Freeze algorithm, transposition semantics, proof conditions, and engine adapter before comparison. |

### Checkpoints, model artifacts, and loader contracts

Statuses use exactly these meanings: `TRACKED_AND_AVAILABLE` is checked into this HEAD and present; `LOCAL_ONLY_VERIFIED` is present only in the inspected checkout and was actually read/loaded; `REFERENCED_BUT_MISSING` is named by repository records but absent here; `NOT_FOUND` means no matching artifact was present in the inspected repository/workspace; `INCOMPATIBLE` means the artifact exists but its verified model/runtime contract cannot satisfy the lane.

| Artifact | Status | Evidence / compatibility boundary |
|---|---|---|
| `data/common_model_snapshot_v1/{manifest.json,parameters.f32le}` | TRACKED_AND_AVAILABLE | Both files exist and are tracked. `common_model_snapshot_v1.rs` performs strict manifest/payload validation; the workspace test `committed_artifact_loads_transactionally_and_reexports_bit_exact` passed in the interrupted run before the user scope stop. This is the common initial-model snapshot, not evidence of a trained evaluation checkpoint. |
| `data/wide_model_snapshot_w128/{manifest.json,parameters.f32le}` | TRACKED_AND_AVAILABLE | Both tracked files exist (the parameter payload is 11,003,016 bytes). Wide runner fixed-parameter loader/tests exist in `native_policy_value_net_v1.rs` / `native_checkpoint_inference_v1.rs`. It is a fixed model snapshot; the inventory found no verified link making it a trained model-guided search checkpoint. |
| Trained native checkpoint Store (for example K2/S4 generation artifacts referenced by evaluation tests and docs) | REFERENCED_BUT_MISSING from the repository-local artifact inventory | Tests can build/use genuine temporary fixtures and validate V3/V4 Store/manifest/checkpoint contracts; repository docs also reference external machine-specific D: evidence. No standalone trained checkpoint payload or Store tree was found under this checkout/evidence directory. A passing temporary-fixture load test does not make a benchmark checkpoint available. |
| `.safetensors`, `.pt`, `.pth`, `.ckpt`, `.mtgckpt` files in repository | NOT_FOUND | Repository/workspace inventory found zero files with these extensions outside the shared `target` cache. Two tracked `.f32le` snapshots above are the exception for model payloads, not standalone trained checkpoints. |
| Checkpoint loader/format contracts | TRACKED_AND_AVAILABLE | Python checkpoint I/O in `python/mtg_kernel_rl/checkpoint*.py`; Rust inference and runners in `native_checkpoint_inference_v1.rs`, `native_checkpoint_runner_v1.rs`; native training Store/manifest V3/V4 contracts in `native_training_store_checkpoint_v3.rs` and `...v4.rs`. Loading is identity/manifest/payload gated; format presence alone does not prove an external checkpoint can load. |
| Search/evaluation logs | TRACKED_AND_AVAILABLE as historical records; not new MADS measurements | Tracked `docs/reports/tensorize_cost_v1/goldens/search-matches-replay-v1.json` and historical report/evidence trees record other contracts/experiments. They do not benchmark MADS and were not reclassified as current performance evidence. |
| MADS teacher shards | NOT_FOUND / NOT_IMPLEMENTED | No MADS shard writer, shard manifest, MADS-labeled dataset, or teacher release path exists. Existing training Stores and unrelated trajectory/export formats are not MADS shards. |

### Benchmark tools

`mtg-kernel/examples/bench_kernel.rs` is a performance-only benchmark tool (source header and command dispatcher at lines 1-121). It includes:

- Snapshot scaling, engine-step throughput, self-play threading, legal-action costs and allocation counts (`section1_snapshot_scaling` through `section5_alloc_profile`, lines 1113-1565).
- JSON lanes `--ceiling-json-v1`, `--three-lane-ceiling-json-v1`, `--fast-actor-ceiling-json-v2`, and `--matched-uniform-runtime-json-v2`; some are restricted to Burn/Rally or actor-count throughput and encode their own host/CPU/topology contracts.

These are useful existing engine/runtime measurement utilities. None is a ready common-position MADS-versus-policy-versus-search matchup, none measures MADS certificate cost against competing searchers, and their historical values are not reused here. No benchmark was executed for MADS-03A.

No reproducible head-to-head MADS position corpus was found. Synthetic Oracle fixtures are correctness fixtures, not fair Pauper match positions. Existing deterministic native replay goldens are owned by other protocols and do not establish a cross-searcher position set.

## Pauper deck availability

The canonical pool in `data/pauper_pool_v1.json` contains nine 60-card mainboards. `data/pauper_support_v1.json` marks all 540 mainboard copies as full support and no partial/no-effect copies. `runtime_decks.rs` build-time-generates the runtime catalog from the checked-in deck/card catalogs, and `generated_runtime_catalog_is_exact_and_fully_supported` verifies all nine are materialized and pass `preflight_fully_supported_deck`.

| Deck ID | Engine executable | Existing throughput lane | MADS real-engine evidence |
|---|---|---|---|
| Wildfire | YES | No deck selector confirmed in listed benchmark CLIs | No |
| Rally | YES | YES (several benchmark CLI usage contracts) | No |
| Affinity | YES | No | No |
| Elves | YES | No | No |
| Spy | YES | No | No |
| Burn | YES | YES; concrete reproducible decision probe | One `CastSpellOrPass` Main1 probe only; not a complete game/benchmark lane |
| Terror | YES | No | No |
| CawGates | YES | No | No |
| Faeries | YES | No | No |

"Engine executable" here means admitted by the current full-card runtime-deck catalog/preflight. It does not prove searcher support for every decision those decks can reach. For this MADS comparison protocol, **no full-game deck lane is yet certified**; Burn is the only concrete decision-probe fixture. The bounded two-Mountain oracle graph is synthetic engine stress, not a Pauper matchup.

## Frozen paired-comparison protocol proposal

Protocol is suitable for a future evaluation harness once participants can consume the same admitted decisions. This is a specification only: `BENCHMARK_PROTOCOL_READY = YES`; no run is `MEASURED`.

1. **Position corpus:** publish immutable position IDs built by the authoritative engine from the same catalog/deck and engine commit. Store complete starting `GameState` for trusted offline evaluator use, plus the exact decision frame. For any player-facing lane, separately store the permitted observation and assert no hidden-state fields cross the agent boundary. Freeze the corpus digest before results.
2. **Paired seeds and seats:** derive deterministic seed assignments per `(position_id, matchup_id, repetition)` from a versioned seed rule and use the same assignment across participants. Construct both play/draw pair members independently through the authoritative engine initialization path, selecting the designated starting player/seat roles at initialization while holding the remaining seed namespace fixed. Never create the paired member by changing `starting_player` in an already-built `GameState`. Publish both initialization inputs, seeds, and seat mapping.
3. **Versions and configuration:** pin git commit, engine/card/runtime-deck catalog hashes, deck IDs/hashes, model manifest and payload digests, searcher version, feature set, Rust/toolchain/target, and evaluator policy. No participant-specific rule/config drift.
4. **Budget:** pre-register equal per-decision wall-clock ceilings and equal aggregate CPU-time ceilings, plus a declared safety ceiling on authoritative transitions/expanded actions. Report all three independently; node/transition counts alone are not equal compute. Freeze warm-up, initialization, and model-inference treatment before the run.
5. **Outcome categories:** retain every assigned position. Report `SUPPORTED_COMPLETED`, `UNSUPPORTED_DECISION`, `TIMEOUT`, `ERROR`, `UNKNOWN/UNRESOLVED`, and certified decision separately. Do not turn unsupported/timeout/unknown into a win/loss label or silently drop them. Predefine the primary paired statistic and the treatment of incomplete pairs.
6. **Authoritative stepping:** apply every selected action through `engine::step`; count accepted transitions. Give each participant an independent state clone from the identical root. Verify the original root remains unchanged and record clone count.
7. **Resources/timing:** collect monotonic wall time and process/thread CPU time separately. Separate process/model initialization, model inference, search, certificate verification and authoritative transition costs. Record per-decision and per-game values and host identity; record peak process RSS with the measurement source/precision stated. Do not compare warm start on one lane with cold start on another.
8. **Search-specific outcomes:** record search status, root action domain, certified root actions, exact root value if resolved, UNKNOWN fraction, certification latency/cost, expansions/transitions, state clones, bound updates and scheduler rebuilds. Record TT probes/hits only for participants that actually implement a TT, alongside the exact key contract/version. Heuristic priors/values remain separate from certified bounds.
9. **Reproducibility:** preserve raw machine-readable results using the existing approved evaluation record formats if they meet the needs; do not add a persistent schema until format review shows a gap. Repeat a fixed subset and compare deterministic non-timing outputs byte-for-byte.
10. **Baseline admission:** require an executable end-to-end smoke on the frozen position corpus for every participant before any matchup. A compatible trained checkpoint and model-guided loader smoke are prerequisites for the trained-policy/model-guided lanes. Classical baselines need reviewed adapters, proof semantics and common-position smoke. MADS needs full supported decision/construction mapping; the exact-TT variant separately needs a closed key/TT proof gate.

### Required later instrumentation

The current Dynamic MADS API exposes authoritative transitions, state clones, admitted/expanded actions, bound updates, scheduler rebuilds and root certification (`DynamicSearchMetricsV1`). It does not report CPU time, wall time, peak RSS, model inference, initialization, certificate-verification time or TT hits (there is no TT). Existing benchmark tools report engine/throughput-specific timings and resource samples, not all these MADS fields. Add reviewed instrumentation in a later benchmark increment, preserving the separation between search, inference, setup and certification costs.

A search using fewer engine transitions is not automatically more efficient than one using more. Report cost in measured wall/CPU time plus resource and outcome metrics under the fixed protocol.

## Unmet prerequisites for empirical performance

- No compatible, available trained-policy checkpoint was verified in this workspace.
- No MADS full-game Pauper position corpus or common evaluator exists.
- DynamicEngineSearchV1 has no production evaluation dispatch and supports only a narrow decision subset.
- Complete live construction/action mapping is absent.
- Exact live DecisionStateKey and safe transposition table are not proven/implemented.
- Fair hidden-information teacher labels are not proven; privileged GameState probes cannot be promoted to player-facing teacher data.
- Existing kernel/model-guided and candidate classical methods have not all been adapted to the exact same roots, information boundary and budget.
- CPU, wall, initialization, inference, certification and peak-memory costs are not yet captured consistently across participants.

## Benchmark gates

```text
BENCHMARK_PROTOCOL_READY = YES
PERFORMANCE_BASELINE_MEASURED = NO
EXACT_TT_GATE = OPEN
FAIR_TEACHER_GATE = OPEN
SHARD_PRODUCTION_AUTHORIZED = NO
```

An open proof gate cannot be replaced by a favorable timing. The next recommended increment is to close the reproducible replay failure and Clippy follow-up outside MADS-03A, then prepare a bounded common-position runner/smoke with pinned available policies. Schedule an optimized build and qualified performance run on the designated benchmark hardware as a separate gate.
