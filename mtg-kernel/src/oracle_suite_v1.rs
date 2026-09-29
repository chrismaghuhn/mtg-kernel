//! Bounded, fully enumerating minimax oracle for isolated deterministic DAG fixtures.
//!
//! This module does not call the MTG engine. `FixtureNodeId` is an exact semantic
//! identity supplied by a fixture author; reusing an ID means the fixture asserts
//! that both paths reach precisely the same node. Results are not live-Magic proofs.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

pub const MAX_COMPLETE_LEGAL_ACTIONS_PER_GAME_NODE_V1: usize = 64;
pub const MAX_CONTINUATIONS_PER_CONSTRUCTION_NODE_V1: usize = 32;
pub const MAX_SEARCH_DEPTH_V1: usize = 32;
pub const MAX_UNIQUE_GAME_NODES_V1: usize = 100_000;
pub const MAX_TOTAL_EDGES_V1: usize = 500_000;
pub const MAX_AUTHORITATIVE_TRANSITIONS_V1: usize = 2_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FixtureNodeId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FixturePlayerV1 {
    P0,
    P1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FixtureOutcomeV1 {
    Loss,
    Draw,
    Win,
}

impl FixtureOutcomeV1 {
    pub const fn value(self) -> i8 {
        match self {
            Self::Loss => -1,
            Self::Draw => 0,
            Self::Win => 1,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixtureEdgeV1 {
    /// Stable, explicit semantic action/construction-choice identity.
    pub stable_id: String,
    /// Position in the complete legal decision order. Must be unique per node.
    pub order: u32,
    pub child: FixtureNodeId,
    /// Fixture-provided integer estimate; used only by deterministic scheduling.
    pub estimated_cost_bucket: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OracleNodeV1 {
    GameDecision {
        id: FixtureNodeId,
        actor: FixturePlayerV1,
        actions: Vec<FixtureEdgeV1>,
    },
    DecisionConstruction {
        id: FixtureNodeId,
        owner_decision: FixtureNodeId,
        actor: FixturePlayerV1,
        /// Distinguishes staged protocol contexts even when the physical board
        /// has not changed.
        protocol_key: String,
        /// Exact already-selected response prefix owned by this construction.
        partial_response: String,
        /// Cursor in the typed staged-decision protocol.
        continuation_cursor: u32,
        choices: Vec<FixtureEdgeV1>,
    },
    Terminal {
        id: FixtureNodeId,
        outcome: FixtureOutcomeV1,
    },
}

impl OracleNodeV1 {
    pub const fn id(&self) -> FixtureNodeId {
        match self {
            Self::GameDecision { id, .. }
            | Self::DecisionConstruction { id, .. }
            | Self::Terminal { id, .. } => *id,
        }
    }

    pub const fn actor(&self) -> Option<FixturePlayerV1> {
        match self {
            Self::GameDecision { actor, .. } | Self::DecisionConstruction { actor, .. } => {
                Some(*actor)
            }
            Self::Terminal { .. } => None,
        }
    }

    pub fn edges(&self) -> &[FixtureEdgeV1] {
        match self {
            Self::GameDecision { actions, .. } => actions,
            Self::DecisionConstruction { choices, .. } => choices,
            Self::Terminal { .. } => &[],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OracleFixtureV1 {
    pub fixture_id: String,
    pub root: FixtureNodeId,
    pub root_player: FixturePlayerV1,
    pub nodes: Vec<OracleNodeV1>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OracleErrorV1 {
    FixtureTooLarge,
    CycleDetected,
    InvalidFixture(String),
}

impl fmt::Display for OracleErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FixtureTooLarge => f.write_str("ORACLE_FIXTURE_TOO_LARGE"),
            Self::CycleDetected => f.write_str("CYCLE_DETECTED"),
            Self::InvalidFixture(message) => write!(f, "INVALID_ORACLE_FIXTURE: {message}"),
        }
    }
}

impl std::error::Error for OracleErrorV1 {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OracleTerminalRecordV1 {
    pub node: FixtureNodeId,
    pub outcome: FixtureOutcomeV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OracleDecisionKindV1 {
    GameDecision,
    DecisionConstruction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OracleLegalDecisionSetV1 {
    pub node: FixtureNodeId,
    pub actor: FixturePlayerV1,
    pub kind: OracleDecisionKindV1,
    pub stable_choices: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct OracleTerminalClassificationV1 {
    pub loss: usize,
    pub draw: usize,
    pub win: usize,
    pub terminals: Vec<OracleTerminalRecordV1>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OracleResultV1 {
    pub root_value: i8,
    pub optimal_root_actions: Vec<String>,
    pub complete_legal_root_actions: Vec<String>,
    pub complete_legal_decisions: Vec<OracleLegalDecisionSetV1>,
    pub unique_game_nodes: usize,
    pub construction_nodes: usize,
    pub terminal_nodes: usize,
    pub total_edges: usize,
    /// Zero for these isolated fixtures. A future explicit engine oracle adapter
    /// must count actual `engine::step` transitions separately.
    pub authoritative_transitions: usize,
    pub terminal_classification: OracleTerminalClassificationV1,
    /// Exact minimax value for every reachable fixture node, useful for checking
    /// MADS intervals after each selective expansion.
    pub node_values: BTreeMap<FixtureNodeId, i8>,
}

impl OracleFixtureV1 {
    pub fn node(&self, id: FixtureNodeId) -> Option<&OracleNodeV1> {
        self.nodes.iter().find(|node| node.id() == id)
    }

    fn indexed_nodes(&self) -> Result<BTreeMap<FixtureNodeId, &OracleNodeV1>, OracleErrorV1> {
        let mut indexed = BTreeMap::new();
        for node in &self.nodes {
            if indexed.insert(node.id(), node).is_some() {
                return Err(OracleErrorV1::InvalidFixture(format!(
                    "duplicate FixtureNodeId {}",
                    node.id().0
                )));
            }
            match node {
                OracleNodeV1::GameDecision { actions, .. } => {
                    if actions.is_empty() {
                        return Err(OracleErrorV1::InvalidFixture(format!(
                            "nonterminal game node {} has no legal actions",
                            node.id().0
                        )));
                    }
                    if actions.len() > MAX_COMPLETE_LEGAL_ACTIONS_PER_GAME_NODE_V1 {
                        return Err(OracleErrorV1::FixtureTooLarge);
                    }
                    validate_edges(actions, node.id())?;
                }
                OracleNodeV1::DecisionConstruction {
                    protocol_key,
                    choices,
                    ..
                } => {
                    if protocol_key.is_empty() || choices.is_empty() {
                        return Err(OracleErrorV1::InvalidFixture(format!(
                            "construction node {} needs a key and at least one continuation",
                            node.id().0
                        )));
                    }
                    if choices.len() > MAX_CONTINUATIONS_PER_CONSTRUCTION_NODE_V1 {
                        return Err(OracleErrorV1::FixtureTooLarge);
                    }
                    validate_edges(choices, node.id())?;
                }
                OracleNodeV1::Terminal { .. } => {}
            }
        }
        let game_nodes = indexed
            .values()
            .filter(|node| matches!(node, OracleNodeV1::GameDecision { .. }))
            .count();
        if game_nodes > MAX_UNIQUE_GAME_NODES_V1 {
            return Err(OracleErrorV1::FixtureTooLarge);
        }
        if !indexed.contains_key(&self.root) {
            return Err(OracleErrorV1::InvalidFixture(
                "root node is absent".to_owned(),
            ));
        }
        let total_edges = indexed
            .values()
            .map(|node| node.edges().len())
            .sum::<usize>();
        if total_edges > MAX_TOTAL_EDGES_V1 {
            return Err(OracleErrorV1::FixtureTooLarge);
        }
        if indexed.values().any(|node| {
            node.edges()
                .iter()
                .any(|edge| !indexed.contains_key(&edge.child))
        }) {
            return Err(OracleErrorV1::InvalidFixture(
                "edge references an absent child".to_owned(),
            ));
        }
        for node in indexed.values() {
            if let OracleNodeV1::DecisionConstruction {
                id,
                owner_decision,
                actor,
                ..
            } = node
            {
                let Some(OracleNodeV1::GameDecision {
                    actor: owner_actor, ..
                }) = indexed.get(owner_decision).copied()
                else {
                    return Err(OracleErrorV1::InvalidFixture(format!(
                        "construction node {} has no owning game decision",
                        id.0
                    )));
                };
                if actor != owner_actor {
                    return Err(OracleErrorV1::InvalidFixture(format!(
                        "construction node {} changed its authoritative chooser",
                        id.0
                    )));
                }
            }
        }
        Ok(indexed)
    }

    /// Structural/cap checks without DFS. MADS performs these checks before it
    /// builds nodes; the oracle additionally verifies acyclicity and depth.
    pub fn validate_structure(&self) -> Result<(), OracleErrorV1> {
        self.indexed_nodes().map(|_| ())
    }

    pub fn validate_acyclic_v1(&self) -> Result<(), OracleErrorV1> {
        let indexed = self.indexed_nodes()?;
        let mut colors = BTreeMap::<FixtureNodeId, u8>::new();
        let mut longest = BTreeMap::<FixtureNodeId, usize>::new();
        visit_shape(self.root, 0, &indexed, &mut colors, &mut longest)?;
        if indexed.keys().any(|id| !colors.contains_key(id)) {
            return Err(OracleErrorV1::InvalidFixture("unreachable node".to_owned()));
        }
        Ok(())
    }

    pub fn solve_oracle_v1(&self) -> Result<OracleResultV1, OracleErrorV1> {
        let indexed = self.indexed_nodes()?;
        if !matches!(indexed[&self.root], OracleNodeV1::GameDecision { actor, .. } if *actor == self.root_player)
        {
            return Err(OracleErrorV1::InvalidFixture(
                "root must be a game decision whose actor is the root player".to_owned(),
            ));
        }
        let mut walk = OracleSolveWalk::new(&indexed, self.root_player);
        let max_depth = walk.visit(self.root, 0)?;
        if max_depth > MAX_SEARCH_DEPTH_V1 {
            return Err(OracleErrorV1::FixtureTooLarge);
        }
        if indexed.keys().any(|id| !walk.colors.contains_key(id)) {
            return Err(OracleErrorV1::InvalidFixture(
                "fixture contains unreachable nodes".to_owned(),
            ));
        }
        let reachable_game_nodes = indexed
            .values()
            .filter(|node| matches!(node, OracleNodeV1::GameDecision { .. }))
            .count();
        if reachable_game_nodes > MAX_UNIQUE_GAME_NODES_V1 {
            return Err(OracleErrorV1::FixtureTooLarge);
        }
        let root_value = walk.values[&self.root];
        let root = indexed[&self.root];
        let complete_legal_root_actions = root
            .edges()
            .iter()
            .map(|edge| edge.stable_id.clone())
            .collect::<Vec<_>>();
        let complete_legal_decisions = indexed
            .values()
            .filter_map(|node| match node {
                OracleNodeV1::GameDecision { id, actor, actions } => {
                    Some(OracleLegalDecisionSetV1 {
                        node: *id,
                        actor: *actor,
                        kind: OracleDecisionKindV1::GameDecision,
                        stable_choices: actions.iter().map(|edge| edge.stable_id.clone()).collect(),
                    })
                }
                OracleNodeV1::DecisionConstruction {
                    id, actor, choices, ..
                } => Some(OracleLegalDecisionSetV1 {
                    node: *id,
                    actor: *actor,
                    kind: OracleDecisionKindV1::DecisionConstruction,
                    stable_choices: choices.iter().map(|edge| edge.stable_id.clone()).collect(),
                }),
                OracleNodeV1::Terminal { .. } => None,
            })
            .collect();
        let optimal_root_actions = root
            .edges()
            .iter()
            .filter(|edge| walk.values[&edge.child] == root_value)
            .map(|edge| edge.stable_id.clone())
            .collect();
        walk.terminals.terminals.sort_by_key(|record| record.node);
        Ok(OracleResultV1 {
            root_value,
            optimal_root_actions,
            complete_legal_root_actions,
            complete_legal_decisions,
            unique_game_nodes: reachable_game_nodes,
            construction_nodes: indexed
                .values()
                .filter(|node| matches!(node, OracleNodeV1::DecisionConstruction { .. }))
                .count(),
            terminal_nodes: walk.terminals.terminals.len(),
            total_edges: indexed.values().map(|node| node.edges().len()).sum(),
            authoritative_transitions: 0,
            terminal_classification: walk.terminals,
            node_values: walk.values,
        })
    }
}

fn visit_shape(
    id: FixtureNodeId,
    depth: usize,
    indexed: &BTreeMap<FixtureNodeId, &OracleNodeV1>,
    colors: &mut BTreeMap<FixtureNodeId, u8>,
    longest: &mut BTreeMap<FixtureNodeId, usize>,
) -> Result<usize, OracleErrorV1> {
    match colors.get(&id).copied() {
        Some(1) => return Err(OracleErrorV1::CycleDetected),
        Some(2) => {
            let suffix = longest[&id];
            return if depth.saturating_add(suffix) > MAX_SEARCH_DEPTH_V1 {
                Err(OracleErrorV1::FixtureTooLarge)
            } else {
                Ok(suffix)
            };
        }
        _ => {}
    }
    if depth > MAX_SEARCH_DEPTH_V1 {
        return Err(OracleErrorV1::FixtureTooLarge);
    }
    colors.insert(id, 1);
    let mut result = 0;
    for edge in indexed[&id].edges() {
        result = result.max(1 + visit_shape(edge.child, depth + 1, indexed, colors, longest)?);
    }
    colors.insert(id, 2);
    longest.insert(id, result);
    Ok(result)
}

/// Admission check for a future explicitly instrumented authoritative adapter.
/// Synthetic fixtures perform zero authoritative transitions; callers must pass
/// an observed count rather than infer it from the fixture edge count.
pub fn check_authoritative_transition_budget_v1(transitions: usize) -> Result<(), OracleErrorV1> {
    if transitions > MAX_AUTHORITATIVE_TRANSITIONS_V1 {
        Err(OracleErrorV1::FixtureTooLarge)
    } else {
        Ok(())
    }
}

fn validate_edges(edges: &[FixtureEdgeV1], owner: FixtureNodeId) -> Result<(), OracleErrorV1> {
    let mut orders = BTreeSet::new();
    let mut ids = BTreeSet::new();
    for edge in edges {
        if edge.stable_id.is_empty()
            || !orders.insert(edge.order)
            || !ids.insert(edge.stable_id.as_str())
        {
            return Err(OracleErrorV1::InvalidFixture(format!(
                "node {} has an empty or duplicate action identity/order",
                owner.0
            )));
        }
    }
    if edges.windows(2).any(|pair| pair[0].order >= pair[1].order) {
        return Err(OracleErrorV1::InvalidFixture(format!(
            "node {} actions are not in stable semantic order",
            owner.0
        )));
    }
    Ok(())
}

struct OracleSolveWalk<'a, 'node> {
    indexed: &'a BTreeMap<FixtureNodeId, &'node OracleNodeV1>,
    root_player: FixturePlayerV1,
    colors: BTreeMap<FixtureNodeId, u8>,
    values: BTreeMap<FixtureNodeId, i8>,
    max_depth_from: BTreeMap<FixtureNodeId, usize>,
    terminals: OracleTerminalClassificationV1,
}

impl<'a, 'node> OracleSolveWalk<'a, 'node> {
    fn new(
        indexed: &'a BTreeMap<FixtureNodeId, &'node OracleNodeV1>,
        root_player: FixturePlayerV1,
    ) -> Self {
        Self {
            indexed,
            root_player,
            colors: BTreeMap::new(),
            values: BTreeMap::new(),
            max_depth_from: BTreeMap::new(),
            terminals: OracleTerminalClassificationV1::default(),
        }
    }

    fn visit(&mut self, id: FixtureNodeId, depth: usize) -> Result<usize, OracleErrorV1> {
        match self.colors.get(&id).copied() {
            Some(1) => return Err(OracleErrorV1::CycleDetected),
            Some(2) => {
                let longest = self.max_depth_from[&id];
                if depth.saturating_add(longest) > MAX_SEARCH_DEPTH_V1 {
                    return Err(OracleErrorV1::FixtureTooLarge);
                }
                return Ok(longest);
            }
            _ => {}
        }
        if depth > MAX_SEARCH_DEPTH_V1 {
            return Err(OracleErrorV1::FixtureTooLarge);
        }
        self.colors.insert(id, 1);
        let node = self
            .indexed
            .get(&id)
            .copied()
            .ok_or_else(|| OracleErrorV1::InvalidFixture(format!("node {} absent", id.0)))?;
        let (value, longest_child) = match node {
            OracleNodeV1::Terminal { outcome, .. } => {
                match outcome {
                    FixtureOutcomeV1::Loss => self.terminals.loss += 1,
                    FixtureOutcomeV1::Draw => self.terminals.draw += 1,
                    FixtureOutcomeV1::Win => self.terminals.win += 1,
                }
                self.terminals.terminals.push(OracleTerminalRecordV1 {
                    node: id,
                    outcome: *outcome,
                });
                (outcome.value(), 0)
            }
            OracleNodeV1::GameDecision { actor, actions, .. }
            | OracleNodeV1::DecisionConstruction {
                actor,
                choices: actions,
                ..
            } => {
                let mut child_values = Vec::with_capacity(actions.len());
                let mut longest = 0usize;
                for edge in actions {
                    let child_depth = self.visit(edge.child, depth + 1)?;
                    longest = longest.max(1 + child_depth);
                    child_values.push(self.values[&edge.child]);
                }
                let maximizing = *actor == self.root_player;
                let value = if maximizing {
                    *child_values
                        .iter()
                        .max()
                        .expect("validated nonempty actions")
                } else {
                    *child_values
                        .iter()
                        .min()
                        .expect("validated nonempty actions")
                };
                (value, longest)
            }
        };
        self.colors.insert(id, 2);
        self.values.insert(id, value);
        self.max_depth_from.insert(id, longest_child);
        Ok(longest_child)
    }
}

impl fmt::Display for FixtureNodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "fixture-node-{}", self.0)
    }
}

/// Shared synthetic control graph used by the independent oracle and MADS
/// differential tests. This graph is not an engine-derived Magic position.
#[cfg(test)]
pub(crate) fn adversarial_control_fixture_v1() -> OracleFixtureV1 {
    let edge = |order, stable_id: &str, child| FixtureEdgeV1 {
        stable_id: stable_id.to_owned(),
        order,
        child: FixtureNodeId(child),
        estimated_cost_bucket: 0,
    };
    OracleFixtureV1 {
        fixture_id: "mads02f-synthetic-adversarial-control-v1".to_owned(),
        root: FixtureNodeId(0),
        root_player: FixturePlayerV1::P0,
        nodes: vec![
            OracleNodeV1::GameDecision {
                id: FixtureNodeId(0),
                actor: FixturePlayerV1::P0,
                actions: vec![
                    edge(0, "safe-a", 1),
                    edge(1, "safe-b", 2),
                    edge(2, "risky", 3),
                ],
            },
            OracleNodeV1::GameDecision {
                id: FixtureNodeId(1),
                actor: FixturePlayerV1::P1,
                actions: vec![edge(0, "allow-win", 4), edge(1, "force-draw", 5)],
            },
            OracleNodeV1::GameDecision {
                id: FixtureNodeId(2),
                actor: FixturePlayerV1::P1,
                actions: vec![edge(0, "allow-win-too", 6), edge(1, "force-draw-too", 7)],
            },
            OracleNodeV1::GameDecision {
                id: FixtureNodeId(3),
                actor: FixturePlayerV1::P1,
                actions: vec![edge(0, "punish", 8), edge(1, "spare", 9)],
            },
            OracleNodeV1::Terminal {
                id: FixtureNodeId(4),
                outcome: FixtureOutcomeV1::Win,
            },
            OracleNodeV1::Terminal {
                id: FixtureNodeId(5),
                outcome: FixtureOutcomeV1::Draw,
            },
            OracleNodeV1::Terminal {
                id: FixtureNodeId(6),
                outcome: FixtureOutcomeV1::Win,
            },
            OracleNodeV1::Terminal {
                id: FixtureNodeId(7),
                outcome: FixtureOutcomeV1::Draw,
            },
            OracleNodeV1::Terminal {
                id: FixtureNodeId(8),
                outcome: FixtureOutcomeV1::Loss,
            },
            OracleNodeV1::Terminal {
                id: FixtureNodeId(9),
                outcome: FixtureOutcomeV1::Draw,
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edge(order: u32, name: &str, child: u32) -> FixtureEdgeV1 {
        FixtureEdgeV1 {
            stable_id: name.to_owned(),
            order,
            child: FixtureNodeId(child),
            estimated_cost_bucket: 0,
        }
    }

    fn fixture(nodes: Vec<OracleNodeV1>) -> OracleFixtureV1 {
        OracleFixtureV1 {
            fixture_id: "oracle-test".to_owned(),
            root: FixtureNodeId(0),
            root_player: FixturePlayerV1::P0,
            nodes,
        }
    }

    #[test]
    fn oracle_solves_max_and_min_and_reports_all_optimal_actions() {
        let f = fixture(vec![
            OracleNodeV1::GameDecision {
                id: FixtureNodeId(0),
                actor: FixturePlayerV1::P0,
                actions: vec![edge(0, "loss", 1), edge(1, "win-a", 2), edge(2, "win-b", 3)],
            },
            OracleNodeV1::Terminal {
                id: FixtureNodeId(1),
                outcome: FixtureOutcomeV1::Loss,
            },
            OracleNodeV1::Terminal {
                id: FixtureNodeId(2),
                outcome: FixtureOutcomeV1::Win,
            },
            OracleNodeV1::Terminal {
                id: FixtureNodeId(3),
                outcome: FixtureOutcomeV1::Win,
            },
        ]);
        let result = f.solve_oracle_v1().unwrap();
        assert_eq!(result.root_value, 1);
        assert_eq!(result.optimal_root_actions, vec!["win-a", "win-b"]);
        assert_eq!(result.complete_legal_root_actions.len(), 3);
        assert_eq!(result.complete_legal_decisions.len(), 1);
        assert_eq!(result.complete_legal_decisions[0].stable_choices.len(), 3);
        assert_eq!(result.terminal_classification.loss, 1);
        assert_eq!(result.terminal_classification.win, 2);

        let min = OracleFixtureV1 {
            fixture_id: "oracle-min-test".to_owned(),
            root: FixtureNodeId(0),
            root_player: FixturePlayerV1::P0,
            nodes: vec![
                OracleNodeV1::GameDecision {
                    id: FixtureNodeId(0),
                    actor: FixturePlayerV1::P0,
                    actions: vec![edge(0, "force-min", 1)],
                },
                OracleNodeV1::GameDecision {
                    id: FixtureNodeId(1),
                    actor: FixturePlayerV1::P1,
                    actions: vec![edge(0, "allow-draw", 2), edge(1, "allow-loss", 3)],
                },
                OracleNodeV1::Terminal {
                    id: FixtureNodeId(2),
                    outcome: FixtureOutcomeV1::Draw,
                },
                OracleNodeV1::Terminal {
                    id: FixtureNodeId(3),
                    outcome: FixtureOutcomeV1::Loss,
                },
            ],
        };
        assert_eq!(min.solve_oracle_v1().unwrap().root_value, -1);
    }

    #[test]
    fn oracle_counts_shared_dag_node_once_and_rejects_cycles() {
        let dag = fixture(vec![
            OracleNodeV1::GameDecision {
                id: FixtureNodeId(0),
                actor: FixturePlayerV1::P0,
                actions: vec![edge(0, "a", 1), edge(1, "b", 2)],
            },
            OracleNodeV1::GameDecision {
                id: FixtureNodeId(1),
                actor: FixturePlayerV1::P1,
                actions: vec![edge(0, "x", 3)],
            },
            OracleNodeV1::GameDecision {
                id: FixtureNodeId(2),
                actor: FixturePlayerV1::P1,
                actions: vec![edge(0, "x", 3)],
            },
            OracleNodeV1::Terminal {
                id: FixtureNodeId(3),
                outcome: FixtureOutcomeV1::Draw,
            },
        ]);
        assert_eq!(dag.solve_oracle_v1().unwrap().unique_game_nodes, 3);
        let cycle = fixture(vec![OracleNodeV1::GameDecision {
            id: FixtureNodeId(0),
            actor: FixturePlayerV1::P0,
            actions: vec![edge(0, "loop", 0)],
        }]);
        assert_eq!(cycle.solve_oracle_v1(), Err(OracleErrorV1::CycleDetected));
    }

    #[test]
    fn oracle_rejects_fixtures_over_admission_caps() {
        let actions = (0..=MAX_COMPLETE_LEGAL_ACTIONS_PER_GAME_NODE_V1)
            .map(|index| edge(index as u32, &format!("a{index}"), 1))
            .collect();
        let oversized = fixture(vec![
            OracleNodeV1::GameDecision {
                id: FixtureNodeId(0),
                actor: FixturePlayerV1::P0,
                actions,
            },
            OracleNodeV1::Terminal {
                id: FixtureNodeId(1),
                outcome: FixtureOutcomeV1::Draw,
            },
        ]);
        assert_eq!(
            oversized.solve_oracle_v1(),
            Err(OracleErrorV1::FixtureTooLarge)
        );
        assert_eq!(
            oversized.solve_oracle_v1().unwrap_err().to_string(),
            "ORACLE_FIXTURE_TOO_LARGE"
        );
        assert_eq!(
            check_authoritative_transition_budget_v1(MAX_AUTHORITATIVE_TRANSITIONS_V1 + 1),
            Err(OracleErrorV1::FixtureTooLarge)
        );

        let choices = (0..=MAX_CONTINUATIONS_PER_CONSTRUCTION_NODE_V1)
            .map(|index| edge(index as u32, &format!("choice-{index}"), index as u32 + 1))
            .collect::<Vec<_>>();
        let mut construction_nodes = vec![OracleNodeV1::DecisionConstruction {
            id: FixtureNodeId(34),
            owner_decision: FixtureNodeId(0),
            actor: FixturePlayerV1::P0,
            protocol_key: "target-choice".to_owned(),
            partial_response: String::new(),
            continuation_cursor: 0,
            choices,
        }];
        construction_nodes.extend((1..=33).map(|id| OracleNodeV1::Terminal {
            id: FixtureNodeId(id),
            outcome: FixtureOutcomeV1::Draw,
        }));
        construction_nodes.push(OracleNodeV1::GameDecision {
            id: FixtureNodeId(0),
            actor: FixturePlayerV1::P0,
            actions: vec![edge(0, "begin", 34)],
        });
        let oversized_construction = fixture(construction_nodes);
        assert_eq!(
            oversized_construction.solve_oracle_v1(),
            Err(OracleErrorV1::FixtureTooLarge)
        );

        let mut deep_nodes = Vec::new();
        for id in 1..=MAX_SEARCH_DEPTH_V1 as u32 {
            let actor = if id % 2 == 0 {
                FixturePlayerV1::P0
            } else {
                FixturePlayerV1::P1
            };
            deep_nodes.push(OracleNodeV1::GameDecision {
                id: FixtureNodeId(id),
                actor,
                actions: vec![edge(0, "next", id + 1)],
            });
        }
        deep_nodes.push(OracleNodeV1::Terminal {
            id: FixtureNodeId(MAX_SEARCH_DEPTH_V1 as u32 + 1),
            outcome: FixtureOutcomeV1::Draw,
        });
        deep_nodes.push(OracleNodeV1::GameDecision {
            id: FixtureNodeId(0),
            actor: FixturePlayerV1::P0,
            actions: vec![edge(0, "start", 1)],
        });
        let too_deep = fixture(deep_nodes);
        assert_eq!(
            too_deep.solve_oracle_v1(),
            Err(OracleErrorV1::FixtureTooLarge)
        );
    }

    /// Synthetic control graph for adversarial search checks. Every leaf is
    /// intentionally mixed so this graph exercises values unavailable from the
    /// MADS-02B all-win, forced-Pass engine position.
    #[test]
    fn adversarial_control_fixture_has_mixed_values_ties_and_max_min_switches() {
        let f = adversarial_control_fixture_v1();
        let result = f.solve_oracle_v1().unwrap();
        assert_eq!(result.root_value, 0);
        assert_eq!(result.optimal_root_actions, ["safe-a", "safe-b"]);
        assert_eq!(
            result.complete_legal_root_actions,
            ["safe-a", "safe-b", "risky"]
        );
        assert_eq!(
            (
                result.unique_game_nodes,
                result.terminal_nodes,
                result.total_edges
            ),
            (4, 6, 9)
        );
        assert_eq!(
            (
                result.terminal_classification.loss,
                result.terminal_classification.draw,
                result.terminal_classification.win
            ),
            (1, 3, 2)
        );
        assert_eq!(result.node_values[&FixtureNodeId(0)], 0);
        assert_eq!(result.node_values[&FixtureNodeId(1)], 0);
        assert_eq!(result.node_values[&FixtureNodeId(3)], -1);
        assert_eq!(result.authoritative_transitions, 0);
    }
}
