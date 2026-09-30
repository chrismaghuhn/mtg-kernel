//! Bounded correctness/instrumentation microbenchmark for Oracle-equivalent
//! synthetic MADS fixture graphs. This is not a Pauper/game-performance lane.
use mtg_kernel::mads_v1::{BoundIntervalV1, MadsGraphV1};
use mtg_kernel::oracle_suite_v1::{
    FixtureEdgeV1, FixtureNodeId, FixtureOutcomeV1, FixturePlayerV1, OracleFixtureV1, OracleNodeV1,
};
use std::time::{Duration, Instant};

const FIXTURE_SEED_V1: u64 = 0x03e0_0001; // identity only; no RNG is consumed
const WARMUPS_V1: usize = 10;
const DEFAULT_ITERATIONS_V1: usize = 100;
const DEFAULT_BUDGET_V1: usize = 64;
const MAX_ITERATIONS_V1: usize = 10_000;
const MAX_BUDGET_V1: usize = 500_000;

#[derive(Clone, Copy)]
enum FixtureEncodingV1 {
    PathLocalTree,
    SharedFixtureDag,
}

impl FixtureEncodingV1 {
    fn as_str(self) -> &'static str {
        match self {
            Self::PathLocalTree => "PATH_LOCAL_TREE_V1",
            Self::SharedFixtureDag => "FIXTURE_ID_DAG_V1",
        }
    }
}

fn edge(order: u32, stable_id: &str, child: u32) -> FixtureEdgeV1 {
    FixtureEdgeV1 {
        stable_id: stable_id.to_owned(),
        order,
        child: FixtureNodeId(child),
        estimated_cost_bucket: 0,
    }
}

fn terminal(id: u32, outcome: FixtureOutcomeV1) -> OracleNodeV1 {
    OracleNodeV1::Terminal {
        id: FixtureNodeId(id),
        outcome,
    }
}

fn fixture(encoding: FixtureEncodingV1) -> OracleFixtureV1 {
    let root_actions = vec![edge(0, "left", 1), edge(1, "right", 2)];
    let mut nodes = vec![
        OracleNodeV1::GameDecision {
            id: FixtureNodeId(1),
            actor: FixturePlayerV1::P1,
            actions: vec![edge(0, "continue-left", 3), edge(1, "draw-left", 4)],
        },
        OracleNodeV1::GameDecision {
            id: FixtureNodeId(2),
            actor: FixturePlayerV1::P1,
            actions: vec![
                edge(
                    0,
                    "continue-right",
                    match encoding {
                        FixtureEncodingV1::PathLocalTree => 7,
                        FixtureEncodingV1::SharedFixtureDag => 3,
                    },
                ),
                edge(
                    1,
                    "draw-right",
                    match encoding {
                        FixtureEncodingV1::PathLocalTree => 8,
                        FixtureEncodingV1::SharedFixtureDag => 4,
                    },
                ),
            ],
        },
        OracleNodeV1::GameDecision {
            id: FixtureNodeId(3),
            actor: FixturePlayerV1::P0,
            actions: vec![edge(0, "win-left", 5), edge(1, "loss-left", 6)],
        },
        terminal(4, FixtureOutcomeV1::Draw),
        terminal(5, FixtureOutcomeV1::Win),
        terminal(6, FixtureOutcomeV1::Loss),
    ];
    if matches!(encoding, FixtureEncodingV1::PathLocalTree) {
        nodes.extend([
            OracleNodeV1::GameDecision {
                id: FixtureNodeId(7),
                actor: FixturePlayerV1::P0,
                actions: vec![edge(0, "win-right", 9), edge(1, "loss-right", 10)],
            },
            terminal(8, FixtureOutcomeV1::Draw),
            terminal(9, FixtureOutcomeV1::Win),
            terminal(10, FixtureOutcomeV1::Loss),
        ]);
    }
    let mut all = vec![OracleNodeV1::GameDecision {
        id: FixtureNodeId(0),
        actor: FixturePlayerV1::P0,
        actions: root_actions,
    }];
    all.append(&mut nodes);
    OracleFixtureV1 {
        fixture_id: format!("mads03e-oracle-paired-v1-seed-{FIXTURE_SEED_V1:016x}"),
        root: FixtureNodeId(0),
        root_player: FixturePlayerV1::P0,
        nodes: all,
    }
}

#[derive(Clone, Copy)]
struct ConfigV1 {
    iterations: usize,
    budget: usize,
}

fn parse_args() -> Result<ConfigV1, String> {
    let mut config = ConfigV1 {
        iterations: DEFAULT_ITERATIONS_V1,
        budget: DEFAULT_BUDGET_V1,
    };
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let value = args
            .next()
            .ok_or_else(|| format!("missing value for {arg}"))?;
        match arg.as_str() {
            "--iterations" => {
                config.iterations = value
                    .parse()
                    .map_err(|_| "invalid --iterations".to_owned())?;
                if config.iterations == 0 || config.iterations > MAX_ITERATIONS_V1 {
                    return Err(format!("--iterations must be 1..={MAX_ITERATIONS_V1}"));
                }
            }
            "--budget" => {
                config.budget = value.parse().map_err(|_| "invalid --budget".to_owned())?;
                if config.budget > MAX_BUDGET_V1 {
                    return Err(format!("--budget must be <= {MAX_BUDGET_V1}"));
                }
            }
            _ => return Err(format!("unknown argument: {arg}")),
        }
    }
    Ok(config)
}

