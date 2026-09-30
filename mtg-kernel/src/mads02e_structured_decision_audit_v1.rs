//! Test-only protocol probes for structured MADS decision construction.
//!
//! These fixtures audit real engine continuations. They do not implement
//! ConstructionNodes, modify rules, or publish teacher labels.

use crate::card_def::card_id_by_name;
use crate::engine::{self, Action, CastMode, Decision};
use crate::event::{self, ProposedEvent};
use crate::ids::{ObjectId, PlayerId};
use crate::policy_surface_v5::PolicyDecisionV5;
use crate::rl::{self, card_name, legal_action_candidates_v5};
use crate::runtime_decks::runtime_deck_by_id;
use crate::state::{Counters, GameObject, GameState, ObjectStateV4, Step, Target, Zone};
use crate::surface_v2::{HarnessSurfaceV2, SurfaceAction, SurfaceDecision};

const CAST_CONTINUATION_SEED: u64 = 0x2;
const APNAP_BATCH_SEED: u64 = 0x02e0_0001;
const FIREBLAST_PROTOCOL_SEED: u64 = 0x02e0_0002;
const RAW_V5_REJECTION_SEED: u64 = 0x02e0_0003;

fn first_p0_main1(state: &mut GameState) -> Decision {
    for _ in 0..32 {
        let decision = engine::advance_until_decision(state);
        match &decision {
            Decision::CastSpellOrPass { player, .. }
                if *player == PlayerId::P0 && state.step == Step::Main1 =>
            {
                return decision;
            }
            Decision::CastSpellOrPass { .. } => {
                engine::step(state, Action::Pass).expect("priority pass before P0 Main1");
            }
            Decision::DeclareAttackers { eligible, .. } if eligible.is_empty() => {
                engine::step(state, Action::DeclareAttackers(Vec::new()))
                    .expect("empty attacker declaration during setup");
            }
            Decision::GameOver { .. } => panic!("fixture ended before P0 Main1"),
            Decision::Halted { .. } => panic!("fixture reached Halted before P0 Main1"),
            other => panic!("unsupported setup protocol before P0 Main1: {other:?}"),
        }
    }
    panic!("P0 Main1 not reached within 32 engine decisions")
}

fn burn_start_with_mountain_and_bolt() -> GameState {
    let burn = runtime_deck_by_id("Burn").expect("official runtime Burn deck");
    let mountain = card_id_by_name("Mountain").unwrap();
    let bolt = card_id_by_name("Lightning Bolt").unwrap();
    assert_eq!(burn.id, "Burn");
    assert_eq!(burn.mainboard_count, 60);
    assert!(burn.card_ids.contains(&mountain));
    assert!(burn.card_ids.contains(&bolt));
    assert_eq!(
        burn.source_sha256,
        "4ebba6b42bb27a0ea55001cee133aada81f0dffd8661b46b012fc5026675aa32"
    );
    assert_eq!(burn.runtime_deck_hash, 0x5fdb_7b92_986b_6fc1);
    let state = rl::build_deck_pair_state(CAST_CONTINUATION_SEED, burn.card_ids, burn.card_ids)
        .expect("cataloged Burn mirror passes deck preflight");
    let opening_hand = &state.players[PlayerId::P0.index()].hand;
    assert!(opening_hand
        .iter()
        .any(|id| state.objects.get(*id).card_def == mountain));
    assert!(opening_hand
        .iter()
        .any(|id| state.objects.get(*id).card_def == bolt));
    state
}

#[test]
fn pending_cast_targets_keep_the_caster_until_the_cast_commits() {
    let mut state = burn_start_with_mountain_and_bolt();
    let decision = first_p0_main1(&mut state);
    let Decision::CastSpellOrPass { land_drops, .. } = decision else {
        unreachable!()
    };
    let mountain = land_drops
        .into_iter()
        .find(|id| state.objects.get(*id).card_def == card_id_by_name("Mountain").unwrap())
        .expect("Burn opening has a legal Mountain land drop");
    engine::step(&mut state, Action::PlayLand(mountain)).unwrap();

    let Decision::CastSpellOrPass { mana_abilities, .. } =
        engine::advance_until_decision(&mut state)
    else {
        panic!("P0 must retain priority after a land play")
    };
    assert!(mana_abilities.contains(&mountain));
    engine::step(&mut state, Action::ActivateManaAbility(mountain)).unwrap();

    let Decision::CastSpellOrPass {
        player,
        castable_spells,
        ..
    } = engine::advance_until_decision(&mut state)
    else {
        panic!("mana activation must return to a priority decision")
    };
    assert_eq!(player, PlayerId::P0);
    let bolt = castable_spells
        .into_iter()
        .find(|id| state.objects.get(*id).card_def == card_id_by_name("Lightning Bolt").unwrap())
        .expect("the opened {R} pays for Lightning Bolt");

    engine::step(&mut state, Action::CastSpell(bolt)).unwrap();
    let target_decision = engine::advance_until_decision(&mut state);
    assert!(matches!(
        target_decision,
        Decision::ChooseTargets {
            player: PlayerId::P0,
            remaining: 1,
            ref legal_targets,
            ..
        } if legal_targets.contains(&Target::Player(PlayerId::P1))
    ));
    assert!(state.engine.pending_cast.is_some());
    assert_eq!(state.stack.last().unwrap().source, bolt);
    assert!(state.stack.last().unwrap().targets.is_empty());

    let before_illegal_pass = state.clone();
    assert!(engine::step(&mut state, Action::Pass).is_err());
    assert_eq!(
        state, before_illegal_pass,
        "pending cast must reject priority actions transactionally"
    );

    engine::step(
        &mut state,
        Action::ChooseTarget(Target::Player(PlayerId::P1)),
    )
    .unwrap();
    assert_eq!(
        state.engine.pending_cast.as_ref().unwrap().targets_chosen,
        [Target::Player(PlayerId::P1)],
        "target answer remains a partial PendingCast until engine advancement"
    );
    let after_cast = engine::advance_until_decision(&mut state);
    assert!(matches!(
        after_cast,
        Decision::CastSpellOrPass {
            player: PlayerId::P0,
            ..
        }
    ));
    assert!(state.engine.pending_cast.is_none());
    let finalized = state.stack.last().unwrap();
    assert_eq!(finalized.source, bolt);
    assert_eq!(finalized.targets, [Target::Player(PlayerId::P1)]);

    engine::step(&mut state, Action::Pass).unwrap();
    assert!(matches!(
        engine::advance_until_decision(&mut state),
        Decision::CastSpellOrPass {
            player: PlayerId::P1,
            ..
        }
    ));
    assert!(state.engine.pending_cast.is_none());
    eprintln!(
        "CAST_CONSTRUCTION seed={CAST_CONTINUATION_SEED:#x} actor_before_commit=P0 actor_after_commit=P0 opponent_priority_after_pass=P1 stack_source={bolt:?}"
    );
}

