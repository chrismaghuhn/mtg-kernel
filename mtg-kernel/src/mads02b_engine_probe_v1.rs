//! MADS-02B bounded engine/oracle integration probes.
//!
//! These tests do not expose privileged game state as policy input. The Burn
//! probe checks an actor-authorized V5 action surface against an independent
//! enumeration of one raw engine decision. The small synthetic graph is
//! explicitly `ENGINE_ORACLE_ONLY`; it is not a fair Magic teacher.

use crate::card_def::card_id_by_name;
use crate::dynamic_engine_search_v1::{DynamicEngineSearchV1, DynamicSearchStatusV1};
use crate::engine::{self, Action, Decision};
use crate::event::{self, ProposedEvent};
use crate::ids::{ObjectId, PlayerId};
use crate::mads_v1::{BoundIntervalV1, CertificationStatusV1, MadsGraphV1};
use crate::oracle_suite_v1::{
    FixtureEdgeV1, FixtureNodeId, FixtureOutcomeV1, FixturePlayerV1, OracleFixtureV1, OracleNodeV1,
    MAX_AUTHORITATIVE_TRANSITIONS_V1, MAX_COMPLETE_LEGAL_ACTIONS_PER_GAME_NODE_V1,
    MAX_SEARCH_DEPTH_V1, MAX_TOTAL_EDGES_V1, MAX_UNIQUE_GAME_NODES_V1,
};
use crate::policy_surface_v5::{PolicyActionV5, PolicyDecisionV5};
use crate::rl::{legal_action_candidates_v5, ActionSemanticV1};
use crate::runtime_decks::runtime_deck_by_id;
use crate::state::GameState;
use crate::surface_v2::{SurfaceAction, SurfaceDecision};

const BURN_PROBE_SEED: u64 = 0x02b0_2026_0929;
const TINY_ORACLE_SEED: u64 = 0x02b0_0001;

fn burn_state(seed: u64) -> GameState {
    let burn = runtime_deck_by_id("Burn").expect("official runtime Burn deck exists");
    assert_eq!(burn.id, "Burn");
    assert_eq!(burn.mainboard_count, 60);
    assert!(burn.is_fully_materialized());
    assert_eq!(
        burn.source_sha256,
        "4ebba6b42bb27a0ea55001cee133aada81f0dffd8661b46b012fc5026675aa32"
    );
    assert_eq!(burn.runtime_deck_hash, 0x5fdb_7b92_986b_6fc1);
    let state = crate::rl::build_deck_pair_state(seed, burn.card_ids, burn.card_ids)
        .expect("official Burn mirror deck pair passes preflight");
    assert_eq!(state.starting_player, PlayerId::P0);
    assert_eq!(state.active_player, PlayerId::P0);
    state
}

/// Move through only forced priority/pass and empty-attacker decisions until
/// the first Main1 CastSpellOrPass with at least two raw legal actions.
fn first_nontrivial_burn_main1(seed: u64) -> (GameState, Decision) {
    let mut state = burn_state(seed);
    for _ in 0..32 {
        let decision = engine::advance_until_decision(&mut state);
        match &decision {
            Decision::CastSpellOrPass { .. } if state.step == crate::state::Step::Main1 => {
                let expected = complete_cast_or_pass_domain(&decision, &state)
                    .expect("probe decision is supported");
                if expected.len() >= 2 {
                    return (state, decision);
                }
                engine::step(&mut state, Action::Pass).expect("forced pass is legal");
            }
            Decision::CastSpellOrPass { .. } => {
                engine::step(&mut state, Action::Pass).expect("priority pass is legal");
            }
            Decision::DeclareAttackers { eligible, .. } if eligible.is_empty() => {
                engine::step(&mut state, Action::DeclareAttackers(Vec::new()))
                    .expect("empty attacker declaration is legal");
            }
            Decision::GameOver { .. } => panic!("Burn probe reached a terminal before Main1"),
            Decision::Halted { .. } => panic!("Burn probe reached UNSUPPORTED_DECISION: Halted"),
            other => panic!("unexpected decision before Burn Main1 probe: {other:?}"),
        }
    }
    panic!("Burn probe did not reach a nontrivial Main1 decision within 32 decisions")
}

