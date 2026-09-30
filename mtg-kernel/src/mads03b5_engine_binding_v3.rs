//! Authoritative, path-local Engine binding for the admitted Bolt/Pass V3 root.
//!
//! This adapter is deliberately narrower than the Engine and does not assign
//! game values. Frame and transition ids are run-local provenance handles, not
//! globally reusable state keys or authenticated signatures.

use crate::card_def::card_id_by_name;
use crate::engine::{self, Action, Decision};
use crate::ids::{ObjectId, PlayerId};
use crate::mads_decision_construction_v1::{
    classify_after_transition_v1, ActorSwitchKindV1, ClassifiedDecisionV1, ConstructionStageV1,
};
use crate::mads_v1::BoundIntervalV1;
use crate::mads_virtual_physical_root_v3::{
    CompleteCandidateDomainAttestationV3, CompleteConstructionDomainContextV3,
    CompleteDomainEvidenceV3, CompletePhysicalActionDomainV3, ConstructionSlotIdentityV3,
    ExpansionSlotIdentityV3, PhysicalAlternativeStateV3, PhysicalRootActionIdentityV3,
    PhysicalRootCandidateV3, PhysicalRootDomainAttestationV3, PrefixProgressAttestationV3,
    PrefixProgressProvenanceV3, RootCriticalFrontierSnapshotV3, SuccessorBindingV3,
    SuccessorProvenanceV3, UnresolvedConstructionEnvelopeV3, VirtualPhysicalRootErrorV3,
    VirtualPhysicalRootV3, VIRTUAL_PHYSICAL_ROOT_SCHEMA_VERSION_V3,
};
use crate::state::{FinalizedCastBindingV1, GameState, StackItemKind, Target};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};

const ENGINE_ACTION_ID_SCHEMA_V3: &str = "engine-action-projection.v3";
const ROOT_STAGE_V3: &str = "cast-spell-or-pass.v3";
const CAST_PROTOCOL_V3: &str = "pending-cast-target.v3";
const CAST_STAGE_V3: &str = "choose-targets.v3";
const ORDER_STRIDE: u32 = 1 << 16;
static NEXT_SESSION: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineBindingErrorV3 {
    InvalidOrStaleRootFrame,
    UnsupportedRootDomain,
    UnsupportedConstructionFrame,
    InvalidTargetContinuation,
    InvalidSuccessorFrame,
    EngineTransitionRejected,
    VirtualRoot(VirtualPhysicalRootErrorV3),
}