fn fixture_object(state: &mut GameState, player: PlayerId, name: &str, zone: Zone) -> ObjectId {
    let card_def = card_id_by_name(name).unwrap_or_else(|| panic!("{name} is in CARD_DEFS"));
    let id = state.objects.push(GameObject {
        card_def,
        name: name.to_string(),
        owner: player,
        controller: player,
        zone,
        tapped: false,
        summoning_sick: false,
        damage: 0,
        counters: Counters::default(),
        attachments: Vec::new(),
        v4: ObjectStateV4::from_card_def(card_def),
        spell_copy_origin: None,
        plotted_turn: None,
        zone_change_count: 0,
    });
    match zone {
        Zone::Battlefield => state.players[player.index()].battlefield.push(id),
        Zone::Hand => state.players[player.index()].hand.push(id),
        other => panic!("fixture_object only installs battlefield/hand objects, got {other:?}"),
    }
    id
}

fn protocol_empty_state(seed: u64) -> GameState {
    GameState::new_from_libraries(&[], &[], card_name, seed)
}

fn settle_fireblast_mode(
    mut state: GameState,
    mode: CastMode,
    sacrifice: &[ObjectId],
) -> GameState {
    engine::step(&mut state, Action::ChooseCastMode(mode)).unwrap();
    let mut next = engine::advance_until_decision(&mut state);
    for &object in sacrifice {
        let Decision::ChooseCostTargets {
            player: PlayerId::P0,
            cost_kind: crate::engine::CostKind::SacrificeLands,
            candidates,
            ..
        } = next
        else {
            panic!("alternative Fireblast mode must expose its sacrifice choice: {next:?}")
        };
        assert!(candidates.contains(&object));
        engine::step(&mut state, Action::ChooseCostTarget(object)).unwrap();
        next = engine::advance_until_decision(&mut state);
    }
    assert!(matches!(
        next,
        Decision::CastSpellOrPass {
            player: PlayerId::P0,
            ..
        }
    ));
    assert!(state.engine.pending_cast.is_none());
    state
}

#[test]
fn fireblast_modes_and_object_payment_paths_remain_distinct() {
    let burn = runtime_deck_by_id("Burn").unwrap();
    let mountain_def = card_id_by_name("Mountain").unwrap();
    let fireblast_def = card_id_by_name("Fireblast").unwrap();
    assert!(burn.card_ids.contains(&mountain_def));
    assert!(burn.card_ids.contains(&fireblast_def));

    // Protocol-only synthetic board: all six existing Mountains and Fireblast
    // are represented consistently in the public arena and zones. This is not
    // a claim that the complete board was reached from a legal game start; no
    // Engine continuation or private field is forged.
    let mut state = protocol_empty_state(FIREBLAST_PROTOCOL_SEED);
    state.step = Step::Main1;
    state.active_player = PlayerId::P0;
    state.priority_player = PlayerId::P0;
    let mountains = (0..6)
        .map(|_| fixture_object(&mut state, PlayerId::P0, "Mountain", Zone::Battlefield))
        .collect::<Vec<_>>();
    let fireblast = fixture_object(&mut state, PlayerId::P0, "Fireblast", Zone::Hand);

    let Decision::CastSpellOrPass {
        player: PlayerId::P0,
        castable_spells,
        ..
    } = engine::advance_until_decision(&mut state)
    else {
        panic!("synthetic Fireblast root must be P0 priority")
    };
    assert!(castable_spells.contains(&fireblast));
    engine::step(&mut state, Action::CastSpell(fireblast)).unwrap();
    assert!(matches!(
        engine::advance_until_decision(&mut state),
        Decision::ChooseTargets {
            player: PlayerId::P0,
            ..
        }
    ));
    engine::step(
        &mut state,
        Action::ChooseTarget(Target::Player(PlayerId::P1)),
    )
    .unwrap();
    let mode_decision = engine::advance_until_decision(&mut state);
    assert!(matches!(
        &mode_decision,
        Decision::ChooseCastMode {
            player: PlayerId::P0,
            options,
            ..
        } if options == &[CastMode::Normal, CastMode::Alternative]
    ));

    let normal = settle_fireblast_mode(state.clone(), CastMode::Normal, &[]);
    let alternative_a = settle_fireblast_mode(
        state.clone(),
        CastMode::Alternative,
        &[mountains[0], mountains[1]],
    );
    let alternative_b =
        settle_fireblast_mode(state, CastMode::Alternative, &[mountains[2], mountains[3]]);

    for successor in [&normal, &alternative_a, &alternative_b] {
        let stack_item = successor.stack.last().unwrap();
        assert_eq!(stack_item.source, fireblast);
        assert_eq!(stack_item.targets, [Target::Player(PlayerId::P1)]);
    }
    assert!(mountains
        .iter()
        .all(|id| normal.players[PlayerId::P0.index()]
            .battlefield
            .contains(id)));
    let alternative_a_graveyard = &alternative_a.players[PlayerId::P0.index()].graveyard;
    let alternative_b_graveyard = &alternative_b.players[PlayerId::P0.index()].graveyard;
    assert_eq!(alternative_a_graveyard.len(), 2);
    assert_eq!(alternative_b_graveyard.len(), 2);
    assert!(alternative_a_graveyard.contains(&mountains[0]));
    assert!(alternative_a_graveyard.contains(&mountains[1]));
    assert!(alternative_b_graveyard.contains(&mountains[2]));
    assert!(alternative_b_graveyard.contains(&mountains[3]));
    assert_ne!(
        normal, alternative_a,
        "normal and sacrifice payment paths differ"
    );
    assert_ne!(
        alternative_a, alternative_b,
        "different legal sacrifice ObjectIds remain distinct"
    );
}

