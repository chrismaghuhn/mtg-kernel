//! Budgeted dynamic search over the authoritative engine transition boundary.
//!
//! This adapter deliberately uses path-local nodes and no transposition table.
//! Only complete, explicitly enumerated decision domains are admitted.
use crate::engine::{self, Action, Decision};
use crate::ids::PlayerId;
use crate::mads_v1::{backup_bounds_v1, critical_support_v1, BoundIntervalV1, MadsRoleV1, WIN_V1};
use crate::state::GameState;

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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DynamicRootActionV1 {
    pub stable_id: String,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DynamicSearchResultV1 {
    pub status: DynamicSearchStatusV1,
    pub chosen_action: Option<String>,
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
    pub const FRONTIER_POLICY: &'static str = crate::mads_v1::FRONTIER_POLICY_V1;
    pub const SUPPORTED_DECISION_KINDS: &'static [&'static str] =
        &["CastSpellOrPass", "DeclareAttackers(empty)", "GameOver"];

    #[cfg(test)]
    pub(crate) fn debug_nodes_v1(&self) -> Vec<(GameState, Decision, BoundIntervalV1)> {
        self.nodes
            .iter()
            .map(|n| (n.state.clone(), n.decision.clone(), n.bounds))
            .collect()
    }

    pub fn new(
        root_state: &GameState,
        root_decision: Decision,
    ) -> Result<Self, DynamicSearchErrorV1> {
        let root_player = actor(&root_decision).ok_or(DynamicSearchErrorV1::UnsupportedDecision)?;
        let root = admit(root_state.clone(), root_decision, root_player, None, 0, &[])
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
                return self.result(DynamicSearchStatusV1::UnsupportedDecision);
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
                    return self.result(DynamicSearchStatusV1::UnresolvedWithinBudget)
                }
                Err(_) => return self.result(DynamicSearchStatusV1::UnsupportedDecision),
            };
            self.metrics.admitted_actions += child.slots.len() as u64;
            let index = self.nodes.len();
            self.nodes.push(child);
            self.nodes[n].slots[a].child = Some(index);
            self.recompute();
            expanded += 1;
        }
        let status = if self.certified().is_empty() {
            DynamicSearchStatusV1::UnresolvedWithinBudget
        } else {
            DynamicSearchStatusV1::Certified
        };
        self.result(status)
    }

    fn next_task(&self) -> Option<(usize, usize)> {
        // Root-critical order: incumbent lower support first, then challenger
        // upper support; deterministic node/action order breaks ties.
        let slots = &self.nodes[self.root].slots;
        let bounds = slots
            .iter()
            .map(|s| {
                s.child
                    .map_or(BoundIntervalV1::UNKNOWN, |c| self.nodes[c].bounds)
            })
            .collect::<Vec<_>>();
        let incumbent =
            (0..slots.len()).max_by_key(|&i| (bounds[i].lower, std::cmp::Reverse(i)))?;
        let challenger = (0..slots.len())
            .filter(|&i| i != incumbent)
            .max_by_key(|&i| (bounds[i].upper, std::cmp::Reverse(i)));
        for (root_action, is_incumbent) in [(Some(incumbent), true), (challenger, false)] {
            let Some(root_action) = root_action else {
                continue;
            };
            if self.nodes[self.root].slots[root_action].child.is_none() {
                return Some((self.root, root_action));
            }
            let mut stack = vec![self.nodes[self.root].slots[root_action].child?];
            while let Some(n) = stack.pop() {
                let node = &self.nodes[n];
                if let Some(role) = node.role {
                    if let Some(a) = node.slots.iter().position(|s| {
                        s.child.is_none()
                            && critical_support_v1(role, is_incumbent, node.bounds, None)
                    }) {
                        return Some((n, a));
                    }
                    for c in node
                        .slots
                        .iter()
                        .rev()
                        .filter_map(|s| s.child)
                        .filter(|&c| {
                            critical_support_v1(
                                role,
                                is_incumbent,
                                node.bounds,
                                Some(self.nodes[c].bounds),
                            )
                        })
                    {
                        stack.push(c);
                    }
                }
            }
        }
        None
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
                bounds: s
                    .child
                    .map_or(BoundIntervalV1::UNKNOWN, |c| self.nodes[c].bounds),
                expanded: s.child.is_some(),
            })
            .collect::<Vec<_>>();
        let cert = self.certified();
        let chosen = (status == DynamicSearchStatusV1::Certified)
            .then(|| cert.first().cloned())
            .flatten();
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
                a.push((format!("Mana:{}:{}", source.0, color_id(choice)), action));
            } else {
                for target in &targets {
                    a.push((
                        format!("ManaCost:{}:{}:{}", source.0, color_id(choice), target.0),
                        Action::ActivateManaAbilityWithCostTarget(*source, choice, *target),
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
