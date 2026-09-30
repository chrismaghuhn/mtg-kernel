//! Typed post-transition decision classification for the MADS construction audit.
//!
//! This versioned adapter validates an authoritative quiescent decision frame
//! and captures a narrow PendingCast -> ChooseTargets construction context.
//! It is intentionally not wired into DynamicEngineSearchV1: a one-step
//! `Action::CastSpell` result is not a complete physical action certificate.
use crate::engine::{self, Action, Decision, PendingCast};
use crate::ids::PlayerId;
use crate::state::{GameState, Target};

pub const DECISION_CONSTRUCTION_ADAPTER_VERSION_V1: u16 = 1;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConstructionStageV1 {
    PendingCastTargetPick,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActorSwitchKindV1 {
    PriorityActorChanged,
    ApnapTriggerGroup,
    ChainLightningCopyDecision,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DecisionConstructionV1 {
    pub adapter_version: u16,
    pub initiator: PlayerId,
    pub stage: ConstructionStageV1,
    /// The engine's entire pending protocol object, including source contract,
    /// ordered targets/contracts, selected modes/costs and resume facts.
    pub pending_cast: PendingCast,
    /// Exact current raw decision and its engine-provided candidate ordering.
    pub decision: Decision,
    pub ordered_target_prefix: Vec<Target>,
    pub remaining_cardinality: u8,
    pub legal_candidates: Vec<Action>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ClassifiedDecisionV1 {
    GameDecision {
        actor: PlayerId,
        decision: Decision,
        construction_starting_actions: Vec<Action>,
    },
    Construction(Box<DecisionConstructionV1>),
    ActorSwitch {
        from: PlayerId,
        to: PlayerId,
        kind: ActorSwitchKindV1,
        decision: Decision,
        construction_starting_actions: Vec<Action>,
    },
    Terminal {
        winner: Option<PlayerId>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecisionClassificationErrorV1 {
    InvalidOrStaleDecisionFrame,
    UnsupportedDecision,
    InvalidConstructionContext,
}

/// Classify only an authoritative, quiescent post-transition frame.
/// Re-evaluation occurs on a clone and must leave it unchanged; this catches
/// stale/reduced `Decision` values without mutating the caller's game state.
pub fn classify_after_transition_v1(
    previous_actor: Option<PlayerId>,
    state: &GameState,
    decision: &Decision,
) -> Result<ClassifiedDecisionV1, DecisionClassificationErrorV1> {
    let mut reproduced_state = state.clone();
    let reproduced_decision = engine::advance_until_decision(&mut reproduced_state);
    if reproduced_state != *state || reproduced_decision != *decision {
        return Err(DecisionClassificationErrorV1::InvalidOrStaleDecisionFrame);
    }

    match decision {
        Decision::GameOver { winner } => {
            return Ok(ClassifiedDecisionV1::Terminal { winner: *winner })
        }
        Decision::Halted { .. } => return Err(DecisionClassificationErrorV1::UnsupportedDecision),
        _ => {}
    }

    if let Some(pending) = state.engine.pending_cast.as_ref() {
        return classify_pending_cast_target_v1(previous_actor, state, decision, pending);
    }
    if state.engine.pending_activation.is_some() {
        return Err(DecisionClassificationErrorV1::UnsupportedDecision);
    }

    let actor =
        decision_actor_v1(decision).ok_or(DecisionClassificationErrorV1::UnsupportedDecision)?;
    if let Some(from) = previous_actor.filter(|from| *from != actor) {
        if let Some(kind) = actor_switch_kind_v1(state, decision, actor) {
            return Ok(ClassifiedDecisionV1::ActorSwitch {
                from,
                to: actor,
                kind,
                decision: decision.clone(),
                construction_starting_actions: construction_starting_actions_v1(decision),
            });
        }
        return Err(DecisionClassificationErrorV1::UnsupportedDecision);
    }

    match decision {
        Decision::CastSpellOrPass { .. } => Ok(ClassifiedDecisionV1::GameDecision {
            actor,
            decision: decision.clone(),
            construction_starting_actions: construction_starting_actions_v1(decision),
        }),
        _ => Err(DecisionClassificationErrorV1::UnsupportedDecision),
    }
}

fn classify_pending_cast_target_v1(
    previous_actor: Option<PlayerId>,
    state: &GameState,
    decision: &Decision,
    pending: &PendingCast,
) -> Result<ClassifiedDecisionV1, DecisionClassificationErrorV1> {
    let Decision::ChooseTargets {
        player,
        spell,
        remaining,
        legal_targets,
        can_finish,
    } = decision
    else {
        // This adapter deliberately admits one proven cast continuation stage.
        // Other PendingCast stages fail closed until their full candidate and
        // completion protocol has equivalent evidence.
        return Err(DecisionClassificationErrorV1::UnsupportedDecision);
    };
    if previous_actor.is_some_and(|actor| actor != pending.controller)
        || *player != pending.controller
        || *spell != pending.spell
        || *remaining == 0
        || *can_finish
        || pending.targets_chosen.len() != pending.target_contracts.len()
        || state.stack.last().is_none_or(|item| {
            item.source != pending.spell || item.controller != pending.controller
        })
        || legal_targets.is_empty()
    {
        return Err(DecisionClassificationErrorV1::InvalidConstructionContext);
    }
    let mut unique_targets = std::collections::HashSet::new();
    if legal_targets
        .iter()
        .any(|target| !unique_targets.insert(*target))
    {
        return Err(DecisionClassificationErrorV1::InvalidConstructionContext);
    }
    let legal_candidates = legal_targets
        .iter()
        .copied()
        .map(Action::ChooseTarget)
        .collect();
    Ok(ClassifiedDecisionV1::Construction(Box::new(
        DecisionConstructionV1 {
            adapter_version: DECISION_CONSTRUCTION_ADAPTER_VERSION_V1,
            initiator: pending.controller,
            stage: ConstructionStageV1::PendingCastTargetPick,
            pending_cast: pending.clone(),
            decision: decision.clone(),
            ordered_target_prefix: pending.targets_chosen.clone(),
            remaining_cardinality: *remaining,
            legal_candidates,
        },
    )))
}

fn actor_switch_kind_v1(
    state: &GameState,
    decision: &Decision,
    actor: PlayerId,
) -> Option<ActorSwitchKindV1> {
    match decision {
        Decision::CastSpellOrPass { .. } => Some(ActorSwitchKindV1::PriorityActorChanged),
        Decision::OrderTriggers { player, pending } => {
            let group_len = state
                .engine
                .pending_triggers
                .iter()
                .take_while(|trigger| trigger.controller == *player)
                .count();
            (*player == actor
                && group_len >= 2
                && pending.as_slice() == &state.engine.pending_triggers[..group_len])
                .then_some(ActorSwitchKindV1::ApnapTriggerGroup)
        }
        Decision::ChooseSpellCopyPayment { player, spell } => state
            .engine
            .pending_spell_copy
            .as_ref()
            .filter(|copy| {
                copy.player == *player && copy.resolving_source == *spell && *player == actor
            })
            .map(|_| ActorSwitchKindV1::ChainLightningCopyDecision),
        Decision::ChooseSpellCopyRetarget { player, copy } => state
            .engine
            .pending_spell_copy
            .as_ref()
            .filter(|pending| {
                pending.player == *player && pending.copy_source == Some(*copy) && *player == actor
            })
            .map(|_| ActorSwitchKindV1::ChainLightningCopyDecision),
        _ => None,
    }
}

fn construction_starting_actions_v1(decision: &Decision) -> Vec<Action> {
    let Decision::CastSpellOrPass {
        castable_spells,
        land_drops,
        activatable_abilities,
        ..
    } = decision
    else {
        return Vec::new();
    };
    castable_spells
        .iter()
        .copied()
        .map(Action::CastSpell)
        .chain(land_drops.iter().copied().map(Action::PlayLand))
        .chain(
            activatable_abilities
                .iter()
                .copied()
                .map(|(source, ability)| Action::ActivateAbility(source, ability)),
        )
        .collect()
}
fn decision_actor_v1(decision: &Decision) -> Option<PlayerId> {
    match decision {
        Decision::CastSpellOrPass { player, .. }
        | Decision::ChooseTargets { player, .. }
        | Decision::ChooseCostTargets { player, .. }
        | Decision::ChooseCastMode { player, .. }
        | Decision::ChooseKicker { player, .. }
        | Decision::ChooseSpellMode { player, .. }
        | Decision::ChooseEffectOption { player, .. }
        | Decision::ChooseEffectTargets { player, .. }
        | Decision::ChooseOptionalCost { player, .. }
        | Decision::ChooseSpellCopyPayment { player, .. }
        | Decision::ChooseSpellCopyRetarget { player, .. }
        | Decision::ChooseMadnessCast { player, .. }
        | Decision::Discard { player, .. }
        | Decision::DeclareAttackers { player, .. }
        | Decision::DeclareBlockers { player, .. }
        | Decision::OrderTriggers { player, .. }
        | Decision::ChooseEffectBoolean { player, .. } => Some(*player),
        Decision::GameOver { .. } | Decision::Halted { .. } => None,
    }
}
