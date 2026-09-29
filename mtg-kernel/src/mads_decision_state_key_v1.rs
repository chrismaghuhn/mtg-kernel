//! Exact identity contract for a future real-engine MADS transposition key.
//!
//! This is deliberately not wired into a searcher or a hash table. The
//! generic surface, construction, and session components must each be an
//! owned, recursively complete structural snapshot before a caller can use
//! this key. `PolicySurfaceV5`, `RlEpisodeSessionV1`, and
//! `FastActorSessionV1` now have structural `PartialEq` implementations that
//! compare their complete private fields. `Decision` remains `PartialEq`
//! only. No production adapter currently constructs this envelope at the
//! live decision boundary, and no semantic bisimulation is claimed. There is
//! intentionally no `Hash` implementation: a future bucket digest must never
//! substitute for `PartialEq` below.

use crate::engine::Decision;
use crate::policy_surface_v5::PolicyActionV5;
use crate::state::GameState;

pub const MADS_DECISION_STATE_KEY_SCHEMA_V1: &str = "mads.decision-state-key.v1";

/// Fail-closed reasons from the owned-session live-capture preflight. This is
/// deliberately not a key and cannot authorize transposition reuse.
#[allow(dead_code)] // Reserved for the not-yet-integrated Dynamic MADS boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LiveDecisionCaptureRejectionV1 {
    NoCurrentDecision,
    TerminalSession,
    StaleSessionBinding,
    UnsupportedDecisionBoundary,
    CandidateSnapshotMismatch,
    DirtyBuild,
    MissingSchedulerContract,
}

/// Authenticate the build inputs needed by a live key. Current MADS has no
/// checked-in engine scheduler contract, so this gate never opens yet.
#[allow(dead_code)] // Kept closed until DynamicEngineSearch owns this boundary.
pub(crate) fn authenticated_namespace_gate_v1(
) -> Result<DecisionStateNamespaceV1, LiveDecisionCaptureRejectionV1> {
    if env!("MTG_KERNEL_BUILD_GIT_CLEAN") != "true" {
        return Err(LiveDecisionCaptureRejectionV1::DirtyBuild);
    }
    let head = env!("MTG_KERNEL_BUILD_GIT_HEAD");
    let tree_digest = env!("MTG_KERNEL_BUILD_TRACKED_TREE_SHA256");
    let tree_contract = env!("MTG_KERNEL_BUILD_TRACKED_TREE_CONTRACT");
    if head.len() != 40
        || !head.bytes().all(|byte| byte.is_ascii_hexdigit())
        || tree_digest.len() != 64
        || !tree_digest.bytes().all(|byte| byte.is_ascii_hexdigit())
        || tree_contract.is_empty()
        || crate::card_def::KERNEL_CARDDB_HASH == 0
    {
        return Err(LiveDecisionCaptureRejectionV1::DirtyBuild);
    }
    // `FRONTIER_POLICY_V1` is only the synthetic reference scheduler. It is
    // not an engine search contract and cannot safely namespace a live key.
    Err(LiveDecisionCaptureRejectionV1::MissingSchedulerContract)
}

/// Immutable engine/search namespace. Every field participates in equality;
/// callers must use content identities, not mutable labels, for the values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecisionStateNamespaceV1 {
    pub engine_source_revision: String,
    pub rules_contract: String,
    pub card_database_identity: String,
    pub card_database_hash: u64,
    pub decision_schema_version: u32,
    pub policy_surface_version: u32,
    pub randomization_contract: String,
    pub scheduler_contract: String,
    pub key_schema: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecisionStateNamespaceInputsV1 {
    pub engine_source_revision: String,
    pub rules_contract: String,
    pub card_database_identity: String,
    pub card_database_hash: u64,
    pub decision_schema_version: u32,
    pub policy_surface_version: u32,
    pub randomization_contract: String,
    pub scheduler_contract: String,
}

impl DecisionStateNamespaceV1 {
    pub fn mads_v1(inputs: DecisionStateNamespaceInputsV1) -> Self {
        Self {
            engine_source_revision: inputs.engine_source_revision,
            rules_contract: inputs.rules_contract,
            card_database_identity: inputs.card_database_identity,
            card_database_hash: inputs.card_database_hash,
            decision_schema_version: inputs.decision_schema_version,
            policy_surface_version: inputs.policy_surface_version,
            randomization_contract: inputs.randomization_contract,
            scheduler_contract: inputs.scheduler_contract,
            key_schema: MADS_DECISION_STATE_KEY_SCHEMA_V1.to_string(),
        }
    }
}

/// Exact comparison envelope. `Surface`, `Construction`, and `Session` are
/// mandatory caller supplied owned snapshots; their `PartialEq` contract must
/// compare every future-relevant field, including private continuation and
/// ordered candidate state. `PartialEq` is structural and recursive for all
/// stored components. This type makes no semantic-equivalence claim beyond
/// those component contracts and must not be used until the real adapters are
/// proven complete.
#[derive(Debug, Clone, PartialEq)]
pub struct DecisionStateKeyContractV1<Surface, Construction, Session> {
    pub namespace: DecisionStateNamespaceV1,
    pub game_state: GameState,
    pub engine_decision: Decision,
    pub policy_surface: Surface,
    /// Canonical order is significant; the same candidates in another order
    /// are a different protocol state.
    pub ordered_policy_candidates: Vec<PolicyActionV5>,
    pub construction: Option<Construction>,
    pub session: Session,
}