#[test]
fn apnap_trigger_group_can_yield_a_distinct_opponent_decision() {
    let mut state = protocol_empty_state(APNAP_BATCH_SEED);
    state.active_player = PlayerId::P0;
    let p0_a = fixture_object(
        &mut state,
        PlayerId::P0,
        "Clockwork Percussionist",
        Zone::Battlefield,
    );
    let p0_b = fixture_object(
        &mut state,
        PlayerId::P0,
        "Clockwork Percussionist",
        Zone::Battlefield,
    );
    let p1_a = fixture_object(
        &mut state,
        PlayerId::P1,
        "Clockwork Percussionist",
        Zone::Battlefield,
    );
    let p1_b = fixture_object(
        &mut state,
        PlayerId::P1,
        "Clockwork Percussionist",
        Zone::Battlefield,
    );

    event::propose_and_commit_batch(
        &mut state,
        [p0_a, p0_b, p1_a, p1_b]
            .into_iter()
            .map(|source| ProposedEvent::zone_change(source, Zone::Graveyard))
            .collect(),
    );
    let pending = crate::trigger::collect_and_process(&mut state);
    assert_eq!(pending.len(), 4);
    assert_eq!(
        pending
            .iter()
            .map(|trigger| trigger.controller)
            .collect::<Vec<_>>(),
        [PlayerId::P0, PlayerId::P0, PlayerId::P1, PlayerId::P1]
    );
    state.engine.pending_triggers.extend(pending);

    let first = engine::advance_until_decision(&mut state);
    assert!(matches!(
        &first,
        Decision::OrderTriggers {
            player: PlayerId::P0,
            pending,
        } if pending.len() == 2
    ));
    engine::step(&mut state, Action::OrderTriggers(vec![1, 0])).unwrap();
    assert_eq!(
        state.stack.len(),
        2,
        "P0's ordered triggers are placed first"
    );
    assert_eq!(state.engine.pending_triggers.len(), 2);
    assert!(state
        .engine
        .pending_triggers
        .iter()
        .all(|trigger| trigger.controller == PlayerId::P1));

    let second = engine::advance_until_decision(&mut state);
    assert!(matches!(
        &second,
        Decision::OrderTriggers {
            player: PlayerId::P1,
            pending,
        } if pending.len() == 2
    ));
    assert!(matches!(
        crate::mads_decision_construction_v1::classify_after_transition_v1(
            Some(PlayerId::P0),
            &state,
            &second
        ),
        Ok(
            crate::mads_decision_construction_v1::ClassifiedDecisionV1::ActorSwitch {
                from: PlayerId::P0,
                to: PlayerId::P1,
                kind: crate::mads_decision_construction_v1::ActorSwitchKindV1::ApnapTriggerGroup,
                ..
            }
        )
    ));
    assert_eq!(state.stack.len(), 2, "no priority window intervenes");
}

#[test]
fn h2_multi_card_discard_is_a_same_actor_microstep_scan_then_one_commit() {
    let mut state = protocol_empty_state(0x02e0_0004);
    let discards = (0..3)
        .map(|_| fixture_object(&mut state, PlayerId::P0, "Mountain", Zone::Hand))
        .collect::<Vec<_>>();
    let hand_before = state.players[PlayerId::P0.index()].hand.clone();
    state.engine.pending_discard = Some(crate::engine::PendingDiscard {
        player: PlayerId::P0,
        count: 2,
        resume: crate::engine::DiscardResume::None,
    });
    let mut surface = HarnessSurfaceV2::new();

    let first = surface.next_decision(&mut state);
    let SurfaceDecision::Decision(Decision::Discard {
        player: PlayerId::P0,
        count: 1,
        choices: first_choices,
    }) = first
    else {
        panic!("H2 must surface the first single-card discard microstep: {first:?}")
    };
    assert_eq!(first_choices, discards);
    let first_candidates = legal_action_candidates_v5(
        &PolicyDecisionV5::Surface(SurfaceDecision::Decision(Decision::Discard {
            player: PlayerId::P0,
            count: 1,
            choices: first_choices.clone(),
        })),
        &state,
    )
    .unwrap();
    assert_eq!(first_candidates.len(), 3);
    assert_ne!(
        first_candidates[0].record.semantic, first_candidates[1].record.semantic,
        "different discarded ObjectIds remain distinct V5 actions"
    );

    surface
        .apply(
            &mut state,
            SurfaceAction::Action(Action::Discard(vec![first_choices[0]])),
        )
        .unwrap();
    assert_eq!(state.players[PlayerId::P0.index()].hand, hand_before);
    assert!(state.engine.pending_discard.is_some());

    let second = surface.next_decision(&mut state);
    let SurfaceDecision::Decision(Decision::Discard {
        player: PlayerId::P0,
        count: 1,
        choices: second_choices,
    }) = second
    else {
        panic!("H2 must surface the second single-card discard microstep: {second:?}")
    };
    assert_eq!(second_choices.len(), 2);
    assert!(!second_choices.contains(&first_choices[0]));
    surface
        .apply(
            &mut state,
            SurfaceAction::Action(Action::Discard(vec![second_choices[0]])),
        )
        .unwrap();

    assert!(state.engine.pending_discard.is_none());
    assert_eq!(state.players[PlayerId::P0.index()].hand.len(), 1);
    assert_eq!(state.players[PlayerId::P0.index()].graveyard.len(), 2);
    assert_eq!(
        state.players[PlayerId::P0.index()].graveyard,
        [first_choices[0], second_choices[0]]
    );
}

