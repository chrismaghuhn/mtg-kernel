//! Independent synthetic oracle stress and falsification tests for MADS V1.
//!
//! This module uses only the public `OracleFixtureV1` and `MadsGraphV1`
//! interfaces. It is test-only: it adds no search algorithm or engine path.

use crate::mads_v1::{
    BoundIntervalV1, CertificationStatusV1, ExpansionRoleMaskV1, ExpansionTaskV1, MadsErrorV1,
    MadsGraphV1, SearchNodeV1,
};
use crate::oracle_suite_v1::{
    check_authoritative_transition_budget_v1, FixtureEdgeV1, FixtureNodeId, FixtureOutcomeV1,
    FixturePlayerV1, OracleErrorV1, OracleFixtureV1, OracleNodeV1,
    MAX_AUTHORITATIVE_TRANSITIONS_V1, MAX_COMPLETE_LEGAL_ACTIONS_PER_GAME_NODE_V1,
    MAX_CONTINUATIONS_PER_CONSTRUCTION_NODE_V1, MAX_SEARCH_DEPTH_V1,
};
use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet};

const ROOT: FixtureNodeId = FixtureNodeId(0);
const DETERMINISTIC_SAMPLE_SEED: u64 = 0x02f0_cafe_0000_0001;
const DETERMINISTIC_SAMPLE_COUNT: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq)]
enum BranchPlan {
    Terminal(FixtureOutcomeV1),
    Min(Vec<FixtureOutcomeV1>),
}

fn all_one_ply_branch_plans() -> Vec<BranchPlan> {
    let outcomes = [
        FixtureOutcomeV1::Loss,
        FixtureOutcomeV1::Draw,
        FixtureOutcomeV1::Win,
    ];
    let mut plans = outcomes
        .iter()
        .copied()
        .map(BranchPlan::Terminal)
        .collect::<Vec<_>>();
    for width in 1..=2 {
        let count = 3usize.pow(width as u32);
        for mut code in 0..count {
            let mut values = Vec::with_capacity(width);
            for _ in 0..width {
                values.push(outcomes[code % outcomes.len()]);
                code /= outcomes.len();
            }
            plans.push(BranchPlan::Min(values));
        }
    }
    plans
}

/// Enumerates the entire finite domain: 1-3 ordered root actions, each
/// independently choosing one of 15 branch programs (a direct terminal, or a
/// one-ply MIN node with 1-2 ordered leaves from {-1,0,+1}).
fn exhaustive_depth_two_domain() -> Vec<OracleFixtureV1> {
    let plans = all_one_ply_branch_plans();
    let capacity = plans.len() + plans.len().pow(2) + plans.len().pow(3);
    let mut fixtures = Vec::with_capacity(capacity);
    for root_width in 1..=3 {
        match root_width {
            1 => {
                for plan in &plans {
                    fixtures.push(fixture_from_plans(
                        std::slice::from_ref(plan),
                        fixtures.len(),
                    ));
                }
            }
            2 => {
                for left in &plans {
                    for right in &plans {
                        fixtures.push(fixture_from_plans(
                            &[left.clone(), right.clone()],
                            fixtures.len(),
                        ));
                    }
                }
            }
            3 => {
                for first in &plans {
                    for second in &plans {
                        for third in &plans {
                            fixtures.push(fixture_from_plans(
                                &[first.clone(), second.clone(), third.clone()],
                                fixtures.len(),
                            ));
                        }
                    }
                }
            }
            _ => unreachable!(),
        }
    }
    fixtures
}

fn fixture_from_plans(plans: &[BranchPlan], ordinal: usize) -> OracleFixtureV1 {
    let mut next_id = 1u32;
    let mut terminals = BTreeMap::<FixtureOutcomeV1, FixtureNodeId>::new();
    let mut nodes = Vec::new();
    let mut root_edges = Vec::with_capacity(plans.len());

    for (action_order, plan) in plans.iter().enumerate() {
        let child = match plan {
            BranchPlan::Terminal(outcome) => {
                intern_terminal(*outcome, &mut next_id, &mut terminals, &mut nodes)
            }
            BranchPlan::Min(outcomes) => {
                let min_id = allocate_id(&mut next_id);
                let actions = outcomes
                    .iter()
                    .enumerate()
                    .map(|(order, outcome)| FixtureEdgeV1 {
                        stable_id: format!("min-{action_order}-leaf-{order}"),
                        order: order as u32,
                        child: intern_terminal(*outcome, &mut next_id, &mut terminals, &mut nodes),
                        estimated_cost_bucket: (order % 2) as u16,
                    })
                    .collect();
                nodes.push(OracleNodeV1::GameDecision {
                    id: min_id,
                    actor: FixturePlayerV1::P1,
                    actions,
                });
                min_id
            }
        };
        root_edges.push(FixtureEdgeV1 {
            stable_id: format!("root-{action_order}"),
            order: action_order as u32,
            child,
            estimated_cost_bucket: 0,
        });
    }
    nodes.push(OracleNodeV1::GameDecision {
        id: ROOT,
        actor: FixturePlayerV1::P0,
        actions: root_edges,
    });

    OracleFixtureV1 {
        fixture_id: format!("mads02f-c-exhaustive-{ordinal:05}"),
        root: ROOT,
        root_player: FixturePlayerV1::P0,
        nodes,
    }
}