impl<Surface, Construction, Session> DecisionStateKeyContractV1<Surface, Construction, Session>
where
    Surface: PartialEq,
    Construction: PartialEq,
    Session: PartialEq,
{
    /// Hashes, if added by a future table adapter, may select a bucket only.
    /// Reuse requires this full structural comparison to succeed.
    pub fn exactly_matches(&self, other: &Self) -> bool {
        self == other
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::Decision;
    use crate::ids::{ObjectId, PlayerId};
    use crate::policy_surface_v5::PolicyActionV5;

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct SurfaceSnapshot {
        scan_cursor: usize,
        ordered_scan_candidates: Vec<ObjectId>,
        suppression_count: u64,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct ConstructionSnapshot {
        protocol: String,
        selected_prefix: Vec<u32>,
        cursor: u32,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct SessionSnapshot {
        episode_id: u64,
        environment_revision: u64,
        policy_steps: u64,
        physical_decisions: u64,
        terminal: bool,
    }

    fn namespace() -> DecisionStateNamespaceV1 {
        DecisionStateNamespaceV1::mads_v1(DecisionStateNamespaceInputsV1 {
            engine_source_revision: "source-tree:fixture-revision".into(),
            rules_contract: "rules-v1".into(),
            card_database_identity: "pauper-db-v1".into(),
            card_database_hash: 17,
            decision_schema_version: 1,
            policy_surface_version: 5,
            randomization_contract: "environment-randomization-v2".into(),
            scheduler_contract: "dynamic-mads-v1".into(),
        })
    }

    fn game_state() -> GameState {
        GameState::new_from_libraries(&[1, 2], &[3, 4], |id| format!("card-{id}"), 99)
    }

    fn key() -> DecisionStateKeyContractV1<SurfaceSnapshot, ConstructionSnapshot, SessionSnapshot> {
        DecisionStateKeyContractV1 {
            namespace: namespace(),
            game_state: game_state(),
            engine_decision: Decision::ChooseKicker {
                player: PlayerId::P0,
                spell: ObjectId(0),
            },
            policy_surface: SurfaceSnapshot {
                scan_cursor: 1,
                ordered_scan_candidates: vec![ObjectId(8), ObjectId(9)],
                suppression_count: 3,
            },
            ordered_policy_candidates: vec![
                PolicyActionV5::ChooseAttackerInclusion {
                    actor: PlayerId::P0,
                    attacker: ObjectId(8),
                    include: false,
                },
                PolicyActionV5::ChooseAttackerInclusion {
                    actor: PlayerId::P0,
                    attacker: ObjectId(8),
                    include: true,
                },
            ],
            construction: Some(ConstructionSnapshot {
                protocol: "choose-targets".into(),
                selected_prefix: vec![2],
                cursor: 1,
            }),
            session: SessionSnapshot {
                episode_id: 4,
                environment_revision: 11,
                policy_steps: 7,
                physical_decisions: 5,
                terminal: false,
            },
        }
    }

    #[test]
    fn exact_key_contract_accepts_structurally_identical_owned_snapshots() {
        let a = key();
        let b = key();
        assert!(a.exactly_matches(&b));
        assert_eq!(a, b);
    }

    #[test]
    fn namespace_equality_covers_all_engine_and_protocol_versions() {
        let base = namespace();

        let mut changed = base.clone();
        changed.rules_contract.push_str("-other");
        assert_ne!(base, changed);
        let mut changed = base.clone();
        changed.card_database_identity.push_str("-other");
        assert_ne!(base, changed);
        let mut changed = base.clone();
        changed.card_database_hash ^= 1;
        assert_ne!(base, changed);
        let mut changed = base.clone();
        changed.decision_schema_version += 1;
        assert_ne!(base, changed);
        let mut changed = base.clone();
        changed.policy_surface_version += 1;
        assert_ne!(base, changed);
        let mut changed = base.clone();
        changed.randomization_contract.push_str("-other");
        assert_ne!(base, changed);
        let mut changed = base.clone();
        changed.scheduler_contract.push_str("-other");
        assert_ne!(base, changed);
        let mut changed = base.clone();
        changed.key_schema.push_str("-other");
        assert_ne!(base, changed);
    }

    #[test]
    fn exact_key_contract_rejects_differences_in_every_identity_component() {
        let base = key();

        let mut paired = base.clone();
        paired.namespace.engine_source_revision.push_str("-other");
        assert!(!base.exactly_matches(&paired));

        let mut paired = base.clone();
        paired.game_state.players[0].library.swap(0, 1);
        assert!(!base.exactly_matches(&paired));

        let mut paired = base.clone();
        paired.engine_decision = Decision::ChooseKicker {
            player: PlayerId::P1,
            spell: ObjectId(0),
        };
        assert!(!base.exactly_matches(&paired));

        let mut paired = base.clone();
        paired.policy_surface.scan_cursor += 1;
        assert!(!base.exactly_matches(&paired));

        let mut paired = base.clone();
        paired.ordered_policy_candidates.reverse();
        assert!(!base.exactly_matches(&paired));

        let mut paired = base.clone();
        paired
            .construction
            .as_mut()
            .unwrap()
            .selected_prefix
            .push(3);
        assert!(!base.exactly_matches(&paired));

        let mut paired = base.clone();
        paired.session.environment_revision += 1;
        assert!(!base.exactly_matches(&paired));
    }
}