#[test]
fn raw_v5_rejects_unreshaped_combat_and_multi_card_discard_protocols() {
    let mut state = protocol_empty_state(RAW_V5_REJECTION_SEED);
    state.step = Step::DeclareAttackers;
    let raw_empty_attackers =
        PolicyDecisionV5::Surface(SurfaceDecision::Decision(Decision::DeclareAttackers {
            player: PlayerId::P0,
            eligible: Vec::new(),
        }));
    let attackers_error = legal_action_candidates_v5(&raw_empty_attackers, &state).unwrap_err();
    assert!(attackers_error
        .to_string()
        .contains("aggregate combat semantic is forbidden"));

    let raw_blockers =
        PolicyDecisionV5::Surface(SurfaceDecision::Decision(Decision::DeclareBlockers {
            player: PlayerId::P1,
            attackers: Vec::new(),
            legal_blockers: Vec::new(),
        }));
    let blockers_error = legal_action_candidates_v5(&raw_blockers, &state).unwrap_err();
    assert!(blockers_error
        .to_string()
        .contains("raw DeclareBlockers is not"));

    let multi_discard = PolicyDecisionV5::Surface(SurfaceDecision::Decision(Decision::Discard {
        player: PlayerId::P0,
        count: 2,
        choices: Vec::new(),
    }));
    let discard_error = legal_action_candidates_v5(&multi_discard, &state).unwrap_err();
    assert!(discard_error
        .to_string()
        .contains("expected count=1 after H2 reshape"));
}
#[test]
fn typed_pending_cast_target_adapter_preserves_domain_and_finalizes_only_after_pick() {
    use crate::mads_decision_construction_v1::{
        classify_after_transition_v1, ClassifiedDecisionV1, ConstructionStageV1,
    };

    let mut state = burn_start_with_mountain_and_bolt();
    let decision = first_p0_main1(&mut state);
    let mut replay_state = burn_start_with_mountain_and_bolt();
    let replay_decision = first_p0_main1(&mut replay_state);
    assert_eq!(
        state, replay_state,
        "same fixture seed reproduces the decision state"
    );
    assert_eq!(
        decision, replay_decision,
        "same fixture seed reproduces the full decision domain"
    );
    let Decision::CastSpellOrPass { land_drops, .. } = decision else {
        unreachable!()
    };
    let mountain = land_drops
        .into_iter()
        .find(|id| state.objects.get(*id).card_def == card_id_by_name("Mountain").unwrap())
        .expect("opening hand contains a legal Mountain");
    engine::step(&mut state, Action::PlayLand(mountain)).unwrap();
    let Decision::CastSpellOrPass { mana_abilities, .. } =
        engine::advance_until_decision(&mut state)
    else {
        panic!("land play returns to priority")
    };
    assert!(mana_abilities.contains(&mountain));
    engine::step(&mut state, Action::ActivateManaAbility(mountain)).unwrap();
    let cast_decision = engine::advance_until_decision(&mut state);
    let Decision::CastSpellOrPass {
        player,
        castable_spells,
        ..
    } = cast_decision.clone()
    else {
        panic!("mana ability returns to priority")
    };
    assert_eq!(player, PlayerId::P0);
    let bolt = castable_spells
        .into_iter()
        .find(|id| state.objects.get(*id).card_def == card_id_by_name("Lightning Bolt").unwrap())
        .expect("opened red mana pays for Lightning Bolt");
    let ClassifiedDecisionV1::GameDecision {
        construction_starting_actions,
        ..
    } = classify_after_transition_v1(Some(PlayerId::P0), &state, &cast_decision)
        .expect("fresh priority decision is a game-decision node")
    else {
        panic!("same-actor priority frame is not a construction node")
    };
    assert!(construction_starting_actions.contains(&Action::CastSpell(bolt)));

    // CastSpell begins a pending physical action; it is not a completed action
    // result. The typed protocol exists until the target is selected and the
    // authoritative walk finalizes the cast.
    engine::step(&mut state, Action::CastSpell(bolt)).unwrap();
    let target_decision = engine::advance_until_decision(&mut state);
    let ClassifiedDecisionV1::Construction(context) =
        classify_after_transition_v1(Some(PlayerId::P0), &state, &target_decision)
            .expect("authoritative target continuation classifies")
    else {
        panic!("pending cast target must remain construction")
    };
    assert_eq!(
        context.adapter_version,
        crate::mads_decision_construction_v1::DECISION_CONSTRUCTION_ADAPTER_VERSION_V1
    );
    assert_eq!(context.initiator, PlayerId::P0);
    assert_eq!(context.stage, ConstructionStageV1::PendingCastTargetPick);
    assert_eq!(context.pending_cast.spell, bolt);
    assert!(context.ordered_target_prefix.is_empty());
    assert_eq!(context.remaining_cardinality, 1);
    let Decision::ChooseTargets {
        legal_targets,
        player,
        spell,
        remaining,
        ..
    } = &context.decision
    else {
        panic!("context retains the exact target decision")
    };
    assert_eq!((*player, *spell, *remaining), (PlayerId::P0, bolt, 1));
    let expected = legal_targets
        .iter()
        .cloned()
        .map(Action::ChooseTarget)
        .collect::<Vec<_>>();
    assert_eq!(context.legal_candidates, expected);

    // Cross-check the complete order against the existing V5 projection and
    // independently apply every listed Engine action on an isolated clone.
    let projected = legal_action_candidates_v5(
        &PolicyDecisionV5::Surface(SurfaceDecision::Decision(target_decision.clone())),
        &state,
    )
    .expect("V5 maps this raw target domain");
    let projected_actions = projected
        .into_iter()
        .map(|candidate| match candidate.policy_action {
            crate::policy_surface_v5::PolicyActionV5::Surface(SurfaceAction::Action(action)) => {
                action
            }
            _ => panic!("raw target candidate unexpectedly became a surface microstep"),
        })
        .collect::<Vec<_>>();
    assert_eq!(context.legal_candidates, projected_actions);
    for candidate in &context.legal_candidates {
        let mut branch = state.clone();
        engine::step(&mut branch, candidate.clone()).expect("every listed target is legal");
        let selected_targets = branch
            .engine
            .pending_cast
            .as_ref()
            .unwrap()
            .targets_chosen
            .clone();
        assert_eq!(selected_targets.len(), 1);
        let next = engine::advance_until_decision(&mut branch);
        assert!(branch.objects.get(bolt).v4.finalized_cast_binding.is_some());
        assert!(matches!(
            next,
            Decision::CastSpellOrPass {
                player: PlayerId::P0,
                ..
            }
        ));
        assert_eq!(branch.stack.last().unwrap().targets, selected_targets);
    }

    let mut stale = target_decision.clone();
    let Decision::ChooseTargets { player, .. } = &mut stale else {
        unreachable!()
    };
    *player = PlayerId::P1;
    assert_eq!(
        classify_after_transition_v1(Some(PlayerId::P0), &state, &stale),
        Err(crate::mads_decision_construction_v1::DecisionClassificationErrorV1::InvalidOrStaleDecisionFrame)
    );

    engine::step(
        &mut state,
        Action::ChooseTarget(Target::Player(PlayerId::P1)),
    )
    .unwrap();
    let finalized_decision = engine::advance_until_decision(&mut state);
    assert!(state.engine.pending_cast.is_none());
    assert_eq!(
        state.stack.last().unwrap().targets,
        [Target::Player(PlayerId::P1)]
    );
    assert!(matches!(
        classify_after_transition_v1(Some(PlayerId::P0), &state, &finalized_decision),
        Ok(ClassifiedDecisionV1::GameDecision {
            actor: PlayerId::P0,
            ..
        })
    ));
}
#[test]
fn dynamic_v2_tracks_complete_lightning_bolt_root_responses_without_certifying_cast_start() {
    use crate::dynamic_engine_search_v1::{
        DynamicEngineSearchV2, DynamicSearchStatusV1, PhysicalActionFinalizationV2,
    };

    let mut root = protocol_empty_state(0x03b2_0001);
    root.turn = 1;
    root.step = Step::Main1;
    root.active_player = PlayerId::P0;
    root.priority_player = PlayerId::P0;
    root.players[PlayerId::P0.index()].mana_pool[3] = 1;
    let bolt = fixture_object(&mut root, PlayerId::P0, "Lightning Bolt", Zone::Hand);
    let root_before = root.clone();
    let root_decision = engine::advance_until_decision(&mut root);
    assert_eq!(
        root, root_before,
        "initial Engine decision must be quiescent"
    );
    let Decision::CastSpellOrPass {
        player,
        castable_spells,
        mana_abilities,
        land_drops,
        activatable_abilities,
        plot_actions,
    } = &root_decision
    else {
        panic!("fixture must begin at an authoritative priority decision")
    };
    assert_eq!(*player, PlayerId::P0);
    assert_eq!(castable_spells, &[bolt]);
    assert!(mana_abilities.is_empty());
    assert!(land_drops.is_empty());
    assert!(activatable_abilities.is_empty());
    assert!(plot_actions.is_empty());
    let independently_enumerated_root = vec![Action::CastSpell(bolt), Action::Pass];
    let v5_root = legal_action_candidates_v5(
        &PolicyDecisionV5::Surface(SurfaceDecision::Decision(root_decision.clone())),
        &root,
    )
    .unwrap()
    .into_iter()
    .map(|candidate| match candidate.policy_action {
        crate::policy_surface_v5::PolicyActionV5::Surface(SurfaceAction::Action(action)) => action,
        _ => panic!("raw priority action unexpectedly became a policy microstep"),
    })
    .collect::<Vec<_>>();
    assert_eq!(independently_enumerated_root, v5_root);

    let mut search = DynamicEngineSearchV2::new(&root, root_decision.clone()).unwrap();
    let budget_zero = search.run_v2(0);
    assert_eq!(
        budget_zero.status,
        DynamicSearchStatusV1::UnresolvedWithinBudget
    );
    assert_eq!(
        budget_zero.root_bounds,
        crate::mads_v1::BoundIntervalV1::UNKNOWN
    );
    assert_eq!(budget_zero.exact_root_value, None);
    assert!(budget_zero.certified_optimal_actions.is_empty());
    assert!(budget_zero.complete_root_actions.is_empty());
    assert!(!budget_zero.root_action_domain_complete);
    assert_eq!(
        budget_zero
            .root_engine_actions
            .iter()
            .map(|entry| entry.engine_action.clone())
            .collect::<Vec<_>>(),
        independently_enumerated_root
    );

    // The versioned dynamic scheduler spends one expansion to announce Bolt.
    // It must expose construction candidates without certifying Action::CastSpell.
    let after_announcement = search.run_v2(1);
    assert_eq!(after_announcement.metrics.expanded_actions, 1);
    assert!(after_announcement.certified_optimal_actions.is_empty());
    assert!(!after_announcement
        .complete_root_actions
        .iter()
        .any(|action| { action.identity.ordered_engine_responses == [Action::CastSpell(bolt)] }));
    let target_candidates = after_announcement
        .incomplete_root_frontier
        .iter()
        .filter(|frontier| {
            frontier.ordered_engine_responses.first() == Some(&Action::CastSpell(bolt))
        })
        .filter_map(|frontier| frontier.ordered_engine_responses.last().cloned())
        .collect::<Vec<_>>();
    let mut post_cast_state = root.clone();
    engine::step(&mut post_cast_state, Action::CastSpell(bolt)).unwrap();
    let target_decision = engine::advance_until_decision(&mut post_cast_state);
    let Decision::ChooseTargets {
        player: target_actor,
        spell,
        remaining,
        legal_targets,
        can_finish,
    } = &target_decision
    else {
        panic!("Lightning Bolt must expose its real target stage")
    };
    assert_eq!((*target_actor, *spell, *remaining), (PlayerId::P0, bolt, 1));
    assert!(!can_finish);
    let expected_targets = legal_targets
        .iter()
        .copied()
        .map(Action::ChooseTarget)
        .collect::<Vec<_>>();
    assert_eq!(expected_targets, target_candidates);
    let v5_targets = legal_action_candidates_v5(
        &PolicyDecisionV5::Surface(SurfaceDecision::Decision(target_decision.clone())),
        &post_cast_state,
    )
    .unwrap()
    .into_iter()
    .map(|candidate| match candidate.policy_action {
        crate::policy_surface_v5::PolicyActionV5::Surface(SurfaceAction::Action(action)) => action,
        _ => panic!("raw target candidate unexpectedly became a policy microstep"),
    })
    .collect::<Vec<_>>();
    assert_eq!(target_candidates, v5_targets);
    let after_second_root_response = search.run_v2(1);
    assert_eq!(after_second_root_response.metrics.expanded_actions, 2);
    assert!(after_second_root_response
        .certified_optimal_actions
        .is_empty());
    assert!(after_second_root_response
        .complete_root_actions
        .iter()
        .any(|action| action.identity.ordered_engine_responses == [Action::Pass]));
    assert_eq!(
        after_second_root_response
            .incomplete_root_frontier
            .iter()
            .filter(|frontier| {
                frontier.ordered_engine_responses.first() == Some(&Action::CastSpell(bolt))
            })
            .count(),
        target_candidates.len(),
        "the public scheduler retains each target as an open construction branch"
    );
    for candidate in &target_candidates {
        let mut branch = root.clone();
        engine::step(&mut branch, Action::CastSpell(bolt)).unwrap();
        let observed_target_decision = engine::advance_until_decision(&mut branch);
        assert_eq!(observed_target_decision, target_decision);
        engine::step(&mut branch, candidate.clone()).unwrap();
        let finalized_decision = engine::advance_until_decision(&mut branch);
        assert!(branch.objects.get(bolt).v4.finalized_cast_binding.is_some());
        assert!(matches!(
            finalized_decision,
            Decision::CastSpellOrPass {
                player: PlayerId::P0,
                ..
            }
        ));
        let target = match candidate {
            Action::ChooseTarget(target) => *target,
            _ => unreachable!(),
        };
        assert_eq!(branch.stack.last().unwrap().targets, [target]);
        assert!(branch.objects.get(bolt).v4.finalized_cast_binding.is_some());
    }
    let result = search.run_v2(8);
    assert_eq!(
        result.status,
        DynamicSearchStatusV1::UnresolvedWithinBudget,
        "the public scheduler must enumerate the admitted domain without inventing a value"
    );
    assert_eq!(result.root_bounds, crate::mads_v1::BoundIntervalV1::UNKNOWN);
    assert_eq!(result.exact_root_value, None);
    assert!(result.root_action_domain_complete);
    assert!(result.incomplete_root_frontier.is_empty());
    assert_eq!(
        result.complete_root_actions.len(),
        target_candidates.len() + 1
    );
    assert_eq!(result.metrics.authoritative_transitions, 4);
    assert_eq!(result.metrics.state_clones, 5);

    assert!(result.complete_root_actions.iter().any(|physical| {
        physical.identity.ordered_engine_responses == [Action::Pass]
            && matches!(
                &physical.identity.finalization_boundary,
                PhysicalActionFinalizationV2::EngineDecisionReached(Decision::CastSpellOrPass {
                    player: PlayerId::P1,
                    ..
                })
            )
    }));
    let mut identities = std::collections::BTreeSet::new();
    for physical in result.complete_root_actions.iter().filter(|physical| {
        matches!(
            physical.identity.ordered_engine_responses.first(),
            Some(Action::CastSpell(source)) if *source == bolt
        )
    }) {
        assert!(identities.insert(physical.stable_semantic_identity.clone()));
        assert_eq!(
            physical.identity.ordered_engine_responses.len(),
            2,
            "the physical answer includes CastSpell and its target response"
        );
        assert_eq!(
            physical.identity.ordered_engine_responses[0],
            Action::CastSpell(bolt)
        );
        let Action::ChooseTarget(target) = physical.identity.ordered_engine_responses[1] else {
            panic!("complete Bolt identity retains its target answer")
        };
        let PhysicalActionFinalizationV2::CastFinalized {
            source,
            stack_item,
            finalized_binding,
        } = &physical.identity.finalization_boundary
        else {
            panic!("complete Bolt response must terminate at authoritative Cast finalization")
        };
        assert_eq!(*source, bolt);
        assert_eq!(stack_item.targets, [target]);
        assert_eq!(finalized_binding.x_value, 0);
        assert!(finalized_binding.chosen_creature_cost.is_none());
        assert_eq!(physical.bounds, crate::mads_v1::BoundIntervalV1::UNKNOWN);
    }
    assert_eq!(
        root, root_before,
        "search owns state clones and leaves caller root unchanged"
    );
}

