//! End-to-end root-critical search over authoritative physical actions.
//!
//! V4 first completes the versioned V3 physical-root domain, then attaches an
//! independent, path-local DynamicEngineSearchV1 to each actual successor
//! GameDecision. Successor searches keep the original physical root player as
//! MAX and propagate only certified V1 intervals back to their V3 alternative.
//! No successor states are merged and no state-key/TT reuse is enabled.

use crate::dynamic_engine_search_v1::{
    DynamicEngineSearchV1, DynamicSearchErrorV1, DynamicSearchStatusV1,
};
use crate::engine::{Action, Decision};
use crate::ids::PlayerId;
use crate::mads03b5_engine_binding_v3::{
    DynamicEngineSearchMetricsV3, DynamicEngineSearchV3, DynamicSearchResultV3,
    EngineBindingErrorV3,
};
use crate::mads_v1::BoundIntervalV1;
use crate::mads_virtual_physical_root_v3::{
    ExpansionSlotIdentityV3, PhysicalAlternativeStateV3, RootCriticalFrontierSnapshotV3,
    RootCriticalFrontierStatusV3, RootRoleMaskV3, VirtualPhysicalRootErrorV3,
};
use crate::state::GameState;
use std::collections::BTreeMap;
use std::time::{Duration, Instant};

const V4_ENGINE_SEARCH_PROTOCOL: &str = "dynamic-engine-successor-search.v4";
const V4_SUCCESSOR_SEARCH_STAGE: &str = "successor-game-decision.v4";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DynamicSearchStatusV4 {
    ExactRootValue,
    CertifiedRootAction,
    UnresolvedWithinBudget,
    BlockedOnUnsupportedSuccessor,
}