/// Independently enumerate the complete raw Action domain for exactly the
/// CastSpellOrPass decision shape. It mirrors the documented Decision fields,
/// not the V5 candidate generator.
fn complete_cast_or_pass_domain(
    decision: &Decision,
    state: &GameState,
) -> Result<Vec<Action>, String> {
    let Decision::CastSpellOrPass {
        player,
        castable_spells,
        mana_abilities,
        land_drops,
        activatable_abilities,
        plot_actions,
    } = decision
    else {
        return Err("UNSUPPORTED_DECISION: expected CastSpellOrPass".to_string());
    };
    let mut actions = Vec::new();
    actions.extend(castable_spells.iter().copied().map(Action::CastSpell));
    for &source in mana_abilities {
        let choices = engine::available_mana_ability_choices(*player, source, state);
        let cost_targets = engine::mana_ability_cost_targets(*player, source, state);
        if choices.is_empty() {
            return Err(format!("mana ability {source:?} has no supported choices"));
        }
        if cost_targets.is_empty() {
            for choice in choices.iter().copied() {
                actions.push(if choices.len() == 1 {
                    Action::ActivateManaAbility(source)
                } else {
                    Action::ActivateManaAbilityChoice(source, choice)
                });
            }
        } else {
            for choice in choices.iter().copied() {
                for target in cost_targets.iter().copied() {
                    actions.push(Action::ActivateManaAbilityWithCostTarget(
                        source, choice, target,
                    ));
                }
            }
        }
    }
    actions.extend(land_drops.iter().copied().map(Action::PlayLand));
    actions.extend(
        activatable_abilities
            .iter()
            .copied()
            .map(|(source, ability)| Action::ActivateAbility(source, ability)),
    );
    actions.extend(plot_actions.iter().copied().map(Action::PlotSpell));
    actions.push(Action::Pass);
    Ok(actions)
}

fn v5_actions(decision: &Decision, state: &GameState) -> Vec<(ActionSemanticV1, String, Action)> {
    legal_action_candidates_v5(
        &PolicyDecisionV5::Surface(SurfaceDecision::Decision(decision.clone())),
        state,
    )
    .expect("V5 projection supports this raw decision")
    .into_iter()
    .map(|candidate| {
        let PolicyActionV5::Surface(SurfaceAction::Action(action)) = candidate.policy_action else {
            panic!("CastSpellOrPass did not map to a raw engine action")
        };
        (
            candidate.record.semantic,
            candidate.record.stable_id,
            action,
        )
    })
    .collect()
}

#[test]
fn real_pauper_burn_cast_or_pass_domain_is_complete_and_reproducible() {
    let (state_a, decision_a) = first_nontrivial_burn_main1(BURN_PROBE_SEED);
    let (state_b, decision_b) = first_nontrivial_burn_main1(BURN_PROBE_SEED);
    assert_eq!(
        state_a, state_b,
        "authoritative states must reproduce exactly"
    );
    assert_eq!(
        decision_a, decision_b,
        "first raw decisions must reproduce exactly"
    );

    let expected = complete_cast_or_pass_domain(&decision_a, &state_a).unwrap();
    let projected = v5_actions(&decision_a, &state_a);
    let projected_actions = projected
        .iter()
        .map(|(_, _, action)| action.clone())
        .collect::<Vec<_>>();
    assert_eq!(
        projected_actions, expected,
        "V5 projection must preserve the full ordered raw domain"
    );
    assert!(expected.len() >= 2);
    assert!(matches!(decision_a, Decision::CastSpellOrPass { .. }));
    assert_eq!(state_a.step, crate::state::Step::Main1);

    let semantics = projected.iter().map(|row| &row.0).collect::<Vec<_>>();
    let stable_ids = projected.iter().map(|row| &row.1).collect::<Vec<_>>();
    assert_eq!(semantics.len(), expected.len());
    assert_eq!(
        semantics
            .iter()
            .collect::<std::collections::HashSet<_>>()
            .len(),
        semantics.len()
    );
    assert_eq!(
        stable_ids
            .iter()
            .collect::<std::collections::HashSet<_>>()
            .len(),
        stable_ids.len()
    );
    let Action::PlayLand(first_land) = expected[0] else {
        panic!("first root action must be the first legal land play")
    };
    let Action::PlayLand(second_land) = expected[1] else {
        panic!("second root action must be the second legal land play")
    };
    assert_ne!(first_land, second_land);
    assert_ne!(projected[0].0, projected[1].0);
    let original_authoritative_state = state_a.clone();
    let mut explicit_step_transitions = 0;
    let burn = runtime_deck_by_id("Burn").unwrap();
    eprintln!(
        "REAL_PAUPER_DECISION_PROBE deck={} source_sha256={} runtime_hash={:016x} seed={:#018x} starting_player=P0 turn={} step={:?} actor={:?} actions={:?}",
        burn.id,
        burn.source_sha256,
        burn.runtime_deck_hash,
        BURN_PROBE_SEED,
        state_a.turn,
        state_a.step,
        match decision_a { Decision::CastSpellOrPass { player, .. } => player, _ => unreachable!() },
        projected.iter().map(|(semantic, _, _)| semantic).collect::<Vec<_>>(),
    );

    for action in expected {
        let mut left = state_a.clone();
        let mut right = state_b.clone();
        engine::step(&mut left, action.clone()).expect("every enumerated action is accepted");
        explicit_step_transitions += 1;
        engine::step(&mut right, action).expect("same action is accepted on independent clone");
        explicit_step_transitions += 1;
        let next_left = engine::advance_until_decision(&mut left);
        let next_right = engine::advance_until_decision(&mut right);
        assert_eq!(
            left, right,
            "identical action over exact state clones diverged"
        );
        assert_eq!(
            next_left, next_right,
            "follow-up decisions diverged for identical seed/action"
        );
    }
    assert_eq!(
        state_a, original_authoritative_state,
        "branching mutated the probe root"
    );
    assert_eq!(explicit_step_transitions, 6);
}