fn allocate_id(next_id: &mut u32) -> FixtureNodeId {
    let id = FixtureNodeId(*next_id);
    *next_id += 1;
    id
}

fn intern_terminal(
    outcome: FixtureOutcomeV1,
    next_id: &mut u32,
    terminals: &mut BTreeMap<FixtureOutcomeV1, FixtureNodeId>,
    nodes: &mut Vec<OracleNodeV1>,
) -> FixtureNodeId {
    if let Some(id) = terminals.get(&outcome) {
        return *id;
    }
    let id = allocate_id(next_id);
    terminals.insert(outcome, id);
    nodes.push(OracleNodeV1::Terminal { id, outcome });
    id
}

/// Reproducible deterministic sampling for deeper MAX/MIN DAGs. This sample
/// family has a fixed topology and varies the leaf labels from a fixed seed;
/// it is explicitly NOT part of the exhaustive finite domain above.
fn sampled_deeper_fixtures() -> Vec<(u64, OracleFixtureV1)> {
    let mut rng = DETERMINISTIC_SAMPLE_SEED;
    let mut sampled = Vec::with_capacity(DETERMINISTIC_SAMPLE_COUNT);
    for sample_index in 0..DETERMINISTIC_SAMPLE_COUNT {
        let seed = splitmix64_next(&mut rng);
        sampled.push((seed, deeper_fixture(seed, sample_index)));
    }
    sampled
}