impl DynamicSearchStatusV4 {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ExactRootValue => "EXACT_ROOT_VALUE",
            Self::CertifiedRootAction => "CERTIFIED_ROOT_ACTION",
            Self::UnresolvedWithinBudget => "UNRESOLVED_WITHIN_BUDGET",
            Self::BlockedOnUnsupportedSuccessor => "BLOCKED_ON_UNSUPPORTED_SUCCESSOR",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DynamicSearchErrorV4 {
    EngineBinding(EngineBindingErrorV3),
    InvalidSuccessorBinding,
}

impl std::fmt::Display for DynamicSearchErrorV4 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for DynamicSearchErrorV4 {}
impl From<EngineBindingErrorV3> for DynamicSearchErrorV4 {
    fn from(value: EngineBindingErrorV3) -> Self {
        Self::EngineBinding(value)
    }
}
impl From<VirtualPhysicalRootErrorV3> for DynamicSearchErrorV4 {
    fn from(_: VirtualPhysicalRootErrorV3) -> Self {
        Self::InvalidSuccessorBinding
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpansionTaskV4 {
    pub physical_root_action_id: String,
    pub root_role_mask: u8,
    pub successor_role_mask: u8,
    pub owner_node_identity: String,
    pub actor: PlayerId,
    pub action_order: u32,
    pub action_id: String,
    pub bound_width: u8,
    pub root_distance: u16,
    pub estimated_cost_bucket: u16,
}

impl ExpansionTaskV4 {
    fn role_rank(&self) -> u8 {
        let incumbent = self.root_role_mask & RootRoleMaskV3::INCUMBENT_LOWER != 0;
        let challenger = self.root_role_mask & RootRoleMaskV3::CHALLENGER_UPPER != 0;
        match (incumbent, challenger) {
            (true, true) => 0,
            (true, false) => 1,
            (false, true) => 2,
            (false, false) => 3,
        }
    }

    fn stable_key(&self) -> (u8, std::cmp::Reverse<u8>, u16, u16, String, String, u32) {
        (
            self.role_rank(),
            std::cmp::Reverse(self.bound_width),
            self.root_distance,
            self.estimated_cost_bucket,
            self.physical_root_action_id.clone(),
            self.owner_node_identity.clone(),
            self.action_order,
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuccessorSearchStateV4 {
    NotStarted,
    Ready,
    Solved,
    Blocked,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PhysicalRootActionResultV4 {
    pub stable_identity: String,
    pub ordered_engine_responses: Vec<Action>,
    pub state: PhysicalAlternativeStateV3,
    pub bounds: BoundIntervalV1,
    pub successor_node_id: Option<String>,
    pub successor_actor: Option<PlayerId>,
    pub successor_search_state: SuccessorSearchStateV4,
    pub unresolved_reason: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DynamicSearchMetricsV4 {
    /// Logical scheduler work items. A construction branch may issue multiple
    /// authoritative Engine transitions within one complete-action task.
    pub search_expansion_units: u64,
    pub physical_construction_units: u64,
    pub successor_expansion_attempts: u64,
    pub authoritative_transitions: u64,
    pub state_clones: u64,
    pub admitted_actions: u64,
    pub expanded_actions: u64,
    pub bound_updates: u64,
    pub frontier_rebuilds: u64,
    pub scheduler_rebuilds: u64,
    pub complete_physical_actions: u64,
    pub successor_game_decisions_admitted: u64,
    pub successor_construction_nodes_admitted: u64,
    pub successor_terminal_nodes_admitted: u64,
    pub successor_domains_revalidated: u64,
    pub successor_domains_admitted_by_v3: u64,
    pub max_decision_nodes: u64,
    pub min_decision_nodes: u64,
    pub unknown_root_alternatives: u64,
    pub unresolved_root_alternatives: u64,
    pub root_alternative_count: u64,
    pub root_certifications: u64,
    pub expansions_to_first_certificate: Option<u64>,
    pub initialization_wall_time: Duration,
    pub successor_initialization_wall_time: Duration,
    pub search_wall_time: Duration,
    /// Not measured by the portable fixture harness.
    pub cpu_time: Option<Duration>,
    /// Requires an external process-RSS sampler; never estimated here.
    pub peak_memory_bytes: Option<u64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DynamicSearchResultV4 {
    pub status: DynamicSearchStatusV4,
    pub root_scheduler_policy: &'static str,
    pub successor_scheduler_policy: &'static str,
    pub physical_root_domain_complete: bool,
    pub root_bounds: BoundIntervalV1,
    pub exact_root_value: Option<i8>,
    pub certified_optimal_actions: Vec<String>,
    pub chosen_certified_action: Option<String>,
    pub physical_root_actions: Vec<PhysicalRootActionResultV4>,
    pub unresolved_reasons: BTreeMap<String, String>,
    pub root_frontier: RootCriticalFrontierSnapshotV3,
    pub executable_frontier: Vec<ExpansionTaskV4>,
    pub metrics: DynamicSearchMetricsV4,
}

#[derive(Debug)]
struct SuccessorSearchV4 {
    successor_node_id: String,
    search: Option<DynamicEngineSearchV1>,
    blocked_reason: Option<String>,
    solved: bool,
    domain_revalidated_by_v1: bool,
    domain_admitted_by_v3: bool,
}

#[derive(Debug)]
pub struct DynamicEngineSearchV4 {
    physical_search: DynamicEngineSearchV3,
    root_player: PlayerId,
    successor_searches: BTreeMap<String, SuccessorSearchV4>,
    last_physical_metrics: DynamicEngineSearchMetricsV3,
    last_physical_result: DynamicSearchResultV3,
    successor_searches_initialized: bool,
    outer_frontier_rebuilds: u64,
    bound_updates: u64,
    work_units: u64,
    physical_work_units: u64,
    successor_expansion_attempts: u64,
    first_certificate_at: Option<u64>,
    initialization_wall_time: Duration,
    successor_initialization_wall_time: Duration,
    search_wall_time: Duration,
}

impl DynamicEngineSearchV4 {
    pub const API_VERSION: u16 = 4;
    pub const SUCCESSOR_SEARCH_POLICY: &'static str = "FRONTIER_ROOT_CRITICAL_PHYSICAL_V4_REBUILD";
    pub const INNER_SUCCESSOR_FRONTIER_POLICY: &'static str =
        DynamicEngineSearchV1::SUCCESSOR_VALUE_FRONTIER_POLICY_V1;

    pub fn new(
        root_state: &GameState,
        root_decision: Decision,
    ) -> Result<Self, DynamicSearchErrorV4> {
        let initialization_start = Instant::now();
        let physical_search = DynamicEngineSearchV3::new(root_state, root_decision)?;
        let root_player = physical_search.root_actor_v3();
        // Budget zero performs no Engine response. The snapshot only initializes
        // the exact raw-root frontier and records its current conservative state.
        let mut physical_search = physical_search;
        let last_physical_result = physical_search.run_v3(0)?;
        let last_physical_metrics = last_physical_result.metrics.clone();
        Ok(Self {
            physical_search,
            root_player,
            successor_searches: BTreeMap::new(),
            last_physical_metrics,
            last_physical_result,
            successor_searches_initialized: false,
            outer_frontier_rebuilds: 0,
            bound_updates: 0,
            work_units: 0,
            physical_work_units: 0,
            successor_expansion_attempts: 0,
            first_certificate_at: None,
            initialization_wall_time: initialization_start.elapsed(),
            successor_initialization_wall_time: Duration::ZERO,
            search_wall_time: Duration::ZERO,
        })
    }

    /// Executes at most `compute_budget` logical expansions. One physical
    /// target task can replay its shared CastSpell prefix and target response;
    /// actual Engine transitions are reported separately from scheduler units.
    pub fn run_v4(
        &mut self,
        compute_budget: usize,
    ) -> Result<DynamicSearchResultV4, DynamicSearchErrorV4> {
        let call_start = Instant::now();
        let mut spent = 0usize;
        while spent < compute_budget {
            if !physical_responses_bound_v4(&self.last_physical_result) {
                self.last_physical_result = self.physical_search.run_v3(1)?;
                self.last_physical_metrics = self.last_physical_result.metrics.clone();
                self.work_units += 1;
                self.physical_work_units += 1;
                self.successor_searches_initialized = false;
                spent += 1;
                if !physical_responses_bound_v4(&self.last_physical_result) {
                    continue;
                }
            }

            if !self.successor_searches_initialized {
                self.initialize_successor_searches_v4()?;
            }
            if spent >= compute_budget {
                break;
            }
            self.last_physical_result = self.physical_search.current_result_v3();
            self.last_physical_metrics = self.last_physical_result.metrics.clone();
            if !self
                .last_physical_result
                .certified_optimal_actions
                .is_empty()
            {
                self.note_certificate_v4();
                break;
            }
            let tasks = self.rebuild_executable_frontier_v4();
            let Some(task) = tasks.first().cloned() else {
                break;
            };
            self.execute_task_v4(&task)?;
            self.work_units += 1;
            spent += 1;
            self.refresh_successor_slots_v4()?;
            self.last_physical_result = self.physical_search.current_result_v3();
            self.last_physical_metrics = self.last_physical_result.metrics.clone();
            if !self
                .last_physical_result
                .certified_optimal_actions
                .is_empty()
            {
                self.note_certificate_v4();
                break;
            }
        }
        self.search_wall_time += call_start.elapsed();
        self.result_v4()
    }

    pub fn current_result_v4(&mut self) -> Result<DynamicSearchResultV4, DynamicSearchErrorV4> {
        self.last_physical_result = self.physical_search.current_result_v3();
        self.last_physical_metrics = self.last_physical_result.metrics.clone();
        self.result_v4()
    }

    fn initialize_successor_searches_v4(&mut self) -> Result<(), DynamicSearchErrorV4> {
        let initialization_start = Instant::now();
        for alternative in &self.last_physical_result.physical_alternatives {
            let Some(binding) = alternative.successor_binding.as_ref() else {
                continue;
            };
            let Some(actor) = binding.next_actor else {
                self.successor_searches.insert(
                    alternative.stable_identity.clone(),
                    SuccessorSearchV4 {
                        successor_node_id: binding.successor_node_id.clone(),
                        search: None,
                        blocked_reason: Some("SUCCESSOR_ACTOR_UNAVAILABLE".to_owned()),
                        solved: false,
                        domain_revalidated_by_v1: false,
                        domain_admitted_by_v3: false,
                    },
                );
                continue;
            };
            let Some((state, decision, actual_actor, v3_domain_admitted)) = self
                .physical_search
                .successor_position_v3(&alternative.stable_identity)
            else {
                return Err(DynamicSearchErrorV4::InvalidSuccessorBinding);
            };
            if actual_actor != actor {
                return Err(DynamicSearchErrorV4::InvalidSuccessorBinding);
            }
            let search = DynamicEngineSearchV1::new_with_root_player_v1(
                state,
                decision.clone(),
                self.root_player,
            );
            let (search, blocked_reason) = match search {
                Ok(search) => (Some(search), None),
                Err(DynamicSearchErrorV1::UnsupportedDecision) => (
                    None,
                    Some("UNSUPPORTED_SUCCESSOR_DECISION_OR_DOMAIN".to_owned()),
                ),
                Err(DynamicSearchErrorV1::InvalidDecisionFrame) => {
                    (None, Some("INVALID_OR_STALE_SUCCESSOR_FRAME".to_owned()))
                }
            };
            let domain_revalidated_by_v1 = search.is_some();
            self.successor_searches.insert(
                alternative.stable_identity.clone(),
                SuccessorSearchV4 {
                    successor_node_id: binding.successor_node_id.clone(),
                    search,
                    blocked_reason,
                    solved: false,
                    domain_revalidated_by_v1,
                    domain_admitted_by_v3: v3_domain_admitted,
                },
            );
        }
        self.successor_searches_initialized = true;
        self.refresh_successor_slots_v4()?;
        self.successor_initialization_wall_time += initialization_start.elapsed();
        Ok(())
    }

    fn refresh_successor_slots_v4(&mut self) -> Result<(), DynamicSearchErrorV4> {
        let alternatives = self
            .last_physical_result
            .physical_alternatives
            .iter()
            .map(|alternative| {
                (
                    alternative.stable_identity.clone(),
                    alternative.bounds,
                    alternative.successor_binding.as_ref().map(|binding| {
                        (
                            binding.physical_owner_id.clone(),
                            binding.successor_node_id.clone(),
                            binding.next_actor,
                        )
                    }),
                )
            })
            .collect::<Vec<_>>();
        for (alternative_id, old_bounds, binding) in alternatives {
            let Some((physical_owner_id, successor_node_id, next_actor)) = binding else {
                continue;
            };
            let Some(successor) = self.successor_searches.get_mut(&alternative_id) else {
                continue;
            };
            let Some(search) = successor.search.as_ref() else {
                self.physical_search.update_successor_bounds_v3(
                    &alternative_id,
                    old_bounds,
                    None,
                )?;
                continue;
            };
            let bounds = search.root_bounds_v1();
            let local_task = search.next_task_descriptor_v1();
            successor.solved = bounds.lower == bounds.upper;
            if !successor.solved && local_task.is_none() && successor.blocked_reason.is_none() {
                successor.blocked_reason = Some("NO_SCHEDULABLE_SUCCESSOR_TASK".to_owned());
            }
            let next_slot = if !successor.solved && successor.blocked_reason.is_none() {
                next_actor.map(|actor| ExpansionSlotIdentityV3 {
                    physical_owner_id: physical_owner_id.clone(),
                    graph_owner_id: successor_node_id.clone(),
                    actor,
                    protocol_identity: V4_ENGINE_SEARCH_PROTOCOL.to_owned(),
                    stage_identity: V4_SUCCESSOR_SEARCH_STAGE.to_owned(),
                    slot_id: format!("continue:{alternative_id}"),
                    action_order: 0,
                    root_distance: 1,
                    estimated_cost_bucket: 1,
                })
            } else {
                None
            };
            if bounds != old_bounds || next_slot.is_some() {
                self.bound_updates += u64::from(bounds != old_bounds);
            }
            self.physical_search
                .update_successor_bounds_v3(&alternative_id, bounds, next_slot)?;
        }
        Ok(())
    }

    fn rebuild_executable_frontier_v4(&mut self) -> Vec<ExpansionTaskV4> {
        self.outer_frontier_rebuilds += 1;
        let mut physical_support = BTreeMap::<String, u8>::new();
        for root_task in &self.last_physical_result.frontier.tasks {
            for support_id in &root_task.root_action_support_ids {
                let role = if root_task
                    .role_mask
                    .contains(RootRoleMaskV3::INCUMBENT_LOWER)
                {
                    RootRoleMaskV3::INCUMBENT_LOWER
                } else {
                    0
                } | if root_task
                    .role_mask
                    .contains(RootRoleMaskV3::CHALLENGER_UPPER)
                {
                    RootRoleMaskV3::CHALLENGER_UPPER
                } else {
                    0
                };
                *physical_support.entry(support_id.clone()).or_default() |= role;
            }
        }
        let mut tasks = BTreeMap::<String, ExpansionTaskV4>::new();
        for (physical_id, root_role_mask) in physical_support {
            let Some(successor) = self.successor_searches.get(&physical_id) else {
                continue;
            };
            if successor.blocked_reason.is_some() || successor.solved {
                continue;
            }
            let Some(search) = successor.search.as_ref() else {
                continue;
            };
            let Some(local) = search.next_task_descriptor_v1() else {
                continue;
            };
            let owner_node_identity =
                path_owner_identity_v4(&successor.successor_node_id, &local.owner_semantic_path);
            let task = ExpansionTaskV4 {
                physical_root_action_id: physical_id.clone(),
                root_role_mask,
                successor_role_mask: local.role_mask,
                owner_node_identity: owner_node_identity.clone(),
                actor: local.actor,
                action_order: local.action_order,
                action_id: local.action_id.clone(),
                bound_width: local.bound_width,
                root_distance: local.min_root_distance.saturating_add(1),
                estimated_cost_bucket: local.estimated_cost_bucket,
            };
            tasks.insert(format!("{owner_node_identity}|{}", local.action_id), task);
        }
        let mut tasks = tasks.into_values().collect::<Vec<_>>();
        tasks.sort_by_key(ExpansionTaskV4::stable_key);
        tasks
    }

    fn execute_task_v4(&mut self, task: &ExpansionTaskV4) -> Result<(), DynamicSearchErrorV4> {
        self.successor_expansion_attempts += 1;
        let Some(successor) = self
            .successor_searches
            .get_mut(&task.physical_root_action_id)
        else {
            return Err(DynamicSearchErrorV4::InvalidSuccessorBinding);
        };
        let Some(search) = successor.search.as_mut() else {
            return Err(DynamicSearchErrorV4::InvalidSuccessorBinding);
        };
        let Some(descriptor) = search.next_task_descriptor_v1() else {
            successor.blocked_reason = Some("NO_SCHEDULABLE_SUCCESSOR_TASK".to_owned());
            return Ok(());
        };
        if descriptor.action_id != task.action_id
            || path_owner_identity_v4(
                &successor.successor_node_id,
                &descriptor.owner_semantic_path,
            ) != task.owner_node_identity
        {
            successor.blocked_reason = Some("STALE_FRONTIER_TASK".to_owned());
            return Ok(());
        }
        match search.expand_task_descriptor_v1(&descriptor) {
            Ok(()) => {}
            Err(DynamicSearchStatusV1::UnsupportedDecision) => {
                successor.blocked_reason = Some(format!(
                    "UNSUPPORTED_SUCCESSOR_ACTION:{}",
                    descriptor.action_id
                ));
            }
            Err(DynamicSearchStatusV1::UnresolvedWithinBudget) => {
                successor.blocked_reason = Some(format!(
                    "CYCLE_OR_STALE_SUCCESSOR_TASK:{}",
                    descriptor.action_id
                ));
            }
            Err(DynamicSearchStatusV1::Certified) => {
                unreachable!("one-task expansion never returns a search-result status")
            }
        }
        Ok(())
    }

    fn note_certificate_v4(&mut self) {
        if self.first_certificate_at.is_none()
            && !self
                .last_physical_result
                .certified_optimal_actions
                .is_empty()
        {
            self.first_certificate_at = Some(self.work_units);
        }
    }

    fn result_v4(&mut self) -> Result<DynamicSearchResultV4, DynamicSearchErrorV4> {
        self.last_physical_result = self.physical_search.current_result_v3();
        self.last_physical_metrics = self.last_physical_result.metrics.clone();
        let certified = self.last_physical_result.certified_optimal_actions.clone();
        if !certified.is_empty() {
            self.note_certificate_v4();
        }
        let blocked = self
            .successor_searches
            .values()
            .any(|search| search.blocked_reason.is_some());
        let status = if self.last_physical_result.root_bounds.lower
            == self.last_physical_result.root_bounds.upper
        {
            DynamicSearchStatusV4::ExactRootValue
        } else if !certified.is_empty() {
            DynamicSearchStatusV4::CertifiedRootAction
        } else if blocked
            || matches!(
                self.last_physical_result.frontier.status,
                RootCriticalFrontierStatusV3::BlockedOnUnevaluatedSuccessor
                    | RootCriticalFrontierStatusV3::ReadyAndBlockedOnUnevaluatedSuccessor
            )
        {
            DynamicSearchStatusV4::BlockedOnUnsupportedSuccessor
        } else {
            DynamicSearchStatusV4::UnresolvedWithinBudget
        };
        let tasks = if self.successor_searches_initialized {
            self.rebuild_executable_frontier_v4()
        } else {
            Vec::new()
        };
        let unresolved_reasons = self
            .successor_searches
            .iter()
            .filter_map(|(id, search)| {
                search
                    .blocked_reason
                    .as_ref()
                    .map(|reason| (id.clone(), reason.clone()))
            })
            .collect::<BTreeMap<_, _>>();
        let physical_root_actions = self
            .last_physical_result
            .physical_alternatives
            .iter()
            .map(|alternative| {
                let search = self.successor_searches.get(&alternative.stable_identity);
                PhysicalRootActionResultV4 {
                    stable_identity: alternative.stable_identity.clone(),
                    ordered_engine_responses: alternative.ordered_engine_responses.clone(),
                    state: alternative.state,
                    bounds: alternative.bounds,
                    successor_node_id: alternative
                        .successor_binding
                        .as_ref()
                        .map(|binding| binding.successor_node_id.clone()),
                    successor_actor: alternative
                        .successor_binding
                        .as_ref()
                        .and_then(|binding| binding.next_actor),
                    successor_search_state: search.map_or(
                        SuccessorSearchStateV4::NotStarted,
                        |search| {
                            if search.blocked_reason.is_some() {
                                SuccessorSearchStateV4::Blocked
                            } else if search.solved {
                                SuccessorSearchStateV4::Solved
                            } else {
                                SuccessorSearchStateV4::Ready
                            }
                        },
                    ),
                    unresolved_reason: search.and_then(|search| search.blocked_reason.clone()),
                }
            })
            .collect::<Vec<_>>();
        let alternative_count = physical_root_actions.len();
        let unknown_roots = physical_root_actions
            .iter()
            .filter(|alternative| alternative.bounds == BoundIntervalV1::UNKNOWN)
            .count();
        let unresolved_roots = physical_root_actions
            .iter()
            .filter(|alternative| alternative.bounds.lower != alternative.bounds.upper)
            .count();
        let inner_metrics = self
            .successor_searches
            .values()
            .filter_map(|search| search.search.as_ref())
            .map(DynamicEngineSearchV1::metrics_v1)
            .collect::<Vec<_>>();
        let inner_role_counts = self
            .successor_searches
            .values()
            .filter_map(|search| search.search.as_ref())
            .map(DynamicEngineSearchV1::role_node_counts_v1)
            .fold((0, 0), |(max_total, min_total), (max_count, min_count)| {
                (max_total + max_count, min_total + min_count)
            });
        let mut metrics = DynamicSearchMetricsV4 {
            search_expansion_units: self.work_units,
            physical_construction_units: self.physical_work_units,
            successor_expansion_attempts: self.successor_expansion_attempts,
            authoritative_transitions: self.last_physical_metrics.authoritative_transitions
                + inner_metrics
                    .iter()
                    .map(|metric| metric.authoritative_transitions)
                    .sum::<u64>(),
            state_clones: self.last_physical_metrics.state_clones
                + inner_metrics
                    .iter()
                    .map(|metric| metric.state_clones)
                    .sum::<u64>(),
            admitted_actions: self.last_physical_metrics.candidate_count
                + inner_metrics
                    .iter()
                    .map(|metric| metric.admitted_actions)
                    .sum::<u64>(),
            expanded_actions: self.last_physical_metrics.authoritative_transitions
                + inner_metrics
                    .iter()
                    .map(|metric| metric.expanded_actions)
                    .sum::<u64>(),
            bound_updates: self.bound_updates
                + inner_metrics
                    .iter()
                    .map(|metric| metric.bound_updates)
                    .sum::<u64>(),
            frontier_rebuilds: self.last_physical_metrics.frontier_rebuilds
                + self.outer_frontier_rebuilds
                + inner_metrics
                    .iter()
                    .map(|metric| metric.scheduler_rebuilds)
                    .sum::<u64>(),
            scheduler_rebuilds: self.outer_frontier_rebuilds
                + inner_metrics
                    .iter()
                    .map(|metric| metric.scheduler_rebuilds)
                    .sum::<u64>(),
            complete_physical_actions: physical_root_actions
                .iter()
                .filter(|alternative| {
                    alternative.state == PhysicalAlternativeStateV3::Completed
                        || alternative.state == PhysicalAlternativeStateV3::SuccessorExpanded
                })
                .count() as u64,
            successor_game_decisions_admitted: self
                .successor_searches
                .values()
                .filter_map(|entry| entry.search.as_ref())
                .map(|search| {
                    let game_nodes = search.node_kind_counts_v1().0;
                    if search.root_is_terminal_v1() {
                        game_nodes
                    } else {
                        game_nodes.saturating_sub(1)
                    }
                })
                .sum(),
            successor_construction_nodes_admitted: self
                .successor_searches
                .values()
                .filter_map(|entry| entry.search.as_ref())
                .map(|search| search.node_kind_counts_v1().1)
                .sum(),
            successor_terminal_nodes_admitted: self
                .successor_searches
                .values()
                .filter_map(|entry| entry.search.as_ref())
                .map(|search| search.node_kind_counts_v1().2)
                .sum(),
            successor_domains_revalidated: self
                .successor_searches
                .values()
                .filter(|search| search.domain_revalidated_by_v1)
                .count() as u64,
            successor_domains_admitted_by_v3: self
                .successor_searches
                .values()
                .filter(|search| search.domain_admitted_by_v3)
                .count() as u64,
            max_decision_nodes: inner_role_counts.0,
            min_decision_nodes: inner_role_counts.1,
            unknown_root_alternatives: unknown_roots as u64,
            unresolved_root_alternatives: unresolved_roots as u64,
            root_alternative_count: physical_root_actions.len() as u64,
            root_certifications: u64::from(!certified.is_empty()),
            expansions_to_first_certificate: self.first_certificate_at,
            initialization_wall_time: self.initialization_wall_time,
            successor_initialization_wall_time: self.successor_initialization_wall_time,
            search_wall_time: self.search_wall_time,
            cpu_time: None,
            peak_memory_bytes: None,
        };
        metrics.complete_physical_actions = metrics
            .complete_physical_actions
            .min(alternative_count as u64);
        let root_bounds = self.last_physical_result.root_bounds;
        // V3 intentionally never claims a value: V4's only value evidence is
        // the freshly propagated certified interval on the V3 root itself.
        let exact_root_value =
            (root_bounds.lower == root_bounds.upper).then_some(root_bounds.lower);
        Ok(DynamicSearchResultV4 {
            status,
            root_scheduler_policy: Self::SUCCESSOR_SEARCH_POLICY,
            successor_scheduler_policy: Self::INNER_SUCCESSOR_FRONTIER_POLICY,
            physical_root_domain_complete: self.last_physical_result.root_domain_complete,
            root_bounds,
            exact_root_value,
            certified_optimal_actions: certified.clone(),
            chosen_certified_action: certified.first().cloned(),
            physical_root_actions,
            unresolved_reasons,
            root_frontier: self.last_physical_result.frontier.clone(),
            executable_frontier: tasks,
            metrics,
        })
    }
}

fn path_owner_identity_v4(successor_node: &str, path: &[u32]) -> String {
    let mut owner = format!("{successor_node}|path");
    for component in path {
        owner.push('|');
        owner.push_str(&component.to_string());
    }
    owner
}

fn physical_responses_bound_v4(result: &DynamicSearchResultV3) -> bool {
    !result.physical_alternatives.is_empty()
        && result.physical_alternatives.iter().all(|alternative| {
            matches!(
                alternative.state,
                PhysicalAlternativeStateV3::Completed
                    | PhysicalAlternativeStateV3::SuccessorExpanded
            ) && alternative.successor_binding.is_some()
        })
}