#[derive(Debug)]
struct EnumeratedEngineGraph {
    fixture: OracleFixtureV1,
    explicit_step_transitions: usize,
    state_nodes: Vec<(GameState, Decision, FixtureNodeId)>,
}

fn tiny_engine_root() -> (GameState, Decision) {
    let mountain = card_id_by_name("Mountain").expect("Mountain card definition");
    let mut state = GameState::new_from_libraries(
        &[mountain, mountain],
        &[mountain, mountain, mountain],
        crate::rl::card_name,
        TINY_ORACLE_SEED,
    );
    for _ in 0..2 {
        event::propose_and_commit(&mut state, ProposedEvent::draw(PlayerId::P0));
        event::propose_and_commit(&mut state, ProposedEvent::draw(PlayerId::P1));
    }
    let decision = loop {
        let current = engine::advance_until_decision(&mut state);
        match &current {
            Decision::CastSpellOrPass { player, .. }
                if state.step == crate::state::Step::Main2 && *player == PlayerId::P1 =>
            {
                break current
            }
            Decision::CastSpellOrPass { .. } => {
                engine::step(&mut state, Action::Pass).expect("priority pass before P1 Main2");
            }
            Decision::DeclareAttackers { eligible, .. } if eligible.is_empty() => {
                engine::step(&mut state, Action::DeclareAttackers(Vec::new()))
                    .expect("empty attacker declaration during setup");
            }
            Decision::Halted { .. } => panic!("tiny fixture halted during setup"),
            other => panic!("unexpected setup decision for tiny fixture: {other:?}"),
        }
    };
    assert_eq!(state.step, crate::state::Step::Main2);
    assert!(matches!(
        decision,
        Decision::CastSpellOrPass {
            player: PlayerId::P1,
            ..
        }
    ));
    (state, decision)
}

fn fixture_player(player: PlayerId) -> FixturePlayerV1 {
    match player {
        PlayerId::P0 => FixturePlayerV1::P0,
        PlayerId::P1 => FixturePlayerV1::P1,
        _ => panic!("invalid player id in two-player fixture: {player:?}"),
    }
}

/// Fixture-local action identities are explicit variant/field encodings; they
/// do not depend on `Debug`, a hash, or object-state equivalence.
fn stable_fixture_action_id(action: &Action) -> Result<String, String> {
    let id_list = |ids: &[ObjectId]| {
        ids.iter()
            .map(|id| id.0.to_string())
            .collect::<Vec<_>>()
            .join(",")
    };
    let color_id = |color: crate::mana::ManaColor| match color {
        crate::mana::ManaColor::W => "W",
        crate::mana::ManaColor::U => "U",
        crate::mana::ManaColor::B => "B",
        crate::mana::ManaColor::R => "R",
        crate::mana::ManaColor::G => "G",
        crate::mana::ManaColor::C => "C",
    };
    match action {
        Action::Pass => Ok("Pass".to_string()),
        Action::PlayLand(source) => Ok(format!("PlayLand:{}", source.0)),
        Action::CastSpell(source) => Ok(format!("CastSpell:{}", source.0)),
        Action::ActivateManaAbility(source) => Ok(format!("ActivateManaAbility:{}", source.0)),
        Action::ActivateManaAbilityChoice(source, color) => Ok(format!(
            "ActivateManaAbilityChoice:{}:{}",
            source.0,
            color_id(*color)
        )),
        Action::ActivateManaAbilityWithCostTarget(source, color, target) => Ok(format!(
            "ActivateManaAbilityWithCostTarget:{}:{}:{}",
            source.0,
            color_id(*color),
            target.0
        )),
        Action::ActivateAbility(source, ability) => {
            Ok(format!("ActivateAbility:{}:{}", source.0, ability))
        }
        Action::PlotSpell(source) => Ok(format!("PlotSpell:{}", source.0)),
        Action::DeclareAttackers(attackers) if attackers.is_empty() => {
            Ok(format!("DeclareAttackers:[{}]", id_list(attackers)))
        }
        _ => Err(format!(
            "UNSUPPORTED_DECISION: no fixture action identity for {action:?}"
        )),
    }
}

