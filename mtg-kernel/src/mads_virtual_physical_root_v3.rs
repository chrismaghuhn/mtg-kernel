//! Additive, path-local virtual root alternatives for complete physical actions.
//!
//! This module owns only the root identity/bound projection. It does not merge
//! engine states, perform Engine transitions, or change DynamicEngineSearchV1/V2.
use crate::ids::PlayerId;
use crate::mads_v1::{backup_bounds_v1, BoundIntervalV1, MadsRoleV1};
use std::collections::{BTreeMap, BTreeSet};

pub const VIRTUAL_PHYSICAL_ROOT_SCHEMA_V3: &str = "mads.virtual-physical-root.v3";
pub const VIRTUAL_PHYSICAL_ROOT_SCHEMA_VERSION_V3: u16 = 3;
pub const PHYSICAL_ROOT_FRONTIER_POLICY_V3: &str = "FRONTIER_ROOT_CRITICAL_PHYSICAL_V3";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VirtualPhysicalRootErrorV3 {
    EmptyIdentity,
    EmptyResponsePath,
    EmptyCandidateDomain,
    InvalidBounds,
    DuplicateEnvelope,
    DuplicateAlternative,
    OrderCollision,
    MissingEnvelope,
    DomainDoesNotMatchEnvelope,
    IncompleteCandidateAttestation,
    InvalidCandidate,
    InvalidProgress,
    InvalidSuccessorBinding,
    InvalidStateTransition,
    RootDomainNotAttested,
}

impl std::fmt::Display for VirtualPhysicalRootErrorV3 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for VirtualPhysicalRootErrorV3 {}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ExpansionSlotIdentityV3 {
    /// The physical decision whose root alternative is being evaluated.
    pub physical_owner_id: String,
    /// The exact current GameDecision or Construction node in the path-local graph.
    pub graph_owner_id: String,
    pub actor: PlayerId,
    pub protocol_identity: String,
    pub stage_identity: String,
    pub slot_id: String,
    pub action_order: u32,
    pub root_distance: u16,
    pub estimated_cost_bucket: u16,
}

impl ExpansionSlotIdentityV3 {
    fn validate(&self, owner_id: &str, actor: PlayerId) -> Result<(), VirtualPhysicalRootErrorV3> {
        if self.physical_owner_id != owner_id
            || self.actor != actor
            || self.graph_owner_id.is_empty()
            || self.protocol_identity.is_empty()
            || self.stage_identity.is_empty()
            || self.slot_id.is_empty()
        {
            return Err(VirtualPhysicalRootErrorV3::EmptyIdentity);
        }
        Ok(())
    }