impl std::fmt::Display for EngineBindingErrorV3 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for EngineBindingErrorV3 {}
impl From<VirtualPhysicalRootErrorV3> for EngineBindingErrorV3 {
    fn from(value: VirtualPhysicalRootErrorV3) -> Self {
        Self::VirtualRoot(value)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DynamicEngineSearchMetricsV3 {
    pub authoritative_transitions: u64,
    pub state_clones: u64,
    pub candidate_count: u64,
    pub complete_physical_alternatives: u64,
    pub open_construction_envelopes: u64,
    pub successor_bindings: u64,
    pub frontier_rebuilds: u64,
    pub unknown_root_actions: u64,
    pub root_certificates: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PhysicalResponseV3 {
    pub stable_identity: String,
    pub admission_evidence_identity: String,
    pub ordered_engine_responses: Vec<Action>,
    pub state: PhysicalAlternativeStateV3,
    pub bounds: BoundIntervalV1,
    pub successor_binding: Option<SuccessorBindingV3>,
    pub next_expansion_slot: Option<ExpansionSlotIdentityV3>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DynamicSearchResultV3 {
    pub raw_root_actions: Vec<Action>,
    pub root_domain_complete: bool,
    pub root_bounds: BoundIntervalV1,
    pub exact_root_value: Option<i8>,
    pub certified_optimal_actions: Vec<String>,
    /// All admitted physical root candidates, including not-yet-finalized
    /// alternatives. `state` distinguishes candidate admission from completion.
    pub physical_alternatives: Vec<PhysicalResponseV3>,
    pub open_construction_envelopes: Vec<String>,
    pub frontier: RootCriticalFrontierSnapshotV3,
    pub metrics: DynamicEngineSearchMetricsV3,
}

#[derive(Debug, Clone)]
struct AuthoritativeFrameV3 {
    identity: String,
    state: GameState,
    decision: Decision,
    actor: PlayerId,
    candidates: Vec<Action>,
    candidate_domain_admitted: bool,
}

#[derive(Debug)]
struct SuccessorPositionV3 {
    state: GameState,
    decision: Decision,
    actor: PlayerId,
    candidate_domain_admitted: bool,
}

#[derive(Debug)]
pub struct DynamicEngineSearchV3 {
    owner_id: String,
    session_id: u64,
    root_actor: PlayerId,
    root_frame: AuthoritativeFrameV3,
    bolt: ObjectId,
    virtual_root: VirtualPhysicalRootV3,
    cast_envelope_id: String,
    target_frame: Option<AuthoritativeFrameV3>,
    cast_expanded: bool,
    pass_expanded: bool,
    expanded_targets: Vec<bool>,
    successor_positions: BTreeMap<String, SuccessorPositionV3>,
    metrics: DynamicEngineSearchMetricsV3,
}

impl DynamicEngineSearchV3 {
    pub const API_VERSION: u16 = 3;
    pub const FRONTIER_POLICY: &'static str =
        "FRONTIER_PHYSICAL_ROOT_ENUMERATION_V3_CAST_THEN_PASS_THEN_TARGET_ORDER";

    pub fn new(
        root_state: &GameState,
        root_decision: Decision,
    ) -> Result<Self, EngineBindingErrorV3> {
        let session_id = NEXT_SESSION.fetch_add(1, Ordering::Relaxed);
        let bolt_def =
            card_id_by_name("Lightning Bolt").ok_or(EngineBindingErrorV3::UnsupportedRootDomain)?;
        let mut metrics = DynamicEngineSearchMetricsV3::default();
        let mut root_probe = root_state.clone();
        metrics.state_clones += 1;
        let reproduced_root_decision = engine::advance_until_decision(&mut root_probe);
        if root_probe != *root_state || reproduced_root_decision != root_decision {
            return Err(EngineBindingErrorV3::InvalidOrStaleRootFrame);
        }
        let Decision::CastSpellOrPass {
            castable_spells,
            mana_abilities,
            land_drops,
            activatable_abilities,
            plot_actions,
            ..
        } = &root_decision
        else {
            return Err(EngineBindingErrorV3::UnsupportedRootDomain);
        };
        if castable_spells.len() != 1
            || !mana_abilities.is_empty()
            || !land_drops.is_empty()
            || !activatable_abilities.is_empty()
            || !plot_actions.is_empty()
            || root_state.objects.get(castable_spells[0]).card_def != bolt_def
        {
            return Err(EngineBindingErrorV3::UnsupportedRootDomain);
        }
        metrics.state_clones += 1; // retain the exact, validated root snapshot
        let root_frame = frame_from_validated_v3(
            root_state.clone(),
            root_decision,
            session_id,
            0,
            "root",
            &mut metrics,
        )?;
        metrics.state_clones += 1;
        let ClassifiedDecisionV1::GameDecision { actor, .. } =
            classify_after_transition_v1(None, &root_frame.state, &root_frame.decision)
                .map_err(|_| EngineBindingErrorV3::InvalidOrStaleRootFrame)?
        else {
            return Err(EngineBindingErrorV3::UnsupportedRootDomain);
        };
        if actor != root_frame.actor {
            return Err(EngineBindingErrorV3::InvalidOrStaleRootFrame);
        }
        let Decision::CastSpellOrPass {
            player,
            castable_spells,
            mana_abilities,
            land_drops,
            activatable_abilities,
            plot_actions,
        } = &root_frame.decision
        else {
            return Err(EngineBindingErrorV3::UnsupportedRootDomain);
        };
        if *player != root_frame.actor
            || castable_spells.len() != 1
            || !mana_abilities.is_empty()
            || !land_drops.is_empty()
            || !activatable_abilities.is_empty()
            || !plot_actions.is_empty()
        {
            return Err(EngineBindingErrorV3::UnsupportedRootDomain);
        }
        let bolt = castable_spells[0];
        if root_frame.state.objects.get(bolt).card_def != bolt_def
            || root_frame.candidates != [Action::CastSpell(bolt), Action::Pass]
        {
            return Err(EngineBindingErrorV3::UnsupportedRootDomain);
        }
        let root_actor = *player;
        let owner_id = root_frame.identity.clone();
        let mut virtual_root = VirtualPhysicalRootV3::new(owner_id.clone(), root_actor)?;
        let cast_id = engine_action_identity_v3(&Action::CastSpell(bolt))
            .ok_or(EngineBindingErrorV3::UnsupportedRootDomain)?;
        let pass_id = engine_action_identity_v3(&Action::Pass).expect("Pass has an identity");
        virtual_root.insert_unresolved_prefix(UnresolvedConstructionEnvelopeV3 {
            physical_owner_id: owner_id.clone(),
            owner_actor: root_actor,
            protocol_identity: CAST_PROTOCOL_V3.to_owned(),
            known_prefix: Vec::new(),
            continuation_identity: root_frame.identity.clone(),
            stable_order: 0,
            next_expansion_slot: slot_v3(
                &owner_id,
                &root_frame,
                CAST_PROTOCOL_V3,
                ROOT_STAGE_V3,
                &cast_id,
                0,
                0,
            ),
            bounds: BoundIntervalV1::UNKNOWN,
        })?;
        let pass_candidate = PhysicalRootCandidateV3 {
            identity: PhysicalRootActionIdentityV3 {
                schema_version: VIRTUAL_PHYSICAL_ROOT_SCHEMA_VERSION_V3,
                physical_owner_id: owner_id.clone(),
                owner_actor: root_actor,
                stable_order: ORDER_STRIDE,
                continuation_identity: root_frame.identity.clone(),
                construction_path_identity: Vec::new(),
                ordered_engine_responses: vec![pass_id.clone()],
            },
            construction_path: Vec::new(),
            next_expansion_slot: slot_v3(
                &owner_id,
                &root_frame,
                CAST_PROTOCOL_V3,
                ROOT_STAGE_V3,
                &pass_id,
                1,
                0,
            ),
            executed_response_prefix: Vec::new(),
        };
        virtual_root.insert_known_alternative(pass_candidate)?;
        let envelope_id = virtual_root
            .envelopes()
            .next()
            .expect("inserted cast envelope")
            .stable_prefix_id();
        attest_raw_root_domain_v3(&mut virtual_root, &root_frame.identity)?;
        Ok(Self {
            owner_id,
            session_id,
            root_actor,
            root_frame,
            bolt,
            virtual_root,
            cast_envelope_id: envelope_id,
            target_frame: None,
            cast_expanded: false,
            pass_expanded: false,
            expanded_targets: Vec::new(),
            successor_positions: BTreeMap::new(),
            metrics,
        })
    }

    pub(crate) fn successor_position_v3(
        &self,
        physical_alternative_id: &str,
    ) -> Option<(&GameState, &Decision, PlayerId, bool)> {
        self.successor_positions
            .get(physical_alternative_id)
            .map(|position| {
                (
                    &position.state,
                    &position.decision,
                    position.actor,
                    position.candidate_domain_admitted,
                )
            })
    }

    pub(crate) fn root_actor_v3(&self) -> PlayerId {
        self.root_actor
    }

    pub(crate) fn update_successor_bounds_v3(
        &mut self,
        physical_alternative_id: &str,
        bounds: BoundIntervalV1,
        next_expansion_slot: Option<ExpansionSlotIdentityV3>,
    ) -> Result<(), EngineBindingErrorV3> {
        self.virtual_root.update_successor_bounds(
            physical_alternative_id,
            bounds,
            next_expansion_slot,
        )?;
        Ok(())
    }

    /// Performs at most `compute_budget` authoritative response expansions.
    /// Root construction and candidate admission are atomic at the CastSpell
    /// transition; successor values intentionally remain UNKNOWN.
    pub fn run_v3(
        &mut self,
        compute_budget: usize,
    ) -> Result<DynamicSearchResultV3, EngineBindingErrorV3> {
        let mut spent = 0;
        while spent < compute_budget {
            if !self.cast_expanded {
                self.expand_cast_v3()?;
                spent += 1;
                continue;
            }
            if !self.pass_expanded {
                self.expand_pass_v3()?;
                spent += 1;
                continue;
            }
            let Some(index) = self.expanded_targets.iter().position(|expanded| !expanded) else {
                break;
            };
            self.expand_target_v3(index)?;
            spent += 1;
        }
        Ok(self.result_v3())
    }

    pub fn current_result_v3(&mut self) -> DynamicSearchResultV3 {
        self.result_v3()
    }

    fn expand_cast_v3(&mut self) -> Result<(), EngineBindingErrorV3> {
        let action = Action::CastSpell(self.bolt);
        let response_id = engine_action_identity_v3(&action).expect("admitted action projection");
        let mut state = self.root_frame.state.clone();
        self.metrics.state_clones += 1;
        engine::step(&mut state, action)
            .map_err(|_| EngineBindingErrorV3::EngineTransitionRejected)?;
        self.metrics.authoritative_transitions += 1;
        let decision = engine::advance_until_decision(&mut state);
        self.metrics.state_clones += 1; // classify_after_transition_v1 quiescence clone
        let ClassifiedDecisionV1::Construction(context) =
            classify_after_transition_v1(Some(self.root_actor), &state, &decision)
                .map_err(|_| EngineBindingErrorV3::UnsupportedConstructionFrame)?
        else {
            return Err(EngineBindingErrorV3::UnsupportedConstructionFrame);
        };
        if context.initiator != self.root_actor
            || context.stage != ConstructionStageV1::PendingCastTargetPick
            || context.pending_cast.spell != self.bolt
            || !context.ordered_target_prefix.is_empty()
            || !context.pending_cast.targets_chosen.is_empty()
            || context.remaining_cardinality != 1
            || context.legal_candidates.is_empty()
            || context.legal_candidates.len() >= ORDER_STRIDE as usize
            || context
                .legal_candidates
                .iter()
                .any(|a| !matches!(a, Action::ChooseTarget(_)))
        {
            return Err(EngineBindingErrorV3::InvalidTargetContinuation);
        }
        let cast_transition_id = transition_identity_v3(&self.root_frame.identity, &response_id, 1);
        let frame = frame_from_validated_v3(
            state,
            decision,
            self.session_id,
            1,
            &cast_transition_id,
            &mut self.metrics,
        )?;
        let target_ids = frame
            .candidates
            .iter()
            .map(|a| {
                engine_action_identity_v3(a)
                    .ok_or(EngineBindingErrorV3::UnsupportedConstructionFrame)
            })
            .collect::<Result<Vec<_>, _>>()?;
        if target_ids.len() != context.legal_candidates.len()
            || frame.candidates != context.legal_candidates
        {
            return Err(EngineBindingErrorV3::InvalidTargetContinuation);
        }
        // The exact transition advances the prefix. The B4 lifecycle method
        // explicitly rewrites the domain seal from old prefix id to new id.
        let target_slot_id = target_ids
            .first()
            .ok_or(EngineBindingErrorV3::InvalidTargetContinuation)?
            .clone();
        let target_slot = slot_v3(
            &self.owner_id,
            &frame,
            CAST_PROTOCOL_V3,
            CAST_STAGE_V3,
            &target_slot_id,
            0,
            1,
        );
        let next_envelope = self.virtual_root.advance_unresolved_prefix(
            &self.cast_envelope_id,
            vec![response_id.clone()],
            target_slot,
            PrefixProgressAttestationV3 {
                physical_owner_id: self.owner_id.clone(),
                actor: self.root_actor,
                from_continuation_identity: self.root_frame.identity.clone(),
                to_continuation_identity: frame.identity.clone(),
                applied_responses: vec![response_id.clone()],
                provenance: PrefixProgressProvenanceV3::AuthoritativeTransition {
                    source_frame_identity: self.root_frame.identity.clone(),
                    transition_identity: cast_transition_id,
                    response_identity: response_id,
                    successor_frame_identity: frame.identity.clone(),
                },
            },
        )?;
        self.cast_envelope_id = next_envelope;

        let mut candidates = Vec::new();
        let mut paths = Vec::new();
        for (index, (action, action_id)) in frame.candidates.iter().zip(&target_ids).enumerate() {
            let order = u32::try_from(index)
                .map_err(|_| EngineBindingErrorV3::UnsupportedConstructionFrame)?;
            let construction = ConstructionSlotIdentityV3 {
                physical_owner_id: self.owner_id.clone(),
                actor: context.initiator,
                protocol_identity: CAST_PROTOCOL_V3.to_owned(),
                stage_identity: CAST_STAGE_V3.to_owned(),
                response_index: 1,
                slot_id: action_id.clone(),
                action_order: order,
            };
            let full_path = vec![
                engine_action_identity_v3(&Action::CastSpell(self.bolt)).unwrap(),
                action_id.clone(),
            ];
            paths.push(full_path.clone());
            candidates.push(PhysicalRootCandidateV3 {
                identity: PhysicalRootActionIdentityV3 {
                    schema_version: VIRTUAL_PHYSICAL_ROOT_SCHEMA_VERSION_V3,
                    physical_owner_id: self.owner_id.clone(),
                    owner_actor: self.root_actor,
                    stable_order: order,
                    continuation_identity: frame.identity.clone(),
                    construction_path_identity: vec![construction.stable_identity()],
                    ordered_engine_responses: full_path,
                },
                construction_path: vec![construction],
                next_expansion_slot: slot_v3(
                    &self.owner_id,
                    &frame,
                    CAST_PROTOCOL_V3,
                    CAST_STAGE_V3,
                    action_id,
                    order,
                    1,
                ),
                executed_response_prefix: vec![engine_action_identity_v3(&Action::CastSpell(
                    self.bolt,
                ))
                .unwrap()],
            });
            debug_assert!(matches!(action, Action::ChooseTarget(_)));
        }
        let domain = CompletePhysicalActionDomainV3::new(
            CompleteConstructionDomainContextV3 {
                physical_owner_id: self.owner_id.clone(),
                owner_actor: self.root_actor,
                protocol_identity: CAST_PROTOCOL_V3.to_owned(),
                stage_identity: CAST_STAGE_V3.to_owned(),
                known_prefix: vec![
                    engine_action_identity_v3(&Action::CastSpell(self.bolt)).unwrap()
                ],
                continuation_identity: frame.identity.clone(),
            },
            CompleteCandidateDomainAttestationV3 {
                ordered_candidate_responses: paths,
                evidence: CompleteDomainEvidenceV3::AuthoritativeEngineFrame {
                    frame_identity: frame.identity.clone(),
                },
            },
            candidates,
        )?;
        self.virtual_root
            .admit_complete_domain(&self.cast_envelope_id, domain)?;
        self.cast_envelope_id.clear();
        self.expanded_targets = vec![false; frame.candidates.len()];
        self.target_frame = Some(frame);
        self.cast_expanded = true;
        Ok(())
    }

    fn expand_pass_v3(&mut self) -> Result<(), EngineBindingErrorV3> {
        let action = Action::Pass;
        let response_id = engine_action_identity_v3(&action).unwrap();
        let mut state = self.root_frame.state.clone();
        self.metrics.state_clones += 1;
        engine::step(&mut state, action)
            .map_err(|_| EngineBindingErrorV3::EngineTransitionRejected)?;
        self.metrics.authoritative_transitions += 1;
        let decision = engine::advance_until_decision(&mut state);
        self.metrics.state_clones += 1;
        let ClassifiedDecisionV1::ActorSwitch {
            from,
            to,
            kind: ActorSwitchKindV1::PriorityActorChanged,
            ..
        } = classify_after_transition_v1(Some(self.root_actor), &state, &decision)
            .map_err(|_| EngineBindingErrorV3::InvalidSuccessorFrame)?
        else {
            return Err(EngineBindingErrorV3::InvalidSuccessorFrame);
        };
        if from != self.root_actor
            || to != self.root_actor.opponent()
            || !matches!(&decision, Decision::CastSpellOrPass { player, .. } if *player == to)
        {
            return Err(EngineBindingErrorV3::InvalidSuccessorFrame);
        }
        let successor = successor_frame_v3(
            state,
            decision,
            self.session_id,
            2,
            &transition_identity_v3(&self.root_frame.identity, &response_id, 2),
            &mut self.metrics,
        )?;
        let id = self
            .virtual_root
            .alternatives()
            .find(|a| a.identity.ordered_engine_responses == [response_id.clone()])
            .map(|a| a.stable_id().to_owned())
            .ok_or(EngineBindingErrorV3::UnsupportedRootDomain)?;
        let transition_id = transition_identity_v3(&self.root_frame.identity, &response_id, 2);
        let binding = successor_binding_v3(SuccessorBindingInputV3 {
            owner: &self.owner_id,
            owner_frame: &self.root_frame.identity,
            ordered_responses: std::slice::from_ref(&response_id),
            alternative_id: &id,
            source_frame: &self.root_frame.identity,
            transition: &transition_id,
            transition_response: &response_id,
            successor: &successor.identity,
            finalization: &framed_local_v3(
                "engine-priority-handoff.v3",
                &[&decision_identity_v3(&successor.decision).unwrap_or_default()],
            ),
            next_actor: successor.actor,
        })?;
        self.virtual_root
            .complete_alternative(&id, &[response_id], binding)?;
        self.successor_positions.insert(
            id,
            SuccessorPositionV3 {
                state: successor.state,
                decision: successor.decision,
                actor: successor.actor,
                candidate_domain_admitted: successor.candidate_domain_admitted,
            },
        );
        self.pass_expanded = true;
        self.metrics.successor_bindings += 1;
        Ok(())
    }

    fn expand_target_v3(&mut self, index: usize) -> Result<(), EngineBindingErrorV3> {
        let admitted_frame = self
            .target_frame
            .as_ref()
            .ok_or(EngineBindingErrorV3::InvalidTargetContinuation)?;
        let admitted_frame_identity = admitted_frame.identity.clone();
        let admitted_decision = admitted_frame.decision.clone();
        let admitted_candidates = admitted_frame.candidates.clone();
        let action = admitted_candidates
            .get(index)
            .cloned()
            .ok_or(EngineBindingErrorV3::InvalidTargetContinuation)?;
        let response_id = engine_action_identity_v3(&action)
            .ok_or(EngineBindingErrorV3::UnsupportedConstructionFrame)?;
        let cast_id = engine_action_identity_v3(&Action::CastSpell(self.bolt)).unwrap();
        let responses = vec![cast_id.clone(), response_id.clone()];
        let alternative_id = self
            .virtual_root
            .alternatives()
            .find(|a| a.identity.ordered_engine_responses == responses)
            .map(|a| a.stable_id().to_owned())
            .ok_or(EngineBindingErrorV3::InvalidTargetContinuation)?;
        // Each complete physical answer is replayed from the original root
        // clone. A shared post-Cast frame is only the admitted candidate
        // domain; it is not a substitute for executing that branch's prefix.
        let mut state = self.root_frame.state.clone();
        self.metrics.state_clones += 1;
        engine::step(&mut state, Action::CastSpell(self.bolt))
            .map_err(|_| EngineBindingErrorV3::EngineTransitionRejected)?;
        self.metrics.authoritative_transitions += 1;
        let target_decision = engine::advance_until_decision(&mut state);
        self.metrics.state_clones += 1;
        let ClassifiedDecisionV1::Construction(context) =
            classify_after_transition_v1(Some(self.root_actor), &state, &target_decision)
                .map_err(|_| EngineBindingErrorV3::InvalidTargetContinuation)?
        else {
            return Err(EngineBindingErrorV3::InvalidTargetContinuation);
        };
        if context.initiator != self.root_actor
            || context.stage != ConstructionStageV1::PendingCastTargetPick
            || context.pending_cast.spell != self.bolt
            || context.remaining_cardinality != 1
            || context.legal_candidates != admitted_candidates
            || target_decision != admitted_decision
        {
            return Err(EngineBindingErrorV3::InvalidTargetContinuation);
        }
        let cast_transition_id = transition_identity_v3(&self.root_frame.identity, &cast_id, 1);
        let replayed_target_frame = frame_from_validated_v3(
            state,
            target_decision,
            self.session_id,
            1,
            &cast_transition_id,
            &mut self.metrics,
        )?;
        if replayed_target_frame.identity != admitted_frame_identity
            || replayed_target_frame.candidates != admitted_candidates
            || !self
                .target_frame
                .as_ref()
                .is_some_and(|admitted| admitted.state == replayed_target_frame.state)
        {
            return Err(EngineBindingErrorV3::InvalidTargetContinuation);
        }
        let mut state = replayed_target_frame.state;
        engine::step(&mut state, action.clone())
            .map_err(|_| EngineBindingErrorV3::EngineTransitionRejected)?;
        self.metrics.authoritative_transitions += 1;
        let decision = engine::advance_until_decision(&mut state);
        self.metrics.state_clones += 1;
        let ClassifiedDecisionV1::GameDecision { actor, .. } =
            classify_after_transition_v1(Some(self.root_actor), &state, &decision)
                .map_err(|_| EngineBindingErrorV3::InvalidSuccessorFrame)?
        else {
            return Err(EngineBindingErrorV3::InvalidSuccessorFrame);
        };
        let selected = match action {
            Action::ChooseTarget(target) => target,
            _ => return Err(EngineBindingErrorV3::InvalidTargetContinuation),
        };
        let finalized = state.engine.pending_cast.is_none()
            && state
                .objects
                .get(self.bolt)
                .v4
                .finalized_cast_binding
                .is_some_and(|binding| {
                    binding.x_value == 0 && binding.chosen_creature_cost.is_none()
                })
            && state.stack.last().is_some_and(|item| {
                item.kind == StackItemKind::Spell
                    && item.source == self.bolt
                    && item.controller == self.root_actor
                    && item.targets == [selected]
            });
        if !finalized || actor != self.root_actor {
            return Err(EngineBindingErrorV3::InvalidSuccessorFrame);
        }
        let transition_id =
            transition_identity_v3(&admitted_frame_identity, &response_id, 3 + index as u64);
        let successor = frame_from_validated_v3(
            state,
            decision,
            self.session_id,
            3 + index as u64,
            &transition_id,
            &mut self.metrics,
        )?;
        let finalized_binding = successor
            .state
            .objects
            .get(self.bolt)
            .v4
            .finalized_cast_binding
            .ok_or(EngineBindingErrorV3::InvalidSuccessorFrame)?;
        let binding = successor_binding_v3(SuccessorBindingInputV3 {
            owner: &self.owner_id,
            owner_frame: &self.root_frame.identity,
            ordered_responses: &responses,
            alternative_id: &alternative_id,
            source_frame: &admitted_frame_identity,
            transition: &transition_id,
            transition_response: &response_id,
            successor: &successor.identity,
            finalization: &finalization_identity_v3(
                self.bolt,
                selected,
                &successor.decision,
                finalized_binding,
            ),
            next_actor: actor,
        })?;
        self.virtual_root
            .complete_alternative(&alternative_id, &responses, binding)?;
        self.successor_positions.insert(
            alternative_id.clone(),
            SuccessorPositionV3 {
                state: successor.state,
                decision: successor.decision,
                actor: successor.actor,
                candidate_domain_admitted: successor.candidate_domain_admitted,
            },
        );
        self.expanded_targets[index] = true;
        self.metrics.successor_bindings += 1;
        Ok(())
    }

    fn result_v3(&mut self) -> DynamicSearchResultV3 {
        let frontier = self.virtual_root.rebuild_root_critical_frontier();
        let certified = self.virtual_root.certified_optimal_alternative_ids();
        let responses = self
            .virtual_root
            .alternatives()
            .map(|a| PhysicalResponseV3 {
                stable_identity: a.stable_id().to_owned(),
                admission_evidence_identity: a.admission_evidence_identity.clone(),
                ordered_engine_responses: a
                    .identity
                    .ordered_engine_responses
                    .iter()
                    .map(|id| action_from_id_v3(id).expect("adapter emitted supported action ID"))
                    .collect(),
                state: a.state,
                bounds: a.bounds,
                successor_binding: a.successor_binding.clone(),
                next_expansion_slot: a.next_expansion_slot.clone(),
            })
            .collect::<Vec<_>>();
        self.metrics.frontier_rebuilds += 1;
        let mut metrics = self.metrics.clone();
        metrics.complete_physical_alternatives = responses
            .iter()
            .filter(|r| {
                matches!(
                    r.state,
                    PhysicalAlternativeStateV3::Completed
                        | PhysicalAlternativeStateV3::SuccessorExpanded
                )
            })
            .count() as u64;
        metrics.open_construction_envelopes = self.virtual_root.envelopes().count() as u64;
        metrics.unknown_root_actions = responses
            .iter()
            .filter(|r| r.bounds == BoundIntervalV1::UNKNOWN)
            .count() as u64
            + metrics.open_construction_envelopes;
        metrics.root_certificates = certified.len() as u64;
        DynamicSearchResultV3 {
            raw_root_actions: self.root_frame.candidates.clone(),
            root_domain_complete: self.virtual_root.root_action_domain_complete(),
            root_bounds: self.virtual_root.root_bounds(),
            exact_root_value: None,
            certified_optimal_actions: certified,
            physical_alternatives: responses,
            open_construction_envelopes: self
                .virtual_root
                .envelopes()
                .map(|e| e.stable_prefix_id())
                .collect(),
            frontier,
            metrics,
        }
    }
}

fn frame_from_validated_v3(
    state: GameState,
    decision: Decision,
    session: u64,
    ordinal: u64,
    parent: &str,
    metrics: &mut DynamicEngineSearchMetricsV3,
) -> Result<AuthoritativeFrameV3, EngineBindingErrorV3> {
    let (actor, candidates): (PlayerId, Vec<Action>) = match &decision {
        Decision::CastSpellOrPass {
            player,
            castable_spells,
            mana_abilities,
            land_drops,
            activatable_abilities,
            plot_actions,
        } if mana_abilities.is_empty()
            && land_drops.is_empty()
            && activatable_abilities.is_empty()
            && plot_actions.is_empty() =>
        {
            (
                *player,
                castable_spells
                    .iter()
                    .copied()
                    .map(Action::CastSpell)
                    .chain(std::iter::once(Action::Pass))
                    .collect(),
            )
        }
        Decision::ChooseTargets {
            player,
            spell: _,
            remaining,
            legal_targets,
            can_finish,
        } if *remaining == 1
            && !*can_finish
            && legal_targets
                .iter()
                .collect::<std::collections::HashSet<_>>()
                .len()
                == legal_targets.len() =>
        {
            (
                *player,
                legal_targets
                    .iter()
                    .copied()
                    .map(Action::ChooseTarget)
                    .collect(),
            )
        }
        _ => return Err(EngineBindingErrorV3::UnsupportedConstructionFrame),
    };
    let candidate_ids = candidates
        .iter()
        .map(|a| {
            engine_action_identity_v3(a).ok_or(EngineBindingErrorV3::UnsupportedConstructionFrame)
        })
        .collect::<Result<Vec<_>, _>>()?;
    if candidate_ids.is_empty() {
        return Err(EngineBindingErrorV3::UnsupportedConstructionFrame);
    }
    let decision_id = decision_identity_v3(&decision)
        .ok_or(EngineBindingErrorV3::UnsupportedConstructionFrame)?;
    let identity = frame_identity_v3(
        session,
        ordinal,
        parent,
        actor,
        &decision_id,
        &candidate_ids,
    );
    metrics.candidate_count += candidates.len() as u64;
    Ok(AuthoritativeFrameV3 {
        identity,
        state,
        decision,
        actor,
        candidates,
        candidate_domain_admitted: true,
    })
}

fn successor_frame_v3(
    state: GameState,
    decision: Decision,
    session: u64,
    ordinal: u64,
    parent: &str,
    metrics: &mut DynamicEngineSearchMetricsV3,
) -> Result<AuthoritativeFrameV3, EngineBindingErrorV3> {
    let unsupported_priority_domain = matches!(
        &decision,
        Decision::CastSpellOrPass {
            mana_abilities,
            land_drops,
            activatable_abilities,
            plot_actions,
            ..
        } if !mana_abilities.is_empty()
            || !land_drops.is_empty()
            || !activatable_abilities.is_empty()
            || !plot_actions.is_empty()
    );
    if !unsupported_priority_domain {
        return frame_from_validated_v3(state, decision, session, ordinal, parent, metrics);
    }

    // Preserve the actual priority successor without claiming that this V3
    // adapter admitted its complete response domain. The V4 search will
    // independently admit that exact frame through its Engine search adapter
    // or keep it blocked and UNKNOWN.
    let Decision::CastSpellOrPass { player, .. } = &decision else {
        return Err(EngineBindingErrorV3::UnsupportedConstructionFrame);
    };
    let mut verification = state.clone();
    metrics.state_clones += 1;
    let reproduced = engine::advance_until_decision(&mut verification);
    if verification != state || reproduced != decision {
        return Err(EngineBindingErrorV3::InvalidOrStaleRootFrame);
    }
    let actor = *player;
    let decision_identity = decision_identity_v3(&decision)
        .ok_or(EngineBindingErrorV3::UnsupportedConstructionFrame)?;
    Ok(AuthoritativeFrameV3 {
        identity: frame_identity_v3(session, ordinal, parent, actor, &decision_identity, &[]),
        state,
        decision,
        actor,
        candidates: Vec::new(),
        candidate_domain_admitted: false,
    })
}

fn frame_identity_v3(
    session: u64,
    ordinal: u64,
    parent: &str,
    actor: PlayerId,
    decision_id: &str,
    candidate_ids: &[String],
) -> String {
    let mut parts = vec![
        session.to_string(),
        ordinal.to_string(),
        parent.to_owned(),
        actor.index().to_string(),
        decision_id.to_owned(),
        candidate_ids.len().to_string(),
    ];
    parts.extend(candidate_ids.iter().cloned());
    let part_refs = parts.iter().map(String::as_str).collect::<Vec<_>>();
    framed_local_v3("mads.engine-frame.v3", &part_refs)
}
fn slot_v3(
    owner: &str,
    frame: &AuthoritativeFrameV3,
    protocol: &str,
    stage: &str,
    slot: &str,
    order: u32,
    distance: u16,
) -> ExpansionSlotIdentityV3 {
    ExpansionSlotIdentityV3 {
        physical_owner_id: owner.to_owned(),
        graph_owner_id: frame.identity.clone(),
        actor: frame.actor,
        protocol_identity: protocol.to_owned(),
        stage_identity: stage.to_owned(),
        slot_id: slot.to_owned(),
        action_order: order,
        root_distance: distance,
        estimated_cost_bucket: 1,
    }
}
fn attest_raw_root_domain_v3(
    root: &mut VirtualPhysicalRootV3,
    root_frame_id: &str,
) -> Result<(), VirtualPhysicalRootErrorV3> {
    let mut entries = root
        .alternatives()
        .map(|a| (a.identity.stable_order, a.stable_id().to_owned()))
        .chain(
            root.envelopes()
                .map(|e| (e.stable_order, e.stable_prefix_id())),
        )
        .collect::<Vec<_>>();
    entries.sort_by_key(|(order, id)| (*order, id.clone()));
    root.attest_root_domain(PhysicalRootDomainAttestationV3 {
        root_decision_identity: root.owner_decision_id.clone(),
        root_actor: root.owner_actor,
        ordered_root_item_ids: entries.into_iter().map(|(_, id)| id).collect(),
        evidence: CompleteDomainEvidenceV3::AuthoritativeEngineFrame {
            frame_identity: root_frame_id.to_owned(),
        },
    })
}
struct SuccessorBindingInputV3<'a> {
    owner: &'a str,
    owner_frame: &'a str,
    ordered_responses: &'a [String],
    alternative_id: &'a str,
    source_frame: &'a str,
    transition: &'a str,
    transition_response: &'a str,
    successor: &'a str,
    finalization: &'a str,
    next_actor: PlayerId,
}

fn successor_binding_v3(
    input: SuccessorBindingInputV3<'_>,
) -> Result<SuccessorBindingV3, EngineBindingErrorV3> {
    let SuccessorBindingInputV3 {
        owner,
        owner_frame,
        ordered_responses,
        alternative_id,
        source_frame,
        transition,
        transition_response,
        successor,
        finalization,
        next_actor,
    } = input;
    Ok(SuccessorBindingV3 {
        physical_owner_id: owner.to_owned(),
        owner_graph_node_id: owner_frame.to_owned(),
        ordered_engine_responses: ordered_responses.to_vec(),
        finalization_boundary: finalization.to_owned(),
        successor_node_id: successor.to_owned(),
        next_expansion_slot: None,
        graph_path_node_ids: if source_frame == owner_frame {
            vec![owner_frame.to_owned(), successor.to_owned()]
        } else {
            vec![
                owner_frame.to_owned(),
                source_frame.to_owned(),
                successor.to_owned(),
            ]
        },
        next_actor: Some(next_actor),
        provenance: SuccessorProvenanceV3::AuthoritativeTransition {
            source_frame_identity: source_frame.to_owned(),
            transition_identity: transition.to_owned(),
            transition_response_identity: transition_response.to_owned(),
            physical_response_identity: alternative_id.to_owned(),
            successor_frame_identity: successor.to_owned(),
        },
    })
}
fn finalization_identity_v3(
    source: ObjectId,
    target: Target,
    decision: &Decision,
    binding: FinalizedCastBindingV1,
) -> String {
    // The admitted Bolt finalization contract requires no creature cost; encode
    // that typed fact directly instead of relying on Debug formatting.
    let cost = if binding.chosen_creature_cost.is_none() {
        "no-creature-cost"
    } else {
        "unsupported-cost"
    };
    framed_local_v3(
        "mads.cast-finalization.v3",
        &[
            &source.0.to_string(),
            &target_id_v3(target),
            &binding.x_value.to_string(),
            cost,
            &decision_identity_v3(decision).unwrap_or_default(),
        ],
    )
}
fn target_id_v3(target: Target) -> String {
    match target {
        Target::Player(p) => format!("player:{}", p.index()),
        Target::Object(o) => format!("object:{}", o.0),
    }
}
fn transition_identity_v3(source: &str, response: &str, ordinal: u64) -> String {
    framed_local_v3(
        "mads.engine-transition.v3",
        &[source, response, &ordinal.to_string()],
    )
}
pub fn engine_action_identity_v3(action: &Action) -> Option<String> {
    let fields = match action {
        Action::CastSpell(o) => vec!["cast".to_owned(), o.0.to_string()],
        Action::Pass => vec!["pass".to_owned()],
        Action::ChooseTarget(Target::Player(p)) => {
            vec!["target-player".to_owned(), p.index().to_string()]
        }
        Action::ChooseTarget(Target::Object(o)) => {
            vec!["target-object".to_owned(), o.0.to_string()]
        }
        _ => return None,
    };
    Some(framed_local_v3(
        ENGINE_ACTION_ID_SCHEMA_V3,
        &fields.iter().map(String::as_str).collect::<Vec<_>>(),
    ))
}
fn action_from_id_v3(id: &str) -> Option<Action> {
    let parts = unframe_local_v3(id, ENGINE_ACTION_ID_SCHEMA_V3)?;
    match parts.as_slice() {
        [kind, value] if kind == "cast" => Some(Action::CastSpell(ObjectId(value.parse().ok()?))),
        [kind] if kind == "pass" => Some(Action::Pass),
        [kind, value] if kind == "target-player" => Some(Action::ChooseTarget(Target::Player(
            PlayerId(value.parse().ok()?),
        ))),
        [kind, value] if kind == "target-object" => Some(Action::ChooseTarget(Target::Object(
            ObjectId(value.parse().ok()?),
        ))),
        _ => None,
    }
}
fn decision_identity_v3(decision: &Decision) -> Option<String> {
    match decision {
        Decision::CastSpellOrPass {
            player,
            castable_spells,
            mana_abilities,
            land_drops,
            activatable_abilities,
            plot_actions,
        } => {
            let mut fields = vec!["priority".to_owned(), player.index().to_string()];
            for (label, values) in [
                ("cast", castable_spells),
                ("mana", mana_abilities),
                ("land", land_drops),
                ("plot", plot_actions),
            ] {
                fields.push(label.to_owned());
                fields.extend(values.iter().map(|v| v.0.to_string()));
            }
            fields.push("activate".to_owned());
            fields.extend(
                activatable_abilities
                    .iter()
                    .flat_map(|(o, a)| [o.0.to_string(), a.to_string()]),
            );
            Some(framed_local_v3(
                "mads.engine-decision.v3",
                &fields.iter().map(String::as_str).collect::<Vec<_>>(),
            ))
        }
        Decision::ChooseTargets {
            player,
            spell,
            remaining,
            legal_targets,
            can_finish,
        } => {
            let mut fields = vec![
                "targets".to_owned(),
                player.index().to_string(),
                spell.0.to_string(),
                remaining.to_string(),
                can_finish.to_string(),
            ];
            fields.extend(legal_targets.iter().copied().map(target_id_v3));
            Some(framed_local_v3(
                "mads.engine-decision.v3",
                &fields.iter().map(String::as_str).collect::<Vec<_>>(),
            ))
        }
        _ => None,
    }
}
fn framed_local_v3(schema: &str, parts: &[&str]) -> String {
    let mut out = format!("{schema}|{}|", parts.len());
    for part in parts {
        out.push_str(&format!("{}:{part}", part.len()));
    }
    out
}
fn unframe_local_v3(value: &str, schema: &str) -> Option<Vec<String>> {
    let rest = value.strip_prefix(schema)?.strip_prefix('|')?;
    let (count, mut rest) = rest.split_once('|')?;
    let count: usize = count.parse().ok()?;
    let mut parts = Vec::with_capacity(count);
    for _ in 0..count {
        let colon = rest.find(':')?;
        let len: usize = rest[..colon].parse().ok()?;
        rest = &rest[colon + 1..];
        if rest.len() < len {
            return None;
        }
        let (part, tail) = rest.split_at(len);
        parts.push(part.to_owned());
        rest = tail;
    }
    rest.is_empty().then_some(parts)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn action_projection_round_trips_admitted_actions() {
        for action in [
            Action::CastSpell(ObjectId(7)),
            Action::Pass,
            Action::ChooseTarget(Target::Player(PlayerId::P1)),
            Action::ChooseTarget(Target::Object(ObjectId(9))),
        ] {
            let id = engine_action_identity_v3(&action).unwrap();
            assert!(!id.is_empty());
            assert_eq!(action_from_id_v3(&id), Some(action));
        }
    }
}