#[test]
fn mads03b5_public_v3_binds_authoritative_bolt_target_and_pass_responses() {
    use crate::dynamic_engine_search_v1::{DynamicEngineSearchV2, DynamicSearchStatusV1};
    use crate::mads03b5_engine_binding_v3::{DynamicEngineSearchV3, EngineBindingErrorV3};
    use crate::mads_virtual_physical_root_v3::{
        PhysicalAlternativeStateV3, RootCriticalFrontierStatusV3,
    };

    let mut root = protocol_empty_state(0x03b5_0001);
    root.turn = 1;
    root.step = Step::Main1;
    root.active_player = PlayerId::P0;
    root.priority_player = PlayerId::P0;
    root.players[PlayerId::P0.index()].mana_pool[3] = 1;
    let bolt = fixture_object(&mut root, PlayerId::P0, "Lightning Bolt", Zone::Hand);
    let root_before = root.clone();
    let root_decision = engine::advance_until_decision(&mut root);

    let mut stale_decision = root_decision.clone();
    let Decision::CastSpellOrPass {
        castable_spells, ..
    } = &mut stale_decision
    else {
        unreachable!()
    };
    castable_spells.clear();
    assert_eq!(
        DynamicEngineSearchV3::new(&root, stale_decision).err(),
        Some(EngineBindingErrorV3::InvalidOrStaleRootFrame),
        "a foreign root decision cannot be admitted"
    );

    let mut v3 = DynamicEngineSearchV3::new(&root, root_decision.clone()).unwrap();
    let budget0 = v3.run_v3(0).unwrap();
    assert_eq!(
        budget0.root_bounds,
        crate::mads_v1::BoundIntervalV1::UNKNOWN
    );
    assert!(budget0.certified_optimal_actions.is_empty());
    assert_eq!(budget0.metrics.authoritative_transitions, 0);
    assert_eq!(budget0.open_construction_envelopes.len(), 1);
    assert!(budget0
        .physical_alternatives
        .iter()
        .all(|a| a.state == PhysicalAlternativeStateV3::KnownCompleteAlternativeNotExpanded));
    let cast_id =
        crate::mads03b5_engine_binding_v3::engine_action_identity_v3(&Action::CastSpell(bolt))
            .unwrap();
    let pass_id =
        crate::mads03b5_engine_binding_v3::engine_action_identity_v3(&Action::Pass).unwrap();
    let budget0_slot_ids = budget0
        .frontier
        .tasks
        .iter()
        .map(|task| task.slot.slot_id.clone())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        budget0_slot_ids,
        [cast_id, pass_id.clone()].into_iter().collect()
    );

    let budget1 = v3.run_v3(1).unwrap();
    assert_eq!(budget1.metrics.authoritative_transitions, 1);
    assert_eq!(
        budget1.open_construction_envelopes.len(),
        0,
        "the target frame was fully admitted atomically"
    );
    assert_eq!(budget1.physical_alternatives.len(), 3);
    assert!(budget1
        .physical_alternatives
        .iter()
        .all(|a| a.state != PhysicalAlternativeStateV3::Completed));
    assert!(budget1.certified_optimal_actions.is_empty());
    assert!(budget1
        .physical_alternatives
        .iter()
        .all(|alternative| alternative.next_expansion_slot.is_some()));
    let target_alternative_ids = budget1
        .physical_alternatives
        .iter()
        .filter(|alternative| {
            alternative.ordered_engine_responses.first() == Some(&Action::CastSpell(bolt))
        })
        .map(|alternative| alternative.stable_identity.clone())
        .collect::<std::collections::BTreeSet<_>>();
    let frontier_target_support = budget1
        .frontier
        .tasks
        .iter()
        .flat_map(|task| task.root_action_support_ids.iter().cloned())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        frontier_target_support, target_alternative_ids,
        "real target ExpansionSlots carry separate virtual root support identities"
    );
    assert!(budget1
        .physical_alternatives
        .iter()
        .any(
            |alternative| alternative.ordered_engine_responses == [Action::Pass]
                && alternative
                    .next_expansion_slot
                    .as_ref()
                    .is_some_and(|slot| slot.slot_id == pass_id)
        ));

    let budget2 = v3.run_v3(1).unwrap();
    assert_eq!(budget2.metrics.authoritative_transitions, 2);
    assert_eq!(
        budget2.metrics.complete_physical_alternatives, 1,
        "only Pass is finalized after two work units"
    );
    assert!(budget2.certified_optimal_actions.is_empty());

    let result = v3.run_v3(8).unwrap();
    assert!(result.root_domain_complete);
    assert_eq!(result.root_bounds, crate::mads_v1::BoundIntervalV1::UNKNOWN);
    assert_eq!(result.exact_root_value, None);
    assert!(result.certified_optimal_actions.is_empty());
    assert_eq!(result.metrics.authoritative_transitions, 6);
    assert_eq!(result.metrics.state_clones, 13);
    assert_eq!(result.metrics.candidate_count, 11);
    assert_eq!(result.metrics.complete_physical_alternatives, 3);
    assert_eq!(result.metrics.successor_bindings, 3);
    assert_eq!(result.metrics.frontier_rebuilds, 4);
    assert_eq!(result.metrics.unknown_root_actions, 3);
    assert_eq!(
        result.frontier.status,
        RootCriticalFrontierStatusV3::BlockedOnUnevaluatedSuccessor
    );
    assert_eq!(result.physical_alternatives.len(), 3);
    assert_eq!(
        result.raw_root_actions,
        [Action::CastSpell(bolt), Action::Pass]
    );
    assert!(result
        .physical_alternatives
        .iter()
        .all(|a| a.bounds == crate::mads_v1::BoundIntervalV1::UNKNOWN));

    let mut identities = std::collections::BTreeSet::new();
    let mut semantic_paths = result.physical_alternatives.iter().map(|a| {
        assert!(identities.insert(a.stable_identity.clone()));
        assert_eq!(a.state, PhysicalAlternativeStateV3::Completed);
        let binding = a.successor_binding.as_ref().expect("completed response is bound to a real successor");
        match &binding.provenance {
            crate::mads_virtual_physical_root_v3::SuccessorProvenanceV3::AuthoritativeTransition { source_frame_identity, transition_identity, transition_response_identity, physical_response_identity, successor_frame_identity } => {
                assert_eq!(source_frame_identity, &a.admission_evidence_identity);
                assert!(!transition_identity.is_empty());
                assert_eq!(successor_frame_identity, &binding.successor_node_id);
                assert_eq!(physical_response_identity, &a.stable_identity);
                assert_eq!(
                    Some(transition_response_identity),
                    a.ordered_engine_responses
                        .last()
                        .and_then(crate::mads03b5_engine_binding_v3::engine_action_identity_v3)
                        .as_ref()
                );
            }
            _ => panic!("engine adapter must emit authoritative transition provenance"),
        }
        assert_eq!(binding.ordered_engine_responses, a.ordered_engine_responses.iter().map(|action| crate::mads03b5_engine_binding_v3::engine_action_identity_v3(action).unwrap()).collect::<Vec<_>>());
        a.ordered_engine_responses
            .iter()
            .map(|action| format!("{action:?}"))
            .collect::<Vec<_>>()
            .join("+")
    }).collect::<Vec<_>>();
    semantic_paths.sort();
    let mut expected_paths = vec![
        vec![
            Action::CastSpell(bolt),
            Action::ChooseTarget(Target::Player(PlayerId::P0)),
        ],
        vec![
            Action::CastSpell(bolt),
            Action::ChooseTarget(Target::Player(PlayerId::P1)),
        ],
        vec![Action::Pass],
    ]
    .into_iter()
    .map(|path| {
        path.iter()
            .map(|action| format!("{action:?}"))
            .collect::<Vec<_>>()
            .join("+")
    })
    .collect::<Vec<_>>();
    expected_paths.sort();
    assert_eq!(semantic_paths, expected_paths);
    for alternative in &result.physical_alternatives {
        if alternative.ordered_engine_responses.first() == Some(&Action::CastSpell(bolt)) {
            let binding = alternative.successor_binding.as_ref().unwrap();
            let target = match alternative.ordered_engine_responses[1] {
                Action::ChooseTarget(target) => target,
                _ => unreachable!(),
            };
            assert!(binding.finalization_boundary.contains(&match target {
                Target::Player(p) => format!("player:{}", p.index()),
                Target::Object(o) => format!("object:{}", o.0),
            }));
            assert_eq!(binding.next_actor, Some(PlayerId::P0));
        } else {
            assert_eq!(alternative.ordered_engine_responses, [Action::Pass]);
            assert_eq!(
                alternative.successor_binding.as_ref().unwrap().next_actor,
                Some(PlayerId::P1)
            );
        }
    }

    let mut v2 = DynamicEngineSearchV2::new(&root, root_decision.clone()).unwrap();
    let v2_result = v2.run_v2(8);
    assert_eq!(
        v2_result.status,
        DynamicSearchStatusV1::UnresolvedWithinBudget
    );
    let normalize = |paths: Vec<Vec<Action>>| {
        let mut keys = paths
            .into_iter()
            .map(|path| {
                path.iter()
                    .map(|a| format!("{a:?}"))
                    .collect::<Vec<_>>()
                    .join("+")
            })
            .collect::<Vec<_>>();
        keys.sort();
        keys
    };
    assert_eq!(
        normalize(
            v2_result
                .complete_root_actions
                .iter()
                .map(|a| a.identity.ordered_engine_responses.clone())
                .collect()
        ),
        semantic_paths,
        "V2 and V3 expose the same typed complete physical response domain"
    );
    let mut repeated = DynamicEngineSearchV3::new(&root, root_decision).unwrap();
    let repeated_result = repeated.run_v3(8).unwrap();
    let normalize_v3 = |items: &[crate::mads03b5_engine_binding_v3::PhysicalResponseV3]| {
        let mut keys = items
            .iter()
            .map(|item| {
                item.ordered_engine_responses
                    .iter()
                    .map(|action| format!("{action:?}"))
                    .collect::<Vec<_>>()
                    .join("+")
            })
            .collect::<Vec<_>>();
        keys.sort();
        keys
    };
    assert_eq!(
        normalize_v3(&repeated_result.physical_alternatives),
        normalize_v3(&result.physical_alternatives)
    );
    assert_eq!(
        repeated_result.metrics.authoritative_transitions,
        result.metrics.authoritative_transitions
    );
    assert_eq!(
        repeated_result.metrics.state_clones,
        result.metrics.state_clones
    );
    assert_ne!(
        repeated_result.physical_alternatives[0].stable_identity,
        result.physical_alternatives[0].stable_identity,
        "run-local provenance identities are deliberately not global reusable state keys"
    );
    assert_eq!(
        root, root_before,
        "all authoritative transitions run on private clones"
    );
    let resumed_after_completion = v3.run_v3(8).unwrap();
    assert_eq!(
        resumed_after_completion.metrics.authoritative_transitions, 6,
        "an already completed response is not transitioned twice on a resumed run"
    );
    assert_eq!(resumed_after_completion.metrics.state_clones, 13);
    assert_eq!(resumed_after_completion.metrics.frontier_rebuilds, 5);
}

