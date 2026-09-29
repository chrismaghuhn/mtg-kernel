//! Budgeted dynamic search over the authoritative engine transition boundary.
//!
//! This adapter deliberately uses path-local nodes and no transposition table.
//! Only complete, explicitly enumerated decision domains are admitted.
use crate::engine::{self, Action, Decision};
use crate::ids::PlayerId;
use crate::mads_v1::{
    backup_bounds_v1, critical_support_v1, BoundIntervalV1, ExpansionRoleMaskV1,
    ExpansionSemanticOrderV1, MadsRoleV1, WIN_V1,
};
use crate::state::GameState;

type DynamicFrontierSortKeyV1 = (
    u8,
    std::cmp::Reverse<u8>,
    u16,
    u16,
    Vec<u32>,
    ExpansionSemanticOrderV1,
);

pub(crate) fn dynamic_path_frontier_sort_key_v1(
    role_mask: u8,
    width: u8,
    distance: u16,
    cost_bucket: u16,
    owner_path: Vec<u32>,
    semantic_order: ExpansionSemanticOrderV1,
) -> DynamicFrontierSortKeyV1 {
    let role_rank = if role_mask & ExpansionRoleMaskV1::INCUMBENT_LOWER != 0
        && role_mask & ExpansionRoleMaskV1::CHALLENGER_UPPER != 0
    {
        0
    } else if role_mask & ExpansionRoleMaskV1::INCUMBENT_LOWER != 0 {
        1
    } else if role_mask & ExpansionRoleMaskV1::CHALLENGER_UPPER != 0 {
        2
    } else {
        3
    };
    (
        role_rank,
        std::cmp::Reverse(width),
        distance,
        cost_bucket,
        owner_path,
        semantic_order,
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DynamicSearchStatusV1 {
    Certified,
    UnresolvedWithinBudget,
    UnsupportedDecision,
}

impl DynamicSearchStatusV1 {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Certified => "CERTIFIED_OPTIMAL_ACTION",
            Self::UnresolvedWithinBudget => "UNRESOLVED_WITHIN_BUDGET",
            Self::UnsupportedDecision => "UNSUPPORTED_DECISION",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DynamicSearchErrorV1 {
    UnsupportedDecision,
    InvalidDecisionFrame,
}

impl std::fmt::Display for DynamicSearchErrorV1 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedDecision => f.write_str("UNSUPPORTED_DECISION"),
            Self::InvalidDecisionFrame => f.write_str("INVALID_ENGINE_DECISION_FRAME"),
        }
    }
}

impl std::error::Error for DynamicSearchErrorV1 {}