fn enumerate_tiny_graph() -> Result<EnumeratedEngineGraph, String> {
    #[allow(clippy::too_many_arguments)]
    fn visit(
        state: &GameState,
        decision: &Decision,
        root_player: PlayerId,
        path: &mut Vec<(GameState, Decision)>,
        next_id: &mut u32,
        transitions: &mut usize,
        edges_total: &mut usize,
        game_nodes_total: &mut usize,
        depth: usize,
        nodes: &mut Vec<OracleNodeV1>,
        state_nodes: &mut Vec<(GameState, Decision, FixtureNodeId)>,
    ) -> Result<FixtureNodeId, String> {
        if depth > MAX_SEARCH_DEPTH_V1 {
            return Err("ORACLE_FIXTURE_TOO_LARGE: search depth".to_string());
        }
        if path
            .iter()
            .any(|(prior_state, prior_decision)| prior_state == state && prior_decision == decision)
        {
            return Err("CYCLE_DETECTED".to_string());
        }
        let id = FixtureNodeId(*next_id);
        *next_id = next_id
            .checked_add(1)
            .ok_or_else(|| "ORACLE_FIXTURE_TOO_LARGE: node id overflow".to_string())?;
        state_nodes.push((state.clone(), decision.clone(), id));
        if let Decision::GameOver { winner } = decision {
            let outcome = match winner {
                Some(winner) if *winner == root_player => FixtureOutcomeV1::Win,
                Some(_) => FixtureOutcomeV1::Loss,
                None => FixtureOutcomeV1::Draw,
            };
            nodes.push(OracleNodeV1::Terminal { id, outcome });
            return Ok(id);
        }
        if let Decision::Halted { .. } = decision {
            return Err("UNSUPPORTED_DECISION: Halted is not an outcome".to_string());
        }

        let actor = match decision {
            Decision::CastSpellOrPass { player, .. }
            | Decision::DeclareAttackers { player, .. } => *player,
            other => return Err(format!("UNSUPPORTED_DECISION: {other:?}")),
        };
        let actions = match decision {
            Decision::CastSpellOrPass { .. } => complete_cast_or_pass_domain(decision, state)?,
            Decision::DeclareAttackers { eligible, .. } if eligible.is_empty() => {
                // The legal subset domain of an empty eligible list has exactly
                // one member: the empty declaration.
                vec![Action::DeclareAttackers(Vec::new())]
            }
            Decision::DeclareAttackers { .. } => {
                return Err("UNSUPPORTED_DECISION: nonempty attacker subset domain".to_string());
            }
            _ => unreachable!(),
        };
        if actions.is_empty() || actions.len() > MAX_COMPLETE_LEGAL_ACTIONS_PER_GAME_NODE_V1 {
            return Err("ORACLE_FIXTURE_TOO_LARGE: complete action domain".to_string());
        }
        *game_nodes_total += 1;
        if *game_nodes_total > MAX_UNIQUE_GAME_NODES_V1 {
            return Err("ORACLE_FIXTURE_TOO_LARGE: unique game nodes".to_string());
        }
        path.push((state.clone(), decision.clone()));
        let mut fixture_edges = Vec::with_capacity(actions.len());
        for (order, action) in actions.into_iter().enumerate() {
            *edges_total += 1;
            if *edges_total > MAX_TOTAL_EDGES_V1 {
                return Err("ORACLE_FIXTURE_TOO_LARGE: total edges".to_string());
            }
            let stable_id = stable_fixture_action_id(&action)?;
            if fixture_edges
                .iter()
                .any(|edge: &FixtureEdgeV1| edge.stable_id == stable_id)
            {
                return Err("INVALID_ORACLE_FIXTURE: duplicate semantic action".to_string());
            }
            let mut child_state = state.clone();
            engine::step(&mut child_state, action)
                .map_err(|error| format!("enumerated action rejected by engine::step: {error}"))?;
            *transitions += 1;
            if *transitions > MAX_AUTHORITATIVE_TRANSITIONS_V1 {
                return Err("ORACLE_FIXTURE_TOO_LARGE: authoritative transitions".to_string());
            }
            let child_decision = engine::advance_until_decision(&mut child_state);
            let child = visit(
                &child_state,
                &child_decision,
                root_player,
                path,
                next_id,
                transitions,
                edges_total,
                game_nodes_total,
                depth + 1,
                nodes,
                state_nodes,
            )?;
            fixture_edges.push(FixtureEdgeV1 {
                stable_id,
                order: u32::try_from(order)
                    .map_err(|_| "ORACLE_FIXTURE_TOO_LARGE: action order".to_string())?,
                child,
                estimated_cost_bucket: 1,
            });
        }
        path.pop();
        nodes.push(OracleNodeV1::GameDecision {
            id,
            actor: fixture_player(actor),
            actions: fixture_edges,
        });
        Ok(id)
    }

    let (state, decision) = tiny_engine_root();
    let root_player = match decision {
        Decision::CastSpellOrPass { player, .. } | Decision::DeclareAttackers { player, .. } => {
            player
        }
        Decision::GameOver { .. } => return Err("tiny fixture root is terminal".to_string()),
        _ => return Err(format!("UNSUPPORTED_DECISION at root: {decision:?}")),
    };
    let mut nodes = Vec::new();
    let mut state_nodes = Vec::new();
    let mut path = Vec::new();
    let mut next_id = 0;
    let mut transitions = 0;
    let mut edges_total = 0;
    let mut game_nodes_total = 0;
    let root = visit(
        &state,
        &decision,
        root_player,
        &mut path,
        &mut next_id,
        &mut transitions,
        &mut edges_total,
        &mut game_nodes_total,
        0,
        &mut nodes,
        &mut state_nodes,
    )?;
    Ok(EnumeratedEngineGraph {
        fixture: OracleFixtureV1 {
            fixture_id: "mads02b-engine-oracle-only-two-mountains-v1".to_string(),
            root,
            root_player: fixture_player(root_player),
            nodes,
        },
        explicit_step_transitions: transitions,
        state_nodes,
    })
}