#[test]
fn mads03b5_rejects_the_entire_unsupported_fireblast_root_domain() {
    use crate::mads03b5_engine_binding_v3::{DynamicEngineSearchV3, EngineBindingErrorV3};

    let mut root = protocol_empty_state(0x03b5_0002);
    root.step = Step::Main1;
    root.active_player = PlayerId::P0;
    root.priority_player = PlayerId::P0;
    for _ in 0..6 {
        fixture_object(&mut root, PlayerId::P0, "Mountain", Zone::Battlefield);
    }
    fixture_object(&mut root, PlayerId::P0, "Fireblast", Zone::Hand);
    let before = root.clone();
    let decision = engine::advance_until_decision(&mut root);
    assert_eq!(
        DynamicEngineSearchV3::new(&root, decision).err(),
        Some(EngineBindingErrorV3::UnsupportedRootDomain),
        "V3 rejects the whole domain when its cast protocol is outside admission"
    );
    assert_eq!(root, before);
}
#[test]
fn dynamic_v2_fails_closed_for_out_of_scope_fireblast_construction() {
    use crate::dynamic_engine_search_v1::{DynamicEngineSearchV2, DynamicSearchErrorV1};

    let mut root = protocol_empty_state(0x03b2_0002);
    root.step = Step::Main1;
    root.active_player = PlayerId::P0;
    root.priority_player = PlayerId::P0;
    for _ in 0..6 {
        fixture_object(&mut root, PlayerId::P0, "Mountain", Zone::Battlefield);
    }
    let fireblast = fixture_object(&mut root, PlayerId::P0, "Fireblast", Zone::Hand);
    let root_before = root.clone();
    let root_decision = engine::advance_until_decision(&mut root);
    assert!(matches!(
        &root_decision,
        Decision::CastSpellOrPass { castable_spells, .. }
            if castable_spells.contains(&fireblast)
    ));

    assert_eq!(
        DynamicEngineSearchV2::new(&root, root_decision).err(),
        Some(DynamicSearchErrorV1::UnsupportedDecision),
        "V2 must reject the entire root domain if it includes out-of-scope construction"
    );
    assert_eq!(
        root, root_before,
        "unsupported construction must preserve caller state"
    );
}