#[derive(Debug, Clone, PartialEq)]
pub struct DynamicRootActionV1 {
    pub stable_id: String,
    pub engine_action: Action,
    pub bounds: BoundIntervalV1,
    pub expanded: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DynamicSearchMetricsV1 {
    pub authoritative_transitions: u64,
    pub state_clones: u64,
    pub admitted_actions: u64,
    pub expanded_actions: u64,
    pub bound_updates: u64,
    pub scheduler_rebuilds: u64,
    pub root_certified: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DynamicSearchResultV1 {
    pub status: DynamicSearchStatusV1,
    pub chosen_action: Option<String>,
    pub chosen_engine_action: Option<Action>,
    pub certified_optimal_actions: Vec<String>,
    pub exact_root_value: Option<i8>,
    pub anytime_action: Option<String>,
    pub root_bounds: BoundIntervalV1,
    pub root_actions: Vec<DynamicRootActionV1>,
    pub metrics: DynamicSearchMetricsV1,
}

#[derive(Debug, Clone)]
struct Slot {
    id: String,
    action: Action,
    child: Option<usize>,
}
#[derive(Debug, Clone)]
struct Node {
    state: GameState,
    decision: Decision,
    role: Option<MadsRoleV1>,
    slots: Vec<Slot>,
    parent: Option<usize>,
    bounds: BoundIntervalV1,
    terminal: bool,
    semantic_path: Vec<u32>,
}

#[derive(Debug, Clone)]
struct DynamicExpansionTask {
    owner: usize,
    slot: usize,
    role_mask: u8,
    root_action_support: Vec<usize>,
    bound_width: u8,
    min_root_distance: u16,
    estimated_cost_bucket: u16,
    owner_semantic_path: Vec<u32>,
}

impl DynamicExpansionTask {
    fn sort_key(&self) -> DynamicFrontierSortKeyV1 {
        dynamic_path_frontier_sort_key_v1(
            self.role_mask,
            self.bound_width,
            self.min_root_distance,
            self.estimated_cost_bucket,
            self.owner_semantic_path.clone(),
            ExpansionSemanticOrderV1::GameAction {
                action_order: self.slot as u32,
            },
        )
    }
}

/// Owns independent state clones. `root_state` is borrowed only for the
/// initial clone; transitions are performed only when a scheduled slot expands.
#[derive(Debug)]
pub struct DynamicEngineSearchV1 {
    nodes: Vec<Node>,
    root: usize,
    root_player: PlayerId,
    metrics: DynamicSearchMetricsV1,
}

impl DynamicEngineSearchV1 {
    pub const API_VERSION: u16 = 1;
    pub const FRONTIER_POLICY: &'static str = "FRONTIER_REBUILD_DYNAMIC_PATH_V1";
    pub const SUPPORTED_DECISION_KINDS: &'static [&'static str] =
        &["CastSpellOrPass", "DeclareAttackers(empty)", "GameOver"];

    #[cfg(test)]
    pub(crate) fn debug_nodes_v1(&self) -> Vec<(GameState, Decision, BoundIntervalV1)> {
        self.nodes
            .iter()
            .map(|n| (n.state.clone(), n.decision.clone(), n.bounds))
            .collect()
    }

    #[cfg(test)]
    pub(crate) fn debug_next_task_v1(
        &self,
    ) -> Option<(
        GameState,
        Decision,
        String,
        u8,
        u8,
        u16,
        u16,
        Vec<u32>,
        Vec<usize>,
    )> {
        let task = self
            .build_frontier_v1()
            .into_values()
            .min_by_key(DynamicExpansionTask::sort_key)?;
        Some((
            self.nodes[task.owner].state.clone(),
            self.nodes[task.owner].decision.clone(),
            self.nodes[task.owner].slots[task.slot].id.clone(),
            task.role_mask,
            task.bound_width,
            task.min_root_distance,
            task.estimated_cost_bucket,
            task.owner_semantic_path,
            task.root_action_support,
        ))
    }

    #[cfg(test)]
    pub(crate) fn debug_frontier_v1(
        &self,
    ) -> Vec<(String, u8, u8, u16, u16, Vec<u32>, Vec<usize>)> {
        let mut tasks = self.build_frontier_v1().into_values().collect::<Vec<_>>();
        tasks.sort_by_key(DynamicExpansionTask::sort_key);
        tasks
            .into_iter()
            .map(|task| {
                (
                    self.nodes[task.owner].slots[task.slot].id.clone(),
                    task.role_mask,
                    task.bound_width,
                    task.min_root_distance,
                    task.estimated_cost_bucket,
                    task.owner_semantic_path,
                    task.root_action_support,
                )
            })
            .collect()
    }

    #[cfg(test)]
    pub(crate) fn debug_expand_semantic_task_v1(
        &mut self,
        owner_path: &[u32],
        action_id: &str,
    ) -> Result<(), DynamicSearchStatusV1> {
        let Some((owner, slot)) = self.nodes.iter().enumerate().find_map(|(owner, node)| {
            if node.semantic_path != owner_path {
                return None;
            }
            node.slots
                .iter()
                .position(|slot| slot.id == action_id && slot.child.is_none())
                .map(|slot| (owner, slot))
        }) else {
            return Err(DynamicSearchStatusV1::UnresolvedWithinBudget);
        };
        self.expand_slot_v1(owner, slot)
    }

    #[cfg(test)]
    pub(crate) fn debug_result_with_status_v1(
        &self,
        status: DynamicSearchStatusV1,
    ) -> DynamicSearchResultV1 {
        self.result(status)
    }

    pub fn new(
        root_state: &GameState,
        root_decision: Decision,
    ) -> Result<Self, DynamicSearchErrorV1> {
        let root_player = actor(&root_decision).ok_or(DynamicSearchErrorV1::UnsupportedDecision)?;
        let mut root_binding = root_state.clone();
        let authoritative_decision = engine::advance_until_decision(&mut root_binding);
        if root_binding != *root_state || authoritative_decision != root_decision {
            return Err(DynamicSearchErrorV1::InvalidDecisionFrame);
        }
        let root = admit(root_binding, root_decision, root_player, None, 0, &[])
            .map_err(|_| DynamicSearchErrorV1::UnsupportedDecision)?;
        let admitted = root.slots.len() as u64;
        Ok(Self {
            nodes: vec![root],
            root: 0,
            root_player,
            metrics: DynamicSearchMetricsV1 {
                admitted_actions: admitted,
                state_clones: 1,
                ..Default::default()
            },
        })
    }

    pub fn run_v1(&mut self, compute_budget: usize) -> DynamicSearchResultV1 {
        let mut expanded = 0;
        while expanded < compute_budget {
            self.metrics.scheduler_rebuilds += 1;
            let Some((n, a)) = self.next_task() else {
                break;
            };
            if let Err(status) = self.expand_slot_v1(n, a) {
                return self.result(status);
            }
            expanded += 1;
        }
        let status = if self.certified().is_empty() {
            DynamicSearchStatusV1::UnresolvedWithinBudget
        } else {
            DynamicSearchStatusV1::Certified
        };
        self.result(status)
    }

    fn expand_slot_v1(&mut self, n: usize, a: usize) -> Result<(), DynamicSearchStatusV1> {
        let (mut state, action, depth) = {
            let node = &self.nodes[n];
            (
                node.state.clone(),
                node.slots[a].action.clone(),
                node_depth(&self.nodes, n) + 1,
            )
        };
        self.metrics.state_clones += 1;
        if engine::step(&mut state, action).is_err() {
            return Err(DynamicSearchStatusV1::UnsupportedDecision);
        }
        self.metrics.authoritative_transitions += 1;
        self.metrics.expanded_actions += 1;
        let decision = engine::advance_until_decision(&mut state);
        let child = match admit(
            state,
            decision,
            self.root_player,
            Some(n),
            depth,
            &self.nodes,
        ) {
            Ok(n) => n,
            Err(error) if error == "CYCLE_DETECTED" => {
                return Err(DynamicSearchStatusV1::UnresolvedWithinBudget)
            }
            Err(_) => return Err(DynamicSearchStatusV1::UnsupportedDecision),
        };
        let mut child = child;
        child.semantic_path = self.nodes[n].semantic_path.clone();
        child.semantic_path.push(a as u32);
        self.metrics.admitted_actions += child.slots.len() as u64;
        let index = self.nodes.len();
        self.nodes.push(child);
        self.nodes[n].slots[a].child = Some(index);
        self.recompute();
        Ok(())
    }

    fn next_task(&self) -> Option<(usize, usize)> {
        self.build_frontier_v1()
            .into_values()
            .min_by_key(DynamicExpansionTask::sort_key)
            .map(|task| (task.owner, task.slot))
    }

    fn build_frontier_v1(
        &self,
    ) -> std::collections::BTreeMap<(usize, usize), DynamicExpansionTask> {
        // Reference frontier rebuild: collect every critical support task,
        // merge role masks/root support, then use MADS-01's exact sort key.
        let slots = &self.nodes[self.root].slots;
        let bounds = slots
            .iter()
            .map(|s| {
                s.child
                    .map_or(BoundIntervalV1::UNKNOWN, |c| self.nodes[c].bounds)
            })
            .collect::<Vec<_>>();
        let Some(incumbent) =
            (0..slots.len()).max_by_key(|&i| (bounds[i].lower, std::cmp::Reverse(i)))
        else {
            return std::collections::BTreeMap::new();
        };
        let challenger = (0..slots.len())
            .filter(|&i| i != incumbent)
            .max_by_key(|&i| (bounds[i].upper, std::cmp::Reverse(i)));
        let mut tasks = std::collections::BTreeMap::<(usize, usize), DynamicExpansionTask>::new();
        for (root_action, is_incumbent) in [(Some(incumbent), true), (challenger, false)] {
            let Some(root_action) = root_action else {
                continue;
            };
            if self.nodes[self.root].slots[root_action].child.is_none() {
                self.insert_dynamic_task(
                    &mut tasks,
                    self.root,
                    root_action,
                    is_incumbent,
                    root_action,
                    0,
                );
                continue;
            }
            let Some(child) = self.nodes[self.root].slots[root_action].child else {
                continue;
            };
            self.collect_dynamic_support(
                child,
                is_incumbent,
                root_action,
                1,
                &mut std::collections::BTreeSet::new(),
                &mut tasks,
            );
        }
        tasks
    }

    fn collect_dynamic_support(
        &self,
        node_index: usize,
        incumbent: bool,
        root_action: usize,
        distance: u16,
        visited: &mut std::collections::BTreeSet<(usize, bool)>,
        tasks: &mut std::collections::BTreeMap<(usize, usize), DynamicExpansionTask>,
    ) {
        if !visited.insert((node_index, incumbent)) {
            return;
        }
        let node = &self.nodes[node_index];
        let Some(role) = node.role else { return };
        for (slot_index, slot) in node.slots.iter().enumerate() {
            if let Some(child) = slot.child {
                if critical_support_v1(role, incumbent, node.bounds, Some(self.nodes[child].bounds))
                {
                    self.collect_dynamic_support(
                        child,
                        incumbent,
                        root_action,
                        distance + 1,
                        visited,
                        tasks,
                    );
                }
            } else if critical_support_v1(role, incumbent, node.bounds, None) {
                self.insert_dynamic_task(
                    tasks,
                    node_index,
                    slot_index,
                    incumbent,
                    root_action,
                    distance,
                );
            }
        }
    }

    fn insert_dynamic_task(
        &self,
        tasks: &mut std::collections::BTreeMap<(usize, usize), DynamicExpansionTask>,
        owner: usize,
        slot: usize,
        incumbent: bool,
        root_action: usize,
        distance: u16,
    ) {
        let node = &self.nodes[owner];
        let entry = tasks
            .entry((owner, slot))
            .or_insert_with(|| DynamicExpansionTask {
                owner,
                slot,
                role_mask: 0,
                root_action_support: Vec::new(),
                bound_width: node.bounds.width(),
                min_root_distance: distance,
                estimated_cost_bucket: 1,
                owner_semantic_path: node.semantic_path.clone(),
            });
        entry.role_mask |= if incumbent {
            ExpansionRoleMaskV1::INCUMBENT_LOWER
        } else {
            ExpansionRoleMaskV1::CHALLENGER_UPPER
        };
        entry.min_root_distance = entry.min_root_distance.min(distance);
        if !entry.root_action_support.contains(&root_action) {
            entry.root_action_support.push(root_action);
            entry.root_action_support.sort_unstable();
        }
    }

    fn recompute(&mut self) {
        for i in (0..self.nodes.len()).rev() {
            let node = &self.nodes[i];
            if node.terminal {
                continue;
            }
            let vals = node
                .slots
                .iter()
                .filter_map(|s| s.child.map(|c| self.nodes[c].bounds))
                .collect::<Vec<_>>();
            if vals.is_empty() {
                continue;
            }
            let b = backup_bounds_v1(node.role.unwrap(), &vals, node.slots.len());
            if b != self.nodes[i].bounds {
                self.nodes[i].bounds = b;
                self.metrics.bound_updates += 1;
            }
        }
    }
    fn certified(&self) -> Vec<String> {
        let root = &self.nodes[self.root];
        root.slots
            .iter()
            .enumerate()
            .filter_map(|(i, s)| {
                let b = s
                    .child
                    .map_or(BoundIntervalV1::UNKNOWN, |c| self.nodes[c].bounds);
                let other = root
                    .slots
                    .iter()
                    .enumerate()
                    .filter(|(j, _)| *j != i)
                    .map(|(_, x)| x.child.map_or(WIN_V1, |c| self.nodes[c].bounds.upper))
                    .max();
                other.is_none_or(|u| b.lower >= u).then(|| s.id.clone())
            })
            .collect()
    }
    fn result(&self, status: DynamicSearchStatusV1) -> DynamicSearchResultV1 {
        let root = &self.nodes[self.root];
        let actions = root
            .slots
            .iter()
            .map(|s| DynamicRootActionV1 {
                stable_id: s.id.clone(),
                engine_action: s.action.clone(),
                bounds: s
                    .child
                    .map_or(BoundIntervalV1::UNKNOWN, |c| self.nodes[c].bounds),
                expanded: s.child.is_some(),
            })
            .collect::<Vec<_>>();
        let cert = self.certified();
        let chosen = cert.first().cloned();
        let chosen_engine_action = root
            .slots
            .iter()
            .find(|slot| Some(&slot.id) == chosen.as_ref())
            .map(|slot| slot.action.clone());
        let root_bounds = root.bounds;
        let anytime = actions
            .iter()
            .max_by_key(|a| (a.bounds.lower, std::cmp::Reverse(a.stable_id.clone())))
            .map(|a| a.stable_id.clone());
        let mut metrics = self.metrics.clone();
        metrics.root_certified = chosen.is_some();
        DynamicSearchResultV1 {
            status,
            exact_root_value: (root_bounds.lower == root_bounds.upper).then_some(root_bounds.lower),
            chosen_action: chosen,
            chosen_engine_action,
            certified_optimal_actions: cert,
            anytime_action: anytime,
            root_bounds,
            root_actions: actions,
            metrics,
        }
    }
}

fn actor(d: &Decision) -> Option<PlayerId> {
    match d {
        Decision::CastSpellOrPass { player, .. } => Some(*player),
        Decision::DeclareAttackers { player, eligible } if eligible.is_empty() => Some(*player),
        _ => None,
    }
}
fn admit(
    state: GameState,
    decision: Decision,
    root: PlayerId,
    parent: Option<usize>,
    depth: usize,
    ancestors: &[Node],
) -> Result<Node, String> {
    if depth > 256 {
        return Err("CYCLE_DETECTED".into());
    }
    let mut cursor = parent;
    while let Some(index) = cursor {
        let prior = &ancestors[index];
        if prior.state == state && prior.decision == decision {
            return Err("CYCLE_DETECTED".into());
        }
        cursor = prior.parent;
    }
    let (actor, actions, terminal) = match &decision {
        Decision::GameOver { winner } => {
            let v = match winner {
                Some(p) if *p == root => 1,
                Some(_) => -1,
                None => 0,
            };
            (None, Vec::new(), Some(v))
        }
        Decision::CastSpellOrPass { player, .. } => (
            Some(*player),
            complete_cast_domain(&decision, &state)?,
            None,
        ),
        Decision::DeclareAttackers { player, eligible } if eligible.is_empty() => (
            Some(*player),
            vec![(
                "DeclareAttackers:[]".into(),
                Action::DeclareAttackers(Vec::new()),
            )],
            None,
        ),
        _ => return Err("UNSUPPORTED_DECISION".into()),
    };
    let slots = actions
        .into_iter()
        .map(|(id, action)| Slot {
            id,
            action,
            child: None,
        })
        .collect::<Vec<_>>();
    if terminal.is_none() && (slots.is_empty() || slots.len() > 100_000) {
        return Err("UNSUPPORTED_DECISION".into());
    }
    let bounds = terminal.map_or(BoundIntervalV1::UNKNOWN, BoundIntervalV1::exact);
    Ok(Node {
        state,
        decision,
        role: actor.map(|p| {
            if p == root {
                MadsRoleV1::Max
            } else {
                MadsRoleV1::Min
            }
        }),
        slots,
        parent,
        bounds,
        terminal: terminal.is_some(),
        semantic_path: Vec::new(),
    })
}
fn complete_cast_domain(d: &Decision, s: &GameState) -> Result<Vec<(String, Action)>, String> {
    let Decision::CastSpellOrPass {
        player,
        castable_spells,
        mana_abilities,
        land_drops,
        activatable_abilities,
        plot_actions,
    } = d
    else {
        return Err("UNSUPPORTED_DECISION".into());
    };
    let mut a = Vec::new();
    let color_id = |color: crate::mana::ManaColor| match color {
        crate::mana::ManaColor::W => "W",
        crate::mana::ManaColor::U => "U",
        crate::mana::ManaColor::B => "B",
        crate::mana::ManaColor::R => "R",
        crate::mana::ManaColor::G => "G",
        crate::mana::ManaColor::C => "C",
    };
    a.extend(
        castable_spells
            .iter()
            .map(|x| (format!("CastSpell:{}", x.0), Action::CastSpell(*x))),
    );
    for source in mana_abilities {
        let choices = engine::available_mana_ability_choices(*player, *source, s);
        let targets = engine::mana_ability_cost_targets(*player, *source, s);
        if choices.is_empty() {
            return Err("UNSUPPORTED_DECISION".into());
        }
        for choice in choices {
            if targets.is_empty() {
                let action =
                    if engine::available_mana_ability_choices(*player, *source, s).len() == 1 {
                        Action::ActivateManaAbility(*source)
                    } else {
                        Action::ActivateManaAbilityChoice(*source, choice)
                    };
                let id = match &action {
                    Action::ActivateManaAbility(source) => {
                        format!("ActivateManaAbility:{}", source.0)
                    }
                    Action::ActivateManaAbilityChoice(source, color) => format!(
                        "ActivateManaAbilityChoice:{}:{}",
                        source.0,
                        color_id(*color)
                    ),
                    _ => unreachable!(),
                };
                a.push((id, action));
            } else {
                for target in &targets {
                    let action =
                        Action::ActivateManaAbilityWithCostTarget(*source, choice, *target);
                    a.push((
                        format!(
                            "ActivateManaAbilityWithCostTarget:{}:{}:{}",
                            source.0,
                            color_id(choice),
                            target.0
                        ),
                        action,
                    ));
                }
            }
        }
    }
    a.extend(
        land_drops
            .iter()
            .map(|x| (format!("PlayLand:{}", x.0), Action::PlayLand(*x))),
    );
    a.extend(activatable_abilities.iter().map(|(x, b)| {
        (
            format!("ActivateAbility:{}:{b}", x.0),
            Action::ActivateAbility(*x, *b),
        )
    }));
    a.extend(
        plot_actions
            .iter()
            .map(|x| (format!("PlotSpell:{}", x.0), Action::PlotSpell(*x))),
    );
    a.push(("Pass".into(), Action::Pass));
    let mut seen = std::collections::BTreeSet::new();
    if a.iter().any(|(id, _)| !seen.insert(id.clone())) {
        return Err("UNSUPPORTED_DECISION".into());
    }
    Ok(a)
}
fn node_depth(nodes: &[Node], mut n: usize) -> usize {
    let mut depth = 0;
    while let Some(parent) = nodes[n].parent {
        depth += 1;
        n = parent;
    }
    depth
}