/// `TEST_ONLY_ENGINE_TREE_HARNESS`: legal actions are admitted for each visited
/// decision, but successor states are created one engine::step at a time. This
/// is not the production MADS scheduler and performs no transposition reuse.
#[derive(Debug)]
struct TestOnlyEngineTreeNode {
    state: GameState,
    decision: Decision,
    actor: Option<PlayerId>,
    actions: Vec<Action>,
    children: Vec<Option<usize>>,
    parent: Option<usize>,
    depth: usize,
    bounds: BoundIntervalV1,
}

#[derive(Debug, Default)]
struct TestOnlyEngineTreeMetrics {
    authoritative_transitions: usize,
    state_clones: usize,
    expanded_nodes: usize,
    expanded_actions: usize,
    legal_action_slots_admitted: usize,
    bound_updates: usize,
}

#[derive(Debug)]
struct TestOnlyEngineTreeHarness {
    nodes: Vec<TestOnlyEngineTreeNode>,
    root: usize,
    root_player: PlayerId,
    metrics: TestOnlyEngineTreeMetrics,
}

impl TestOnlyEngineTreeHarness {
    fn new(state: GameState, decision: Decision) -> Result<Self, String> {
        let root_player = match decision {
            Decision::CastSpellOrPass { player, .. }
            | Decision::DeclareAttackers { player, .. } => player,
            _ => return Err("UNSUPPORTED_DECISION at dynamic root".to_string()),
        };
        let mut search = Self {
            nodes: Vec::new(),
            root: 0,
            root_player,
            metrics: TestOnlyEngineTreeMetrics::default(),
        };
        let root = search.admit_node(state, decision, None, 0)?;
        search.root = root;
        Ok(search)
    }

    fn admit_node(
        &mut self,
        state: GameState,
        decision: Decision,
        parent: Option<usize>,
        depth: usize,
    ) -> Result<usize, String> {
        if depth > MAX_SEARCH_DEPTH_V1 {
            return Err("ORACLE_FIXTURE_TOO_LARGE: dynamic search depth".to_string());
        }
        let (actor, actions, terminal_value) = match &decision {
            Decision::GameOver { winner } => (
                None,
                Vec::new(),
                Some(match winner {
                    Some(winner) if *winner == self.root_player => 1,
                    Some(_) => -1,
                    None => 0,
                }),
            ),
            Decision::Halted { .. } => {
                return Err("UNSUPPORTED_DECISION: Halted is not a terminal value".to_string());
            }
            Decision::CastSpellOrPass { player, .. } => (
                Some(*player),
                complete_cast_or_pass_domain(&decision, &state)?,
                None,
            ),
            Decision::DeclareAttackers { player, eligible } if eligible.is_empty() => (
                Some(*player),
                vec![Action::DeclareAttackers(Vec::new())],
                None,
            ),
            Decision::DeclareAttackers { .. } => {
                return Err("UNSUPPORTED_DECISION: nonempty attacker subset domain".to_string());
            }
            other => return Err(format!("UNSUPPORTED_DECISION: {other:?}")),
        };
        if terminal_value.is_none()
            && (actions.is_empty() || actions.len() > MAX_COMPLETE_LEGAL_ACTIONS_PER_GAME_NODE_V1)
        {
            return Err("ORACLE_FIXTURE_TOO_LARGE: complete dynamic action domain".to_string());
        }
        if self.nodes.len() >= MAX_UNIQUE_GAME_NODES_V1 {
            return Err("ORACLE_FIXTURE_TOO_LARGE: dynamic unique game nodes".to_string());
        }
        let bounds = terminal_value.map_or(BoundIntervalV1::UNKNOWN, BoundIntervalV1::exact);
        let index = self.nodes.len();
        if let Some(parent_index) = parent {
            let mut ancestor = Some(parent_index);
            while let Some(ancestor_index) = ancestor {
                let prior = &self.nodes[ancestor_index];
                if prior.state == state && prior.decision == decision {
                    return Err("CYCLE_DETECTED".to_string());
                }
                ancestor = prior.parent;
            }
        }
        self.metrics.legal_action_slots_admitted += actions.len();
        self.nodes.push(TestOnlyEngineTreeNode {
            state,
            decision,
            actor,
            children: vec![None; actions.len()],
            actions,
            parent,
            depth,
            bounds,
        });
        Ok(index)
    }

    fn expand_one(&mut self) -> Result<bool, String> {
        let slot = self
            .nodes
            .iter()
            .enumerate()
            .find_map(|(node_index, node)| {
                node.children
                    .iter()
                    .position(Option::is_none)
                    .map(|action_index| (node_index, action_index))
            });
        let Some((node_index, action_index)) = slot else {
            return Ok(false);
        };
        let (mut child_state, action, depth) = {
            let node = &self.nodes[node_index];
            (
                node.state.clone(),
                node.actions[action_index].clone(),
                node.depth + 1,
            )
        };
        self.metrics.state_clones += 1;
        engine::step(&mut child_state, action)
            .map_err(|error| format!("dynamic admitted action rejected: {error}"))?;
        self.metrics.authoritative_transitions += 1;
        self.metrics.expanded_actions += 1;
        let child_decision = engine::advance_until_decision(&mut child_state);
        let child = self.admit_node(child_state, child_decision, Some(node_index), depth)?;
        self.nodes[node_index].children[action_index] = Some(child);
        self.metrics.expanded_nodes += 1;
        self.recompute_bounds();
        Ok(true)
    }