fn splitmix64_next(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

fn deeper_fixture(seed: u64, sample_index: usize) -> OracleFixtureV1 {
    let mut state = seed;
    let mut nodes = Vec::new();
    let mut terminal_cache = BTreeMap::new();
    let mut next_id = 1u32;

    // ROOT(MAX) -> A/B(MIN) -> C/D(MAX) -> interned terminals.
    // Several parents can share a decision node or terminal by identity.
    let shared_max = allocate_id(&mut next_id);
    let left_min = allocate_id(&mut next_id);
    let right_min = allocate_id(&mut next_id);
    let max_b = allocate_id(&mut next_id);
    let terminal_edges = |prefix: &str,
                          state: &mut u64,
                          next_id: &mut u32,
                          terminal_cache: &mut BTreeMap<FixtureOutcomeV1, FixtureNodeId>,
                          nodes: &mut Vec<OracleNodeV1>| {
        (0..3)
            .map(|order| {
                let outcome = outcome_from_roll(splitmix64_next(state));
                FixtureEdgeV1 {
                    stable_id: format!("{prefix}-leaf-{order}"),
                    order,
                    child: intern_terminal(outcome, next_id, terminal_cache, nodes),
                    estimated_cost_bucket: (splitmix64_next(state) % 2) as u16,
                }
            })
            .collect::<Vec<_>>()
    };

    let shared_actions = terminal_edges(
        "shared-max",
        &mut state,
        &mut next_id,
        &mut terminal_cache,
        &mut nodes,
    );
    nodes.push(OracleNodeV1::GameDecision {
        id: shared_max,
        actor: FixturePlayerV1::P0,
        actions: shared_actions,
    });
    let max_b_actions = terminal_edges(
        "max-b",
        &mut state,
        &mut next_id,
        &mut terminal_cache,
        &mut nodes,
    );
    nodes.push(OracleNodeV1::GameDecision {
        id: max_b,
        actor: FixturePlayerV1::P0,
        actions: max_b_actions,
    });
    nodes.push(OracleNodeV1::GameDecision {
        id: left_min,
        actor: FixturePlayerV1::P1,
        actions: vec![
            FixtureEdgeV1 {
                stable_id: "left-to-shared".into(),
                order: 0,
                child: shared_max,
                estimated_cost_bucket: 0,
            },
            FixtureEdgeV1 {
                stable_id: "left-to-max-b".into(),
                order: 1,
                child: max_b,
                estimated_cost_bucket: 0,
            },
        ],
    });
    nodes.push(OracleNodeV1::GameDecision {
        id: right_min,
        actor: FixturePlayerV1::P1,
        actions: vec![
            FixtureEdgeV1 {
                stable_id: "right-to-shared".into(),
                order: 0,
                child: shared_max,
                estimated_cost_bucket: 0,
            },
            FixtureEdgeV1 {
                stable_id: "right-to-max-b".into(),
                order: 1,
                child: max_b,
                estimated_cost_bucket: 0,
            },
        ],
    });
    nodes.push(OracleNodeV1::GameDecision {
        id: ROOT,
        actor: FixturePlayerV1::P0,
        actions: vec![
            FixtureEdgeV1 {
                stable_id: "root-left".into(),
                order: 0,
                child: left_min,
                estimated_cost_bucket: 0,
            },
            FixtureEdgeV1 {
                stable_id: "root-right".into(),
                order: 1,
                child: right_min,
                estimated_cost_bucket: 0,
            },
        ],
    });

    OracleFixtureV1 {
        fixture_id: format!("mads02f-c-seed-{seed:016x}-sample-{sample_index:03}"),
        root: ROOT,
        root_player: FixturePlayerV1::P0,
        nodes,
    }
}

fn outcome_from_roll(roll: u64) -> FixtureOutcomeV1 {
    match roll % 3 {
        0 => FixtureOutcomeV1::Loss,
        1 => FixtureOutcomeV1::Draw,
        _ => FixtureOutcomeV1::Win,
    }
}

fn diagnostic(
    fixture: &OracleFixtureV1,
    expansion: usize,
    task: Option<&ExpansionTaskV1>,
) -> String {
    let task = task.map_or_else(|| "none".to_owned(), |task| format!("{task:?}"));
    format!(
        "fixture={} expansion={} task={} graph={:?}",
        fixture.fixture_id, expansion, task, fixture.nodes
    )
}

fn check_graph_against_oracle(
    fixture: &OracleFixtureV1,
    oracle_values: &BTreeMap<FixtureNodeId, i8>,
    search: &MadsGraphV1<'_>,
    expansion: usize,
    task: Option<&ExpansionTaskV1>,
) {
    let result = search.result_v1();
    let context = format!(
        "{} status={:?} root_bounds={:?} root_actions={:?} certified={:?} unique={:?} chosen={:?} anytime={}",
        diagnostic(fixture, expansion, task),
        result.status,
        result.root_bounds,
        result.root_actions,
        result.certified_optimal_actions,
        result.certified_unique_action,
        result.chosen_action,
        result.anytime_action
    );
    for node in search.nodes() {
        let value = oracle_values
            .get(&node.fixture_key())
            .unwrap_or_else(|| panic!("missing oracle node value; {context}"));
        let bounds = node.bounds();
        assert!(
            bounds.lower <= *value && *value <= bounds.upper,
            "bound violation node={} oracle={} interval=[{},{}]; {}",
            node.fixture_key().0,
            value,
            bounds.lower,
            bounds.upper,
            context
        );
        assert!(bounds.is_valid(), "invalid interval; {context}");
    }

    let root_value = oracle_values[&fixture.root];
    let root_bounds = search.root_bounds();
    assert!(
        root_bounds.lower <= root_value && root_value <= root_bounds.upper,
        "root oracle={} interval=[{},{}]; {}",
        root_value,
        root_bounds.lower,
        root_bounds.upper,
        context
    );

    let root = fixture.node(fixture.root).expect("fixture root exists");
    let expected_actions = root
        .edges()
        .iter()
        .map(|edge| edge.stable_id.as_str())
        .collect::<Vec<_>>();
    let actual_actions = search.root_action_bounds_v1();
    assert_eq!(
        actual_actions
            .iter()
            .map(|action| action.stable_id.as_str())
            .collect::<Vec<_>>(),
        expected_actions,
        "root action domain or order changed; {context}"
    );
    for (root_edge, action) in root.edges().iter().zip(&actual_actions) {
        if !action.expanded {
            assert_eq!(
                action.bounds,
                BoundIntervalV1::UNKNOWN,
                "unexpanded action lost UNKNOWN bounds: {}; {}",
                action.stable_id,
                context
            );
        } else {
            let child_value = oracle_values[&root_edge.child];
            assert!(
                action.bounds.lower <= child_value && child_value <= action.bounds.upper,
                "root action {} child {} oracle={} interval=[{},{}]; {}",
                action.stable_id,
                root_edge.child.0,
                child_value,
                action.bounds.lower,
                action.bounds.upper,
                context
            );
        }
    }

    let oracle_root = oracle_values[&fixture.root];
    let oracle_optimal = root
        .edges()
        .iter()
        .filter(|edge| oracle_values[&edge.child] == oracle_root)
        .map(|edge| edge.stable_id.clone())
        .collect::<Vec<_>>();
    assert!(
        result
            .certified_optimal_actions
            .iter()
            .all(|action| oracle_optimal.contains(action)),
        "nonoptimal action certified: {:?}, oracle optimal={:?}; {}",
        result.certified_optimal_actions,
        oracle_optimal,
        context
    );
    if let Some(unique) = &result.certified_unique_action {
        assert_eq!(
            oracle_optimal,
            vec![unique.clone()],
            "unique certificate did not match unique oracle optimum; {context}"
        );
    }
    assert!(
        expected_actions.contains(&result.anytime_action.as_str()),
        "anytime action is outside the legal root domain; {context}"
    );
    match result.status {
        CertificationStatusV1::Certified => {
            let chosen = result
                .chosen_action
                .as_ref()
                .expect("certified result must carry a chosen action");
            assert!(
                oracle_optimal.contains(chosen),
                "certified chosen action is not oracle-optimal; {context}"
            );
        }
        CertificationStatusV1::UnresolvedWithinBudget => assert!(
            result.chosen_action.is_none(),
            "unresolved result exposed an anytime action as certified; {context}"
        ),
    }
}

#[test]
fn bounded_exhaustive_depth_two_dags_match_oracle_after_every_expansion() {
    let fixtures = exhaustive_depth_two_domain();
    assert_eq!(all_one_ply_branch_plans().len(), 15);
    assert_eq!(fixtures.len(), 3_615);
    let mut checked_expansions = 0usize;
    let mut found_certified_but_inexact_root = false;

    for fixture in &fixtures {
        let oracle = fixture.solve_oracle_v1().expect("generated DAG is valid");
        let mut search = MadsGraphV1::new(fixture).expect("generated DAG admits to MADS");
        check_graph_against_oracle(fixture, &oracle.node_values, &search, 0, None);
        if search.result_v1().status == CertificationStatusV1::Certified
            && search.root_bounds() != BoundIntervalV1::exact(oracle.root_value)
        {
            found_certified_but_inexact_root = true;
        }

        let mut expansion = 0usize;
        loop {
            let before = search.metrics().expanded_actions;
            let task = search
                .expand_next_v1()
                .expect("reference frontier task expands");
            let Some(task) = task else { break };
            expansion += 1;
            checked_expansions += 1;
            assert_eq!(
                search.metrics().expanded_actions,
                before + 1,
                "one expansion must add one parent edge; {}",
                diagnostic(fixture, expansion, Some(&task))
            );
            check_graph_against_oracle(
                fixture,
                &oracle.node_values,
                &search,
                expansion,
                Some(&task),
            );
        }

        let result = search.result_v1();
        let all_root_actions_exact = result
            .root_actions
            .iter()
            .all(|action| action.expanded && action.bounds.lower == action.bounds.upper);
        if all_root_actions_exact {
            let expected = fixture
                .node(fixture.root)
                .unwrap()
                .edges()
                .iter()
                .filter(|edge| oracle.node_values[&edge.child] == oracle.root_value)
                .map(|edge| edge.stable_id.clone())
                .collect::<Vec<_>>();
            assert_eq!(result.certified_optimal_actions, expected);
        }
    }
    assert!(checked_expansions > fixtures.len());
    assert!(
        found_certified_but_inexact_root,
        "the domain must distinguish action certification from exact root resolution"
    );
    eprintln!(
        "MADS02F_C_EXHAUSTIVE fixtures={} expansions_checked={}",
        fixtures.len(),
        checked_expansions
    );
}

#[test]
fn deterministic_deep_dag_sample_checks_bounds_and_transpositions() {
    let sampled = sampled_deeper_fixtures();
    assert_eq!(sampled.len(), DETERMINISTIC_SAMPLE_COUNT);
    let mut seeds = BTreeSet::new();
    let mut checked_expansions = 0usize;
    for (seed, fixture) in &sampled {
        assert!(seeds.insert(*seed), "duplicate sample seed {seed:#x}");
        let oracle = fixture.solve_oracle_v1().unwrap_or_else(|error| {
            panic!(
                "sample oracle error seed={seed:#018x}: {error}; graph={:?}",
                fixture.nodes
            )
        });
        let mut search = MadsGraphV1::new(fixture).unwrap_or_else(|error| {
            panic!(
                "sample MADS admission error seed={seed:#018x}: {error}; graph={:?}",
                fixture.nodes
            )
        });
        check_graph_against_oracle(fixture, &oracle.node_values, &search, 0, None);
        let mut expansion = 0;
        loop {
            let task = search.expand_next_v1().unwrap();
            let Some(task) = task else { break };
            expansion += 1;
            checked_expansions += 1;
            check_graph_against_oracle(
                fixture,
                &oracle.node_values,
                &search,
                expansion,
                Some(&task),
            );
        }
        assert!(
            search.metrics().valid_tt_hits > 0,
            "expected fixture-local shared-child reuse, seed={seed:#018x}; graph={:?}",
            fixture.nodes
        );
    }
    assert!(checked_expansions > sampled.len());
    eprintln!(
        "MADS02F_C_SAMPLED seed_root={DETERMINISTIC_SAMPLE_SEED:#018x} fixtures={} expansions_checked={}",
        sampled.len(),
        checked_expansions
    );
}

#[test]
fn budgets_never_expand_past_limit_or_certify_a_nonoptimal_action() {
    let fixtures = exhaustive_depth_two_domain();
    for fixture in &fixtures {
        let oracle = fixture.solve_oracle_v1().unwrap();
        let max_budget = oracle.total_edges;
        for budget in [0, 1, 2, max_budget] {
            let mut search = MadsGraphV1::new(fixture).unwrap();
            let result = search.run_v1(budget).unwrap();
            assert!(
                search.metrics().expanded_actions as usize <= budget,
                "budget overrun budget={budget}; {}",
                diagnostic(fixture, 0, None)
            );
            if budget == 0 {
                assert_eq!(search.metrics().expanded_actions, 0);
            }
            check_graph_against_oracle(fixture, &oracle.node_values, &search, budget, None);
            if result.status == CertificationStatusV1::UnresolvedWithinBudget {
                assert!(result.chosen_action.is_none());
                assert!(result.certified_optimal_actions.is_empty());
            }
        }

        let mut first_certificate_search = MadsGraphV1::new(fixture).unwrap();
        let mut budget_to_certificate = 0usize;
        while first_certificate_search.result_v1().status
            == CertificationStatusV1::UnresolvedWithinBudget
            && budget_to_certificate < max_budget
        {
            let before = first_certificate_search.metrics().expanded_actions;
            let result = first_certificate_search.run_v1(1).unwrap();
            budget_to_certificate +=
                (first_certificate_search.metrics().expanded_actions - before) as usize;
            if result.status == CertificationStatusV1::UnresolvedWithinBudget
                && first_certificate_search.metrics().expanded_actions == before
            {
                break;
            }
        }
        let mut exact_threshold = MadsGraphV1::new(fixture).unwrap();
        let threshold_result = exact_threshold.run_v1(budget_to_certificate).unwrap();
        check_graph_against_oracle(
            fixture,
            &oracle.node_values,
            &exact_threshold,
            budget_to_certificate,
            None,
        );
        if first_certificate_search.result_v1().status == CertificationStatusV1::Certified {
            assert_eq!(threshold_result.status, CertificationStatusV1::Certified);
        }
    }
}

fn shared_transposition_fixture() -> OracleFixtureV1 {
    OracleFixtureV1 {
        fixture_id: "mads02f-c-shared-c-two-parents".into(),
        root: ROOT,
        root_player: FixturePlayerV1::P0,
        nodes: vec![
            OracleNodeV1::GameDecision {
                id: ROOT,
                actor: FixturePlayerV1::P0,
                actions: vec![edge(0, "root-a", 1), edge(1, "root-b", 2)],
            },
            OracleNodeV1::GameDecision {
                id: FixtureNodeId(1),
                actor: FixturePlayerV1::P1,
                actions: vec![edge(0, "a-to-c", 3)],
            },
            OracleNodeV1::GameDecision {
                id: FixtureNodeId(2),
                actor: FixturePlayerV1::P1,
                actions: vec![edge(0, "b-to-c", 3)],
            },
            OracleNodeV1::GameDecision {
                id: FixtureNodeId(3),
                actor: FixturePlayerV1::P0,
                actions: vec![
                    edge(0, "c-loss", 4),
                    edge(1, "c-draw", 5),
                    edge(2, "c-win", 6),
                ],
            },
            terminal(4, FixtureOutcomeV1::Loss),
            terminal(5, FixtureOutcomeV1::Draw),
            terminal(6, FixtureOutcomeV1::Win),
        ],
    }
}

fn edge(order: u32, stable_id: &str, child: u32) -> FixtureEdgeV1 {
    FixtureEdgeV1 {
        stable_id: stable_id.into(),
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

#[test]
fn shared_child_reuse_propagates_bounds_to_all_parents() {
    let fixture = shared_transposition_fixture();
    let oracle = fixture.solve_oracle_v1().unwrap();
    let mut search = MadsGraphV1::new(&fixture).unwrap();
    check_graph_against_oracle(&fixture, &oracle.node_values, &search, 0, None);

    let mut expanded_slots = BTreeSet::new();
    let mut expansion_number = 0;
    let mut saw_shared_c = false;
    loop {
        let tasks = search.rebuild_frontier_v1().unwrap().to_vec();
        assert_eq!(
            tasks,
            search.rebuild_frontier_v1().unwrap(),
            "frontier rebuild must be deterministic"
        );
        assert!(
            tasks
                .windows(2)
                .all(|pair| scheduler_key(&pair[0]) <= scheduler_key(&pair[1])),
            "frontier order disagrees with reference sort: {tasks:?}"
        );
        let unique_slots = tasks
            .iter()
            .map(|task| (task.owner, task.action_order))
            .collect::<BTreeSet<_>>();
        assert_eq!(unique_slots.len(), tasks.len(), "duplicate frontier task");
        let Some(task) = tasks.first().cloned() else {
            break;
        };
        assert!(expanded_slots.insert((task.owner, task.action_order)));
        expansion_number += 1;
        let old_parent_bounds = search
            .nodes()
            .iter()
            .filter(|node| matches!(node, SearchNodeV1::GameDecisionNode(n) if n.fixture_key.0 == 1 || n.fixture_key.0 == 2))
            .map(|node| (node.fixture_key(), node.bounds()))
            .collect::<BTreeMap<_, _>>();
        let old_updates = search.metrics().bound_updates;
        let expanded = search
            .expand_next_v1()
            .unwrap()
            .expect("frontier was nonempty");
        assert_eq!(expanded, task);
        check_graph_against_oracle(
            &fixture,
            &oracle.node_values,
            &search,
            expansion_number,
            Some(&task),
        );
        if task.owner == FixtureNodeId(3) {
            assert!(search.metrics().bound_updates >= old_updates);
            if task.action_order == 2 {
                assert!(search.metrics().bound_updates > old_updates);
                let new_parent_bounds = search
                    .nodes()
                    .iter()
                    .filter(|node| matches!(node, SearchNodeV1::GameDecisionNode(n) if n.fixture_key.0 == 1 || n.fixture_key.0 == 2))
                    .map(|node| (node.fixture_key(), node.bounds()))
                    .collect::<BTreeMap<_, _>>();
                assert!(old_parent_bounds != new_parent_bounds);
            }
        }
        saw_shared_c |= search
            .nodes()
            .iter()
            .filter(|node| node.fixture_key() == FixtureNodeId(3))
            .count()
            == 1
            && search.metrics().valid_tt_hits > 0;
    }
    assert!(saw_shared_c, "shared C was not reused");
    assert_eq!(
        search.root_bounds(),
        BoundIntervalV1::exact(oracle.root_value)
    );
    assert_eq!(search.metrics().valid_tt_hits, 1);
}

fn scheduler_key(task: &ExpansionTaskV1) -> (u8, Reverse<u8>, u16, u16, FixtureNodeId, u32) {
    let both = task.role_mask.supports_both_primary();
    let lower = task
        .role_mask
        .contains(ExpansionRoleMaskV1::INCUMBENT_LOWER);
    let upper = task
        .role_mask
        .contains(ExpansionRoleMaskV1::CHALLENGER_UPPER);
    let role_rank = if both {
        0
    } else if lower {
        1
    } else if upper {
        2
    } else {
        3
    };
    (
        role_rank,
        Reverse(task.bound_width),
        task.min_root_distance,
        task.estimated_cost_bucket,
        task.owner,
        task.action_order,
    )
}

#[test]
fn scheduler_ties_choose_stable_challenger_and_action_order() {
    let fixture = shared_transposition_fixture();
    let mut search = MadsGraphV1::new(&fixture).unwrap();
    let initial = search.rebuild_frontier_v1().unwrap().to_vec();
    assert_eq!(initial.len(), 2);
    assert_eq!(initial[0].action_id, "root-a");
    assert_eq!(initial[1].action_id, "root-b");
    assert!(initial[0]
        .role_mask
        .contains(ExpansionRoleMaskV1::INCUMBENT_LOWER));
    assert!(initial[1]
        .role_mask
        .contains(ExpansionRoleMaskV1::CHALLENGER_UPPER));
    assert_eq!(
        initial,
        search.rebuild_frontier_v1().unwrap(),
        "equal scheduler inputs must have stable order"
    );

    let tied_challengers = OracleFixtureV1 {
        fixture_id: "mads02f-c-three-equal-root-challengers".into(),
        root: ROOT,
        root_player: FixturePlayerV1::P0,
        nodes: vec![
            OracleNodeV1::GameDecision {
                id: ROOT,
                actor: FixturePlayerV1::P0,
                actions: vec![
                    edge(0, "tie-a", 1),
                    edge(1, "tie-b", 1),
                    edge(2, "tie-c", 1),
                ],
            },
            OracleNodeV1::GameDecision {
                id: FixtureNodeId(1),
                actor: FixturePlayerV1::P1,
                actions: vec![edge(0, "same-loss", 2), edge(1, "same-win", 3)],
            },
            terminal(2, FixtureOutcomeV1::Loss),
            terminal(3, FixtureOutcomeV1::Win),
        ],
    };
    let mut tied_search = MadsGraphV1::new(&tied_challengers).unwrap();
    let tied_tasks = tied_search.rebuild_frontier_v1().unwrap().to_vec();
    assert_eq!(
        tied_tasks
            .iter()
            .map(|task| task.action_id.as_str())
            .collect::<Vec<_>>(),
        vec!["tie-a", "tie-b"],
        "equal challenger tie must select the earliest semantic action"
    );
    assert!(
        !tied_tasks[0].role_mask.supports_both_primary(),
        "root-action support starts with distinct lower/upper tasks"
    );
    assert_eq!(
        tied_tasks,
        tied_search.rebuild_frontier_v1().unwrap(),
        "three-way tie must rebuild deterministically"
    );
}

fn simple_fixture(root: FixtureNodeId, nodes: Vec<OracleNodeV1>) -> OracleFixtureV1 {
    OracleFixtureV1 {
        fixture_id: "mads02f-c-invalid-fixture".into(),
        root,
        root_player: FixturePlayerV1::P0,
        nodes,
    }
}

fn assert_oracle_invalid(fixture: &OracleFixtureV1) {
    assert!(matches!(
        fixture.solve_oracle_v1(),
        Err(OracleErrorV1::InvalidFixture(_))
    ));
}

#[test]
fn invalid_fixture_cycle_unreachable_child_and_duplicate_identity_are_rejected() {
    let cycle = simple_fixture(
        ROOT,
        vec![OracleNodeV1::GameDecision {
            id: ROOT,
            actor: FixturePlayerV1::P0,
            actions: vec![edge(0, "cycle", 0)],
        }],
    );
    assert_eq!(cycle.solve_oracle_v1(), Err(OracleErrorV1::CycleDetected));
    assert!(matches!(
        MadsGraphV1::new(&cycle),
        Err(MadsErrorV1::Oracle(_))
    ));

    let unreachable = simple_fixture(
        ROOT,
        vec![
            OracleNodeV1::GameDecision {
                id: ROOT,
                actor: FixturePlayerV1::P0,
                actions: vec![edge(0, "to-terminal", 1)],
            },
            terminal(1, FixtureOutcomeV1::Draw),
            terminal(2, FixtureOutcomeV1::Win),
        ],
    );
    assert_oracle_invalid(&unreachable);

    let missing_child = simple_fixture(
        ROOT,
        vec![OracleNodeV1::GameDecision {
            id: ROOT,
            actor: FixturePlayerV1::P0,
            actions: vec![edge(0, "missing", 99)],
        }],
    );
    assert_oracle_invalid(&missing_child);

    let duplicate_identity = simple_fixture(
        ROOT,
        vec![
            OracleNodeV1::GameDecision {
                id: ROOT,
                actor: FixturePlayerV1::P0,
                actions: vec![edge(0, "same", 1), edge(1, "same", 1)],
            },
            terminal(1, FixtureOutcomeV1::Draw),
        ],
    );
    assert_oracle_invalid(&duplicate_identity);
}

#[test]
fn invalid_root_construction_owner_type_and_admission_caps_use_oracle_errors() {
    let terminal_root = simple_fixture(ROOT, vec![terminal(0, FixtureOutcomeV1::Win)]);
    assert!(matches!(
        terminal_root.solve_oracle_v1(),
        Err(OracleErrorV1::InvalidFixture(_))
    ));
    assert!(matches!(
        MadsGraphV1::new(&terminal_root),
        Err(MadsErrorV1::InvalidRoot(_))
    ));

    let wrong_root_actor = simple_fixture(
        ROOT,
        vec![
            OracleNodeV1::GameDecision {
                id: ROOT,
                actor: FixturePlayerV1::P1,
                actions: vec![edge(0, "leaf", 1)],
            },
            terminal(1, FixtureOutcomeV1::Win),
        ],
    );
    assert_oracle_invalid(&wrong_root_actor);
    assert!(matches!(
        MadsGraphV1::new(&wrong_root_actor),
        Err(MadsErrorV1::InvalidRoot(_))
    ));

    let construction_owner_mismatch = simple_fixture(
        ROOT,
        vec![
            OracleNodeV1::GameDecision {
                id: ROOT,
                actor: FixturePlayerV1::P0,
                actions: vec![edge(0, "construct", 1)],
            },
            OracleNodeV1::DecisionConstruction {
                id: FixtureNodeId(1),
                owner_decision: ROOT,
                actor: FixturePlayerV1::P1,
                protocol_key: "target-choice".into(),
                partial_response: "".into(),
                continuation_cursor: 0,
                choices: vec![edge(0, "done", 2)],
            },
            terminal(2, FixtureOutcomeV1::Draw),
        ],
    );
    assert_oracle_invalid(&construction_owner_mismatch);

    let too_many_actions = simple_fixture(
        ROOT,
        vec![
            OracleNodeV1::GameDecision {
                id: ROOT,
                actor: FixturePlayerV1::P0,
                actions: (0..=MAX_COMPLETE_LEGAL_ACTIONS_PER_GAME_NODE_V1)
                    .map(|order| edge(order as u32, &format!("a-{order}"), 1))
                    .collect(),
            },
            terminal(1, FixtureOutcomeV1::Draw),
        ],
    );
    assert_eq!(
        too_many_actions.solve_oracle_v1(),
        Err(OracleErrorV1::FixtureTooLarge)
    );

    let too_many_continuations = simple_fixture(
        ROOT,
        vec![
            OracleNodeV1::GameDecision {
                id: ROOT,
                actor: FixturePlayerV1::P0,
                actions: vec![edge(0, "construct", 1)],
            },
            OracleNodeV1::DecisionConstruction {
                id: FixtureNodeId(1),
                owner_decision: ROOT,
                actor: FixturePlayerV1::P0,
                protocol_key: "continuation".into(),
                partial_response: "".into(),
                continuation_cursor: 0,
                choices: (0..=MAX_CONTINUATIONS_PER_CONSTRUCTION_NODE_V1)
                    .map(|order| edge(order as u32, &format!("c-{order}"), 2))
                    .collect(),
            },
            terminal(2, FixtureOutcomeV1::Draw),
        ],
    );
    assert_eq!(
        too_many_continuations.solve_oracle_v1(),
        Err(OracleErrorV1::FixtureTooLarge)
    );

    let mut deep_nodes = Vec::new();
    for id in 0..=MAX_SEARCH_DEPTH_V1 as u32 {
        deep_nodes.push(OracleNodeV1::GameDecision {
            id: FixtureNodeId(id),
            actor: if id % 2 == 0 {
                FixturePlayerV1::P0
            } else {
                FixturePlayerV1::P1
            },
            actions: vec![edge(0, &format!("next-{id}"), id + 1)],
        });
    }
    deep_nodes.push(terminal(
        MAX_SEARCH_DEPTH_V1 as u32 + 1,
        FixtureOutcomeV1::Draw,
    ));
    let too_deep = simple_fixture(ROOT, deep_nodes);
    assert_eq!(
        too_deep.solve_oracle_v1(),
        Err(OracleErrorV1::FixtureTooLarge)
    );

    assert_eq!(
        check_authoritative_transition_budget_v1(MAX_AUTHORITATIVE_TRANSITIONS_V1),
        Ok(())
    );
    assert_eq!(
        check_authoritative_transition_budget_v1(MAX_AUTHORITATIVE_TRANSITIONS_V1 + 1),
        Err(OracleErrorV1::FixtureTooLarge)
    );
}