#[derive(Default)]
struct MeasurementsV1 {
    initialization_wall: Duration,
    search_wall: Duration,
    unknown_runs: usize,
    last_result: Option<mtg_kernel::mads_v1::SearchResultV1>,
}

fn one_run(
    fixture: &OracleFixtureV1,
    expansion_budget: usize,
) -> Result<(Duration, Duration, mtg_kernel::mads_v1::SearchResultV1), String> {
    let initialization = Instant::now();
    let mut search = MadsGraphV1::new(fixture).map_err(|error| error.to_string())?;
    let initialization_wall = initialization.elapsed();
    let run = Instant::now();
    // run_v1 stops at certification or budget exhaustion. It never substitutes
    // an anytime action or heuristic estimate for a certified result.
    let result = search
        .run_v1(expansion_budget)
        .map_err(|error| error.to_string())?;
    let search_wall = run.elapsed();
    Ok((initialization_wall, search_wall, result))
}

fn validate_result_against_oracle(
    stage: &str,
    result: &mtg_kernel::mads_v1::SearchResultV1,
    oracle: &mtg_kernel::oracle_suite_v1::OracleResultV1,
) -> Result<(), String> {
    if result.root_bounds.lower > oracle.root_value || result.root_bounds.upper < oracle.root_value
    {
        return Err(format!(
            "{stage}: MADS root bounds exclude exact Oracle value"
        ));
    }
    let observed_root_actions = result
        .root_actions
        .iter()
        .map(|action| action.stable_id.clone())
        .collect::<Vec<_>>();
    if observed_root_actions != oracle.complete_legal_root_actions {
        return Err(format!(
            "{stage}: MADS root-action domain/order differs from Oracle"
        ));
    }
    let mut seen = std::collections::HashSet::new();
    for action in &result.certified_optimal_actions {
        if !seen.insert(action.as_str()) {
            return Err(format!("{stage}: duplicate certified root action"));
        }
        if !oracle.optimal_root_actions.contains(action) {
            return Err(format!(
                "{stage}: certified root action {action} is not Oracle-optimal"
            ));
        }
    }
    match result.status {
        mtg_kernel::mads_v1::CertificationStatusV1::Certified => {
            let chosen = result
                .chosen_action
                .as_deref()
                .ok_or_else(|| format!("{stage}: Certified status has no chosen certificate"))?;
            if !result
                .certified_optimal_actions
                .iter()
                .any(|action| action == chosen)
            {
                return Err(format!(
                    "{stage}: chosen action is absent from certificate set"
                ));
            }
            if !oracle
                .optimal_root_actions
                .iter()
                .any(|action| action == chosen)
            {
                return Err(format!(
                    "{stage}: chosen certified action is not Oracle-optimal"
                ));
            }
        }
        mtg_kernel::mads_v1::CertificationStatusV1::UnresolvedWithinBudget => {
            if result.chosen_action.is_some() || !result.certified_optimal_actions.is_empty() {
                return Err(format!(
                    "{stage}: unresolved result carries a chosen/certified action"
                ));
            }
        }
    }
    Ok(())
}
fn measure(
    fixture: &OracleFixtureV1,
    oracle: &mtg_kernel::oracle_suite_v1::OracleResultV1,
    iterations: usize,
    budget: usize,
) -> Result<MeasurementsV1, String> {
    for _ in 0..WARMUPS_V1 {
        let (_, _, result) = one_run(fixture, budget)?;
        validate_result_against_oracle("warmup", &result, oracle)?;
    }
    let mut measured = MeasurementsV1::default();
    for _ in 0..iterations {
        let (initialization_wall, search_wall, result) = one_run(fixture, budget)?;
        validate_result_against_oracle("measured run", &result, oracle)?;
        measured.initialization_wall += initialization_wall;
        measured.search_wall += search_wall;
        measured.unknown_runs += usize::from(
            result.status == mtg_kernel::mads_v1::CertificationStatusV1::UnresolvedWithinBudget,
        );
        measured.last_result = Some(result);
    }
    Ok(measured)
}

fn exact_value(bounds: BoundIntervalV1) -> &'static str {
    if bounds.lower == bounds.upper {
        match bounds.lower {
            -1 => "LOSS",
            0 => "DRAW",
            1 => "WIN",
            _ => "INVALID",
        }
    } else {
        "UNKNOWN"
    }
}