    fn recompute_bounds(&mut self) {
        for index in (0..self.nodes.len()).rev() {
            let node = &self.nodes[index];
            let Some(actor) = node.actor else { continue };
            let role_max = actor == self.root_player;
            let complete = node.children.iter().all(Option::is_some);
            let children = node
                .children
                .iter()
                .flatten()
                .map(|child| self.nodes[*child].bounds)
                .collect::<Vec<_>>();
            let next = if role_max {
                BoundIntervalV1 {
                    lower: children.iter().map(|bound| bound.lower).max().unwrap_or(-1),
                    upper: if complete {
                        children.iter().map(|bound| bound.upper).max().unwrap_or(-1)
                    } else {
                        1
                    },
                }
            } else {
                BoundIntervalV1 {
                    lower: if complete {
                        children.iter().map(|bound| bound.lower).min().unwrap_or(1)
                    } else {
                        -1
                    },
                    upper: children.iter().map(|bound| bound.upper).min().unwrap_or(1),
                }
            };
            if self.nodes[index].bounds != next {
                self.nodes[index].bounds = next;
                self.metrics.bound_updates += 1;
            }
        }
    }

    fn root_certified_actions(&self) -> Vec<String> {
        let root = &self.nodes[self.root];
        root.actions
            .iter()
            .enumerate()
            .filter_map(|(index, action)| {
                let bounds = root.children[index]
                    .map(|child| self.nodes[child].bounds)
                    .unwrap_or(BoundIntervalV1::UNKNOWN);
                let best_other_upper = root
                    .actions
                    .iter()
                    .enumerate()
                    .filter(|(other, _)| *other != index)
                    .map(|(other, _)| {
                        root.children[other]
                            .map(|child| self.nodes[child].bounds.upper)
                            .unwrap_or(1)
                    })
                    .max();
                best_other_upper
                    .is_none_or(|upper| bounds.lower >= upper)
                    .then(|| format!("{action:?}"))
            })
            .collect()
    }
}

#[test]
fn bounded_engine_graph_matches_independent_oracle_and_mads_bounds() {
    let enumerated = enumerate_tiny_graph().expect("small graph fully enumerates");
    let fixture = &enumerated.fixture;
    fixture.validate_structure().unwrap();
    fixture.validate_acyclic_v1().unwrap();
    let oracle = fixture.solve_oracle_v1().unwrap();
    assert_eq!(fixture.root_player, FixturePlayerV1::P1);
    assert_eq!(oracle.unique_game_nodes, 295);
    assert_eq!(oracle.total_edges, 331);
    assert_eq!(oracle.terminal_nodes, 37);
    assert_eq!(oracle.terminal_classification.loss, 0);
    assert_eq!(oracle.terminal_classification.draw, 0);
    assert_eq!(oracle.terminal_classification.win, 37);
    assert_eq!(
        oracle.root_value, 1,
        "value must be relative to root player P1"
    );
    assert_eq!(oracle.complete_legal_root_actions, ["Pass"]);
    for terminal in &oracle.terminal_classification.terminals {
        let (_, decision, node_id) = enumerated
            .state_nodes
            .iter()
            .find(|(_, _, node_id)| *node_id == terminal.node)
            .expect("every oracle terminal originates at an enumerated engine node");
        assert_eq!(*node_id, terminal.node);
        assert!(matches!(
            decision,
            Decision::GameOver {
                winner: Some(PlayerId::P1)
            }
        ));
    }
    assert_eq!(
        oracle.authoritative_transitions, 0,
        "OracleSuite itself is engine-independent"
    );

    let mut mads = MadsGraphV1::new(fixture).unwrap();
    loop {
        let expanded = mads.expand_next_v1().unwrap();
        for node in mads.nodes() {
            let value = oracle
                .node_values
                .get(&node.fixture_key())
                .expect("every created MADS node belongs to the solved oracle fixture");
            let bounds = node.bounds();
            assert!(
                bounds.lower <= *value && *value <= bounds.upper,
                "unsound interval {:?} for fixture node {:?} with oracle value {value}",
                bounds,
                node.fixture_key()
            );
        }
        if expanded.is_none() {
            break;
        }
    }
    let result = mads.result_v1();
    assert_eq!(result.status, CertificationStatusV1::Certified);
    assert_eq!(
        result
            .root_actions
            .iter()
            .map(|action| action.stable_id.clone())
            .collect::<Vec<_>>(),
        oracle.complete_legal_root_actions,
        "MADS must retain every complete root action"
    );
    assert_eq!(
        result.certified_optimal_actions,
        oracle.optimal_root_actions
    );
    assert!(result
        .chosen_action
        .as_ref()
        .is_some_and(|chosen| oracle.optimal_root_actions.contains(chosen)));
    assert_eq!(result.root_bounds.lower, oracle.root_value);
    assert_eq!(result.root_bounds.upper, oracle.root_value);
    assert_eq!(
        result.metrics.valid_tt_hits, 0,
        "fixture tree uses no transpositions"
    );
    eprintln!(
        "BOUNDED_ENGINE_ORACLE fixture={} root_actions={:?} oracle_value={} nodes={} edges={} transitions={} terminal_loss={} terminal_draw={} terminal_win={} mads_nodes={} mads_expanded_actions={} certified={:?}",
        fixture.fixture_id,
        oracle.complete_legal_root_actions,
        oracle.root_value,
        oracle.unique_game_nodes,
        oracle.total_edges,
        enumerated.explicit_step_transitions,
        oracle.terminal_classification.loss,
        oracle.terminal_classification.draw,
        oracle.terminal_classification.win,
        result.metrics.graph_nodes_created,
        result.metrics.expanded_actions,
        result.certified_optimal_actions,
    );

    // Transition count records explicit accepted engine::step calls at edges;
    // automatic phase work performed inside advance_until_decision is not a step.
    assert_eq!(enumerated.explicit_step_transitions, oracle.total_edges);
}

