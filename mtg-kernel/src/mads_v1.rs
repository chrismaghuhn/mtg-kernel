//! MADS V1 correctness-first interval graph over `OracleSuiteV1` fixtures.
//!
//! This is an isolated algorithmic reference core. It does not search MTG
//! `GameState`s, consume hidden information, or enable a real-engine TT. Fixture
//! action lists are complete/eagerly admitted; MADS lazily expands successor
//! graph nodes only. `FixtureNodeId` equality is exact only inside one fixture.

use crate::oracle_suite_v1::{
    FixtureEdgeV1, FixtureNodeId, FixtureOutcomeV1, FixturePlayerV1, OracleErrorV1,
    OracleFixtureV1, OracleNodeV1,
};
use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::fmt;
use std::hash::BuildHasher;
use std::time::{Duration, Instant};

pub const LOSS_V1: i8 = -1;
pub const DRAW_V1: i8 = 0;
pub const WIN_V1: i8 = 1;
pub const FRONTIER_POLICY_V1: &str = "FRONTIER_REBUILD_REFERENCE";
pub const ACTION_ADMISSION_V1: &str = "EAGER_ACTION_ADMISSION";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MadsRoleV1 {
    Max,
    Min,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoundIntervalV1 {
    pub lower: i8,
    pub upper: i8,
}

impl BoundIntervalV1 {
    pub const UNKNOWN: Self = Self {
        lower: LOSS_V1,
        upper: WIN_V1,
    };

    pub const fn exact(value: i8) -> Self {
        Self {
            lower: value,
            upper: value,
        }
    }

    pub const fn width(self) -> u8 {
        (self.upper - self.lower) as u8
    }

    pub const fn is_valid(self) -> bool {
        self.lower >= LOSS_V1 && self.lower <= self.upper && self.upper <= WIN_V1
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MadsErrorV1 {
    Oracle(OracleErrorV1),
    CycleDetected,
    InvalidRoot(String),
    InvalidHeuristic,
    InternalInvariant(String),
}

impl fmt::Display for MadsErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Oracle(error) => error.fmt(f),
            Self::CycleDetected => f.write_str("CYCLE_DETECTED"),
            Self::InvalidRoot(message) => write!(f, "INVALID_MADS_ROOT: {message}"),
            Self::InvalidHeuristic => f.write_str("INVALID_HEURISTIC_VALUE"),
            Self::InternalInvariant(message) => write!(f, "MADS_INVARIANT: {message}"),
        }
    }
}

impl std::error::Error for MadsErrorV1 {}

impl From<OracleErrorV1> for MadsErrorV1 {
    fn from(value: OracleErrorV1) -> Self {
        Self::Oracle(value)
    }
}

#[derive(Debug, Clone)]
struct ActionSlotV1 {
    stable_id: String,
    order: u32,
    child_fixture_id: FixtureNodeId,
    child: Option<usize>,
    cost_bucket: u16,
}

#[derive(Debug, Clone)]
pub struct GameDecisionNodeV1 {
    pub fixture_key: FixtureNodeId,
    pub actor: FixturePlayerV1,
    pub role: MadsRoleV1,
    pub bounds: BoundIntervalV1,
    pub v_hat: f64,
    actions: Vec<ActionSlotV1>,
}

#[derive(Debug, Clone)]
pub struct DecisionConstructionNodeV1 {
    pub fixture_key: FixtureNodeId,
    pub actor: FixturePlayerV1,
    pub role: MadsRoleV1,
    pub protocol_key: String,
    pub partial_response: String,
    pub continuation_cursor: u32,
    /// Identifies the owning physical decision protocol. Construction choices
    /// retain the same actor/role; they do not create an automatic turn switch.
    pub owner_decision: FixtureNodeId,
    pub bounds: BoundIntervalV1,
    pub v_hat: f64,
    choices: Vec<ActionSlotV1>,
}

#[derive(Debug, Clone)]
pub struct TerminalNodeV1 {
    pub fixture_key: FixtureNodeId,
    pub outcome: FixtureOutcomeV1,
    pub bounds: BoundIntervalV1,
    pub v_hat: f64,
}

#[derive(Debug, Clone)]
pub enum SearchNodeV1 {
    GameDecisionNode(GameDecisionNodeV1),
    DecisionConstructionNode(DecisionConstructionNodeV1),
    TerminalNode(TerminalNodeV1),
}

impl SearchNodeV1 {
    pub fn fixture_key(&self) -> FixtureNodeId {
        match self {
            Self::GameDecisionNode(node) => node.fixture_key,
            Self::DecisionConstructionNode(node) => node.fixture_key,
            Self::TerminalNode(node) => node.fixture_key,
        }
    }

    pub fn bounds(&self) -> BoundIntervalV1 {
        match self {
            Self::GameDecisionNode(node) => node.bounds,
            Self::DecisionConstructionNode(node) => node.bounds,
            Self::TerminalNode(node) => node.bounds,
        }
    }

    pub fn v_hat(&self) -> f64 {
        match self {
            Self::GameDecisionNode(node) => node.v_hat,
            Self::DecisionConstructionNode(node) => node.v_hat,
            Self::TerminalNode(node) => node.v_hat,
        }
    }

    fn actor_role(&self) -> Option<(FixturePlayerV1, MadsRoleV1)> {
        match self {
            Self::GameDecisionNode(node) => Some((node.actor, node.role)),
            Self::DecisionConstructionNode(node) => Some((node.actor, node.role)),
            Self::TerminalNode(_) => None,
        }
    }

    fn slots(&self) -> &[ActionSlotV1] {
        match self {
            Self::GameDecisionNode(node) => &node.actions,
            Self::DecisionConstructionNode(node) => &node.choices,
            Self::TerminalNode(_) => &[],
        }
    }

    fn slots_mut(&mut self) -> Option<&mut [ActionSlotV1]> {
        match self {
            Self::GameDecisionNode(node) => Some(&mut node.actions),
            Self::DecisionConstructionNode(node) => Some(&mut node.choices),
            Self::TerminalNode(_) => None,
        }
    }

    fn set_bounds(&mut self, bounds: BoundIntervalV1) {
        match self {
            Self::GameDecisionNode(node) => node.bounds = bounds,
            Self::DecisionConstructionNode(node) => node.bounds = bounds,
            Self::TerminalNode(_) => {}
        }
    }