fn report(
    encoding: FixtureEncodingV1,
    fixture: &OracleFixtureV1,
    oracle: &mtg_kernel::oracle_suite_v1::OracleResultV1,
    oracle_wall: Duration,
    measurements: &MeasurementsV1,
    iterations: usize,
    budget: usize,
) {
    let result = measurements
        .last_result
        .as_ref()
        .expect("measured at least once");
    let metrics = &result.metrics;
    let divisor = iterations as u128;
    println!("variant={}", encoding.as_str());
    println!(
        "commit={} clean={} tracked_tree_sha256={}",
        env!("MTG_KERNEL_BUILD_GIT_HEAD"),
        env!("MTG_KERNEL_BUILD_GIT_CLEAN"),
        env!("MTG_KERNEL_BUILD_TRACKED_TREE_SHA256")
    );
    println!(
        "fixture={} seed=0x{FIXTURE_SEED_V1:016x} rng_consumed=false",
        fixture.fixture_id
    );
    println!("fixture_scope=synthetic_oracle_equivalent decision_graph; no MTG evaluation");
    println!(
        "budget_expansions={} warmups={} iterations={}",
        budget, WARMUPS_V1, iterations
    );
    println!(
        "oracle_root_value={} oracle_optimal_actions={:?} oracle_legal_root_actions={:?} oracle_game_nodes={} oracle_edges={} oracle_solve_wall_single_ns={}",
        oracle.root_value,
        oracle.optimal_root_actions,
        oracle.complete_legal_root_actions,
        oracle.unique_game_nodes,
        oracle.total_edges,
        oracle_wall.as_nanos(),
    );
    println!(
        "mads_initialization_wall_avg_ns={}",
        measurements.initialization_wall.as_nanos() / divisor
    );
    println!(
        "mads_search_wall_avg_ns={}",
        measurements.search_wall.as_nanos() / divisor
    );
    println!("certification_wall=INCLUDED_IN_SEARCH_WALL_NOT_SEPARATELY_INSTRUMENTED");
    println!(
        "status={:?} root_bounds={:?} exact_root_value={}",
        result.status,
        result.root_bounds,
        exact_value(result.root_bounds)
    );
    println!(
        "certified_root_actions={:?} unknown_runs={}/{}",
        result.certified_optimal_actions, measurements.unknown_runs, iterations
    );
    println!(
        "search_metrics transitions={} state_clones={} expanded_actions={} scheduler_rebuilds={} bound_updates={} fixture_interner_lookups={} fixture_interner_hits={}",
        metrics.authoritative_transitions,
        metrics.state_clones,
        metrics.expanded_actions,
        metrics.frontier_rebuilds,
        metrics.bound_updates,
        metrics.tt_lookups,
        metrics.valid_tt_hits
    );
    println!(
        "model_initialization=NOT_APPLICABLE model_inference=NOT_APPLICABLE cpu_time=UNAVAILABLE peak_rss=UNAVAILABLE"
    );
}

fn main() {
    let config = parse_args().unwrap_or_else(|error| {
        eprintln!("{error}");
        eprintln!("usage: cargo run --example bench_mads_synthetic_v1 -- [--iterations N] [--budget EXPANSIONS]");
        std::process::exit(2);
    });
    let path_local = fixture(FixtureEncodingV1::PathLocalTree);
    let shared_dag = fixture(FixtureEncodingV1::SharedFixtureDag);
    let path_oracle_start = Instant::now();
    let path_oracle = path_local
        .solve_oracle_v1()
        .expect("frozen path-local microfixture is valid");
    let path_oracle_wall = path_oracle_start.elapsed();
    let dag_oracle_start = Instant::now();
    let dag_oracle = shared_dag
        .solve_oracle_v1()
        .expect("frozen shared-DAG microfixture is valid");
    let dag_oracle_wall = dag_oracle_start.elapsed();
    assert_eq!(path_oracle.root_value, dag_oracle.root_value);
    assert_eq!(
        path_oracle.optimal_root_actions,
        dag_oracle.optimal_root_actions
    );
    assert_eq!(
        path_oracle.complete_legal_root_actions,
        dag_oracle.complete_legal_root_actions
    );

    println!("MADS-03E synthetic Oracle microbenchmark (no persistent output file)");
    println!(
        "host_os={} host_arch={} host_cpu={} build_profile={}",
        std::env::consts::OS,
        std::env::consts::ARCH,
        std::env::var("PROCESSOR_IDENTIFIER").unwrap_or_else(|_| "UNAVAILABLE".to_owned()),
        if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        }
    );
    for (encoding, fixture, oracle, oracle_wall) in [
        (
            FixtureEncodingV1::PathLocalTree,
            &path_local,
            &path_oracle,
            path_oracle_wall,
        ),
        (
            FixtureEncodingV1::SharedFixtureDag,
            &shared_dag,
            &dag_oracle,
            dag_oracle_wall,
        ),
    ] {
        let measurements = measure(fixture, oracle, config.iterations, config.budget)
            .unwrap_or_else(|error| {
                panic!("{} failed its Oracle check: {error}", encoding.as_str())
            });
        report(
            encoding,
            fixture,
            oracle,
            oracle_wall,
            &measurements,
            config.iterations,
            config.budget,
        );
    }
    println!("COMPARABLE_PERFORMANCE=NOT_MEASURED; OPTIMIZED_BENCHMARK=NOT_RUN_DEFERRED");
}