#[test]
fn dynamic_tree_expands_one_successor_at_a_time_and_matches_bounded_oracle() {
    let enumerated = enumerate_tiny_graph().expect("small graph fully enumerates");
    let oracle = enumerated.fixture.solve_oracle_v1().unwrap();
    let (root_state, root_decision) = tiny_engine_root();
    let original_state = root_state.clone();
    let zero_budget =
        TestOnlyEngineTreeHarness::new(root_state.clone(), root_decision.clone()).unwrap();
    assert_eq!(zero_budget.metrics.authoritative_transitions, 0);
    assert_eq!(
        zero_budget.nodes[zero_budget.root].bounds,
        BoundIntervalV1::UNKNOWN
    );
    let mut one_expansion =
        TestOnlyEngineTreeHarness::new(root_state.clone(), root_decision.clone()).unwrap();
    assert!(one_expansion.expand_one().unwrap());
    assert_eq!(one_expansion.metrics.authoritative_transitions, 1);
    assert_eq!(
        one_expansion.nodes[one_expansion.root].bounds,
        BoundIntervalV1::UNKNOWN,
        "one unresolved child cannot become a terminal value"
    );

    let mut dynamic = TestOnlyEngineTreeHarness::new(root_state.clone(), root_decision).unwrap();
    while dynamic.expand_one().unwrap() {
        for node in &dynamic.nodes {
            let matching_values = enumerated
                .state_nodes
                .iter()
                .filter(|(state, decision, _)| state == &node.state && decision == &node.decision)
                .filter_map(|(_, _, id)| oracle.node_values.get(id).copied())
                .collect::<Vec<_>>();
            assert!(
                !matching_values.is_empty(),
                "dynamic node absent from full oracle walk"
            );
            assert!(
                matching_values
                    .iter()
                    .all(|value| *value == matching_values[0]),
                "identical exact state/decision had path-dependent oracle values"
            );
            assert!(
                node.bounds.lower <= matching_values[0] && matching_values[0] <= node.bounds.upper,
                "dynamic interval {:?} excludes oracle value {}",
                node.bounds,
                matching_values[0]
            );
        }
    }
    let root = &dynamic.nodes[dynamic.root];
    assert_eq!(root.bounds.lower, oracle.root_value);
    assert_eq!(root.bounds.upper, oracle.root_value);
    assert_eq!(
        dynamic.root_certified_actions(),
        oracle.optimal_root_actions
    );
    assert_eq!(
        dynamic.metrics.authoritative_transitions,
        oracle.total_edges
    );
    assert_eq!(dynamic.metrics.expanded_actions, oracle.total_edges);
    assert_eq!(
        root_state, original_state,
        "branching mutated the caller's root state"
    );
    eprintln!(
        "TEST_ONLY_ENGINE_TREE_HARNESS nodes={} transitions={} clones={} expanded_actions={} admitted_actions={} bound_updates={} root_bounds={:?} certified={:?}",
        dynamic.nodes.len(),
        dynamic.metrics.authoritative_transitions,
        dynamic.metrics.state_clones,
        dynamic.metrics.expanded_actions,
        dynamic.metrics.legal_action_slots_admitted,
        dynamic.metrics.bound_updates,
        root.bounds,
        dynamic.root_certified_actions(),
    );
}

