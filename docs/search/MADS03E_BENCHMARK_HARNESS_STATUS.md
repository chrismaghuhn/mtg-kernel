# MADS-03E Benchmark Harness Status

**Base:** `0d7703c3958db0f90d657b45d31db328b775febd` (`origin/main` at task start)
**Branch:** `feat/MADS-03E-benchmark-harness`
**Harness:** `mtg-kernel/examples/bench_mads_synthetic_v1.rs`

## Scope and baseline availability

No compatible trained checkpoint or common full-game MADS/agent evaluation lane is present in this workspace. The MADS live-key gate is open; productive TT reuse remains disabled. Existing model-guided or kernel-native search paths do not consume this synthetic fixture graph under the same action/information boundary, so they are not presented as comparison partners.

The harness compares the existing `MadsGraphV1` algorithm on two Oracle-equivalent encodings of one small synthetic game-decision tree: unrolled path-local child IDs and a shared-child fixture-ID DAG. `OracleSuiteV1` independently solves both graphs and the harness checks that root values, optimal root actions and complete root action domains agree before measurement. MADS bounds must contain the Oracle value in every measured run. Fixture identity is explicit; these fixture IDs are not real Engine state keys and this is not a Pauper/bot evaluation. `DynamicEngineSearchV1` is not included: it consumes privileged `GameState`/`Decision` frames and has a narrow supported-decision boundary, while this harness accepts complete synthetic Oracle fixtures. No common authoritative adapter maps those inputs without changing the comparison boundary. Kernel-native and model-guided searchers are also not included because no compatible trained checkpoint and same-position decision/information adapter are available.

The tool writes a transient human-readable report to stdout only. It introduces no persistent benchmark artifact schema. It supports bounded `--iterations` and `--budget` inputs and performs ten warmups before recording averages.

## Measured run

Command:

```text
cargo run --locked -p mtg-kernel --example bench_mads_synthetic_v1 -j 3 -- --iterations 100 --budget 64
```

This final review-correction run used the clean harness commit below. Both Oracle-equivalent fixtures were checked before the run for equal exact root values, optimal root-action sets, and complete ordered root-action domains.

```text
OS / architecture: Windows / x86_64
CPU identifier: AMD64 Family 25 Model 33 Stepping 2, AuthenticAMD
Build profile: Debug
Git HEAD: 747bd5a64524dc6013b181a935ba5b2d94c8bc3a
Build clean: true
Tracked-tree SHA-256: 2757ca831c3d149a0296ae48707d8b19536011a95244865f58d19e2c10fc37a2
Fixture identity: mads03e-oracle-paired-v1-seed-0000000003e00001
Seed: 0x0000000003e00001 (identity only; no RNG consumed)
Warmups / measured iterations / expansion budget: 10 / 100 / 64
```

Observed report values:

| Variant | Oracle nodes / edges | Oracle root | Oracle solve wall (single invocation) | MADS init wall avg | MADS search wall avg | Search status / value | Certified actions | Fixture interner lookups / hits |
|---|---:|---:|---:|---:|---:|---|---|---:|
| `PATH_LOCAL_TREE_V1` | 5 / 10 | DRAW (0) | 173,400 ns | 50,159 ns | 88,494 ns | Certified / exact DRAW | `[left]` | 7 / 0 |
| `FIXTURE_ID_DAG_V1` | 4 / 8 | DRAW (0) | 26,800 ns | 35,555 ns | 85,372 ns | Certified / exact DRAW | `[left, right]` | 7 / 2 |

For every warmup and measured run, the harness requires: MADS root bounds contain the Oracle value; the ordered root-action domain equals the complete Oracle domain; every certified action is Oracle-optimal; a `Certified` status carries a nonempty certificate and an Oracle-optimal chosen certified action; and an unresolved result carries no chosen/certified action. Both variants reported `unknown_runs=0/100`, 7 expanded action slots, 8 scheduler rebuilds, 6 bound updates, 0 authoritative Engine transitions, and 0 state clones. The fixture DAG created 6 search nodes versus 8 for the path-local encoding; two duplicate fixture nodes were avoided.

CPU time was unavailable from the harness, peak RSS was unavailable, and certification cost is included in search wall time rather than separately instrumented. These small synthetic timings are noisy harness observations only. The encodings use different fixture graph sizes; this does not establish a comparable search-performance baseline or a Magic speedup.

## Verification

- `cargo fmt --all -- --check` — passed.
- `cargo run --locked -p mtg-kernel --example bench_mads_synthetic_v1 -j 3 -- --iterations 100 --budget 64` — completed on the clean harness commit; all per-run Oracle checks passed.
- `cargo clippy --locked -p mtg-kernel --example bench_mads_synthetic_v1 -j 3 -- -D warnings` — passed on the final implementation.
- No full workspace, Release, Thin-LTO or CUDA suite was run. The optimized benchmark build remains deferred to designated benchmark hardware.
## Gate decision

```text
BENCHMARK_HARNESS = IMPLEMENTED
COMPARABLE_PERFORMANCE = NOT_MEASURED
OPTIMIZED_BENCHMARK = NOT_RUN_DEFERRED
EXACT_TT_GATE = OPEN
FAIR_TEACHER_GATE = OPEN
```

This harness can validate correctness and collect counters on a small deterministic Oracle graph. It cannot substitute for identical supported Magic positions, trained-policy checkpoint availability, Engine/Searcher adapters, or a qualified optimized benchmark run. No Kaggle throughput or empirical search-speed claim is made.

## Next prerequisites

1. Keep MADS-03C key and MADS-03D productive TT gates closed until their proof obligations are satisfied.
2. Establish a common supported Engine decision set and authoritative starting-position corpus.
3. Locate and verify compatible trained checkpoints before admitting trained-policy/model-guided variants.
4. Add reviewed instrumentation for CPU time, peak RSS and separately attributable certification costs when a comparative lane exists.
5. Run an optimized build and preregistered comparison on the designated benchmark hardware in a separate increment.