    pub fn stable_identity(&self) -> String {
        framed_identity_v3(
            "mads.expansion-slot.v3",
            &[
                &self.physical_owner_id,
                &self.graph_owner_id,
                &self.actor.index().to_string(),
                &self.protocol_identity,
                &self.stage_identity,
                &self.slot_id,
                &self.action_order.to_string(),
            ],
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ConstructionSlotIdentityV3 {
    pub physical_owner_id: String,
    pub actor: PlayerId,
    pub protocol_identity: String,
    pub stage_identity: String,
    /// Index of the response in `ordered_engine_responses`.
    pub response_index: usize,
    pub slot_id: String,
    pub action_order: u32,
}

impl ConstructionSlotIdentityV3 {
    pub fn stable_identity(&self) -> String {
        framed_identity_v3(
            "mads.construction-slot.v3",
            &[
                self.physical_owner_id.clone(),
                self.actor.index().to_string(),
                self.protocol_identity.clone(),
                self.stage_identity.clone(),
                self.response_index.to_string(),
                self.slot_id.clone(),
                self.action_order.to_string(),
            ],
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalRootActionIdentityV3 {
    pub schema_version: u16,
    pub physical_owner_id: String,
    pub owner_actor: PlayerId,
    pub stable_order: u32,
    pub continuation_identity: String,
    pub construction_path_identity: Vec<String>,
    pub ordered_engine_responses: Vec<String>,
}

impl PhysicalRootActionIdentityV3 {
    pub fn stable_semantic_id(&self) -> String {
        let mut parts = vec![
            self.physical_owner_id.clone(),
            self.owner_actor.index().to_string(),
            self.stable_order.to_string(),
            self.continuation_identity.clone(),
        ];
        parts.extend(self.construction_path_identity.iter().cloned());
        parts.push("ordered-responses".to_owned());
        parts.extend(self.ordered_engine_responses.iter().cloned());
        framed_identity_v3("mads.physical-root-action.v3", &parts)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuccessorBindingV3 {
    pub physical_owner_id: String,
    pub owner_graph_node_id: String,
    pub ordered_engine_responses: Vec<String>,
    pub finalization_boundary: String,
    pub successor_node_id: String,
    /// First expansion slot exposed by the successor GameDecision, if one is
    /// available. Missing work is reported as blocked, never as exhausted.
    pub next_expansion_slot: Option<ExpansionSlotIdentityV3>,
    /// Exact path-local ancestry at the point this successor was admitted.
    /// Repeated IDs are rejected as a cycle; this is not a reusable state key.
    pub graph_path_node_ids: Vec<String>,
    pub next_actor: Option<PlayerId>,
    pub provenance: SuccessorProvenanceV3,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SuccessorProvenanceV3 {
    AuthoritativeTransition {
        source_frame_identity: String,
        transition_identity: String,
        physical_response_identity: String,
    },
    OracleFixture {
        domain_fixture_identity: String,
        successor_fixture_identity: String,
        physical_response_identity: String,
    },
}

impl SuccessorBindingV3 {
    fn validate_for(
        &self,
        identity: &PhysicalRootActionIdentityV3,
        admission_evidence_identity: &str,
    ) -> Result<(), VirtualPhysicalRootErrorV3> {
        let response_identity = identity.stable_semantic_id();
        let provenance_matches = match &self.provenance {
            SuccessorProvenanceV3::AuthoritativeTransition {
                source_frame_identity,
                transition_identity,
                physical_response_identity,
            } => {
                source_frame_identity == admission_evidence_identity
                    && !transition_identity.is_empty()
                    && physical_response_identity == &response_identity
            }
            SuccessorProvenanceV3::OracleFixture {
                domain_fixture_identity,
                successor_fixture_identity,
                physical_response_identity,
            } => {
                domain_fixture_identity == admission_evidence_identity
                    && !successor_fixture_identity.is_empty()
                    && physical_response_identity == &response_identity
            }
        };
        if self.physical_owner_id != identity.physical_owner_id
            || self.owner_graph_node_id.is_empty()
            || self.ordered_engine_responses != identity.ordered_engine_responses
            || self.finalization_boundary.is_empty()
            || self.successor_node_id.is_empty()
            || self.graph_path_node_ids.first() != Some(&self.owner_graph_node_id)
            || self.graph_path_node_ids.last() != Some(&self.successor_node_id)
            || self.graph_path_node_ids.iter().any(String::is_empty)
            || self
                .graph_path_node_ids
                .iter()
                .collect::<BTreeSet<_>>()
                .len()
                != self.graph_path_node_ids.len()
            || !provenance_matches
            || self.next_expansion_slot.as_ref().is_some_and(|slot| {
                slot.validate(&self.physical_owner_id, slot.actor).is_err()
                    || slot.graph_owner_id != self.successor_node_id
                    || self.next_actor != Some(slot.actor)
            })
            || (self.next_actor.is_none() && self.next_expansion_slot.is_some())
        {
            return Err(VirtualPhysicalRootErrorV3::InvalidSuccessorBinding);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhysicalAlternativeStateV3 {
    KnownCompleteAlternativeNotExpanded,
    PartiallyConstructed,
    Completed,
    SuccessorExpanded,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalRootCandidateV3 {
    pub identity: PhysicalRootActionIdentityV3,
    pub construction_path: Vec<ConstructionSlotIdentityV3>,
    /// Current next authoritative action/choice slot. Shared prefix actions
    /// deliberately use the same slot identity across alternatives.
    pub next_expansion_slot: ExpansionSlotIdentityV3,
    /// Authoritative responses already applied before this candidate domain was
    /// admitted. Empty for a candidate known at the original decision frame.
    pub executed_response_prefix: Vec<String>,
}

impl PhysicalRootCandidateV3 {
    fn validate(&self) -> Result<(), VirtualPhysicalRootErrorV3> {
        let identity = &self.identity;
        if identity.schema_version != VIRTUAL_PHYSICAL_ROOT_SCHEMA_VERSION_V3
            || identity.physical_owner_id.is_empty()
            || identity.continuation_identity.is_empty()
            || identity.ordered_engine_responses.is_empty()
            || identity
                .ordered_engine_responses
                .iter()
                .any(String::is_empty)
            || self.executed_response_prefix.len() >= identity.ordered_engine_responses.len()
            || identity.ordered_engine_responses[..self.executed_response_prefix.len()]
                != self.executed_response_prefix
            || identity.construction_path_identity
                != self
                    .construction_path
                    .iter()
                    .map(ConstructionSlotIdentityV3::stable_identity)
                    .collect::<Vec<_>>()
        {
            return Err(VirtualPhysicalRootErrorV3::InvalidCandidate);
        }
        self.next_expansion_slot
            .validate(&identity.physical_owner_id, identity.owner_actor)?;
        if self.next_expansion_slot.slot_id
            != identity.ordered_engine_responses[self.executed_response_prefix.len()]
        {
            return Err(VirtualPhysicalRootErrorV3::InvalidCandidate);
        }
        for step in &self.construction_path {
            if step.physical_owner_id != identity.physical_owner_id
                || step.actor != identity.owner_actor
                || step.protocol_identity.is_empty()
                || step.stage_identity.is_empty()
                || step.slot_id.is_empty()
                || step.response_index >= identity.ordered_engine_responses.len()
                || identity.ordered_engine_responses[step.response_index] != step.slot_id
            {
                return Err(VirtualPhysicalRootErrorV3::InvalidCandidate);
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompleteDomainEvidenceV3 {
    AuthoritativeEngineFrame { frame_identity: String },
    OracleFixture { fixture_identity: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrefixProgressProvenanceV3 {
    AuthoritativeTransition { transition_identity: String },
    OracleFixture { fixture_identity: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrefixProgressAttestationV3 {
    pub physical_owner_id: String,
    pub actor: PlayerId,
    pub from_continuation_identity: String,
    pub to_continuation_identity: String,
    pub applied_responses: Vec<String>,
    pub provenance: PrefixProgressProvenanceV3,
}

impl PrefixProgressAttestationV3 {
    fn provenance_identity(&self) -> &str {
        match &self.provenance {
            PrefixProgressProvenanceV3::AuthoritativeTransition {
                transition_identity,
            } => transition_identity,
            PrefixProgressProvenanceV3::OracleFixture { fixture_identity } => fixture_identity,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompleteConstructionDomainContextV3 {
    pub physical_owner_id: String,
    pub owner_actor: PlayerId,
    pub protocol_identity: String,
    pub stage_identity: String,
    pub known_prefix: Vec<String>,
    pub continuation_identity: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompleteCandidateDomainAttestationV3 {
    pub ordered_candidate_responses: Vec<Vec<String>>,
    pub evidence: CompleteDomainEvidenceV3,
}

impl CompleteDomainEvidenceV3 {
    fn identity(&self) -> &str {
        match self {
            Self::AuthoritativeEngineFrame { frame_identity } => frame_identity,
            Self::OracleFixture { fixture_identity } => fixture_identity,
        }
    }
}

/// A caller attestation that `ordered_candidate_responses` is the entire legal
/// ordered domain observed at this owner/protocol/prefix. Structural validation
/// checks candidate equality; engine/oracle provenance must be established by
/// the caller and is not inferred from this value object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletePhysicalActionDomainV3 {
    pub physical_owner_id: String,
    pub owner_actor: PlayerId,
    pub protocol_identity: String,
    pub stage_identity: String,
    pub known_prefix: Vec<String>,
    pub continuation_identity: String,
    pub ordered_candidate_responses: Vec<Vec<String>>,
    pub evidence: CompleteDomainEvidenceV3,
    pub candidates: Vec<PhysicalRootCandidateV3>,
}

impl CompletePhysicalActionDomainV3 {
    pub fn new(
        context: CompleteConstructionDomainContextV3,
        attestation: CompleteCandidateDomainAttestationV3,
        candidates: Vec<PhysicalRootCandidateV3>,
    ) -> Result<Self, VirtualPhysicalRootErrorV3> {
        let domain = Self {
            physical_owner_id: context.physical_owner_id,
            owner_actor: context.owner_actor,
            protocol_identity: context.protocol_identity,
            stage_identity: context.stage_identity,
            known_prefix: context.known_prefix,
            continuation_identity: context.continuation_identity,
            ordered_candidate_responses: attestation.ordered_candidate_responses,
            evidence: attestation.evidence,
            candidates,
        };
        domain.validate()?;
        Ok(domain)
    }

    fn validate(&self) -> Result<(), VirtualPhysicalRootErrorV3> {
        if self.physical_owner_id.is_empty()
            || self.protocol_identity.is_empty()
            || self.stage_identity.is_empty()
            || self.continuation_identity.is_empty()
            || self.evidence.identity().is_empty()
            || self.candidates.is_empty()
        {
            return Err(VirtualPhysicalRootErrorV3::EmptyIdentity);
        }
        if self.ordered_candidate_responses.is_empty()
            || self
                .ordered_candidate_responses
                .iter()
                .any(|path| path.is_empty() || path.iter().any(String::is_empty))
        {
            return Err(VirtualPhysicalRootErrorV3::EmptyCandidateDomain);
        }
        let mut seen_paths = BTreeSet::new();
        let mut seen_orders = BTreeSet::new();
        let candidate_paths = self
            .candidates
            .iter()
            .map(|candidate| {
                candidate.validate()?;
                if candidate.identity.physical_owner_id != self.physical_owner_id
                    || candidate.identity.owner_actor != self.owner_actor
                    || candidate.identity.continuation_identity != self.continuation_identity
                    || candidate.executed_response_prefix != self.known_prefix
                    || candidate
                        .identity
                        .ordered_engine_responses
                        .get(..self.known_prefix.len())
                        != Some(self.known_prefix.as_slice())
                    || candidate
                        .construction_path
                        .iter()
                        .any(|step| step.protocol_identity != self.protocol_identity)
                    || candidate.next_expansion_slot.protocol_identity != self.protocol_identity
                    || candidate.next_expansion_slot.stage_identity != self.stage_identity
                    || !seen_orders.insert(candidate.identity.stable_order)
                    || !seen_paths.insert(candidate.identity.ordered_engine_responses.clone())
                {
                    return Err(VirtualPhysicalRootErrorV3::InvalidCandidate);
                }
                Ok(candidate.identity.ordered_engine_responses.clone())
            })
            .collect::<Result<Vec<_>, VirtualPhysicalRootErrorV3>>()?;
        if candidate_paths != self.ordered_candidate_responses {
            return Err(VirtualPhysicalRootErrorV3::IncompleteCandidateAttestation);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnresolvedConstructionEnvelopeV3 {
    pub physical_owner_id: String,
    pub owner_actor: PlayerId,
    pub protocol_identity: String,
    pub known_prefix: Vec<String>,
    pub continuation_identity: String,
    pub stable_order: u32,
    pub next_expansion_slot: ExpansionSlotIdentityV3,
    pub bounds: BoundIntervalV1,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalRootDomainAttestationV3 {
    pub root_decision_identity: String,
    pub root_actor: PlayerId,
    pub ordered_root_item_ids: Vec<String>,
    pub evidence: CompleteDomainEvidenceV3,
}

impl UnresolvedConstructionEnvelopeV3 {
    pub fn stable_prefix_id(&self) -> String {
        let mut parts = vec![
            self.physical_owner_id.clone(),
            self.owner_actor.index().to_string(),
            self.protocol_identity.clone(),
            self.continuation_identity.clone(),
            self.next_expansion_slot.graph_owner_id.clone(),
            self.next_expansion_slot.stage_identity.clone(),
            self.next_expansion_slot.slot_id.clone(),
        ];
        parts.extend(self.known_prefix.iter().cloned());
        framed_identity_v3("mads.unresolved-construction-prefix.v3", &parts)
    }

    fn validate(&self) -> Result<(), VirtualPhysicalRootErrorV3> {
        if self.physical_owner_id.is_empty()
            || self.protocol_identity.is_empty()
            || self.continuation_identity.is_empty()
            || self.known_prefix.iter().any(String::is_empty)
            || self.bounds != BoundIntervalV1::UNKNOWN
        {
            return Err(VirtualPhysicalRootErrorV3::InvalidCandidate);
        }
        self.next_expansion_slot
            .validate(&self.physical_owner_id, self.owner_actor)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalRootAlternativeV3 {
    pub identity: PhysicalRootActionIdentityV3,
    pub admission_evidence_identity: String,
    pub construction_path: Vec<ConstructionSlotIdentityV3>,
    pub state: PhysicalAlternativeStateV3,
    pub executed_response_prefix: Vec<String>,
    pub successor_binding: Option<SuccessorBindingV3>,
    pub bounds: BoundIntervalV1,
    pub next_expansion_slot: Option<ExpansionSlotIdentityV3>,
}

impl PhysicalRootAlternativeV3 {
    pub fn stable_id(&self) -> String {
        self.identity.stable_semantic_id()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RootRoleMaskV3(u8);

impl RootRoleMaskV3 {
    pub const INCUMBENT_LOWER: u8 = 1;
    pub const CHALLENGER_UPPER: u8 = 2;

    pub const fn contains(self, role: u8) -> bool {
        self.0 & role != 0
    }

    pub const fn supports_both(self) -> bool {
        self.contains(Self::INCUMBENT_LOWER) && self.contains(Self::CHALLENGER_UPPER)
    }

    fn insert(&mut self, role: u8) {
        self.0 |= role;
    }

    fn rank(self) -> u8 {
        if self.supports_both() {
            0
        } else if self.contains(Self::INCUMBENT_LOWER) {
            1
        } else if self.contains(Self::CHALLENGER_UPPER) {
            2
        } else {
            3
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RootCriticalTaskV3 {
    pub slot: ExpansionSlotIdentityV3,
    pub role_mask: RootRoleMaskV3,
    /// Only complete physical alternatives are listed here.
    pub root_action_support_ids: Vec<String>,
    /// Prefix support is kept in a separate namespace and is never promoted to
    /// a future physical alternative's identity.
    pub unresolved_prefix_support_ids: Vec<String>,
    pub bound_width: u8,
    pub min_root_distance: u16,
    pub estimated_cost_bucket: u16,
    pub stable_semantic_tiebreak: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RootCriticalFrontierStatusV3 {
    DomainNotAttested,
    Ready,
    ReadyAndBlockedOnUnevaluatedSuccessor,
    BlockedOnUnevaluatedSuccessor,
    Exhausted,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RootCriticalFrontierSnapshotV3 {
    pub status: RootCriticalFrontierStatusV3,
    pub tasks: Vec<RootCriticalTaskV3>,
    pub blocked_on_unevaluated_successor: Vec<String>,
}

impl RootCriticalTaskV3 {
    pub fn scheduler_key(&self) -> (u8, std::cmp::Reverse<u8>, u16, u16, String, String) {
        (
            self.role_mask.rank(),
            std::cmp::Reverse(self.bound_width),
            self.min_root_distance,
            self.estimated_cost_bucket,
            self.slot.stable_identity(),
            self.stable_semantic_tiebreak.clone(),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VirtualPhysicalRootV3 {
    pub owner_decision_id: String,
    pub owner_actor: PlayerId,
    alternatives: BTreeMap<String, PhysicalRootAlternativeV3>,
    envelopes: BTreeMap<String, UnresolvedConstructionEnvelopeV3>,
    root_domain_attestation: Option<PhysicalRootDomainAttestationV3>,
}

impl VirtualPhysicalRootV3 {
    pub fn new(
        owner_decision_id: impl Into<String>,
        owner_actor: PlayerId,
    ) -> Result<Self, VirtualPhysicalRootErrorV3> {
        let owner_decision_id = owner_decision_id.into();
        if owner_decision_id.is_empty() {
            return Err(VirtualPhysicalRootErrorV3::EmptyIdentity);
        }
        Ok(Self {
            owner_decision_id,
            owner_actor,
            alternatives: BTreeMap::new(),
            envelopes: BTreeMap::new(),
            root_domain_attestation: None,
        })
    }

    pub fn alternatives(&self) -> impl Iterator<Item = &PhysicalRootAlternativeV3> {
        self.alternatives.values()
    }

    pub fn envelopes(&self) -> impl Iterator<Item = &UnresolvedConstructionEnvelopeV3> {
        self.envelopes.values()
    }

    /// Seals the current raw-root projection only when an independent caller
    /// attests the exact ordered set of alternatives and unresolved prefixes.
    pub fn attest_root_domain(
        &mut self,
        attestation: PhysicalRootDomainAttestationV3,
    ) -> Result<(), VirtualPhysicalRootErrorV3> {
        if attestation.root_decision_identity != self.owner_decision_id
            || attestation.root_actor != self.owner_actor
            || attestation.evidence.identity().is_empty()
        {
            return Err(VirtualPhysicalRootErrorV3::DomainDoesNotMatchEnvelope);
        }
        let mut items = self
            .alternatives
            .values()
            .map(|alternative| (alternative.identity.stable_order, alternative.stable_id()))
            .chain(
                self.envelopes
                    .iter()
                    .map(|(id, envelope)| (envelope.stable_order, id.clone())),
            )
            .collect::<Vec<_>>();
        items.sort_by_key(|(order, id)| (*order, id.clone()));
        let expected = items.into_iter().map(|(_, id)| id).collect::<Vec<_>>();
        if expected.is_empty() || expected != attestation.ordered_root_item_ids {
            return Err(VirtualPhysicalRootErrorV3::IncompleteCandidateAttestation);
        }
        let evidence_identity = attestation.evidence.identity().to_owned();
        for alternative in self.alternatives.values_mut() {
            if alternative.admission_evidence_identity.is_empty() {
                alternative.admission_evidence_identity = evidence_identity.clone();
            }
        }
        self.root_domain_attestation = Some(attestation);
        Ok(())
    }

    pub fn insert_unresolved_prefix(
        &mut self,
        envelope: UnresolvedConstructionEnvelopeV3,
    ) -> Result<String, VirtualPhysicalRootErrorV3> {
        envelope.validate()?;
        if envelope.physical_owner_id != self.owner_decision_id
            || envelope.owner_actor != self.owner_actor
        {
            return Err(VirtualPhysicalRootErrorV3::DomainDoesNotMatchEnvelope);
        }
        self.ensure_order_available(envelope.stable_order, None)?;
        let id = envelope.stable_prefix_id();
        if self.envelopes.contains_key(&id) {
            return Err(VirtualPhysicalRootErrorV3::DuplicateEnvelope);
        }
        self.root_domain_attestation = None;
        self.envelopes.insert(id.clone(), envelope);
        Ok(id)
    }

    pub fn insert_known_alternative(
        &mut self,
        candidate: PhysicalRootCandidateV3,
    ) -> Result<String, VirtualPhysicalRootErrorV3> {
        candidate.validate()?;
        if candidate.identity.physical_owner_id != self.owner_decision_id
            || candidate.identity.owner_actor != self.owner_actor
            || !candidate.executed_response_prefix.is_empty()
        {
            return Err(VirtualPhysicalRootErrorV3::InvalidCandidate);
        }
        self.ensure_order_available(candidate.identity.stable_order, None)?;
        self.root_domain_attestation = None;
        self.insert_candidate(candidate, Vec::new(), String::new())
    }

    pub fn advance_unresolved_prefix(
        &mut self,
        envelope_id: &str,
        executed_prefix: Vec<String>,
        next_expansion_slot: ExpansionSlotIdentityV3,
        progress: PrefixProgressAttestationV3,
    ) -> Result<String, VirtualPhysicalRootErrorV3> {
        let mut envelope = self
            .envelopes
            .get(envelope_id)
            .cloned()
            .ok_or(VirtualPhysicalRootErrorV3::MissingEnvelope)?;
        if executed_prefix.len() <= envelope.known_prefix.len()
            || executed_prefix[..envelope.known_prefix.len()] != envelope.known_prefix
            || executed_prefix.iter().any(String::is_empty)
            || progress.physical_owner_id != envelope.physical_owner_id
            || progress.actor != envelope.owner_actor
            || progress.from_continuation_identity != envelope.continuation_identity
            || progress.to_continuation_identity.is_empty()
            || progress.applied_responses.is_empty()
            || progress.applied_responses != executed_prefix[envelope.known_prefix.len()..]
            || progress.provenance_identity().is_empty()
        {
            return Err(VirtualPhysicalRootErrorV3::InvalidProgress);
        }
        next_expansion_slot.validate(&self.owner_decision_id, self.owner_actor)?;
        if next_expansion_slot.protocol_identity != envelope.protocol_identity {
            return Err(VirtualPhysicalRootErrorV3::DomainDoesNotMatchEnvelope);
        }
        envelope.known_prefix = executed_prefix;
        envelope.continuation_identity = progress.to_continuation_identity;
        envelope.next_expansion_slot = next_expansion_slot;
        let next_id = envelope.stable_prefix_id();
        if next_id != envelope_id && self.envelopes.contains_key(&next_id) {
            return Err(VirtualPhysicalRootErrorV3::DuplicateEnvelope);
        }
        self.envelopes.remove(envelope_id);
        self.envelopes.insert(next_id.clone(), envelope);
        Ok(next_id)
    }

    /// Replaces an unresolved prefix only after a caller supplies an attested,
    /// exact ordered complete domain. Validation is atomic: failures leave the
    /// envelope and alternatives unchanged.
    pub fn admit_complete_domain(
        &mut self,
        envelope_id: &str,
        domain: CompletePhysicalActionDomainV3,
    ) -> Result<Vec<String>, VirtualPhysicalRootErrorV3> {
        if self.root_domain_attestation.is_none() {
            return Err(VirtualPhysicalRootErrorV3::RootDomainNotAttested);
        }
        domain.validate()?;
        let envelope = self
            .envelopes
            .get(envelope_id)
            .ok_or(VirtualPhysicalRootErrorV3::MissingEnvelope)?;
        if domain.physical_owner_id != envelope.physical_owner_id
            || domain.owner_actor != envelope.owner_actor
            || domain.protocol_identity != envelope.protocol_identity
            || domain.stage_identity != envelope.next_expansion_slot.stage_identity
            || domain.known_prefix != envelope.known_prefix
            || domain.continuation_identity != envelope.continuation_identity
        {
            return Err(VirtualPhysicalRootErrorV3::DomainDoesNotMatchEnvelope);
        }
        let evidence_identity = domain.evidence.identity().to_owned();
        let ids = domain
            .candidates
            .iter()
            .map(|candidate| candidate.identity.stable_semantic_id())
            .collect::<Vec<_>>();
        let candidate_orders = domain
            .candidates
            .iter()
            .map(|candidate| candidate.identity.stable_order)
            .collect::<BTreeSet<_>>();
        if ids.iter().collect::<BTreeSet<_>>().len() != ids.len()
            || candidate_orders.len() != domain.candidates.len()
        {
            return Err(VirtualPhysicalRootErrorV3::DuplicateAlternative);
        }
        for candidate in &domain.candidates {
            self.ensure_order_available(candidate.identity.stable_order, Some(envelope_id))?;
            if self
                .alternatives
                .contains_key(&candidate.identity.stable_semantic_id())
            {
                return Err(VirtualPhysicalRootErrorV3::DuplicateAlternative);
            }
        }

        // All validation has completed; now replace the prefix as one mutation.
        self.envelopes.remove(envelope_id);
        for candidate in domain.candidates {
            let id = candidate.identity.stable_semantic_id();
            let alternative = PhysicalRootAlternativeV3 {
                identity: candidate.identity,
                admission_evidence_identity: evidence_identity.clone(),
                construction_path: candidate.construction_path,
                state: PhysicalAlternativeStateV3::PartiallyConstructed,
                executed_response_prefix: candidate.executed_response_prefix,
                successor_binding: None,
                bounds: BoundIntervalV1::UNKNOWN,
                next_expansion_slot: Some(candidate.next_expansion_slot),
            };
            self.alternatives.insert(id.clone(), alternative);
        }
        Ok(ids)
    }

    pub fn mark_partially_constructed(
        &mut self,
        alternative_id: &str,
        executed_prefix: Vec<String>,
        next_expansion_slot: ExpansionSlotIdentityV3,
    ) -> Result<(), VirtualPhysicalRootErrorV3> {
        let alternative = self
            .alternatives
            .get_mut(alternative_id)
            .ok_or(VirtualPhysicalRootErrorV3::DuplicateAlternative)?;
        if !matches!(
            alternative.state,
            PhysicalAlternativeStateV3::KnownCompleteAlternativeNotExpanded
                | PhysicalAlternativeStateV3::PartiallyConstructed
        ) || executed_prefix.len() <= alternative.executed_response_prefix.len()
            || executed_prefix.len() >= alternative.identity.ordered_engine_responses.len()
            || executed_prefix[..alternative.executed_response_prefix.len()]
                != alternative.executed_response_prefix
            || executed_prefix
                != alternative.identity.ordered_engine_responses[..executed_prefix.len()]
        {
            return Err(VirtualPhysicalRootErrorV3::InvalidProgress);
        }
        next_expansion_slot.validate(
            &alternative.identity.physical_owner_id,
            alternative.identity.owner_actor,
        )?;
        if next_expansion_slot.slot_id
            != alternative.identity.ordered_engine_responses[executed_prefix.len()]
        {
            return Err(VirtualPhysicalRootErrorV3::InvalidProgress);
        }
        if !alternative.construction_path.is_empty()
            && !alternative.construction_path.iter().any(|step| {
                step.response_index == executed_prefix.len()
                    && step.actor == next_expansion_slot.actor
                    && step.protocol_identity == next_expansion_slot.protocol_identity
                    && step.stage_identity == next_expansion_slot.stage_identity
                    && step.slot_id == next_expansion_slot.slot_id
            })
        {
            return Err(VirtualPhysicalRootErrorV3::InvalidProgress);
        }
        alternative.executed_response_prefix = executed_prefix;
        alternative.state = PhysicalAlternativeStateV3::PartiallyConstructed;
        alternative.next_expansion_slot = Some(next_expansion_slot);
        Ok(())
    }

    pub fn complete_alternative(
        &mut self,
        alternative_id: &str,
        executed_responses: &[String],
        successor_binding: SuccessorBindingV3,
    ) -> Result<(), VirtualPhysicalRootErrorV3> {
        let alternative = self
            .alternatives
            .get_mut(alternative_id)
            .ok_or(VirtualPhysicalRootErrorV3::DuplicateAlternative)?;
        if !matches!(
            alternative.state,
            PhysicalAlternativeStateV3::KnownCompleteAlternativeNotExpanded
                | PhysicalAlternativeStateV3::PartiallyConstructed
        ) || executed_responses != alternative.identity.ordered_engine_responses
        {
            return Err(VirtualPhysicalRootErrorV3::InvalidProgress);
        }
        successor_binding.validate_for(
            &alternative.identity,
            &alternative.admission_evidence_identity,
        )?;
        let successor_task = successor_binding.next_expansion_slot.clone();
        alternative.executed_response_prefix = executed_responses.to_vec();
        alternative.successor_binding = Some(successor_binding);
        alternative.state = PhysicalAlternativeStateV3::Completed;
        alternative.bounds = BoundIntervalV1::UNKNOWN;
        alternative.next_expansion_slot = successor_task;
        Ok(())
    }

    pub fn update_successor_bounds(
        &mut self,
        alternative_id: &str,
        bounds: BoundIntervalV1,
        next_expansion_slot: Option<ExpansionSlotIdentityV3>,
    ) -> Result<(), VirtualPhysicalRootErrorV3> {
        if !bounds.is_valid() {
            return Err(VirtualPhysicalRootErrorV3::InvalidBounds);
        }
        let alternative = self
            .alternatives
            .get_mut(alternative_id)
            .ok_or(VirtualPhysicalRootErrorV3::DuplicateAlternative)?;
        if alternative.state != PhysicalAlternativeStateV3::Completed
            && alternative.state != PhysicalAlternativeStateV3::SuccessorExpanded
        {
            return Err(VirtualPhysicalRootErrorV3::InvalidStateTransition);
        }
        if alternative.successor_binding.is_none() {
            return Err(VirtualPhysicalRootErrorV3::InvalidSuccessorBinding);
        }
        let successor_binding = alternative
            .successor_binding
            .as_ref()
            .ok_or(VirtualPhysicalRootErrorV3::InvalidSuccessorBinding)?;
        if next_expansion_slot.as_ref().is_some_and(|slot| {
            slot.validate(&alternative.identity.physical_owner_id, slot.actor)
                .is_err()
                || slot.graph_owner_id != successor_binding.successor_node_id
                || successor_binding.next_actor != Some(slot.actor)
        }) {
            return Err(VirtualPhysicalRootErrorV3::InvalidSuccessorBinding);
        }
        alternative.bounds = bounds;
        alternative.state = PhysicalAlternativeStateV3::SuccessorExpanded;
        alternative.next_expansion_slot = next_expansion_slot;
        Ok(())
    }

    pub fn root_bounds(&self) -> BoundIntervalV1 {
        let mut bounds = self
            .alternatives
            .values()
            .map(|alternative| alternative.bounds)
            .chain(self.envelopes.values().map(|envelope| envelope.bounds))
            .collect::<Vec<_>>();
        if self.root_domain_attestation.is_none() {
            bounds.push(BoundIntervalV1::UNKNOWN);
        }
        backup_bounds_v1(MadsRoleV1::Max, &bounds, bounds.len())
    }

    pub fn root_action_domain_complete(&self) -> bool {
        self.root_domain_attestation.is_some() && self.envelopes.is_empty()
    }

    pub fn certified_optimal_alternative_ids(&self) -> Vec<String> {
        if self.root_domain_attestation.is_none() {
            return Vec::new();
        }
        self.alternatives
            .iter()
            .filter_map(|(id, candidate)| {
                if candidate.state != PhysicalAlternativeStateV3::SuccessorExpanded {
                    return None;
                }
                let best_other_upper = self
                    .alternatives
                    .iter()
                    .filter(|(other_id, _)| *other_id != id)
                    .map(|(_, other)| other.bounds.upper)
                    .chain(
                        self.envelopes
                            .values()
                            .map(|envelope| envelope.bounds.upper),
                    )
                    .max();
                best_other_upper
                    .is_none_or(|upper| candidate.bounds.lower >= upper)
                    .then(|| id.clone())
            })
            .collect()
    }

    /// Rebuilds critical tasks and makes an empty-but-incomplete frontier
    /// explicit instead of treating it as exhausted search.
    pub fn rebuild_root_critical_frontier(&self) -> RootCriticalFrontierSnapshotV3 {
        if self.root_domain_attestation.is_none() {
            return RootCriticalFrontierSnapshotV3 {
                status: RootCriticalFrontierStatusV3::DomainNotAttested,
                tasks: Vec::new(),
                blocked_on_unevaluated_successor: Vec::new(),
            };
        }
        let mut items = Vec::with_capacity(self.alternatives.len() + self.envelopes.len());
        for (id, alternative) in &self.alternatives {
            items.push(RootItemV3 {
                id: id.clone(),
                order: alternative.identity.stable_order,
                bounds: alternative.bounds,
                task: alternative.next_expansion_slot.clone(),
                is_prefix: false,
            });
        }
        for (id, envelope) in &self.envelopes {
            items.push(RootItemV3 {
                id: id.clone(),
                order: envelope.stable_order,
                bounds: envelope.bounds,
                task: Some(envelope.next_expansion_slot.clone()),
                is_prefix: true,
            });
        }
        let incumbent = items
            .iter()
            .max_by_key(|item| (item.bounds.lower, std::cmp::Reverse(item.order)))
            .map(|item| item.id.as_str());
        let challenger = items
            .iter()
            .filter(|item| Some(item.id.as_str()) != incumbent)
            .max_by_key(|item| (item.bounds.upper, std::cmp::Reverse(item.order)))
            .map(|item| item.id.as_str());
        let mut blocked = Vec::new();
        let mut tasks = BTreeMap::<String, RootCriticalTaskV3>::new();
        for item in &items {
            let Some(task_slot) = &item.task else {
                let is_critical =
                    Some(item.id.as_str()) == incumbent || Some(item.id.as_str()) == challenger;
                if is_critical && item.bounds.width() > 0 {
                    blocked.push(item.id.clone());
                }
                continue;
            };
            let role = if Some(item.id.as_str()) == incumbent {
                Some(RootRoleMaskV3::INCUMBENT_LOWER)
            } else if Some(item.id.as_str()) == challenger {
                Some(RootRoleMaskV3::CHALLENGER_UPPER)
            } else {
                None
            };
            let Some(role) = role else { continue };
            let task_id = task_slot.stable_identity();
            let task = tasks
                .entry(task_id.clone())
                .or_insert_with(|| RootCriticalTaskV3 {
                    slot: task_slot.clone(),
                    role_mask: RootRoleMaskV3::default(),
                    root_action_support_ids: Vec::new(),
                    unresolved_prefix_support_ids: Vec::new(),
                    bound_width: item.bounds.width(),
                    min_root_distance: task_slot.root_distance,
                    estimated_cost_bucket: task_slot.estimated_cost_bucket,
                    stable_semantic_tiebreak: task_id,
                });
            task.role_mask.insert(role);
            if item.is_prefix {
                task.unresolved_prefix_support_ids.push(item.id.clone());
            } else {
                task.root_action_support_ids.push(item.id.clone());
            }
            task.bound_width = task.bound_width.max(item.bounds.width());
            task.min_root_distance = task.min_root_distance.min(task_slot.root_distance);
            task.estimated_cost_bucket = task
                .estimated_cost_bucket
                .min(task_slot.estimated_cost_bucket);
        }
        let mut frontier = tasks.into_values().collect::<Vec<_>>();
        for task in &mut frontier {
            task.root_action_support_ids.sort();
            task.root_action_support_ids.dedup();
            task.unresolved_prefix_support_ids.sort();
            task.unresolved_prefix_support_ids.dedup();
        }
        frontier.sort_by_key(RootCriticalTaskV3::scheduler_key);
        blocked.sort();
        let status = match (frontier.is_empty(), blocked.is_empty()) {
            (false, true) => RootCriticalFrontierStatusV3::Ready,
            (false, false) => RootCriticalFrontierStatusV3::ReadyAndBlockedOnUnevaluatedSuccessor,
            (true, false) => RootCriticalFrontierStatusV3::BlockedOnUnevaluatedSuccessor,
            (true, true) => RootCriticalFrontierStatusV3::Exhausted,
        };
        RootCriticalFrontierSnapshotV3 {
            status,
            tasks: frontier,
            blocked_on_unevaluated_successor: blocked,
        }
    }

    pub fn rebuild_root_critical_frontier_snapshot_v3(&self) -> RootCriticalFrontierSnapshotV3 {
        self.rebuild_root_critical_frontier()
    }

    fn ensure_order_available(
        &self,
        order: u32,
        replaced_envelope: Option<&str>,
    ) -> Result<(), VirtualPhysicalRootErrorV3> {
        if self
            .alternatives
            .values()
            .any(|alternative| alternative.identity.stable_order == order)
            || self.envelopes.iter().any(|(id, envelope)| {
                Some(id.as_str()) != replaced_envelope && envelope.stable_order == order
            })
        {
            return Err(VirtualPhysicalRootErrorV3::OrderCollision);
        }
        Ok(())
    }

    fn insert_candidate(
        &mut self,
        candidate: PhysicalRootCandidateV3,
        executed_prefix: Vec<String>,
        admission_evidence_identity: String,
    ) -> Result<String, VirtualPhysicalRootErrorV3> {
        let id = candidate.identity.stable_semantic_id();
        if self.alternatives.contains_key(&id) {
            return Err(VirtualPhysicalRootErrorV3::DuplicateAlternative);
        }
        let state = if executed_prefix.is_empty() {
            PhysicalAlternativeStateV3::KnownCompleteAlternativeNotExpanded
        } else {
            PhysicalAlternativeStateV3::PartiallyConstructed
        };
        let alternative = PhysicalRootAlternativeV3 {
            identity: candidate.identity,
            admission_evidence_identity,
            construction_path: candidate.construction_path,
            state,
            executed_response_prefix: executed_prefix,
            successor_binding: None,
            bounds: BoundIntervalV1::UNKNOWN,
            next_expansion_slot: Some(candidate.next_expansion_slot),
        };
        self.alternatives.insert(id.clone(), alternative);
        Ok(id)
    }
}

#[derive(Debug)]
struct RootItemV3 {
    id: String,
    order: u32,
    bounds: BoundIntervalV1,
    task: Option<ExpansionSlotIdentityV3>,
    is_prefix: bool,
}

fn framed_identity_v3<S: AsRef<str>>(namespace: &str, parts: &[S]) -> String {
    let mut result = namespace.to_owned();
    for part in parts {
        let part = part.as_ref();
        result.push_str(&format!("|{}:{part}", part.len()));
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mads_v1::MadsGraphV1;
    use crate::oracle_suite_v1::{
        FixtureEdgeV1, FixtureNodeId, FixtureOutcomeV1, FixturePlayerV1, OracleFixtureV1,
        OracleNodeV1,
    };

    const OWNER: &str = "engine-decision:p0:revision-17";
    const PROTOCOL: &str = "lightning-bolt-target-v1";
    const ROOT_CONTINUATION: &str = "priority-stage:revision-17";
    const CONTINUATION: &str = "bolt:ObjectId(7):target-stage-0";
    const CAST: &str = "cast-bolt:object-7";
    const TARGET_P0: &str = "target:player-0";
    const TARGET_P1: &str = "target:player-1";
    const PASS: &str = "pass";
    const OTHER: &str = "other";

    fn edge(order: u32, id: &str, child: u32) -> FixtureEdgeV1 {
        FixtureEdgeV1 {
            stable_id: id.to_owned(),
            order,
            child: FixtureNodeId(child),
            estimated_cost_bucket: 1,
        }
    }

    fn oracle_pair() -> (OracleFixtureV1, OracleFixtureV1) {
        let suffix_nodes = vec![
            OracleNodeV1::GameDecision {
                id: FixtureNodeId(2),
                actor: FixturePlayerV1::P0,
                actions: vec![edge(0, "win", 5), edge(1, "draw", 6)],
            },
            OracleNodeV1::GameDecision {
                id: FixtureNodeId(3),
                actor: FixturePlayerV1::P1,
                actions: vec![edge(0, "loss", 7), edge(1, "draw", 6)],
            },
            OracleNodeV1::GameDecision {
                id: FixtureNodeId(4),
                actor: FixturePlayerV1::P1,
                actions: vec![edge(0, "draw", 6), edge(1, "win", 5)],
            },
            OracleNodeV1::GameDecision {
                id: FixtureNodeId(8),
                actor: FixturePlayerV1::P1,
                actions: vec![edge(0, "draw", 6), edge(1, "win", 5)],
            },
            OracleNodeV1::Terminal {
                id: FixtureNodeId(5),
                outcome: FixtureOutcomeV1::Win,
            },
            OracleNodeV1::Terminal {
                id: FixtureNodeId(6),
                outcome: FixtureOutcomeV1::Draw,
            },
            OracleNodeV1::Terminal {
                id: FixtureNodeId(7),
                outcome: FixtureOutcomeV1::Loss,
            },
        ];
        let mut nested_nodes = vec![OracleNodeV1::DecisionConstruction {
            id: FixtureNodeId(1),
            owner_decision: FixtureNodeId(0),
            actor: FixturePlayerV1::P0,
            protocol_key: PROTOCOL.to_owned(),
            partial_response: CAST.to_owned(),
            continuation_cursor: 1,
            choices: vec![edge(0, "target-P0", 2), edge(1, "target-P1", 3)],
        }];
        nested_nodes.extend(suffix_nodes.clone());
        let nested = OracleFixtureV1 {
            fixture_id: "mads03b4-nested-cast-target-v1".to_owned(),
            root: FixtureNodeId(0),
            root_player: FixturePlayerV1::P0,
            nodes: std::iter::once(OracleNodeV1::GameDecision {
                id: FixtureNodeId(0),
                actor: FixturePlayerV1::P0,
                actions: vec![
                    edge(0, "cast-bolt", 1),
                    edge(2, "pass", 4),
                    edge(3, "other", 8),
                ],
            })
            .chain(nested_nodes)
            .collect(),
        };
        let flat = OracleFixtureV1 {
            fixture_id: "mads03b4-flat-physical-root-v1".to_owned(),
            root: FixtureNodeId(0),
            root_player: FixturePlayerV1::P0,
            nodes: std::iter::once(OracleNodeV1::GameDecision {
                id: FixtureNodeId(0),
                actor: FixturePlayerV1::P0,
                actions: vec![
                    edge(0, "cast-bolt/target-P0", 2),
                    edge(1, "cast-bolt/target-P1", 3),
                    edge(2, "pass", 4),
                    edge(3, "other", 8),
                ],
            })
            .chain(suffix_nodes)
            .collect(),
        };
        (nested, flat)
    }

    fn slot(
        graph_owner: &str,
        protocol: &str,
        stage: &str,
        slot_id: &str,
        order: u32,
        distance: u16,
    ) -> ExpansionSlotIdentityV3 {
        ExpansionSlotIdentityV3 {
            physical_owner_id: OWNER.to_owned(),
            graph_owner_id: graph_owner.to_owned(),
            actor: PlayerId::P0,
            protocol_identity: protocol.to_owned(),
            stage_identity: stage.to_owned(),
            slot_id: slot_id.to_owned(),
            action_order: order,
            root_distance: distance,
            estimated_cost_bucket: 1,
        }
    }

    fn actor_slot(
        graph_owner: &str,
        actor: PlayerId,
        protocol: &str,
        stage: &str,
        slot_id: &str,
        order: u32,
        distance: u16,
    ) -> ExpansionSlotIdentityV3 {
        let mut expansion = slot(graph_owner, protocol, stage, slot_id, order, distance);
        expansion.actor = actor;
        expansion
    }

    fn cast_prefix_progress(from_continuation_identity: &str) -> PrefixProgressAttestationV3 {
        PrefixProgressAttestationV3 {
            physical_owner_id: OWNER.to_owned(),
            actor: PlayerId::P0,
            from_continuation_identity: from_continuation_identity.to_owned(),
            to_continuation_identity: CONTINUATION.to_owned(),
            applied_responses: vec![CAST.to_owned()],
            provenance: PrefixProgressProvenanceV3::OracleFixture {
                fixture_identity: "mads03b4-nested-cast-target-v1".to_owned(),
            },
        }
    }

    fn target_candidate(order: u32, target_response: &str) -> PhysicalRootCandidateV3 {
        let construction_path = vec![ConstructionSlotIdentityV3 {
            physical_owner_id: OWNER.to_owned(),
            actor: PlayerId::P0,
            protocol_identity: PROTOCOL.to_owned(),
            stage_identity: "target-pick-stage-0".to_owned(),
            response_index: 1,
            slot_id: target_response.to_owned(),
            action_order: order,
        }];
        PhysicalRootCandidateV3 {
            identity: PhysicalRootActionIdentityV3 {
                schema_version: VIRTUAL_PHYSICAL_ROOT_SCHEMA_VERSION_V3,
                physical_owner_id: OWNER.to_owned(),
                owner_actor: PlayerId::P0,
                stable_order: order,
                continuation_identity: CONTINUATION.to_owned(),
                construction_path_identity: construction_path
                    .iter()
                    .map(ConstructionSlotIdentityV3::stable_identity)
                    .collect(),
                ordered_engine_responses: vec![CAST.to_owned(), target_response.to_owned()],
            },
            construction_path,
            next_expansion_slot: slot(
                "construction:bolt:target-stage-0",
                PROTOCOL,
                "target-pick-stage-0",
                target_response,
                order,
                1,
            ),
            executed_response_prefix: vec![CAST.to_owned()],
        }
    }

    fn root_with_prefix() -> (VirtualPhysicalRootV3, String, String, String) {
        let mut root = VirtualPhysicalRootV3::new(OWNER, PlayerId::P0).unwrap();
        let pass = PhysicalRootCandidateV3 {
            identity: PhysicalRootActionIdentityV3 {
                schema_version: VIRTUAL_PHYSICAL_ROOT_SCHEMA_VERSION_V3,
                physical_owner_id: OWNER.to_owned(),
                owner_actor: PlayerId::P0,
                stable_order: 2,
                continuation_identity: "priority-response".to_owned(),
                construction_path_identity: Vec::new(),
                ordered_engine_responses: vec![PASS.to_owned()],
            },
            construction_path: Vec::new(),
            next_expansion_slot: slot("game:root", "priority", "root", PASS, 2, 0),
            executed_response_prefix: Vec::new(),
        };
        let pass_id = root.insert_known_alternative(pass).unwrap();
        let other = PhysicalRootCandidateV3 {
            identity: PhysicalRootActionIdentityV3 {
                schema_version: VIRTUAL_PHYSICAL_ROOT_SCHEMA_VERSION_V3,
                physical_owner_id: OWNER.to_owned(),
                owner_actor: PlayerId::P0,
                stable_order: 3,
                continuation_identity: "priority-response".to_owned(),
                construction_path_identity: Vec::new(),
                ordered_engine_responses: vec![OTHER.to_owned()],
            },
            construction_path: Vec::new(),
            next_expansion_slot: slot("game:root", "priority", "root", OTHER, 3, 0),
            executed_response_prefix: Vec::new(),
        };
        let other_id = root.insert_known_alternative(other).unwrap();
        let envelope = UnresolvedConstructionEnvelopeV3 {
            physical_owner_id: OWNER.to_owned(),
            owner_actor: PlayerId::P0,
            protocol_identity: PROTOCOL.to_owned(),
            known_prefix: Vec::new(),
            continuation_identity: ROOT_CONTINUATION.to_owned(),
            stable_order: 0,
            next_expansion_slot: slot("game:root", "priority", "root", CAST, 0, 0),
            bounds: BoundIntervalV1::UNKNOWN,
        };
        let envelope_id = root.insert_unresolved_prefix(envelope).unwrap();
        root.attest_root_domain(PhysicalRootDomainAttestationV3 {
            root_decision_identity: OWNER.to_owned(),
            root_actor: PlayerId::P0,
            ordered_root_item_ids: vec![envelope_id.clone(), pass_id.clone(), other_id.clone()],
            evidence: CompleteDomainEvidenceV3::OracleFixture {
                fixture_identity: "mads03b4-nested-cast-target-v1".to_owned(),
            },
        })
        .unwrap();
        (root, pass_id, other_id, envelope_id)
    }

    fn oracle_domain() -> CompletePhysicalActionDomainV3 {
        let candidates = vec![
            target_candidate(0, TARGET_P0),
            target_candidate(1, TARGET_P1),
        ];
        CompletePhysicalActionDomainV3::new(
            domain_context(),
            CompleteCandidateDomainAttestationV3 {
                ordered_candidate_responses: vec![
                    vec![CAST.to_owned(), TARGET_P0.to_owned()],
                    vec![CAST.to_owned(), TARGET_P1.to_owned()],
                ],
                evidence: CompleteDomainEvidenceV3::OracleFixture {
                    fixture_identity: "mads03b4-flat-physical-root-v1".to_owned(),
                },
            },
            candidates,
        )
        .unwrap()
    }

    fn domain_context() -> CompleteConstructionDomainContextV3 {
        CompleteConstructionDomainContextV3 {
            physical_owner_id: OWNER.to_owned(),
            owner_actor: PlayerId::P0,
            protocol_identity: PROTOCOL.to_owned(),
            stage_identity: "target-pick-stage-0".to_owned(),
            known_prefix: vec![CAST.to_owned()],
            continuation_identity: CONTINUATION.to_owned(),
        }
    }

    fn attestation(
        paths: Vec<Vec<String>>,
        fixture_identity: &str,
    ) -> CompleteCandidateDomainAttestationV3 {
        CompleteCandidateDomainAttestationV3 {
            ordered_candidate_responses: paths,
            evidence: CompleteDomainEvidenceV3::OracleFixture {
                fixture_identity: fixture_identity.to_owned(),
            },
        }
    }

    fn reference_frontier(root: &VirtualPhysicalRootV3) -> Vec<RootCriticalTaskV3> {
        let mut roots = root
            .alternatives
            .iter()
            .map(|(id, alt)| RootItemV3 {
                id: id.clone(),
                order: alt.identity.stable_order,
                bounds: alt.bounds,
                task: alt.next_expansion_slot.clone(),
                is_prefix: false,
            })
            .chain(root.envelopes.iter().map(|(id, envelope)| RootItemV3 {
                id: id.clone(),
                order: envelope.stable_order,
                bounds: envelope.bounds,
                task: Some(envelope.next_expansion_slot.clone()),
                is_prefix: true,
            }))
            .collect::<Vec<_>>();
        roots.sort_by_key(|item| (item.order, item.id.clone()));
        let incumbent = roots
            .iter()
            .max_by_key(|item| (item.bounds.lower, std::cmp::Reverse(item.order)))
            .map(|item| item.id.clone());
        let challenger = roots
            .iter()
            .filter(|item| Some(&item.id) != incumbent.as_ref())
            .max_by_key(|item| (item.bounds.upper, std::cmp::Reverse(item.order)))
            .map(|item| item.id.clone());
        let mut result = BTreeMap::<String, RootCriticalTaskV3>::new();
        for item in roots {
            let Some(role) = (if Some(&item.id) == incumbent.as_ref() {
                Some(RootRoleMaskV3::INCUMBENT_LOWER)
            } else if Some(&item.id) == challenger.as_ref() {
                Some(RootRoleMaskV3::CHALLENGER_UPPER)
            } else {
                None
            }) else {
                continue;
            };
            let Some(slot) = item.task else { continue };
            let key = slot.stable_identity();
            let task = result
                .entry(key.clone())
                .or_insert_with(|| RootCriticalTaskV3 {
                    slot: slot.clone(),
                    role_mask: RootRoleMaskV3::default(),
                    root_action_support_ids: Vec::new(),
                    unresolved_prefix_support_ids: Vec::new(),
                    bound_width: item.bounds.width(),
                    min_root_distance: slot.root_distance,
                    estimated_cost_bucket: slot.estimated_cost_bucket,
                    stable_semantic_tiebreak: key,
                });
            task.role_mask.insert(role);
            if item.is_prefix {
                task.unresolved_prefix_support_ids.push(item.id);
            } else {
                task.root_action_support_ids.push(item.id);
            }
            task.bound_width = task.bound_width.max(item.bounds.width());
        }
        let mut tasks = result.into_values().collect::<Vec<_>>();
        for task in &mut tasks {
            task.root_action_support_ids.sort();
            task.unresolved_prefix_support_ids.sort();
        }
        tasks.sort_by_key(RootCriticalTaskV3::scheduler_key);
        tasks
    }

    #[test]
    fn virtual_root_prefix_admission_and_bounds_match_flat_oracle() {
        let (nested_fixture, flat_fixture) = oracle_pair();
        let nested_truth = nested_fixture.solve_oracle_v1().unwrap();
        let flat_truth = flat_fixture.solve_oracle_v1().unwrap();
        assert_eq!(nested_truth.root_value, flat_truth.root_value);
        assert_eq!(flat_truth.root_value, 1);
        assert_eq!(flat_truth.optimal_root_actions, ["cast-bolt/target-P0"]);
        assert_eq!(flat_truth.node_values[&FixtureNodeId(2)], 1);
        assert_eq!(flat_truth.node_values[&FixtureNodeId(3)], -1);
        assert_eq!(flat_truth.node_values[&FixtureNodeId(4)], 0);
        assert_eq!(flat_truth.node_values[&FixtureNodeId(8)], 0);

        let mut root = MadsGraphV1::new(&flat_fixture).unwrap();
        while root.expand_next_v1().unwrap().is_some() {}
        assert_eq!(
            root.result_v1().certified_optimal_actions,
            ["cast-bolt/target-P0"]
        );

        let (mut virtual_root, pass_id, other_id, envelope_id) = root_with_prefix();
        assert!(!virtual_root.root_action_domain_complete());
        assert_eq!(virtual_root.root_bounds(), BoundIntervalV1::UNKNOWN);
        assert!(virtual_root.certified_optimal_alternative_ids().is_empty());
        let initial_frontier = virtual_root.rebuild_root_critical_frontier();
        assert_eq!(initial_frontier.status, RootCriticalFrontierStatusV3::Ready);
        assert_eq!(initial_frontier.tasks.len(), 2);
        assert_eq!(initial_frontier.tasks, reference_frontier(&virtual_root));
        assert!(initial_frontier
            .tasks
            .iter()
            .any(|task| task.unresolved_prefix_support_ids == [envelope_id.clone()]));

        // Resolve Pass from its independently known Oracle successor while the
        // Cast construction envelope remains open. The envelope keeps U=WIN,
        // so this lower-valued alternative is not certified.
        let pass_responses = vec![PASS.to_owned()];
        virtual_root
            .complete_alternative(
                &pass_id,
                &pass_responses,
                SuccessorBindingV3 {
                    physical_owner_id: OWNER.to_owned(),
                    owner_graph_node_id: "game:root".to_owned(),
                    ordered_engine_responses: pass_responses.clone(),
                    finalization_boundary: "priority-passed-to-opponent".to_owned(),
                    successor_node_id: "oracle-successor:4".to_owned(),
                    next_expansion_slot: Some(actor_slot(
                        "oracle-successor:4",
                        PlayerId::P1,
                        "priority",
                        "post-pass",
                        "draw",
                        0,
                        1,
                    )),
                    graph_path_node_ids: vec![
                        "game:root".to_owned(),
                        "oracle-successor:4".to_owned(),
                    ],
                    next_actor: Some(PlayerId::P1),
                    provenance: SuccessorProvenanceV3::OracleFixture {
                        domain_fixture_identity: nested_fixture.fixture_id.clone(),
                        successor_fixture_identity: flat_fixture.fixture_id.clone(),
                        physical_response_identity: pass_id.clone(),
                    },
                },
            )
            .unwrap();
        virtual_root
            .update_successor_bounds(&pass_id, BoundIntervalV1::exact(0), None)
            .unwrap();
        let other_responses = vec![OTHER.to_owned()];
        virtual_root
            .complete_alternative(
                &other_id,
                &other_responses,
                SuccessorBindingV3 {
                    physical_owner_id: OWNER.to_owned(),
                    owner_graph_node_id: "game:root".to_owned(),
                    ordered_engine_responses: other_responses.clone(),
                    finalization_boundary: "priority-action-complete".to_owned(),
                    successor_node_id: "oracle-successor:8".to_owned(),
                    next_expansion_slot: Some(actor_slot(
                        "oracle-successor:8",
                        PlayerId::P1,
                        "priority",
                        "post-other",
                        "draw",
                        0,
                        1,
                    )),
                    graph_path_node_ids: vec![
                        "game:root".to_owned(),
                        "oracle-successor:8".to_owned(),
                    ],
                    next_actor: Some(PlayerId::P1),
                    provenance: SuccessorProvenanceV3::OracleFixture {
                        domain_fixture_identity: nested_fixture.fixture_id.clone(),
                        successor_fixture_identity: flat_fixture.fixture_id.clone(),
                        physical_response_identity: other_id.clone(),
                    },
                },
            )
            .unwrap();
        virtual_root
            .update_successor_bounds(&other_id, BoundIntervalV1::exact(0), None)
            .unwrap();
        assert_eq!(
            virtual_root.alternatives[&other_id].bounds,
            BoundIntervalV1::exact(flat_truth.node_values[&FixtureNodeId(8)])
        );
        assert_eq!(
            virtual_root.root_bounds(),
            BoundIntervalV1 { lower: 0, upper: 1 }
        );
        assert!(virtual_root.certified_optimal_alternative_ids().is_empty());
        let envelope_frontier = virtual_root.rebuild_root_critical_frontier();
        assert_eq!(
            envelope_frontier.status,
            RootCriticalFrontierStatusV3::Ready
        );
        assert_eq!(envelope_frontier.tasks, reference_frontier(&virtual_root));
        assert!(envelope_frontier
            .tasks
            .iter()
            .any(|task| task.unresolved_prefix_support_ids == [envelope_id.clone()]));

        let initial_envelope_id = envelope_id.clone();
        let envelope_id = virtual_root
            .advance_unresolved_prefix(
                &envelope_id,
                vec![CAST.to_owned()],
                slot(
                    "construction:bolt:target-stage-0",
                    PROTOCOL,
                    "target-pick-stage-0",
                    "enumerate-complete-target-domain",
                    0,
                    1,
                ),
                cast_prefix_progress(ROOT_CONTINUATION),
            )
            .unwrap();
        assert_ne!(initial_envelope_id, envelope_id);
        assert!(!virtual_root.root_action_domain_complete());
        let domain = oracle_domain();
        let candidate_ids = virtual_root
            .admit_complete_domain(&envelope_id, domain)
            .unwrap();
        assert_eq!(candidate_ids.len(), 2);
        assert!(virtual_root.root_action_domain_complete());
        assert_eq!(virtual_root.alternatives.len(), 4);
        let p0_id = virtual_root
            .alternatives()
            .find(|alternative| alternative.identity.ordered_engine_responses[1] == TARGET_P0)
            .unwrap()
            .stable_id();
        let p1_id = virtual_root
            .alternatives()
            .find(|alternative| alternative.identity.ordered_engine_responses[1] == TARGET_P1)
            .unwrap()
            .stable_id();
        assert_ne!(p0_id, p1_id);
        assert_eq!(
            virtual_root.alternatives[&p0_id].state,
            PhysicalAlternativeStateV3::PartiallyConstructed
        );
        assert_eq!(
            virtual_root.alternatives[&p0_id].executed_response_prefix,
            vec![CAST.to_owned()]
        );
        let mut same_response_different_stage = target_candidate(0, TARGET_P0);
        same_response_different_stage.construction_path[0].stage_identity =
            "different-target-stage".to_owned();
        same_response_different_stage
            .identity
            .construction_path_identity = same_response_different_stage
            .construction_path
            .iter()
            .map(ConstructionSlotIdentityV3::stable_identity)
            .collect();
        assert_ne!(
            same_response_different_stage.identity.stable_semantic_id(),
            p0_id,
            "continuation stages participate in physical alternative identity"
        );
        assert_ne!(
            virtual_root.alternatives[&p0_id].next_expansion_slot,
            virtual_root.alternatives[&p1_id].next_expansion_slot
        );
        let wrong_prefix = virtual_root.mark_partially_constructed(
            &p0_id,
            vec!["stale-cast-prefix".to_owned()],
            slot(
                "construction:bolt:target-stage-0",
                PROTOCOL,
                "target-pick-stage-0",
                TARGET_P0,
                0,
                1,
            ),
        );
        assert_eq!(
            wrong_prefix,
            Err(VirtualPhysicalRootErrorV3::InvalidProgress)
        );
        assert_eq!(
            virtual_root.alternatives[&p0_id].bounds,
            BoundIntervalV1::UNKNOWN
        );
        assert_eq!(
            virtual_root.alternatives[&p1_id].bounds,
            BoundIntervalV1::UNKNOWN
        );
        let targets_frontier = virtual_root.rebuild_root_critical_frontier();
        assert_eq!(targets_frontier.status, RootCriticalFrontierStatusV3::Ready);
        assert_eq!(targets_frontier.tasks, reference_frontier(&virtual_root));
        assert!(targets_frontier
            .tasks
            .iter()
            .all(|task| !task.unresolved_prefix_support_ids.contains(&envelope_id)));
        assert!(targets_frontier.tasks.iter().all(|task| {
            task.root_action_support_ids
                .iter()
                .all(|id| id == &p0_id || id == &p1_id)
        }));

        // A full response cannot acquire a value before its exact successor
        // binding is recorded. Different targets bind to different successors
        // and keep distinct bound cells/support IDs.
        for (id, responses, node, value, next_actor) in [
            (
                p0_id.as_str(),
                vec![CAST.to_owned(), TARGET_P0.to_owned()],
                "oracle-successor:2",
                1,
                Some(PlayerId::P0),
            ),
            (
                p1_id.as_str(),
                vec![CAST.to_owned(), TARGET_P1.to_owned()],
                "oracle-successor:3",
                -1,
                Some(PlayerId::P1),
            ),
        ] {
            virtual_root
                .complete_alternative(
                    id,
                    &responses,
                    SuccessorBindingV3 {
                        physical_owner_id: OWNER.to_owned(),
                        owner_graph_node_id: "game:root".to_owned(),
                        ordered_engine_responses: responses.clone(),
                        finalization_boundary: format!("cast-finalized:{node}"),
                        successor_node_id: node.to_owned(),
                        next_expansion_slot: Some(actor_slot(
                            node,
                            next_actor.unwrap_or(PlayerId::P0),
                            "priority",
                            "post-cast",
                            "resolve-next",
                            0,
                            1,
                        )),
                        graph_path_node_ids: vec!["game:root".to_owned(), node.to_owned()],
                        next_actor,
                        provenance: SuccessorProvenanceV3::OracleFixture {
                            domain_fixture_identity: flat_fixture.fixture_id.clone(),
                            successor_fixture_identity: nested_fixture.fixture_id.clone(),
                            physical_response_identity: id.to_owned(),
                        },
                    },
                )
                .unwrap();
            assert_eq!(
                virtual_root.alternatives[id].bounds,
                BoundIntervalV1::UNKNOWN,
                "completion alone does not create a value"
            );
            virtual_root
                .update_successor_bounds(id, BoundIntervalV1::exact(value), None)
                .unwrap();
            let oracle_action_value =
                flat_truth.node_values[&FixtureNodeId(if value == 1 { 2 } else { 3 })];
            let alternative = &virtual_root.alternatives[id];
            assert!(alternative.bounds.lower <= oracle_action_value);
            assert!(oracle_action_value <= alternative.bounds.upper);
            assert_eq!(
                alternative
                    .successor_binding
                    .as_ref()
                    .unwrap()
                    .successor_node_id,
                node
            );
            if id == p0_id.as_str() {
                assert_eq!(
                    virtual_root.alternatives[&p0_id].bounds,
                    BoundIntervalV1::exact(1)
                );
                assert_eq!(
                    virtual_root.alternatives[&p1_id].bounds,
                    BoundIntervalV1::UNKNOWN,
                    "updating one target bound must not update the other target cell"
                );
                let frontier = virtual_root.rebuild_root_critical_frontier();
                assert_eq!(frontier.status, RootCriticalFrontierStatusV3::Ready);
                assert_eq!(frontier.tasks, reference_frontier(&virtual_root));
                assert_eq!(frontier.tasks.len(), 1);
                assert_eq!(
                    frontier.tasks[0].root_action_support_ids.as_slice(),
                    std::slice::from_ref(&p1_id)
                );
                assert!(frontier.tasks[0]
                    .role_mask
                    .contains(RootRoleMaskV3::CHALLENGER_UPPER));
            }
        }
        assert_eq!(virtual_root.root_bounds(), BoundIntervalV1::exact(1));
        assert_eq!(
            virtual_root.certified_optimal_alternative_ids().as_slice(),
            std::slice::from_ref(&p0_id)
        );
    }

    #[test]
    fn complete_domain_attestation_is_atomic_and_rejects_missing_or_duplicate_candidates() {
        let (mut root, _, _, envelope_id) = root_with_prefix();
        let stale_progress = root.advance_unresolved_prefix(
            &envelope_id,
            vec![CAST.to_owned()],
            slot(
                "construction:bolt:target-stage-0",
                PROTOCOL,
                "target-pick-stage-0",
                "enumerate-complete-target-domain",
                0,
                1,
            ),
            cast_prefix_progress("stale-decision-revision"),
        );
        assert_eq!(
            stale_progress,
            Err(VirtualPhysicalRootErrorV3::InvalidProgress)
        );
        let mut wrong_actor_progress = cast_prefix_progress(ROOT_CONTINUATION);
        wrong_actor_progress.actor = PlayerId::P1;
        assert_eq!(
            root.advance_unresolved_prefix(
                &envelope_id,
                vec![CAST.to_owned()],
                slot(
                    "construction:bolt:target-stage-0",
                    PROTOCOL,
                    "target-pick-stage-0",
                    "enumerate-complete-target-domain",
                    0,
                    1,
                ),
                wrong_actor_progress,
            ),
            Err(VirtualPhysicalRootErrorV3::InvalidProgress)
        );
        assert!(root.envelopes.contains_key(&envelope_id));
        let envelope_id = root
            .advance_unresolved_prefix(
                &envelope_id,
                vec![CAST.to_owned()],
                slot(
                    "construction:bolt:target-stage-0",
                    PROTOCOL,
                    "target-pick-stage-0",
                    "enumerate-complete-target-domain",
                    0,
                    1,
                ),
                cast_prefix_progress(ROOT_CONTINUATION),
            )
            .unwrap();
        let incomplete = CompletePhysicalActionDomainV3::new(
            domain_context(),
            attestation(
                vec![
                    vec![CAST.to_owned(), TARGET_P0.to_owned()],
                    vec![CAST.to_owned(), TARGET_P1.to_owned()],
                ],
                "oracle",
            ),
            vec![target_candidate(0, TARGET_P0)],
        );
        assert_eq!(
            incomplete.unwrap_err(),
            VirtualPhysicalRootErrorV3::IncompleteCandidateAttestation
        );
        assert!(root.envelopes.contains_key(&envelope_id));
        assert_eq!(root.alternatives.len(), 2);

        let duplicate = CompletePhysicalActionDomainV3::new(
            domain_context(),
            attestation(
                vec![
                    vec![CAST.to_owned(), TARGET_P0.to_owned()],
                    vec![CAST.to_owned(), TARGET_P0.to_owned()],
                ],
                "oracle",
            ),
            vec![
                target_candidate(0, TARGET_P0),
                target_candidate(1, TARGET_P0),
            ],
        );
        assert_eq!(
            duplicate.unwrap_err(),
            VirtualPhysicalRootErrorV3::InvalidCandidate
        );
        assert!(root.envelopes.contains_key(&envelope_id));
        assert_eq!(root.alternatives.len(), 2);

        let mut wrong_actor = target_candidate(0, TARGET_P0);
        wrong_actor.identity.owner_actor = PlayerId::P1;
        wrong_actor.construction_path[0].actor = PlayerId::P1;
        wrong_actor.next_expansion_slot.actor = PlayerId::P1;
        wrong_actor.identity.construction_path_identity = wrong_actor
            .construction_path
            .iter()
            .map(ConstructionSlotIdentityV3::stable_identity)
            .collect();
        assert!(CompletePhysicalActionDomainV3::new(
            domain_context(),
            attestation(vec![vec![CAST.to_owned(), TARGET_P0.to_owned()]], "oracle",),
            vec![wrong_actor],
        )
        .is_err());

        let mut stale_owner = target_candidate(0, TARGET_P0);
        stale_owner.identity.physical_owner_id = "stale-decision-revision".to_owned();
        stale_owner.construction_path[0].physical_owner_id = "stale-decision-revision".to_owned();
        stale_owner.next_expansion_slot.physical_owner_id = "stale-decision-revision".to_owned();
        stale_owner.identity.construction_path_identity = stale_owner
            .construction_path
            .iter()
            .map(ConstructionSlotIdentityV3::stable_identity)
            .collect();
        assert!(CompletePhysicalActionDomainV3::new(
            domain_context(),
            attestation(vec![vec![CAST.to_owned(), TARGET_P0.to_owned()]], "oracle",),
            vec![stale_owner],
        )
        .is_err());

        let mut wrong_stage = target_candidate(0, TARGET_P0);
        wrong_stage.construction_path[0].stage_identity = "stale-stage-99".to_owned();
        wrong_stage.next_expansion_slot.stage_identity = "stale-stage-99".to_owned();
        wrong_stage.identity.construction_path_identity = wrong_stage
            .construction_path
            .iter()
            .map(ConstructionSlotIdentityV3::stable_identity)
            .collect();
        assert!(CompletePhysicalActionDomainV3::new(
            domain_context(),
            attestation(vec![vec![CAST.to_owned(), TARGET_P0.to_owned()]], "oracle",),
            vec![wrong_stage],
        )
        .is_err());

        let mut wrong_continuation = target_candidate(0, TARGET_P0);
        wrong_continuation.identity.continuation_identity = "stale-continuation".to_owned();
        assert!(CompletePhysicalActionDomainV3::new(
            domain_context(),
            attestation(vec![vec![CAST.to_owned(), TARGET_P0.to_owned()]], "oracle",),
            vec![wrong_continuation],
        )
        .is_err());

        assert!(root.envelopes.contains_key(&envelope_id));
        assert_eq!(root.alternatives.len(), 2);
    }

    #[test]
    fn successor_binding_rejects_owner_changes_and_path_cycles() {
        let (mut root, _, _, envelope_id) = root_with_prefix();
        let envelope_id = root
            .advance_unresolved_prefix(
                &envelope_id,
                vec![CAST.to_owned()],
                slot(
                    "construction:bolt:target-stage-0",
                    PROTOCOL,
                    "target-pick-stage-0",
                    "enumerate-complete-target-domain",
                    0,
                    1,
                ),
                cast_prefix_progress(ROOT_CONTINUATION),
            )
            .unwrap();
        let p0_id = root
            .admit_complete_domain(&envelope_id, oracle_domain())
            .unwrap()[0]
            .clone();
        let responses = vec![CAST.to_owned(), TARGET_P0.to_owned()];
        for (owner, successor, path) in [
            (
                "other-physical-owner",
                "oracle-successor:2",
                vec!["game:root".to_owned(), "oracle-successor:2".to_owned()],
            ),
            (
                OWNER,
                "game:root",
                vec![
                    "game:root".to_owned(),
                    "oracle-successor:2".to_owned(),
                    "game:root".to_owned(),
                ],
            ),
        ] {
            assert_eq!(
                root.complete_alternative(
                    &p0_id,
                    &responses,
                    SuccessorBindingV3 {
                        physical_owner_id: owner.to_owned(),
                        owner_graph_node_id: "game:root".to_owned(),
                        ordered_engine_responses: responses.clone(),
                        finalization_boundary: "cast-finalized".to_owned(),
                        successor_node_id: successor.to_owned(),
                        next_expansion_slot: Some(actor_slot(
                            successor,
                            PlayerId::P0,
                            "priority",
                            "post-cast",
                            "continue",
                            0,
                            1,
                        )),
                        graph_path_node_ids: path,
                        next_actor: Some(PlayerId::P0),
                        provenance: SuccessorProvenanceV3::OracleFixture {
                            domain_fixture_identity: "mads03b4-flat-physical-root-v1".to_owned(),
                            successor_fixture_identity: "mads03b4-flat-physical-root-v1".to_owned(),
                            physical_response_identity: p0_id.clone(),
                        },
                    },
                ),
                Err(VirtualPhysicalRootErrorV3::InvalidSuccessorBinding)
            );
            assert_eq!(
                root.alternatives[&p0_id].state,
                PhysicalAlternativeStateV3::PartiallyConstructed,
                "failed bindings are atomic"
            );
        }
    }

    #[test]
    fn transition_identity_is_distinct_but_linked_to_source_frame_and_response() {
        let (mut root, _, _, initial_envelope_id) = root_with_prefix();
        let envelope_id = root
            .advance_unresolved_prefix(
                &initial_envelope_id,
                vec![CAST.to_owned()],
                slot(
                    "construction:bolt:target-stage-0",
                    PROTOCOL,
                    "target-pick-stage-0",
                    "enumerate-complete-target-domain",
                    0,
                    1,
                ),
                cast_prefix_progress(ROOT_CONTINUATION),
            )
            .unwrap();
        let frame_identity = "engine-frame:decision-revision-17";
        let domain = CompletePhysicalActionDomainV3::new(
            domain_context(),
            CompleteCandidateDomainAttestationV3 {
                ordered_candidate_responses: vec![
                    vec![CAST.to_owned(), TARGET_P0.to_owned()],
                    vec![CAST.to_owned(), TARGET_P1.to_owned()],
                ],
                evidence: CompleteDomainEvidenceV3::AuthoritativeEngineFrame {
                    frame_identity: frame_identity.to_owned(),
                },
            },
            vec![
                target_candidate(0, TARGET_P0),
                target_candidate(1, TARGET_P1),
            ],
        )
        .unwrap();
        let p0_id = root.admit_complete_domain(&envelope_id, domain).unwrap()[0].clone();
        let responses = vec![CAST.to_owned(), TARGET_P0.to_owned()];
        let make_binding = |source_frame_identity: &str| SuccessorBindingV3 {
            physical_owner_id: OWNER.to_owned(),
            owner_graph_node_id: "game:root".to_owned(),
            ordered_engine_responses: responses.clone(),
            finalization_boundary: "cast-finalized".to_owned(),
            successor_node_id: "engine-successor:2".to_owned(),
            next_expansion_slot: None,
            graph_path_node_ids: vec!["game:root".to_owned(), "engine-successor:2".to_owned()],
            next_actor: Some(PlayerId::P0),
            provenance: SuccessorProvenanceV3::AuthoritativeTransition {
                source_frame_identity: source_frame_identity.to_owned(),
                transition_identity: "engine-transition:91".to_owned(),
                physical_response_identity: p0_id.clone(),
            },
        };
        assert_ne!(frame_identity, "engine-transition:91");
        assert_eq!(
            root.complete_alternative(&p0_id, &responses, make_binding("other-frame")),
            Err(VirtualPhysicalRootErrorV3::InvalidSuccessorBinding)
        );
        assert_eq!(
            root.alternatives[&p0_id].state,
            PhysicalAlternativeStateV3::PartiallyConstructed
        );
        root.complete_alternative(&p0_id, &responses, make_binding(frame_identity))
            .unwrap();
        assert_eq!(
            root.alternatives[&p0_id].state,
            PhysicalAlternativeStateV3::Completed
        );
    }

    #[test]
    fn successor_bound_update_rejects_wrong_successor_node_and_actor_atomically() {
        let (mut root, _, _, initial_envelope_id) = root_with_prefix();
        let envelope_id = root
            .advance_unresolved_prefix(
                &initial_envelope_id,
                vec![CAST.to_owned()],
                slot(
                    "construction:bolt:target-stage-0",
                    PROTOCOL,
                    "target-pick-stage-0",
                    "enumerate-complete-target-domain",
                    0,
                    1,
                ),
                cast_prefix_progress(ROOT_CONTINUATION),
            )
            .unwrap();
        let p0_id = root
            .admit_complete_domain(&envelope_id, oracle_domain())
            .unwrap()[0]
            .clone();
        let responses = vec![CAST.to_owned(), TARGET_P0.to_owned()];
        let expected_task = actor_slot(
            "oracle-successor:2",
            PlayerId::P0,
            "priority",
            "post-cast",
            "continue",
            0,
            1,
        );
        root.complete_alternative(
            &p0_id,
            &responses,
            SuccessorBindingV3 {
                physical_owner_id: OWNER.to_owned(),
                owner_graph_node_id: "game:root".to_owned(),
                ordered_engine_responses: responses.clone(),
                finalization_boundary: "cast-finalized".to_owned(),
                successor_node_id: "oracle-successor:2".to_owned(),
                next_expansion_slot: Some(expected_task.clone()),
                graph_path_node_ids: vec!["game:root".to_owned(), "oracle-successor:2".to_owned()],
                next_actor: Some(PlayerId::P0),
                provenance: SuccessorProvenanceV3::OracleFixture {
                    domain_fixture_identity: "mads03b4-flat-physical-root-v1".to_owned(),
                    successor_fixture_identity: "mads03b4-successor-values-v1".to_owned(),
                    physical_response_identity: p0_id.clone(),
                },
            },
        )
        .unwrap();

        let wrong_node_task = actor_slot(
            "other-successor-node",
            PlayerId::P0,
            "priority",
            "post-cast",
            "continue",
            0,
            1,
        );
        assert_eq!(
            root.update_successor_bounds(&p0_id, BoundIntervalV1::exact(0), Some(wrong_node_task)),
            Err(VirtualPhysicalRootErrorV3::InvalidSuccessorBinding)
        );

        let wrong_actor_task = actor_slot(
            "oracle-successor:2",
            PlayerId::P1,
            "priority",
            "post-cast",
            "continue",
            0,
            1,
        );
        assert_eq!(
            root.update_successor_bounds(&p0_id, BoundIntervalV1::exact(0), Some(wrong_actor_task)),
            Err(VirtualPhysicalRootErrorV3::InvalidSuccessorBinding)
        );

        let alternative = &root.alternatives[&p0_id];
        assert_eq!(alternative.state, PhysicalAlternativeStateV3::Completed);
        assert_eq!(alternative.bounds, BoundIntervalV1::UNKNOWN);
        assert_eq!(alternative.next_expansion_slot, Some(expected_task));
    }

    #[test]
    fn shared_cast_prefix_task_unions_roles_but_keeps_target_root_ids_distinct() {
        let mut root = VirtualPhysicalRootV3::new(OWNER, PlayerId::P0).unwrap();
        let mut p0 = target_candidate(0, TARGET_P0);
        p0.executed_response_prefix.clear();
        p0.next_expansion_slot = slot("game:root", "priority", "root", CAST, 0, 0);
        let mut p1 = target_candidate(1, TARGET_P1);
        p1.executed_response_prefix.clear();
        p1.next_expansion_slot = slot("game:root", "priority", "root", CAST, 0, 0);
        let p0_id = root.insert_known_alternative(p0).unwrap();
        let p1_id = root.insert_known_alternative(p1).unwrap();
        assert_ne!(p0_id, p1_id);
        assert!(!root.root_action_domain_complete());
        assert_eq!(root.root_bounds(), BoundIntervalV1::UNKNOWN);
        assert!(root.certified_optimal_alternative_ids().is_empty());
        let unattested = root.rebuild_root_critical_frontier();
        assert_eq!(
            unattested.status,
            RootCriticalFrontierStatusV3::DomainNotAttested
        );
        assert!(unattested.tasks.is_empty());
        assert_eq!(
            root.attest_root_domain(PhysicalRootDomainAttestationV3 {
                root_decision_identity: OWNER.to_owned(),
                root_actor: PlayerId::P0,
                ordered_root_item_ids: vec![p0_id.clone()],
                evidence: CompleteDomainEvidenceV3::OracleFixture {
                    fixture_identity: "mads03b4-flat-physical-root-v1".to_owned(),
                },
            }),
            Err(VirtualPhysicalRootErrorV3::IncompleteCandidateAttestation)
        );
        root.attest_root_domain(PhysicalRootDomainAttestationV3 {
            root_decision_identity: OWNER.to_owned(),
            root_actor: PlayerId::P0,
            ordered_root_item_ids: vec![p0_id.clone(), p1_id.clone()],
            evidence: CompleteDomainEvidenceV3::OracleFixture {
                fixture_identity: "mads03b4-flat-physical-root-v1".to_owned(),
            },
        })
        .unwrap();

        let frontier = root.rebuild_root_critical_frontier();
        assert_eq!(frontier.status, RootCriticalFrontierStatusV3::Ready);
        assert_eq!(frontier.tasks, reference_frontier(&root));
        assert_eq!(
            frontier.tasks.len(),
            1,
            "the shared CastSpell slot appears once"
        );
        assert_eq!(frontier.tasks[0].slot.slot_id, CAST);
        assert!(frontier.tasks[0].role_mask.supports_both());
        assert_eq!(frontier.tasks[0].root_action_support_ids, [p0_id, p1_id]);
        assert!(frontier.tasks[0].unresolved_prefix_support_ids.is_empty());
        assert_eq!(root.alternatives.len(), 2);
        assert_ne!(
            root.alternatives.values().next().unwrap().bounds,
            BoundIntervalV1::exact(1),
            "an unexpanded shared prefix gives neither alternative a value"
        );
    }

    #[test]
    fn empty_frontier_reports_critical_unevaluated_successor_instead_of_exhausted() {
        let mut root = VirtualPhysicalRootV3::new(OWNER, PlayerId::P0).unwrap();
        let make_candidate =
            |order: u32, response: &str, task: ExpansionSlotIdentityV3| PhysicalRootCandidateV3 {
                identity: PhysicalRootActionIdentityV3 {
                    schema_version: VIRTUAL_PHYSICAL_ROOT_SCHEMA_VERSION_V3,
                    physical_owner_id: OWNER.to_owned(),
                    owner_actor: PlayerId::P0,
                    stable_order: order,
                    continuation_identity: "scheduler-states-v1".to_owned(),
                    construction_path_identity: Vec::new(),
                    ordered_engine_responses: vec![response.to_owned()],
                },
                construction_path: Vec::new(),
                next_expansion_slot: task,
                executed_response_prefix: Vec::new(),
            };
        let a_id = root
            .insert_known_alternative(make_candidate(
                0,
                "A",
                slot("game:root", "priority", "root", "A", 0, 0),
            ))
            .unwrap();
        let b_id = root
            .insert_known_alternative(make_candidate(
                1,
                "B",
                slot("game:root", "priority", "root", "B", 1, 0),
            ))
            .unwrap();
        let c_id = root
            .insert_known_alternative(make_candidate(
                2,
                "C",
                slot("game:root", "priority", "root", "C", 2, 0),
            ))
            .unwrap();
        root.attest_root_domain(PhysicalRootDomainAttestationV3 {
            root_decision_identity: OWNER.to_owned(),
            root_actor: PlayerId::P0,
            ordered_root_item_ids: vec![a_id.clone(), b_id.clone(), c_id.clone()],
            evidence: CompleteDomainEvidenceV3::OracleFixture {
                fixture_identity: "frontier-status-fixture".to_owned(),
            },
        })
        .unwrap();

        for (id, response, successor, next_slot) in [
            (a_id.as_str(), "A", "successor:A", None),
            (b_id.as_str(), "B", "successor:B", None),
            (
                c_id.as_str(),
                "C",
                "successor:C",
                Some(actor_slot(
                    "successor:C",
                    PlayerId::P1,
                    "priority",
                    "response",
                    "continue-C",
                    0,
                    1,
                )),
            ),
        ] {
            let responses = vec![response.to_owned()];
            root.complete_alternative(
                id,
                &responses,
                SuccessorBindingV3 {
                    physical_owner_id: OWNER.to_owned(),
                    owner_graph_node_id: "game:root".to_owned(),
                    ordered_engine_responses: responses.clone(),
                    finalization_boundary: "physical-action-complete".to_owned(),
                    successor_node_id: successor.to_owned(),
                    next_expansion_slot: next_slot,
                    graph_path_node_ids: vec!["game:root".to_owned(), successor.to_owned()],
                    next_actor: Some(PlayerId::P1),
                    provenance: SuccessorProvenanceV3::OracleFixture {
                        domain_fixture_identity: "frontier-status-fixture".to_owned(),
                        successor_fixture_identity: "frontier-successors-fixture".to_owned(),
                        physical_response_identity: id.to_owned(),
                    },
                },
            )
            .unwrap();
        }
        root.update_successor_bounds(&a_id, BoundIntervalV1::exact(0), None)
            .unwrap();
        root.update_successor_bounds(&b_id, BoundIntervalV1::UNKNOWN, None)
            .unwrap();
        root.update_successor_bounds(
            &c_id,
            BoundIntervalV1::UNKNOWN,
            Some(actor_slot(
                "successor:C",
                PlayerId::P1,
                "priority",
                "response",
                "continue-C",
                0,
                1,
            )),
        )
        .unwrap();

        let snapshot = root.rebuild_root_critical_frontier();
        assert_eq!(snapshot.tasks, reference_frontier(&root));
        assert!(snapshot.tasks.is_empty());
        assert_eq!(
            snapshot.status,
            RootCriticalFrontierStatusV3::BlockedOnUnevaluatedSuccessor
        );
        assert_eq!(snapshot.blocked_on_unevaluated_successor, [b_id]);
        assert_eq!(root.root_bounds(), BoundIntervalV1 { lower: 0, upper: 1 });
        assert!(root.certified_optimal_alternative_ids().is_empty());
    }
}