    fn set_v_hat(&mut self, value: f64) {
        match self {
            Self::GameDecisionNode(node) => node.v_hat = value,
            Self::DecisionConstructionNode(node) => node.v_hat = value,
            Self::TerminalNode(node) => node.v_hat = value,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ExpansionRoleMaskV1(u8);

impl ExpansionRoleMaskV1 {
    pub const INCUMBENT_LOWER: u8 = 1;
    pub const CHALLENGER_UPPER: u8 = 2;
    pub const OTHER_ROOT_LOWER: u8 = 4;
    pub const OTHER_ROOT_UPPER: u8 = 8;

    pub const fn contains(self, role: u8) -> bool {
        self.0 & role != 0
    }

    fn insert(&mut self, role: u8) {
        self.0 |= role;
    }

    pub const fn supports_both_primary(self) -> bool {
        self.contains(Self::INCUMBENT_LOWER) && self.contains(Self::CHALLENGER_UPPER)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpansionTaskV1 {
    pub owner: FixtureNodeId,
    pub action_order: u32,
    pub action_id: String,
    pub child: FixtureNodeId,
    pub role_mask: ExpansionRoleMaskV1,
    pub root_action_support: Vec<u32>,
    pub bound_width: u8,
    pub min_root_distance: u16,
    pub estimated_cost_bucket: u16,
    construction_key_or_action_order: ExpansionSemanticOrderV1,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum ExpansionSemanticOrderV1 {
    GameAction {
        action_order: u32,
    },
    Construction {
        protocol_key: String,
        partial_response: String,
        continuation_cursor: u32,
        action_order: u32,
    },
}

impl ExpansionTaskV1 {
    fn role_rank(&self) -> u8 {
        if self.role_mask.supports_both_primary() {
            0
        } else if self
            .role_mask
            .contains(ExpansionRoleMaskV1::INCUMBENT_LOWER)
        {
            1
        } else if self
            .role_mask
            .contains(ExpansionRoleMaskV1::CHALLENGER_UPPER)
        {
            2
        } else {
            3
        }
    }

    fn scheduler_key(
        &self,
    ) -> (
        u8,
        std::cmp::Reverse<u8>,
        u16,
        u16,
        FixtureNodeId,
        ExpansionSemanticOrderV1,
    ) {
        (
            self.role_rank(),
            std::cmp::Reverse(self.bound_width),
            self.min_root_distance,
            self.estimated_cost_bucket,
            self.owner,
            self.construction_key_or_action_order.clone(),
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CertificationStatusV1 {
    Certified,
    UnresolvedWithinBudget,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RootActionBoundV1 {
    pub stable_id: String,
    pub order: u32,
    pub bounds: BoundIntervalV1,
    pub expanded: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SearchResultV1 {
    pub status: CertificationStatusV1,
    pub chosen_action: Option<String>,
    pub certified_optimal_actions: Vec<String>,
    pub certified_unique_action: Option<String>,
    pub root_bounds: BoundIntervalV1,
    pub root_actions: Vec<RootActionBoundV1>,
    pub anytime_action: String,
    pub metrics: SearchMetricsV1,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SearchMetricsV1 {
    pub authoritative_transitions: u64,
    pub state_clones: u64,
    pub graph_nodes_created: u64,
    pub expanded_nodes: u64,
    pub expanded_actions: u64,
    pub legal_action_slots_admitted: u64,
    pub open_action_slots: u64,
    pub tt_lookups: u64,
    pub valid_tt_hits: u64,
    pub bound_updates: u64,
    pub frontier_rebuilds: u64,
    /// Number of `run_v1` calls which returned a certified root action.
    pub root_certifications: u64,
    /// Number of `run_v1` calls which exhausted their budget unresolved.
    pub unknown_roots: u64,
    /// Algorithm-level fixtures execute no OS-thread CPU timer. The field is
    /// intentionally absent rather than substituting wall time for CPU time.
    pub cpu_time: Option<Duration>,
    pub wall_time: Duration,
    /// Filled only by an external RSS harness; not estimated from node counts.
    pub peak_memory_bytes: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum CriticalRoleV1 {
    IncumbentLower,
    ChallengerUpper,
}

#[derive(Debug)]
pub struct MadsGraphV1<'a> {
    fixture_nodes: BTreeMap<FixtureNodeId, &'a OracleNodeV1>,
    root_player: FixturePlayerV1,
    nodes: Vec<SearchNodeV1>,
    node_by_fixture_id: HashMap<FixtureNodeId, usize>,
    reverse_parents: Vec<BTreeSet<usize>>,
    root_index: usize,
    metrics: SearchMetricsV1,
    frontier: Vec<ExpansionTaskV1>,
    heuristic_by_fixture_id: BTreeMap<FixtureNodeId, f64>,
}

impl<'a> MadsGraphV1<'a> {
    pub fn new(fixture: &'a OracleFixtureV1) -> Result<Self, MadsErrorV1> {
        fixture.validate_structure()?;
        fixture.validate_acyclic_v1()?;
        let fixture_nodes = fixture
            .nodes
            .iter()
            .map(|node| (node.id(), node))
            .collect::<BTreeMap<_, _>>();
        let Some(root_node) = fixture_nodes.get(&fixture.root).copied() else {
            return Err(MadsErrorV1::InvalidRoot(
                "fixture root is absent".to_owned(),
            ));
        };
        let OracleNodeV1::GameDecision { actor, .. } = root_node else {
            return Err(MadsErrorV1::InvalidRoot(
                "root must be a complete game-decision node".to_owned(),
            ));
        };
        if *actor != fixture.root_player {
            return Err(MadsErrorV1::InvalidRoot(
                "root player must be the authoritative root actor".to_owned(),
            ));
        }
        let root = make_search_node(root_node, fixture.root_player);
        let slots = root.slots().len() as u64;
        let mut node_by_fixture_id = HashMap::new();
        node_by_fixture_id.insert(fixture.root, 0);
        Ok(Self {
            fixture_nodes,
            root_player: fixture.root_player,
            nodes: vec![root],
            node_by_fixture_id,
            reverse_parents: vec![BTreeSet::new()],
            root_index: 0,
            metrics: SearchMetricsV1 {
                graph_nodes_created: 1,
                legal_action_slots_admitted: slots,
                ..SearchMetricsV1::default()
            },
            frontier: Vec::new(),
            heuristic_by_fixture_id: BTreeMap::new(),
        })
    }

    pub fn nodes(&self) -> &[SearchNodeV1] {
        &self.nodes
    }

    pub fn metrics(&self) -> &SearchMetricsV1 {
        &self.metrics
    }

    pub fn root_bounds(&self) -> BoundIntervalV1 {
        self.nodes[self.root_index].bounds()
    }

    pub fn set_heuristic_v1(
        &mut self,
        fixture_node: FixtureNodeId,
        value: f64,
    ) -> Result<(), MadsErrorV1> {
        if !value.is_finite() || !(-1.0..=1.0).contains(&value) {
            return Err(MadsErrorV1::InvalidHeuristic);
        }
        if matches!(
            self.fixture_nodes.get(&fixture_node),
            Some(OracleNodeV1::Terminal { .. })
        ) {
            return Err(MadsErrorV1::InvalidHeuristic);
        }
        if !self.fixture_nodes.contains_key(&fixture_node) {
            return Err(MadsErrorV1::InternalInvariant(
                "heuristic fixture node is absent".to_owned(),
            ));
        }
        self.heuristic_by_fixture_id.insert(fixture_node, value);
        if let Some(index) = self.node_by_fixture_id.get(&fixture_node).copied() {
            self.nodes[index].set_v_hat(value);
        }
        Ok(())
    }

    /// Returns the current deterministically rebuilt task list.
    pub fn rebuild_frontier_v1(&mut self) -> Result<&[ExpansionTaskV1], MadsErrorV1> {
        self.frontier = self.build_frontier()?;
        self.frontier.sort_by_key(ExpansionTaskV1::scheduler_key);
        self.metrics.frontier_rebuilds += 1;
        self.metrics.open_action_slots = self
            .nodes
            .iter()
            .map(|node| {
                node.slots()
                    .iter()
                    .filter(|slot| slot.child.is_none())
                    .count() as u64
            })
            .sum();
        Ok(&self.frontier)
    }

    /// Expands exactly one reference frontier task. The edge was already part
    /// of the complete fixture decision set (`EAGER_ACTION_ADMISSION`); this
    /// operation creates/reuses the child graph node only.
    pub fn expand_next_v1(&mut self) -> Result<Option<ExpansionTaskV1>, MadsErrorV1> {
        if self.frontier.is_empty() {
            self.rebuild_frontier_v1()?;
        }
        let Some(task) = self.frontier.first().cloned() else {
            return Ok(None);
        };
        self.expand_task(&task)?;
        self.rebuild_frontier_v1()?;
        Ok(Some(task))
    }

    pub fn run_v1(&mut self, expansion_budget: usize) -> Result<SearchResultV1, MadsErrorV1> {
        let start = Instant::now();
        let mut expansions = 0usize;
        loop {
            if !self.certified_root_actions().is_empty() {
                break;
            }
            if expansions >= expansion_budget {
                break;
            }
            let Some(_) = self.expand_next_v1()? else {
                break;
            };
            expansions += 1;
        }
        if self.certified_root_actions().is_empty() {
            self.metrics.unknown_roots += 1;
        } else {
            self.metrics.root_certifications += 1;
        }
        self.metrics.wall_time += start.elapsed();
        Ok(self.result_v1())
    }

    pub fn root_action_bounds_v1(&self) -> Vec<RootActionBoundV1> {
        self.nodes[self.root_index]
            .slots()
            .iter()
            .map(|slot| {
                let bounds = slot
                    .child
                    .map_or(BoundIntervalV1::UNKNOWN, |child| self.nodes[child].bounds());
                RootActionBoundV1 {
                    stable_id: slot.stable_id.clone(),
                    order: slot.order,
                    bounds,
                    expanded: slot.child.is_some(),
                }
            })
            .collect()
    }

    pub fn result_v1(&self) -> SearchResultV1 {
        let actions = self.root_action_bounds_v1();
        let certified = self.certified_root_actions();
        let anytime_action = actions
            .iter()
            .max_by(|left, right| {
                let left_v = self.root_child_v_hat(left.order);
                let right_v = self.root_child_v_hat(right.order);
                left_v
                    .total_cmp(&right_v)
                    .then_with(|| left.bounds.lower.cmp(&right.bounds.lower))
                    .then_with(|| right.order.cmp(&left.order))
            })
            .expect("root has at least one legal action")
            .stable_id
            .clone();
        let unique = certified.iter().find_map(|action| {
            let action_bound = actions.iter().find(|row| row.stable_id == *action)?;
            let best_other_upper = actions
                .iter()
                .filter(|row| row.stable_id != *action)
                .map(|row| row.bounds.upper)
                .max();
            (best_other_upper.is_none_or(|upper| action_bound.bounds.lower > upper))
                .then(|| action.clone())
        });
        let status = if certified.is_empty() {
            CertificationStatusV1::UnresolvedWithinBudget
        } else {
            CertificationStatusV1::Certified
        };
        let chosen_action = if status == CertificationStatusV1::Certified {
            certified.first().cloned()
        } else {
            None
        };
        debug_assert!(chosen_action
            .as_ref()
            .is_none_or(|chosen| certified.contains(chosen)));
        let root_bounds = self.nodes[self.root_index].bounds();
        SearchResultV1 {
            status,
            chosen_action,
            certified_optimal_actions: certified,
            certified_unique_action: unique,
            root_bounds,
            root_actions: actions,
            anytime_action,
            metrics: self.metrics.clone(),
        }
    }

    fn root_child_v_hat(&self, order: u32) -> f64 {
        let Some(slot) = self.nodes[self.root_index]
            .slots()
            .iter()
            .find(|slot| slot.order == order)
        else {
            return 0.0;
        };
        slot.child.map_or(0.0, |child| self.nodes[child].v_hat())
    }

    fn certified_root_actions(&self) -> Vec<String> {
        let actions = self.root_action_bounds_v1();
        actions
            .iter()
            .filter(|candidate| {
                let best_other_upper = actions
                    .iter()
                    .filter(|other| other.order != candidate.order)
                    .map(|other| other.bounds.upper)
                    .max();
                best_other_upper.is_none_or(|upper| candidate.bounds.lower >= upper)
            })
            .map(|candidate| candidate.stable_id.clone())
            .collect()
    }

    fn build_frontier(&self) -> Result<Vec<ExpansionTaskV1>, MadsErrorV1> {
        let root_actions = self.root_action_bounds_v1();
        let incumbent = root_actions
            .iter()
            .max_by_key(|action| (action.bounds.lower, std::cmp::Reverse(action.order)))
            .ok_or_else(|| MadsErrorV1::InternalInvariant("root has no action slots".to_owned()))?;
        let challenger = root_actions
            .iter()
            .filter(|action| action.order != incumbent.order)
            .max_by_key(|action| (action.bounds.upper, std::cmp::Reverse(action.order)));

        let mut tasks = BTreeMap::<(FixtureNodeId, u32), ExpansionTaskV1>::new();
        for (role, root_action) in [
            Some((CriticalRoleV1::IncumbentLower, incumbent)),
            challenger.map(|action| (CriticalRoleV1::ChallengerUpper, action)),
        ]
        .into_iter()
        .flatten()
        {
            let root_slot_index = self.nodes[self.root_index]
                .slots()
                .iter()
                .position(|slot| slot.order == root_action.order)
                .ok_or_else(|| {
                    MadsErrorV1::InternalInvariant("root action order missing".to_owned())
                })?;
            let root_slot = &self.nodes[self.root_index].slots()[root_slot_index];
            if let Some(child) = root_slot.child {
                self.collect_support(
                    child,
                    role,
                    root_action.order,
                    1,
                    &mut BTreeSet::new(),
                    &mut tasks,
                )?;
            } else {
                self.insert_task(
                    self.root_index,
                    root_slot_index,
                    role,
                    root_action.order,
                    0,
                    &mut tasks,
                )?;
            }
        }
        Ok(tasks.into_values().collect())
    }

    fn collect_support(
        &self,
        node_index: usize,
        role: CriticalRoleV1,
        root_order: u32,
        distance: u16,
        visited: &mut BTreeSet<(usize, CriticalRoleV1)>,
        tasks: &mut BTreeMap<(FixtureNodeId, u32), ExpansionTaskV1>,
    ) -> Result<(), MadsErrorV1> {
        if !visited.insert((node_index, role)) {
            return Ok(());
        }
        let node = &self.nodes[node_index];
        let Some((_, node_role)) = node.actor_role() else {
            return Ok(());
        };
        let parent_bounds = node.bounds();
        let slots = node.slots();
        let expanded = slots
            .iter()
            .filter(|slot| slot.child.is_some())
            .collect::<Vec<_>>();
        let unexpanded = slots
            .iter()
            .enumerate()
            .filter(|(_, slot)| slot.child.is_none())
            .collect::<Vec<_>>();
        let (selected_expanded, select_unexpanded) = match (role, node_role) {
            (CriticalRoleV1::IncumbentLower, MadsRoleV1::Max) => (
                expanded
                    .iter()
                    .copied()
                    .filter(|slot| {
                        slot.child.is_some_and(|child| {
                            self.nodes[child].bounds().lower == parent_bounds.lower
                        })
                    })
                    .collect::<Vec<_>>(),
                parent_bounds.lower == LOSS_V1,
            ),
            (CriticalRoleV1::IncumbentLower, MadsRoleV1::Min) => (
                expanded
                    .iter()
                    .copied()
                    .filter(|slot| {
                        slot.child.is_some_and(|child| {
                            self.nodes[child].bounds().lower == parent_bounds.lower
                        })
                    })
                    .collect::<Vec<_>>(),
                !unexpanded.is_empty(),
            ),
            (CriticalRoleV1::ChallengerUpper, MadsRoleV1::Min) => (
                expanded
                    .iter()
                    .copied()
                    .filter(|slot| {
                        slot.child.is_some_and(|child| {
                            self.nodes[child].bounds().upper == parent_bounds.upper
                        })
                    })
                    .collect::<Vec<_>>(),
                parent_bounds.upper == WIN_V1,
            ),
            (CriticalRoleV1::ChallengerUpper, MadsRoleV1::Max) => (
                expanded
                    .iter()
                    .copied()
                    .filter(|slot| {
                        slot.child.is_some_and(|child| {
                            self.nodes[child].bounds().upper == parent_bounds.upper
                        })
                    })
                    .collect::<Vec<_>>(),
                !unexpanded.is_empty(),
            ),
        };

        let node_distance = distance.saturating_add(1);
        for (slot_index, _) in unexpanded {
            if select_unexpanded {
                self.insert_task(node_index, slot_index, role, root_order, distance, tasks)?;
            }
        }
        for slot in selected_expanded {
            let child = slot.child.expect("filtered expanded slot");
            let child_distance = node_distance;
            self.collect_support(child, role, root_order, child_distance, visited, tasks)?;
        }
        Ok(())
    }

    fn insert_task(
        &self,
        owner_index: usize,
        slot_index: usize,
        role: CriticalRoleV1,
        root_order: u32,
        distance: u16,
        tasks: &mut BTreeMap<(FixtureNodeId, u32), ExpansionTaskV1>,
    ) -> Result<(), MadsErrorV1> {
        let owner_node = self.nodes.get(owner_index).ok_or_else(|| {
            MadsErrorV1::InternalInvariant("task owner index is absent".to_owned())
        })?;
        let slot = owner_node.slots().get(slot_index).ok_or_else(|| {
            MadsErrorV1::InternalInvariant("task slot index is absent".to_owned())
        })?;
        if slot.child.is_some() {
            return Err(MadsErrorV1::InternalInvariant(
                "frontier selected an expanded slot".to_owned(),
            ));
        }
        let owner = owner_node.fixture_key();
        let construction_key_or_action_order = match self.fixture_nodes.get(&owner).copied() {
            Some(OracleNodeV1::DecisionConstruction {
                protocol_key,
                partial_response,
                continuation_cursor,
                ..
            }) => ExpansionSemanticOrderV1::Construction {
                protocol_key: protocol_key.clone(),
                partial_response: partial_response.clone(),
                continuation_cursor: *continuation_cursor,
                action_order: slot.order,
            },
            _ => ExpansionSemanticOrderV1::GameAction {
                action_order: slot.order,
            },
        };
        let entry = tasks
            .entry((owner, slot.order))
            .or_insert_with(|| ExpansionTaskV1 {
                owner,
                action_order: slot.order,
                action_id: slot.stable_id.clone(),
                child: slot.child_fixture_id,
                role_mask: ExpansionRoleMaskV1::default(),
                root_action_support: Vec::new(),
                bound_width: owner_node.bounds().width(),
                min_root_distance: distance,
                estimated_cost_bucket: slot.cost_bucket,
                construction_key_or_action_order,
            });
        match role {
            CriticalRoleV1::IncumbentLower => {
                entry.role_mask.insert(ExpansionRoleMaskV1::INCUMBENT_LOWER)
            }
            CriticalRoleV1::ChallengerUpper => entry
                .role_mask
                .insert(ExpansionRoleMaskV1::CHALLENGER_UPPER),
        }
        entry.min_root_distance = entry.min_root_distance.min(distance);
        entry.root_action_support.push(root_order);
        entry.root_action_support.sort_unstable();
        entry.root_action_support.dedup();
        Ok(())
    }

    fn expand_task(&mut self, task: &ExpansionTaskV1) -> Result<(), MadsErrorV1> {
        let owner_index = *self.node_by_fixture_id.get(&task.owner).ok_or_else(|| {
            MadsErrorV1::InternalInvariant("frontier owner is not in graph".to_owned())
        })?;
        let slot_index = self.nodes[owner_index]
            .slots()
            .iter()
            .position(|slot| slot.order == task.action_order && slot.stable_id == task.action_id)
            .ok_or_else(|| {
                MadsErrorV1::InternalInvariant("frontier action slot is absent".to_owned())
            })?;
        let child_id = self.nodes[owner_index].slots()[slot_index].child_fixture_id;
        if child_id != task.child {
            return Err(MadsErrorV1::InternalInvariant(
                "frontier child identity changed".to_owned(),
            ));
        }
        self.metrics.tt_lookups += 1;
        let child_index =
            if let Some(index) = fixture_key_lookup_v1(&self.node_by_fixture_id, child_id) {
                let source_node = self.fixture_nodes.get(&child_id).copied().ok_or_else(|| {
                    MadsErrorV1::InternalInvariant("fixture child disappeared".to_owned())
                })?;
                if self.nodes[index].fixture_key() != child_id
                    || !source_matches_search_node(source_node, &self.nodes[index])
                {
                    return Err(MadsErrorV1::InternalInvariant(
                        "exact fixture identity maps to a different node payload".to_owned(),
                    ));
                }
                self.metrics.valid_tt_hits += 1;
                index
            } else {
                let source_node = self.fixture_nodes.get(&child_id).copied().ok_or_else(|| {
                    MadsErrorV1::InternalInvariant("fixture child disappeared".to_owned())
                })?;
                let mut node = make_search_node(source_node, self.root_player);
                if let Some(value) = self.heuristic_by_fixture_id.get(&child_id).copied() {
                    node.set_v_hat(value);
                }
                let action_count = node.slots().len() as u64;
                let index = self.nodes.len();
                self.nodes.push(node);
                self.reverse_parents.push(BTreeSet::new());
                self.node_by_fixture_id.insert(child_id, index);
                self.metrics.graph_nodes_created += 1;
                self.metrics.legal_action_slots_admitted += action_count;
                index
            };
        if child_index == owner_index || self.path_exists(child_index, owner_index) {
            return Err(MadsErrorV1::CycleDetected);
        }
        if self.nodes[owner_index].slots()[slot_index].child.is_some() {
            return Err(MadsErrorV1::InternalInvariant(
                "action slot expanded twice".to_owned(),
            ));
        }
        self.nodes[owner_index]
            .slots_mut()
            .expect("nonterminal task owner")
            .get_mut(slot_index)
            .expect("task slot checked")
            .child = Some(child_index);
        self.reverse_parents[child_index].insert(owner_index);
        self.metrics.expanded_actions += 1;
        self.metrics.authoritative_transitions = 0;
        self.metrics.state_clones = 0;
        self.metrics.expanded_nodes = self
            .nodes
            .iter()
            .filter(|node| node.slots().iter().any(|slot| slot.child.is_some()))
            .count() as u64;
        self.recompute_and_propagate(owner_index)?;
        Ok(())
    }

    fn path_exists(&self, start: usize, sought: usize) -> bool {
        let mut stack = vec![start];
        let mut seen = BTreeSet::new();
        while let Some(index) = stack.pop() {
            if index == sought {
                return true;
            }
            if !seen.insert(index) {
                continue;
            }
            for slot in self.nodes[index].slots() {
                if let Some(child) = slot.child {
                    stack.push(child);
                }
            }
        }
        false
    }

    fn recompute_and_propagate(&mut self, changed: usize) -> Result<(), MadsErrorV1> {
        let mut queue = VecDeque::from([changed]);
        let mut queued = BTreeSet::from([changed]);
        while let Some(index) = queue.pop_front() {
            queued.remove(&index);
            let old = self.nodes[index].bounds();
            let new = self.recompute_node(index)?;
            if new != old {
                self.nodes[index].set_bounds(new);
                self.metrics.bound_updates += 1;
                let parents = self.reverse_parents[index]
                    .iter()
                    .copied()
                    .collect::<Vec<_>>();
                for parent in parents {
                    if queued.insert(parent) {
                        queue.push_back(parent);
                    }
                }
            }
        }
        Ok(())
    }

    fn recompute_node(&self, index: usize) -> Result<BoundIntervalV1, MadsErrorV1> {
        let node = &self.nodes[index];
        let Some((_, role)) = node.actor_role() else {
            return Ok(node.bounds());
        };
        let slots = node.slots();
        let expanded = slots
            .iter()
            .filter_map(|slot| slot.child.map(|child| self.nodes[child].bounds()))
            .collect::<Vec<_>>();
        let has_unexpanded = expanded.len() < slots.len();
        if expanded.is_empty() {
            return Ok(BoundIntervalV1::UNKNOWN);
        }
        let bounds = if has_unexpanded {
            match role {
                MadsRoleV1::Max => BoundIntervalV1 {
                    lower: expanded.iter().map(|b| b.lower).max().unwrap_or(LOSS_V1),
                    upper: WIN_V1,
                },
                MadsRoleV1::Min => BoundIntervalV1 {
                    lower: LOSS_V1,
                    upper: expanded.iter().map(|b| b.upper).min().unwrap_or(WIN_V1),
                },
            }
        } else {
            match role {
                MadsRoleV1::Max => BoundIntervalV1 {
                    lower: expanded.iter().map(|b| b.lower).max().unwrap_or(LOSS_V1),
                    upper: expanded.iter().map(|b| b.upper).max().unwrap_or(LOSS_V1),
                },
                MadsRoleV1::Min => BoundIntervalV1 {
                    lower: expanded.iter().map(|b| b.lower).min().unwrap_or(WIN_V1),
                    upper: expanded.iter().map(|b| b.upper).min().unwrap_or(WIN_V1),
                },
            }
        };
        if !bounds.is_valid() {
            return Err(MadsErrorV1::InternalInvariant(format!(
                "invalid bounds [{}, {}] at fixture node {}",
                bounds.lower,
                bounds.upper,
                node.fixture_key().0
            )));
        }
        Ok(bounds)
    }
}

fn make_search_node(node: &OracleNodeV1, root_player: FixturePlayerV1) -> SearchNodeV1 {
    let role_for = |actor| {
        if actor == root_player {
            MadsRoleV1::Max
        } else {
            MadsRoleV1::Min
        }
    };
    match node {
        OracleNodeV1::GameDecision { id, actor, actions } => {
            SearchNodeV1::GameDecisionNode(GameDecisionNodeV1 {
                fixture_key: *id,
                actor: *actor,
                role: role_for(*actor),
                bounds: BoundIntervalV1::UNKNOWN,
                v_hat: 0.0,
                actions: actions.iter().map(make_slot).collect(),
            })
        }
        OracleNodeV1::DecisionConstruction {
            id,
            owner_decision,
            actor,
            protocol_key,
            partial_response,
            continuation_cursor,
            choices,
        } => SearchNodeV1::DecisionConstructionNode(DecisionConstructionNodeV1 {
            fixture_key: *id,
            actor: *actor,
            role: role_for(*actor),
            protocol_key: protocol_key.clone(),
            partial_response: partial_response.clone(),
            continuation_cursor: *continuation_cursor,
            owner_decision: *owner_decision,
            bounds: BoundIntervalV1::UNKNOWN,
            v_hat: 0.0,
            choices: choices.iter().map(make_slot).collect(),
        }),
        OracleNodeV1::Terminal { id, outcome } => SearchNodeV1::TerminalNode(TerminalNodeV1 {
            fixture_key: *id,
            outcome: *outcome,
            bounds: BoundIntervalV1::exact(outcome.value()),
            v_hat: f64::from(outcome.value()),
        }),
    }
}

fn make_slot(edge: &FixtureEdgeV1) -> ActionSlotV1 {
    ActionSlotV1 {
        stable_id: edge.stable_id.clone(),
        order: edge.order,
        child_fixture_id: edge.child,
        child: None,
        cost_bucket: edge.estimated_cost_bucket,
    }
}

fn source_matches_search_node(source: &OracleNodeV1, actual: &SearchNodeV1) -> bool {
    match (source, actual) {
        (
            OracleNodeV1::GameDecision { id, actor, actions },
            SearchNodeV1::GameDecisionNode(node),
        ) => {
            *id == node.fixture_key
                && *actor == node.actor
                && actions.len() == node.actions.len()
                && actions.iter().zip(&node.actions).all(|(edge, slot)| {
                    edge.stable_id == slot.stable_id
                        && edge.order == slot.order
                        && edge.child == slot.child_fixture_id
                        && edge.estimated_cost_bucket == slot.cost_bucket
                })
        }
        (
            OracleNodeV1::DecisionConstruction {
                id,
                owner_decision,
                actor,
                protocol_key,
                partial_response,
                continuation_cursor,
                choices,
            },
            SearchNodeV1::DecisionConstructionNode(node),
        ) => {
            *id == node.fixture_key
                && *actor == node.actor
                && *owner_decision == node.owner_decision
                && protocol_key == &node.protocol_key
                && partial_response == &node.partial_response
                && *continuation_cursor == node.continuation_cursor
                && choices.len() == node.choices.len()
                && choices.iter().zip(&node.choices).all(|(edge, slot)| {
                    edge.stable_id == slot.stable_id
                        && edge.order == slot.order
                        && edge.child == slot.child_fixture_id
                        && edge.estimated_cost_bucket == slot.cost_bucket
                })
        }
        (OracleNodeV1::Terminal { id, outcome }, SearchNodeV1::TerminalNode(node)) => {
            *id == node.fixture_key && *outcome == node.outcome
        }
        _ => false,
    }
}

/// Production lookup primitive, generic over the hash builder so tests can
/// force collisions. The map's exact `Eq` check decides identity.
fn fixture_key_lookup_v1<S: BuildHasher>(
    table: &HashMap<FixtureNodeId, usize, S>,
    key: FixtureNodeId,
) -> Option<usize> {
    table.get(&key).copied()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::oracle_suite_v1::{OracleNodeV1, OracleResultV1};
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasherDefault, Hasher};

    fn edge(order: u32, id: &str, child: u32) -> FixtureEdgeV1 {
        FixtureEdgeV1 {
            stable_id: id.to_owned(),
            order,
            child: FixtureNodeId(child),
            estimated_cost_bucket: 0,
        }
    }

    fn fixture(root_actions: Vec<FixtureEdgeV1>, nodes: Vec<OracleNodeV1>) -> OracleFixtureV1 {
        let mut all = vec![OracleNodeV1::GameDecision {
            id: FixtureNodeId(0),
            actor: FixturePlayerV1::P0,
            actions: root_actions,
        }];
        all.extend(nodes);
        OracleFixtureV1 {
            fixture_id: "mads-test".to_owned(),
            root: FixtureNodeId(0),
            root_player: FixturePlayerV1::P0,
            nodes: all,
        }
    }

    fn terminal(id: u32, outcome: FixtureOutcomeV1) -> OracleNodeV1 {
        OracleNodeV1::Terminal {
            id: FixtureNodeId(id),
            outcome,
        }
    }

    fn oracle(fixture: &OracleFixtureV1) -> OracleResultV1 {
        fixture.solve_oracle_v1().unwrap()
    }

    fn force_root_action(search: &mut MadsGraphV1<'_>, order: u32) {
        let slot_index = search.nodes[search.root_index]
            .slots()
            .iter()
            .position(|slot| slot.order == order)
            .unwrap();
        let mut tasks = BTreeMap::new();
        search
            .insert_task(
                search.root_index,
                slot_index,
                CriticalRoleV1::IncumbentLower,
                order,
                0,
                &mut tasks,
            )
            .unwrap();
        search.expand_task(tasks.values().next().unwrap()).unwrap();
    }

    #[test]
    fn max_tree_certifies_the_last_unseen_best_action_only_after_expansion() {
        let f = fixture(
            vec![
                edge(0, "first-loss", 1),
                edge(1, "second-draw", 2),
                edge(2, "last-win", 3),
            ],
            vec![
                terminal(1, FixtureOutcomeV1::Loss),
                terminal(2, FixtureOutcomeV1::Draw),
                terminal(3, FixtureOutcomeV1::Win),
            ],
        );
        let truth = oracle(&f);
        assert_eq!(truth.root_value, WIN_V1);
        let mut search = MadsGraphV1::new(&f).unwrap();
        let r = search.run_v1(0).unwrap();
        assert_eq!(r.status, CertificationStatusV1::UnresolvedWithinBudget);
        assert!(r
            .root_actions
            .iter()
            .all(|row| row.bounds == BoundIntervalV1::UNKNOWN));
        let first = search.expand_next_v1().unwrap().unwrap();
        assert_eq!(first.action_order, 0);
        assert_eq!(search.root_bounds(), BoundIntervalV1::UNKNOWN);
        assert_eq!(
            search.root_action_bounds_v1()[0].bounds,
            BoundIntervalV1::exact(LOSS_V1)
        );
        let second = search.expand_next_v1().unwrap().unwrap();
        assert_eq!(second.action_order, 1);
        assert_eq!(
            search.root_bounds(),
            BoundIntervalV1 {
                lower: DRAW_V1,
                upper: WIN_V1
            }
        );
        let third = search.expand_next_v1().unwrap().unwrap();
        assert_eq!(third.action_order, 2);
        assert_eq!(search.root_bounds(), BoundIntervalV1::exact(WIN_V1));
        while search.expand_next_v1().unwrap().is_some() {
            assert!(search.nodes.iter().all(|node| {
                let bounds = node.bounds();
                bounds.is_valid()
                    && bounds.lower <= truth.node_values[&node.fixture_key()]
                    && truth.node_values[&node.fixture_key()] <= bounds.upper
            }));
        }
        let r = search.result_v1();
        assert_eq!(r.status, CertificationStatusV1::Certified);
        assert_eq!(r.certified_optimal_actions, vec!["last-win"]);
        assert_eq!(r.certified_unique_action.as_deref(), Some("last-win"));
    }

    #[test]
    fn min_node_keeps_lower_envelope_open_until_last_action() {
        let f = fixture(
            vec![edge(0, "force-min", 1)],
            vec![
                OracleNodeV1::GameDecision {
                    id: FixtureNodeId(1),
                    actor: FixturePlayerV1::P1,
                    actions: vec![edge(0, "safe", 2), edge(1, "last-loss", 3)],
                },
                terminal(2, FixtureOutcomeV1::Draw),
                terminal(3, FixtureOutcomeV1::Loss),
            ],
        );
        let truth = oracle(&f);
        let mut search = MadsGraphV1::new(&f).unwrap();
        search.expand_next_v1().unwrap(); // root -> MIN node
        let min_index = *search.node_by_fixture_id.get(&FixtureNodeId(1)).unwrap();
        assert_eq!(search.nodes[min_index].bounds(), BoundIntervalV1::UNKNOWN);
        let task = search.rebuild_frontier_v1().unwrap()[0].clone();
        assert_eq!(task.owner, FixtureNodeId(1));
        search.expand_next_v1().unwrap();
        assert_eq!(search.nodes[min_index].bounds().lower, LOSS_V1);
        while search.expand_next_v1().unwrap().is_some() {
            for node in &search.nodes {
                let true_value = truth.node_values[&node.fixture_key()];
                assert!(node.bounds().lower <= true_value && true_value <= node.bounds().upper);
            }
        }
        assert_eq!(
            search.result_v1().root_bounds,
            BoundIntervalV1::exact(LOSS_V1)
        );
    }

    #[test]
    fn shared_descendant_propagates_to_every_parent_and_roles_merge_once() {
        let f = fixture(
            vec![edge(0, "incumbent", 1), edge(1, "challenger", 1)],
            vec![
                OracleNodeV1::GameDecision {
                    id: FixtureNodeId(1),
                    actor: FixturePlayerV1::P0,
                    actions: vec![edge(0, "eventual-win", 2)],
                },
                terminal(2, FixtureOutcomeV1::Win),
            ],
        );
        let truth = oracle(&f);
        let mut search = MadsGraphV1::new(&f).unwrap();
        // Start from the critical-DAG state after both root edges have been
        // constructed; then verify shared-task deduplication/role union.
        force_root_action(&mut search, 0);
        force_root_action(&mut search, 1);
        let shared = *search.node_by_fixture_id.get(&FixtureNodeId(1)).unwrap();
        assert_eq!(search.reverse_parents[shared].len(), 1); // both root edges share one parent node
        let tasks = search.rebuild_frontier_v1().unwrap().to_vec();
        let shared_task = tasks
            .iter()
            .find(|task| task.owner == FixtureNodeId(1))
            .unwrap();
        assert!(shared_task.role_mask.supports_both_primary());
        assert_eq!(shared_task.root_action_support, vec![0, 1]);
        search.expand_next_v1().unwrap(); // shared node -> terminal
        let result = search.result_v1();
        assert_eq!(
            result.certified_optimal_actions,
            vec!["incumbent", "challenger"]
        );
        assert_eq!(result.root_bounds, BoundIntervalV1::exact(truth.root_value));
        assert_eq!(search.metrics.valid_tt_hits, 1);
    }

    #[test]
    fn tiny_budget_returns_unknown_and_anytime_never_changes_bounds() {
        let f = fixture(
            vec![edge(0, "a", 1), edge(1, "b", 2)],
            vec![
                terminal(1, FixtureOutcomeV1::Draw),
                terminal(2, FixtureOutcomeV1::Win),
            ],
        );
        let mut search = MadsGraphV1::new(&f).unwrap();
        search.set_heuristic_v1(FixtureNodeId(0), 0.9).unwrap();
        let r = search.run_v1(1).unwrap();
        assert_eq!(r.status, CertificationStatusV1::UnresolvedWithinBudget);
        assert!(r.chosen_action.is_none());
        assert_eq!(
            r.root_bounds,
            BoundIntervalV1 {
                lower: DRAW_V1,
                upper: WIN_V1
            }
        );
        assert_eq!(r.anytime_action, "a");
        assert_eq!(search.metrics().unknown_roots, 1);
        assert_eq!(
            search.set_heuristic_v1(FixtureNodeId(0), 1.1),
            Err(MadsErrorV1::InvalidHeuristic)
        );
    }

    #[test]
    fn tied_root_actions_use_stable_semantic_order() {
        let f = fixture(
            vec![edge(0, "alpha", 1), edge(1, "beta", 2)],
            vec![
                terminal(1, FixtureOutcomeV1::Draw),
                terminal(2, FixtureOutcomeV1::Draw),
            ],
        );
        let mut search = MadsGraphV1::new(&f).unwrap();
        let result = search.run_v1(8).unwrap();
        assert_eq!(result.certified_optimal_actions, vec!["alpha", "beta"]);
        assert!(result.certified_unique_action.is_none());
        assert_eq!(result.chosen_action.as_deref(), Some("alpha"));
    }

    #[test]
    fn adversarial_control_differential_checks_every_interval_and_root_certificate() {
        let f = crate::oracle_suite_v1::adversarial_control_fixture_v1();
        let truth = f.solve_oracle_v1().unwrap();
        assert_eq!(truth.root_value, DRAW_V1);
        assert_eq!(truth.optimal_root_actions, ["safe-a", "safe-b"]);
        assert_eq!(
            truth.complete_legal_root_actions,
            ["safe-a", "safe-b", "risky"]
        );

        let root_edges = f.node(f.root).unwrap().edges();
        let mut search = MadsGraphV1::new(&f).unwrap();
        loop {
            // Eager action admission means every legal root action must remain
            // visible even when its successor has not been constructed yet.
            let action_bounds = search.root_action_bounds_v1();
            assert_eq!(
                action_bounds
                    .iter()
                    .map(|action| action.stable_id.as_str())
                    .collect::<Vec<_>>(),
                truth
                    .complete_legal_root_actions
                    .iter()
                    .map(String::as_str)
                    .collect::<Vec<_>>()
            );
            for (root_edge, action) in root_edges.iter().zip(&action_bounds) {
                assert_eq!(action.order, root_edge.order);
                if !action.expanded {
                    assert_eq!(action.bounds, BoundIntervalV1::UNKNOWN);
                }
                let exact_action_value = truth.node_values[&root_edge.child];
                assert!(action.bounds.lower <= exact_action_value);
                assert!(exact_action_value <= action.bounds.upper);
            }

            for node in search.nodes() {
                let exact = truth.node_values[&node.fixture_key()];
                let bounds = node.bounds();
                assert!(
                    bounds.is_valid() && bounds.lower <= exact && exact <= bounds.upper,
                    "MADS bounds {:?} excluded oracle value {exact} at {:?}",
                    bounds,
                    node.fixture_key()
                );
            }
            let root_bounds = search.root_bounds();
            assert!(root_bounds.lower <= truth.root_value && truth.root_value <= root_bounds.upper);
            let partial_result = search.result_v1();
            assert!(partial_result
                .certified_optimal_actions
                .iter()
                .all(|action| truth.optimal_root_actions.contains(action)));
            assert!(partial_result
                .chosen_action
                .as_ref()
                .is_none_or(|action| truth.optimal_root_actions.contains(action)));

            if search.expand_next_v1().unwrap().is_none() {
                break;
            }
        }

        let result = search.result_v1();
        assert_eq!(result.status, CertificationStatusV1::Certified);
        assert_eq!(result.root_bounds, BoundIntervalV1::exact(truth.root_value));
        assert_eq!(result.certified_optimal_actions, truth.optimal_root_actions);
        assert_eq!(
            result.certified_unique_action, None,
            "the oracle has a root tie"
        );
        assert!(result
            .chosen_action
            .as_ref()
            .is_some_and(|action| truth.optimal_root_actions.contains(action)));
        assert!(result.root_actions.iter().all(|action| action.expanded));
    }

    #[test]
    fn certified_choice_comes_from_the_proven_set_when_lower_bounds_tie() {
        let f = fixture(
            vec![edge(0, "A", 1), edge(1, "B", 2)],
            vec![
                terminal(1, FixtureOutcomeV1::Draw),
                OracleNodeV1::GameDecision {
                    id: FixtureNodeId(2),
                    actor: FixturePlayerV1::P0,
                    actions: vec![edge(0, "first-draw", 3), edge(1, "later-win", 4)],
                },
                terminal(3, FixtureOutcomeV1::Draw),
                terminal(4, FixtureOutcomeV1::Win),
            ],
        );
        let truth = oracle(&f);
        assert_eq!(truth.root_value, WIN_V1);
        assert_eq!(truth.optimal_root_actions, vec!["B"]);

        let mut search = MadsGraphV1::new(&f).unwrap();
        assert_eq!(search.expand_next_v1().unwrap().unwrap().action_order, 0);
        assert_eq!(search.expand_next_v1().unwrap().unwrap().action_order, 1);
        let last_expansion = search.expand_next_v1().unwrap().unwrap();
        assert_eq!(last_expansion.owner, FixtureNodeId(2));
        assert_eq!(last_expansion.action_order, 0);

        let result = search.run_v1(0).unwrap();
        assert_eq!(
            result.root_actions[0].bounds,
            BoundIntervalV1::exact(DRAW_V1)
        );
        assert_eq!(
            result.root_actions[1].bounds,
            BoundIntervalV1 {
                lower: DRAW_V1,
                upper: WIN_V1
            }
        );
        assert_eq!(result.certified_optimal_actions, vec!["B"]);
        assert_eq!(result.chosen_action.as_deref(), Some("B"));
        assert!(result
            .chosen_action
            .as_ref()
            .is_some_and(|chosen| result.certified_optimal_actions.contains(chosen)));
        assert!(truth
            .optimal_root_actions
            .contains(result.chosen_action.as_ref().unwrap()));
        assert_eq!(result.metrics.root_certifications, 1);
        assert_eq!(search.metrics().root_certifications, 1);
    }

    #[test]
    fn interval_bounds_are_independent_of_expansion_order_for_the_same_prefix_set() {
        let f = fixture(
            vec![edge(0, "loss", 1), edge(1, "draw", 2), edge(2, "win", 3)],
            vec![
                terminal(1, FixtureOutcomeV1::Loss),
                terminal(2, FixtureOutcomeV1::Draw),
                terminal(3, FixtureOutcomeV1::Win),
            ],
        );
        let truth = oracle(&f);
        let mut left_to_right = MadsGraphV1::new(&f).unwrap();
        force_root_action(&mut left_to_right, 0);
        force_root_action(&mut left_to_right, 1);
        let mut right_to_left = MadsGraphV1::new(&f).unwrap();
        force_root_action(&mut right_to_left, 1);
        force_root_action(&mut right_to_left, 0);
        assert_eq!(left_to_right.root_bounds(), right_to_left.root_bounds());
        assert_eq!(
            left_to_right.root_bounds(),
            BoundIntervalV1 {
                lower: DRAW_V1,
                upper: WIN_V1
            }
        );
        force_root_action(&mut left_to_right, 2);
        force_root_action(&mut right_to_left, 2);
        assert_eq!(
            left_to_right.root_bounds(),
            BoundIntervalV1::exact(truth.root_value)
        );
        assert_eq!(left_to_right.root_bounds(), right_to_left.root_bounds());
    }

    #[test]
    fn construction_nodes_preserve_actor_and_distinct_protocol_context() {
        let f = fixture(
            vec![edge(0, "cast-A", 1), edge(1, "cast-B", 2)],
            vec![
                OracleNodeV1::DecisionConstruction {
                    id: FixtureNodeId(1),
                    owner_decision: FixtureNodeId(0),
                    actor: FixturePlayerV1::P0,
                    protocol_key: "target-choice".to_owned(),
                    partial_response: "spell-A/mode-0".to_owned(),
                    continuation_cursor: 1,
                    choices: vec![edge(0, "target-A", 3)],
                },
                OracleNodeV1::DecisionConstruction {
                    id: FixtureNodeId(2),
                    owner_decision: FixtureNodeId(0),
                    actor: FixturePlayerV1::P0,
                    protocol_key: "target-choice".to_owned(),
                    partial_response: "spell-B/mode-1".to_owned(),
                    continuation_cursor: 1,
                    choices: vec![edge(0, "target-B", 3)],
                },
                terminal(3, FixtureOutcomeV1::Win),
            ],
        );
        let mut search = MadsGraphV1::new(&f).unwrap();
        force_root_action(&mut search, 0);
        force_root_action(&mut search, 1);
        let a = search.node_by_fixture_id[&FixtureNodeId(1)];
        let b = search.node_by_fixture_id[&FixtureNodeId(2)];
        assert_ne!(a, b);
        assert!(matches!(
            &search.nodes[a],
            SearchNodeV1::DecisionConstructionNode(DecisionConstructionNodeV1 {
                role: MadsRoleV1::Max,
                ..
            })
        ));
        assert!(matches!(
            &search.nodes[b],
            SearchNodeV1::DecisionConstructionNode(DecisionConstructionNodeV1 {
                role: MadsRoleV1::Max,
                ..
            })
        ));
    }

    #[test]
    fn fixture_cycle_is_rejected_without_draw_substitution() {
        let f = fixture(vec![edge(0, "cycle", 0)], vec![]);
        assert_eq!(
            MadsGraphV1::new(&f).unwrap_err(),
            MadsErrorV1::Oracle(OracleErrorV1::CycleDetected)
        );
        assert_eq!(
            f.solve_oracle_v1().unwrap_err(),
            OracleErrorV1::CycleDetected
        );
    }

    #[test]
    fn unreachable_fixture_nodes_are_rejected_before_mads_search() {
        let f = fixture(
            vec![edge(0, "only-action", 1)],
            vec![
                terminal(1, FixtureOutcomeV1::Draw),
                terminal(2, FixtureOutcomeV1::Win),
            ],
        );
        let expected =
            MadsErrorV1::Oracle(OracleErrorV1::InvalidFixture("unreachable node".to_owned()));
        assert_eq!(MadsGraphV1::new(&f).unwrap_err(), expected);
        assert_eq!(
            f.solve_oracle_v1().unwrap_err().to_string(),
            "INVALID_ORACLE_FIXTURE: fixture contains unreachable nodes"
        );
    }

    #[test]
    fn heuristics_never_enter_certified_bounds() {
        let f = fixture(
            vec![edge(0, "loss", 1), edge(1, "win", 2)],
            vec![
                OracleNodeV1::GameDecision {
                    id: FixtureNodeId(1),
                    actor: FixturePlayerV1::P1,
                    actions: vec![edge(0, "forced-loss", 3)],
                },
                OracleNodeV1::GameDecision {
                    id: FixtureNodeId(2),
                    actor: FixturePlayerV1::P1,
                    actions: vec![edge(0, "forced-win", 4)],
                },
                terminal(3, FixtureOutcomeV1::Loss),
                terminal(4, FixtureOutcomeV1::Win),
            ],
        );
        let truth = oracle(&f);
        let mut search = MadsGraphV1::new(&f).unwrap();
        search.set_heuristic_v1(FixtureNodeId(0), -1.0).unwrap();
        search.set_heuristic_v1(FixtureNodeId(1), 1.0).unwrap();
        while search.expand_next_v1().unwrap().is_some() {}
        let result = search.result_v1();
        assert_eq!(result.root_bounds, BoundIntervalV1::exact(truth.root_value));
        assert_eq!(result.certified_optimal_actions, vec!["win"]);
    }

    #[test]
    fn node_input_order_does_not_change_oracle_or_scheduler_choice() {
        let nodes = vec![
            terminal(1, FixtureOutcomeV1::Loss),
            terminal(2, FixtureOutcomeV1::Win),
        ];
        let f1 = fixture(vec![edge(0, "loss", 1), edge(1, "win", 2)], nodes.clone());
        let f2 = fixture(
            vec![edge(0, "loss", 1), edge(1, "win", 2)],
            nodes.into_iter().rev().collect(),
        );
        assert_eq!(oracle(&f1), oracle(&f2));
        let mut a = MadsGraphV1::new(&f1).unwrap();
        let mut b = MadsGraphV1::new(&f2).unwrap();
        assert_eq!(
            a.rebuild_frontier_v1().unwrap(),
            b.rebuild_frontier_v1().unwrap()
        );
        assert_eq!(
            a.run_v1(8).unwrap().certified_optimal_actions,
            b.run_v1(8).unwrap().certified_optimal_actions
        );
    }

    #[test]
    fn hash_collisions_do_not_merge_unequal_fixture_keys() {
        #[derive(Default)]
        struct CollisionHasher;
        impl Hasher for CollisionHasher {
            fn finish(&self) -> u64 {
                0
            }
            fn write(&mut self, _bytes: &[u8]) {}
        }
        type CollisionBuild = BuildHasherDefault<CollisionHasher>;
        let mut table: HashMap<FixtureNodeId, usize, CollisionBuild> = HashMap::default();
        table.insert(FixtureNodeId(1), 7);
        table.insert(FixtureNodeId(2), 8);
        assert_eq!(fixture_key_lookup_v1(&table, FixtureNodeId(1)), Some(7));
        assert_eq!(fixture_key_lookup_v1(&table, FixtureNodeId(2)), Some(8));
        assert_eq!(table.len(), 2);
        let _normal_hasher_is_the_production_fixture_default: RandomState = RandomState::new();
    }

    #[test]
    fn worklist_propagates_bounds_through_multiple_reverse_parents() {
        let f = fixture(
            vec![edge(0, "root-a", 1), edge(1, "root-b", 2)],
            vec![
                OracleNodeV1::GameDecision {
                    id: FixtureNodeId(1),
                    actor: FixturePlayerV1::P0,
                    actions: vec![edge(0, "shared-a", 3)],
                },
                OracleNodeV1::GameDecision {
                    id: FixtureNodeId(2),
                    actor: FixturePlayerV1::P0,
                    actions: vec![edge(0, "shared-b", 3)],
                },
                OracleNodeV1::GameDecision {
                    id: FixtureNodeId(3),
                    actor: FixturePlayerV1::P0,
                    actions: vec![edge(0, "win", 4)],
                },
                terminal(4, FixtureOutcomeV1::Win),
            ],
        );
        let truth = oracle(&f);
        let mut search = MadsGraphV1::new(&f).unwrap();
        while search.expand_next_v1().unwrap().is_some() {
            for node in &search.nodes {
                let value = truth.node_values[&node.fixture_key()];
                assert!(node.bounds().lower <= value && value <= node.bounds().upper);
            }
        }
        assert_eq!(search.root_bounds(), BoundIntervalV1::exact(WIN_V1));
        let shared = search.node_by_fixture_id[&FixtureNodeId(3)];
        assert_eq!(search.reverse_parents[shared].len(), 2);
        assert_eq!(search.metrics.bound_updates, 4);
    }

    #[test]
    fn oracle_mads_first_comparison_smoke() {
        let f = fixture(
            vec![edge(0, "left", 1), edge(1, "right", 1)],
            vec![
                OracleNodeV1::GameDecision {
                    id: FixtureNodeId(1),
                    actor: FixturePlayerV1::P1,
                    actions: vec![edge(0, "continue", 2)],
                },
                terminal(2, FixtureOutcomeV1::Draw),
            ],
        );
        let oracle_start = Instant::now();
        let truth = oracle(&f);
        let oracle_wall = oracle_start.elapsed();
        let mads_start = Instant::now();
        let mut search = MadsGraphV1::new(&f).unwrap();
        let result = search.run_v1(8).unwrap();
        let mads_wall = mads_start.elapsed();
        assert_eq!(truth.root_value, result.root_bounds.lower);
        assert_eq!(truth.root_value, result.root_bounds.upper);
        assert_eq!(result.certified_optimal_actions, vec!["left", "right"]);
        assert_eq!(result.metrics.valid_tt_hits, 1);
        println!(
            "MADS01 comparison fixture={} oracle_value={} oracle_game_nodes={} oracle_edges={} oracle_wall_ns={} mads_nodes={} mads_expanded_actions={} mads_tt_hits={} mads_authoritative_transitions={} mads_state_clones={} mads_wall_ns={} cpu_time=unavailable peak_memory=unmeasured",
            f.fixture_id,
            truth.root_value,
            truth.unique_game_nodes,
            truth.total_edges,
            oracle_wall.as_nanos(),
            result.metrics.graph_nodes_created,
            result.metrics.expanded_actions,
            result.metrics.valid_tt_hits,
            result.metrics.authoritative_transitions,
            result.metrics.state_clones,
            mads_wall.as_nanos(),
        );
    }
}