#[test]
fn production_dynamic_search_v1_matches_bounded_oracle_and_obeys_budget() {
    let enumerated = enumerate_tiny_graph().expect("bounded fixture enumerates");
    let oracle = enumerated.fixture.solve_oracle_v1().unwrap();
    let (root_state, root_decision) = tiny_engine_root();
    let original = root_state.clone();
    let mut search = DynamicEngineSearchV1::new(&root_state, root_decision).unwrap();
    let zero = search.run_v1(0);
    assert_eq!(zero.metrics.authoritative_transitions, 0);
    assert_eq!(
        zero.metrics.state_clones, 1,
        "the authoritative root is cloned once"
    );
    assert_eq!(zero.root_bounds, BoundIntervalV1::UNKNOWN);
    assert!(zero.root_actions.iter().all(|a| !a.expanded));
    assert_eq!(root_state, original);
    for _ in 0..oracle.total_edges {
        let before = search.run_v1(0).metrics.authoritative_transitions;
        let result = search.run_v1(1);
        assert_ne!(result.status, DynamicSearchStatusV1::UnsupportedDecision);
        for (state, decision, bounds) in search.debug_nodes_v1() {
            let values = enumerated
                .state_nodes
                .iter()
                .filter(|(s, d, _)| s == &state && d == &decision)
                .filter_map(|(_, _, id)| oracle.node_values.get(id).copied())
                .collect::<Vec<_>>();
            assert!(!values.is_empty());
            assert!(
                values
                    .iter()
                    .all(|v| bounds.lower <= *v && *v <= bounds.upper),
                "MADS bound {:?} excludes oracle value {:?}",
                bounds,
                values
            );
        }
        if result.metrics.authoritative_transitions == before {
            break;
        }
    }
    let result = search.run_v1(0);
    assert_eq!(result.status, DynamicSearchStatusV1::Certified);
    assert!(result
        .exact_root_value
        .is_none_or(|value| value == oracle.root_value));
    assert_eq!(
        result.certified_optimal_actions,
        oracle.optimal_root_actions
    );
    assert!(result
        .chosen_action
        .as_ref()
        .is_some_and(|a| oracle.optimal_root_actions.contains(a)));
    assert!(result.metrics.authoritative_transitions as usize <= oracle.total_edges);
    assert_eq!(root_state, original);
}

#[test]
fn halted_is_rejected_instead_of_becoming_an_oracle_outcome() {
    let (state, decision) = tiny_engine_root();
    assert!(DynamicEngineSearchV1::new(
        &state,
        Decision::Halted {
            mechanic: engine::UnsupportedMechanic::InvalidEffectContinuation,
            source: ObjectId(0),
        },
    )
    .is_err());
    let mut dynamic = TestOnlyEngineTreeHarness::new(state.clone(), decision).unwrap();
    let prior_node_count = dynamic.nodes.len();
    let error = dynamic
        .admit_node(
            state,
            Decision::Halted {
                mechanic: engine::UnsupportedMechanic::InvalidEffectContinuation,
                source: ObjectId(0),
            },
            Some(dynamic.root),
            1,
        )
        .unwrap_err();
    assert!(error.contains("Halted"));
    assert_eq!(dynamic.nodes.len(), prior_node_count);
    assert_eq!(dynamic.metrics.authoritative_transitions, 0);
}

#[test]
fn unsupported_decisions_fail_closed_and_incomplete_or_overlimit_fixtures_are_rejected() {
    let (state, decision) = tiny_engine_root();
    let mut harness = TestOnlyEngineTreeHarness::new(state.clone(), decision).unwrap();
    let nodes_before = harness.nodes.len();
    let unsupported = harness
        .admit_node(
            state,
            Decision::DeclareBlockers {
                player: PlayerId::P1,
                attackers: Vec::new(),
                legal_blockers: Vec::new(),
            },
            Some(harness.root),
            1,
        )
        .unwrap_err();
    assert!(unsupported.starts_with("UNSUPPORTED_DECISION"));
    assert_eq!(harness.nodes.len(), nodes_before);
    assert_eq!(harness.metrics.authoritative_transitions, 0);

    let enumerated = enumerate_tiny_graph().unwrap();
    let mut incomplete = enumerated.fixture.clone();
    let terminal = incomplete
        .nodes
        .iter()
        .find_map(|node| matches!(node, OracleNodeV1::Terminal { .. }).then_some(node.id()))
        .expect("fixture has at least one actual terminal");
    incomplete.nodes.retain(|node| node.id() != terminal);
    assert!(incomplete.solve_oracle_v1().is_err());
    assert!(MadsGraphV1::new(&incomplete).is_err());

    let mut oversized = enumerated.fixture;
    let root_id = oversized.root;
    let root = oversized
        .nodes
        .iter_mut()
        .find(|node| node.id() == root_id)
        .unwrap();
    let OracleNodeV1::GameDecision { actions, .. } = root else {
        panic!("fixture root must be a game decision")
    };
    let child = actions[0].child;
    *actions = (0..=MAX_COMPLETE_LEGAL_ACTIONS_PER_GAME_NODE_V1)
        .map(|order| FixtureEdgeV1 {
            stable_id: format!("over-limit-action-{order}"),
            order: u32::try_from(order).unwrap(),
            child,
            estimated_cost_bucket: 1,
        })
        .collect();
    assert!(matches!(
        oversized.solve_oracle_v1(),
        Err(crate::oracle_suite_v1::OracleErrorV1::FixtureTooLarge)
    ));
    assert!(MadsGraphV1::new(&oversized).is_err());
}
