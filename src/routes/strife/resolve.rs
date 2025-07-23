use std::collections::HashMap;
use axum::{Extension, Form};
use axum::response::{Html, IntoResponse};
use itertools::Itertools;
use sqlx::MySqlPool;
use tokio::sync::broadcast::Sender;
use crate::broadcast::BroadcastMessage;
use crate::routes::character::{Character, Strifer};
use crate::Result;
use crate::status::{generate_glitch_string, StrifeBonus, StrifeBonusType, StrifeStatus, StrifeStatusType};
use super::{Action, ActionSubmission};

pub async fn strife_resolve(
    mut character: Character,
    Extension(db): Extension<MySqlPool>,
    Extension(sse): Extension<Sender<BroadcastMessage>>,
    Form(form): Form<ActionSubmission>
) -> Result<impl IntoResponse> {

    if character.strife.strife_id.is_none() {
        return Ok(Html("You are not currently engaged in strife!<br />"));
    }

    if !character.strife.is_leader {
        return Ok(Html("You are not the leader of your current strife. Only the leader can advance the round.<br />"));
    }

    // Will build up this array with the results
    let mut strifers = Strifer::from_strife_id(character.strife.strife_id.unwrap(), &db).await?;
    let mut updated_status: HashMap<i64, Vec<StrifeStatus>> = HashMap::new();
    let mut player_side = 0i8;
    let mut unstuck = false;
    let mut output = String::from("A whole mess of strifing takes place.<br />");
    
    let mut append = |mut s: String| {
        s.push_str("<br />");
        output.push_str(s.as_str());
    };

    let mut push_status = |id: &i64, s: &[StrifeStatus]| {
        updated_status.entry(*id)
            .or_insert(vec![])
            .extend_from_slice(s);
    };

    // First iteration to initialize structures and handle effects
    for i in 0..strifers.len() {
        let mut strifer = strifers[i].clone();
        strifer.damage_dealt = 0;
        strifer.damage_taken = 0;
        strifer.attacks = 1;
        strifer.time_attack = false;
        strifer.luck += strifer.brief_luck;
        strifer.bonuses.extend(strifer.equipment_bonuses.clone());
        push_status(&strifer.id, strifer.statuses.as_slice());
        // updated_status.insert(strifer.id, strifer.statuses.clone());

        if strifer.owner_id.is_some_and(|o| o == character.id) {
            player_side = strifer.side;
        }

        strifer.active = if strifer.last_active.is_empty() { None } else { Some(strifer.last_active.clone()) };
        strifer.passive = if strifer.last_passive.is_empty() { None } else { Some(strifer.last_passive.clone()) };

        if form.actions.contains_key(&strifer.id) && strifer.is_controllable && strifer.owner_id.is_some_and(|o| o == character.id) {
            let Action { active, passive } = form.actions.get(&strifer.id).unwrap();

            strifer.active = Some(active.clone());
            strifer.passive = Some(passive.clone());

            strifer.last_active = active.clone();
            strifer.last_passive = passive.clone();
        }

        for i in 0..strifer.statuses.len() {
            let status = strifer.statuses[i].clone();
            match &status.status_type {
                StrifeStatusType::Timestop => {
                    strifer.statuses.push(StrifeStatus::new(StrifeStatusType::CantAttack));
                }
                StrifeStatusType::Hopeless => {
                    if rand::random_range(1..=100) <= 50 {
                        strifer.statuses.push(StrifeStatus::new(StrifeStatusType::CantAttack));
                        append(format!("{} feels too despondent to attack this round. How sad.", strifer.name))
                    }
                }
                StrifeStatusType::Glitched => {
                    if rand::random_range(1..=100) <= 20 {
                        strifer.statuses.push(StrifeStatus::new(StrifeStatusType::CantAttack));
                        push_status(&strifer.id, &[StrifeStatus::with_duration(StrifeStatusType::Glitched, 6)]);
                        append(generate_glitch_string());
                    }
                }
                StrifeStatusType::Disoriented => {
                    strifer.teamwork = (strifer.teamwork as f64 / 2.0).floor() as i64;
                }
                StrifeStatusType::Distracted => {
                    strifer.attacks -= 1;
                }
                StrifeStatusType::Fury => {
                    strifer.attacks += 1;
                }
                StrifeStatusType::Knockdown => {
                    strifer.statuses.push(StrifeStatus::new(StrifeStatusType::CantAttack));
                }
                StrifeStatusType::Charmed(side) => {
                    strifer.side = *side as i8;
                }
                StrifeStatusType::Paralyzed(severity) => {
                    if rand::random_range(1..=100) <= *severity {
                        strifer.statuses.push(StrifeStatus::new(StrifeStatusType::CantAttack));
                        append(format!("{} is fully paralyzed!", strifer.name));
                    }
                }
                StrifeStatusType::Pinata => {
                    strifer.statuses.extend_from_slice(&[
                        StrifeStatus::new(StrifeStatusType::CantAttack),
                        StrifeStatus::new(StrifeStatusType::CantDefend),
                    ]);
                }
                StrifeStatusType::HasEffect(effect) => {
                    strifer.effects.push(effect
                        .replace("@", ":") // Effect string uses @ as a separator, replace with : to match other separators
                        .replace("&", "@")); // Effect string can contain a status effect string to inflict on an enemy, which wants to use @
                }
                StrifeStatusType::HasResist(resistance) => {
                    let [name, value] = resistance
                        .replace("@", ":")
                        .split(":").map(|s| s.to_string()).collect_array::<2>().unwrap();

                    *strifer.resistances.entry(name)
                        .or_insert(0) += value.parse::<i64>()?;
                }
                StrifeStatusType::Isolated => {
                    strifer.teamwork = 0;
                }
                StrifeStatusType::Unstuck => {
                    if rand::random_range(1..=100) <= 40 {
                        append(format!("{} is fake as shit this round.", strifer.name));
                        strifers.retain_mut(|s| s.id != strifer.id); // TODO: It's generally bad practice to remove an element from a list you're actively iterating
                        unstuck = true;
                    }
                }
                StrifeStatusType::LuckBoost(boost) => {
                    strifer.luck += boost;
                }
                _ => {}
            }
        }

        for ability in &strifer.abilities {
            match ability.as_str() {
                "1" if strifer.active.as_ref().is_some_and(|a| a == "AGGRESS") => { // Passive Aggress (ID 1)
                    strifer.teamwork += 10;
                    append(format!("{}'s Level 7 Seer ability Passive Aggress activates! They cooperate more effectively with allies this round.", strifer.name));
                },
                "3" if strifer.active.as_ref().is_some_and(|a| a == "ASSAULT") => { // Chaotic Assault (ID 3)
                    let min_roll = (-50.0 + (strifer.luck as f64 / 2.0)).floor().min(150.0) as i64; // 100 luck means no negative roll
                    let modifier = strifer.power as f64 * (rand::random_range(min_roll..=150) as f64 / 100.0);
                    strifer.bonuses.push(StrifeBonus::new(StrifeBonusType::Offense, modifier.floor() as i64));
                    append(format!("{}'s Lv. 397 Minstreltech Chaotic Assault activates! Their attack power goes haywire.", strifer.name));
                },
                "7" => { // Aspect Fighter (ID 7)
                    strifer.effects.push(format!("AFFINITY:{}:10", strifer.aspect.clone().unwrap_or_else(String::new)));
                    append(format!("{}'s Lv. 83 Knightskill Aspect Fighter activates! They gain affinity on their attacks.", strifer.name));
                },
                "11" => { // Blockhead (ID 11)
                    let amount = (strifer.max_power as f64 / 33.3).ceil() as i64; // 3% of max power recovered every round if it's drained.
                    push_status(&strifer.id, &[StrifeStatus::new(StrifeStatusType::Recovery(amount))]);
                    append(format!("{}'s Level 29 Mindcraft Blockhead activates! A combination of combat focus and stubbornness removes some of the power drain affecting them, if any.", strifer.name));
                },
                "17" => { // Blood Bonds (ID 17)
                    strifer.teamwork += 10;
                    append(format!("{}'s Lv. 78 Bloodbending Blood Bonds activate! GO GO TEAMWORK!", strifer.name));
                },
                "19" => { // Light's Favour (ID 19)
                    strifer.luck += 5 + (strifer.echeladder as f64 / 60.0).floor() as i64; // 15% at max rung, 5% minimum.
                    // No message. Luck is mysterious. oooooOOOOOooooo!
                },
                "-1" => { // Typheus's fire ability (ID -1)
                    // SHOULD be designed to encourage "freedom" in some sense...not sure how.
                    const LIKELIHOOD: i64 = 20; // Is used whenever possible, but conditions don't always allow it.
                    const COST: i64 = 9;
                    let roll = rand::random_range(1..=100);
                    if roll <= LIKELIHOOD && strifer.energy >= COST && !strifer.used_bonus_action {
                        strifer.used_bonus_action = true;
                        strifer.energy -= COST;
                        append(format!("{} breathes on one of the small flames illuminating the battle. The entire chamber is engulfed in fire!", strifer.name));

                        const DAMAGE: i64 = 1000;

                        strifers.iter_mut()
                            .filter(|s| s.side != strifer.side)
                            .for_each(|s| {
                                s.health -= DAMAGE;
                                s.damage_taken += DAMAGE;
                            });
                    }
                },
                "-2" => { // Sophia's self-buff (ID -2)
                    // Designed to encourage a speedy takedown
                    append(format!("{} draws on some inner strength, becoming more powerful.", strifer.name));
                    strifer.bonuses.push(StrifeBonus::permanent(StrifeBonusType::Power, 1000));
                },
                "-3" => { // Hemera's flat damage ability (ID -3)
                    // Designed to put a bit of pressure on the Life player
                    const LIKELIHOOD: i64 = 25; // Is used whenever possible, but conditions don't always allow it.
                    const COST: i64 = 13;

                    let roll = rand::random_range(1..=100);
                    if roll <= LIKELIHOOD && strifer.energy >= COST && !strifer.used_bonus_action {
                        strifer.used_bonus_action = true;
                        strifer.energy -= COST;
                        append(format!("{} reaches up above your head and gives your Health Vial a good flick.", strifer.name));
                        strifers.iter_mut()
                            .filter(|s| s.side != strifer.side)
                            .for_each(|s| {
                                let damage = (s.max_health as f64 / 6.0).floor() as i64;
                                s.health -= damage;
                                s.damage_taken += damage;
                            });
                    }
                },
                "-4" => { // Hemera's self-heal (ID -4)
                    // Designed to require a longer battle plan
                    let likelihood = ((1.0 - (strifer.health as f64 / strifer.max_health as f64)) * 100.0).floor() as i64; // More likely to be used at low HP
                    let roll = rand::random_range(1..=100);

                    const COST: i64 = 20;

                    if roll <= likelihood && strifer.energy >= COST { // Ability is effortless, no subaction required.
                        strifer.energy -= COST;
                        append(format!("{} closes her eyes for a moment, her wounds beginning to close as well.", strifer.name));
                        strifer.health += (strifer.max_health as f64 / 10.0).floor() as i64;
                    }
                },
                "-6" => { // Abraxas's Hope aura attack (ID -6)
                    // Designed to require the Hope player to take Abraxas down fast to avoid taking heavy damage
                    let likelihood = ((strifer.health as f64 / strifer.max_health as f64) * 100.0).floor() as i64; // More likely to be used at high HP
                    let roll = rand::random_range(1..=100);

                    const COST: i64 = 8;
                    if roll <= likelihood && strifer.energy >= COST && !strifer.used_bonus_action {
                        strifer.used_bonus_action = true;
                        strifer.energy -= COST;
                        append(format!("{} bathes the battle in a brilliant, damaging aura!", strifer.name));

                        let damage = (2000.0 * (strifer.health as f64 / strifer.max_health as f64)).floor() as i64;

                        strifers.iter_mut()
                            .filter(|s| s.side != strifer.side)
                            .for_each(|s| {
                                s.health -= damage;
                                s.damage_taken += damage;
                            });
                    }
                },
                "-7" => { // Cetus's random ability selection (ID -7)
                    //Designed to force the Light player to adapt to rapidly changing conditions and fortunes
                    const COST: i64 = 5; // Cheap to make up for being randomized
                    if strifer.energy >= COST && !strifer.used_bonus_action {
                        strifer.used_bonus_action = true;
                        strifer.energy -= COST;

                        let roll = rand::random_range(1..=100);
                        let (damage, power_down, status, bonuses) = if roll == 100 {
                            append(format!("{} fires a gigantic beam of light from her gaping maw!", strifer.name));
                            (Some(rand::random_range(5888..=8888)), None, None, None)
                        } else if roll >= 80 {
                            append(format!("{} thrashes around, injuring you and making it difficult to strike reliably.", strifer.name));
                            strifer.bonuses.push(StrifeBonus::new(StrifeBonusType::Defense, 2000));
                            (Some(rand::random_range(1..=1000)), None, None, None)
                        } else if roll >= 60 {
                            append(format!("{} bends probability, making a mockery of your defensive efforts!", strifer.name));
                            (None, None, None, Some(vec![
                                StrifeBonus::with_duration(StrifeBonusType::Defense, 3, -1888),
                                StrifeBonus::with_duration(StrifeBonusType::Accuse, 3, -1888),
                                StrifeBonus::with_duration(StrifeBonusType::Abjure, 3, -1888),
                                StrifeBonus::with_duration(StrifeBonusType::Abstain, 3, -1888),
                            ]))
                        } else if roll >= 40 {
                            append(format!("{} creates a brilliant flash of light, blinding you both physically and to the currents of fortune.", strifer.name));
                            // Large effect this round, moderate for a couple rounds after, small lingering
                            (None, Some(88), Some(StrifeStatus::with_duration(StrifeStatusType::Unlucky, 3)), Some(vec![
                                StrifeBonus::with_duration(StrifeBonusType::Offense, 3, -888),
                                StrifeBonus::with_duration(StrifeBonusType::Offense, 1, -888),
                            ]))
                        } else if roll >= 20 {
                            append(format!("{} fires a gigantic beam of light from her gaping maw, but it mostly misses.", strifer.name));
                            (Some(rand::random_range(200..=1000)), None, None, None)
                        } else {
                            append(format!("{} casts a spell, a light shining from her briefly.", strifer.name));
                            strifer.brief_luck += 10;
                            (None, None, None, None)
                        };

                        strifers.iter_mut()
                            .filter(|s| s.side != strifer.side)
                            .for_each(|s| {
                                if let Some(damage) = damage {
                                    s.health -= damage;
                                    s.damage_taken += damage;
                                }

                                if let Some(power_down) = power_down {
                                    s.power -= power_down;
                                }

                                if let Some(status) = &status {
                                    // Don't want unlucky appearing and affecting before the player can do anything
                                    push_status(&s.id, &[status.clone()]);
                                }

                                if let Some(bonuses) = bonuses.to_owned() {
                                    s.bonuses.extend(bonuses);
                                }
                            });
                    }
                },
                "-10" => { // Armok's multi-limbed multiattack. May be possessed by other monsters! (ID -10)
                    const COST: i64 = 13;
                    if strifer.energy >= COST && !strifer.used_bonus_action {
                        strifer.used_bonus_action = true;
                        strifer.attacks += 1;
                        append(format!("{} strikes out at you with multiple appendages!", strifer.name));
                    }
                },
                "-11" => { // Moros's aura of doom (ID -11)
                    append(format!("{}'s presence draws you slowly, inevitably, towards your doom.", strifer.name));
                    strifers.iter_mut()
                        .filter(|s| s.side != strifer.side)
                        .for_each(|s| {
                            let damage = (s.max_health as f64 / 40.0).floor() as i64; // Saps 2.5% of max HP per round
                            s.health -= damage;
                            s.damage_taken += damage;
                        });
                },
                _ => {}
            }
        }

        match strifer.current_motif.as_str() {
            "Light/I" => {
                strifer.brief_luck = 100;
            },
            "Time/I" => {
                strifer.attacks += 3;
            },
            "Time/II" => {
                strifers.iter_mut()
                    .filter(|s| s.id != strifer.id)
                    .for_each(|s| {
                        s.statuses.extend_from_slice(&[
                            StrifeStatus::new(StrifeStatusType::CantAttack),
                            StrifeStatus::new(StrifeStatusType::CantDefend),
                        ])
                    });
            }
            _ => {}
        }

        strifers[i] = strifer;
    }

    let mut leader_offense: HashMap<i8, i64> = HashMap::new(); // i8: side, i64: offense bonus
    let mut leader_defense: HashMap<i8, i64> = HashMap::new(); // i8: side, i64: defense bonus
    let mut side_defense  : HashMap<i8, i64> = HashMap::new(); // i8: side, i64: defense bonus
    let mut teamwork      : HashMap<i8, i64> = HashMap::new(); // i8: side, i64: teamwork bonus

    // Second iteration to handle leader boosts (needs to happen after all the effects are handled)
    for strifer in &strifers {
        // Only take into account non-leaders for the boosts
        if strifer.is_leader {
            continue;
        }

        let power = strifer.power();
        *leader_offense.entry(strifer.side)
            .or_insert(0) += (power.offense as f64 * (strifer.teamwork as f64 / 100.0)).floor() as i64;
        *leader_defense.entry(strifer.side)
            .or_insert(0) += (power.defense as f64 * (strifer.teamwork as f64 / 100.0)).floor() as i64;
    }

    // Third iteration to apply boosts to leaders and build up defense boosts for non-leaders
    for strifer in &mut strifers {
        // Only apply leadership boosts to leaders
        if !strifer.is_leader {
            continue;
        }

        if let Some(offense_boost) = leader_offense.get(&strifer.side) {
            strifer.bonuses.push(StrifeBonus::new(StrifeBonusType::Offense, *offense_boost));
        }

        if let Some(defense_boost) = leader_defense.get(&strifer.side) {
            strifer.bonuses.push(StrifeBonus::new(StrifeBonusType::Defense, *defense_boost));
        }

        // Depends on the bonuses that were set up above
        let power = strifer.power();
        *side_defense.entry(strifer.side)
            .or_insert(0) = (power.defense as f64 * (strifer.teamwork as f64 / 100.0)).floor() as i64;
        *teamwork.entry(strifer.side)
            .or_insert(0) = strifer.teamwork;
    }

    // Fourth iteration to apply teamwork and side defense boosts to non-leaders
    for strifer in &mut strifers {
        // Only apply teamwork and side defense boosts to non-leaders
        if strifer.is_leader {
            continue;
        }

        if side_defense.get(&strifer.side).is_some() {
            let power = strifer.power();

            // The below line subtracts the amount of this strifer's defensive power that got into the teamwide boost from the boost they receive.
            // This is to prevent them from getting to double-up on their own defensive power due to teamwork. (Otherwise two players with 100% teamwork
            // working together would have 3x defense each, which is an issue)
            let bonus = side_defense.get(&strifer.side).unwrap_or(&0) -
                (power.defense as f64 * (strifer.teamwork as f64 / 100.0) * (*teamwork.get(&strifer.side).unwrap_or(&0) as f64 / 100.0)).floor() as i64;
            strifer.bonuses.push(StrifeBonus::new(StrifeBonusType::Defense, bonus));
        }
    }

    // Fifth + O(n^2) iteration to finally STRIFE!
    for i in 0..strifers.len() {
        for t in 0..strifers.len() {
            let mut attacker = strifers[i].clone();
            let mut target = strifers[t].clone();
            if attacker.side == target.side {
                // Target is on the same team as the attacker! Skip!
                continue;
            }

            for mut i in 0..attacker.attacks {
                let attacker_power = attacker.power();
                let target_power = target.power();

                let min_offense_roll = (90 + (attacker.luck as f64 / 5.0).floor() as i64).min(110);
                let min_defense_roll = (90 + (target.luck as f64 / 5.0).floor() as i64).min(110);

                let offense = (attacker_power.offense as f64 * (rand::random_range(min_offense_roll..=110) as f64 / 100.0)).floor() as i64;

                if attacker.owner_id.is_some_and(|o| o == character.id) {
                    character.set_stat_max("maxdamage", offense);
                }

                let defense = (target_power.defense as f64 * (rand::random_range(min_defense_roll..=110) as f64 / 100.0)).floor() as i64;
                let mut damage = offense - defense;

                let damage_difference = damage.max(0);

                // Calculate damage effects
                let mut flat_damage = 0i64; // Anything added to `flat_damage` will be added to the damage without being affected by multipliers of any kind.
                let inherit_offense = attacker.statuses.iter().find(|s| s.status_type.is_inheritance()).is_some();

                // Check attacker abilities for relevance
                for ability in &attacker.abilities {
                    // Abilities on the attacker that affect their damage dealt have an entry here
                    match ability.as_str() {
                        "21" => { // Inevitability (ID 21)
                            let multiplier = 1.0 + (target.health as f64 / target.max_health as f64); // 1 at full health, 2 at no health. Nice and simple.
                            damage = (damage as f64 * multiplier).ceil() as i64;
                            // No message. It activates literally every attack on an enemy who's not on full health.
                        },
                        "22" => {
                            let min_roll = (1 + (attacker.luck as f64 / 12.0).floor() as i64).min(100);
                            let roll = rand::random_range(min_roll..=100);
                            let target_roll = (100.0 - (target.echeladder as f64 / 20.0)) as i64; // Approx. 30% chance at max rung
                            if roll >= target_roll || (inherit_offense && !attacker.time_attack) {
                                append(format!("{}'s Lv. 327 Timetech Broken Record activates! Time skips back as they attack {}, allowing them to strike again.", attacker.name, target.name));
                                attacker.time_attack = true;
                                i -= 1; // One more go!
                            }
                        },
                        _ => {}
                    }
                }

                // Check target statuses for relevance.
                let mut inherit_defense = false;
                for status in &target.statuses {
                    // Statuses on the target that affect their damage taken have an entry here
                    match status.status_type {
                        StrifeStatusType::WateryGel => {
                            damage = (damage as f64 * 1.15).floor() as i64;
                        },
                        StrifeStatusType::Inheritance => {
                            inherit_defense = true;
                        },
                        _ => {}
                    }
                }

                // Check target abilities for relevance.
                for ability in &target.abilities {
                    // Abilities on the target that affect their damage taken have an entry here
                    match ability.as_str() {
                        "4" => { // Dissipate: Check for activation
                            let min_roll = (1 + (target.luck as f64 / 12.0).floor() as i64).min(100);
                            let roll = rand::random_range(min_roll..=100);
                            let target_roll = (100.0 - (target.echeladder as f64 / 40.0)) as i64;
                            if roll >= target_roll || inherit_defense {
                                append(format!("{} briefly transforms into wind and evades {}'s attack!", target.name, attacker.name));
                                damage = 0;
                            }
                        },
                        "13" => {
                            if damage_difference > 0 { // Attack causes damage, attacker will receive recoil
                                let recoil = (damage_difference as f64 / 10.0).ceil() as i64;
                                attacker.health -= recoil;
                                append(format!("{}'s Lv. 239 Spacebending Spatial Warp activates, dealing recoil damage to {}!", target.name, attacker.name));
                            }
                        },
                        "-8" => { // Metis's decision point
                            if !target.used_bonus_action { // Metis has an action and needs to decide what to do with it
                                let dodge_chance = (damage as f64 / 100.0) as i64; // Guaranteed dodge at 10k damage and above
                                const DODGE_COST: i64 = 17;

                                let roll = rand::random_range(1..=100);
                                // Original condition: $roll <= $dodgechance && $strifers[$i]['energy'] <= $dodgecost
                                // $roll: roll, $dodgechance: dodge_chance, $strifers[$i]['energy]: attacker.energy, $dodgecost: DODGE_COST
                                if roll <= dodge_chance && target.energy >= DODGE_COST {
                                    // Roll passes and Metis has enough energy for a perfect dodge.
                                    append(format!("{} perfectly predicts {}'s attack, avoiding it flawlessly!", target.name, attacker.name));
                                    target.energy -= DODGE_COST;
                                    target.used_bonus_action = true;
                                    damage = 0;
                                } else {
                                    // Not dodging. Take down data from this enemy to make a decision with later.
                                    target.next_offense = attacker_power.offense;
                                    target.next_defense = attacker_power.defense;
                                    target.next_used_bonus_action = attacker.used_bonus_action;
                                }
                            }
                        }
                        _ => {},
                    }
                }

                // On-hit effects only apply if damage is dealt!
                if damage > 0 {
                    // NOTE - We append to updated_status because PowerLoss and PowerGain need to go off post-round
                    match attacker.current_motif.as_str() {
                        "Mind/I" => {
                            flat_damage += 413;
                            push_status(&target.id, &[StrifeStatus::new(StrifeStatusType::PowerLoss(2413))]);
                        },
                        "Blood/I" => {
                            flat_damage += 690;
                            push_status(&target.id, &[StrifeStatus::with_duration(StrifeStatusType::Bleeding(5), 3)]);

                            strifers
                                .iter_mut()
                                .filter(|s| s.side == attacker.side)
                                .for_each(|s| push_status(&s.id, &[StrifeStatus::new(StrifeStatusType::PowerGain(690))]));
                        },
                        "Doom/I" => {
                            let factor = (3.0 - (((target.health as f64 + target.damage_taken as f64) / target.max_health as f64) * 2.0))
                                .clamp(1.0, 3.0);
                            damage = (damage as f64 * factor).ceil() as i64;
                        },
                        "Void/I" => {
                            target.power = ((rand::random_range(80..=100) as f64 / 100.0) * target.power as f64).floor() as i64;
                            flat_damage += ((rand::random_range(0..=20) as f64 / 100.0) * target.max_health as f64).floor() as i64;
                        },
                        "Space/I" => {
                            if target.is_leader {
                                flat_damage += 40000;
                            }
                        },
                        "Breath/II" => {
                            flat_damage += 5000;
                            push_status(&target.id, &[StrifeStatus::with_duration(StrifeStatusType::Knockdown, 2)]);
                        },
                        "Heart/II" => {
                            flat_damage += target.power * 2;
                        },
                        "Blood/II" => {
                            push_status(&target.id, &[
                                StrifeStatus::with_duration(StrifeStatusType::Isolated, 3),
                                StrifeStatus::new(StrifeStatusType::PowerLoss(690))
                            ]);
                        },
                        "Doom/II" => {
                            if target.is_leader {
                                flat_damage += 50000;
                                push_status(&target.id, &[
                                    StrifeStatus::with_duration(StrifeStatusType::Isolated, 2),
                                    StrifeStatus::with_duration(StrifeStatusType::Hopeless, 2),
                                ]);
                            }
                        },
                        "Rage/II" => {
                            let side = rand::random_range(0u8..=255u8);
                            push_status(&target.id, &[StrifeStatus::with_duration(StrifeStatusType::Charmed(side), 2)]);
                        },
                        "Void/II" => {
                            push_status(&target.id, &[StrifeStatus::with_duration(StrifeStatusType::Unstuck, 5)]);
                        },
                        "Space/II" => {
                            push_status(&target.id, &[
                                StrifeStatus::permanent(StrifeStatusType::Burning(2500)),
                                StrifeStatus::permanent(StrifeStatusType::Burning(2500)),
                                StrifeStatus::permanent(StrifeStatusType::Burning(2500)),
                                StrifeStatus::permanent(StrifeStatusType::Burning(2500)),
                                StrifeStatus::permanent(StrifeStatusType::Burning(2500)),
                                StrifeStatus::permanent(StrifeStatusType::Burning(2500)),
                            ]);
                            flat_damage += 10000;
                        },
                        "Light/II" => {
                            damage += offense;
                            append("A critical hit!".to_string());
                        }
                        _ => {}
                    }

                    // Lastly, check on-hit effects, since we don't want these affecting the outcome of THIS attack.
                    for effect in &attacker.effects {
                        let current_effect = effect.split(":").collect_vec();
                        match current_effect[0] {
                            // On-hit affects the attacker produces are processed here
                            "AFFINITY" => { // Format is AFFINITY:<type>:<percentage>|
                                let resist_factor = 1.0 - (target.resistance_to(current_effect[1]) as f64 / 100.0);
                                let multiplier = (current_effect[2].parse::<i64>()? as f64 * resist_factor) / 100.0;
                                damage += (damage as f64 * multiplier) as i64;
                            },
                            "RANDAMAGE" => { // Format is RANDAMAGE:<%variance>|
                                let variance = current_effect[1].parse::<i64>()? as f64;
                                let luck_factor = ((attacker.luck as f64 / 100.0) * variance)
                                    .min(variance * 2.0);
                                let roll = rand::random_range((100 - (variance + luck_factor) as i64)..=(100 + variance as i64)) as f64 / 100.0;
                                damage = (damage as f64 * roll).floor() as i64;
                            },
                            "LIFESTEAL" => { // Format is LIFESTEAL:<%chance>:<%absorbed>|
                                let lowest_result = (1 + (attacker.luck as f64 / 12.0).floor() as i64)
                                    .min(100);
                                let roll = rand::random_range(lowest_result..=100);
                                let target_roll = 100 - current_effect[1].parse::<i64>()?;
                                if roll > target_roll {
                                    let resist_factor = 1.0 - (target.resistance_to("Blood") as f64 / 100.0);
                                    let multiplier = (current_effect[2].parse::<i64>()? as f64 * resist_factor) / 100.0;
                                    attacker.health += (damage as f64 * multiplier) as i64;
                                    append(format!("{} drains life from {}!", attacker.name, target.name));
                                }
                            },
                            "KNOCKDOWN" => { // Format is KNOCKDOWN:<%multiplier>|
                                let resist_factor = 1.0 - (target.resistance_to("Breath") as f64 / 100.0);
                                let effective_damage = damage as f64 * (current_effect[1].parse::<i64>()? as f64 / 100.0) * resist_factor;
                                let target_roll = 100 - ((effective_damage / target.max_health as f64) * 400.0) as i64;
                                let lowest_result = (1 + (attacker.luck as f64 / 20.0).floor() as i64)
                                    .min(100);
                                let roll = rand::random_range(lowest_result..=100);
                                if roll > target_roll {
                                    append(format!("{} is sent flying by the force of {}'s blow!", target.name, attacker.name));
                                    push_status(&target.id, &[StrifeStatus::with_duration(StrifeStatusType::Knockdown, 2)]);
                                }
                            },
                            "COMPUTER" => { /* Don't Care */ }
                            _ => { // Format for "apply the first entry as a status effect": <status string>:<chance>:<resistance>:<message>|
                                // The vast majority of on-hit effects will simply fall into this category.
                                // <status string> has all :'s replaced with @'s so it stays coherent when we do the explode
                                let lowest_result = (1 + (attacker.luck as f64 / 12.0).floor() as i64)
                                    .min(100);
                                let roll = rand::random_range(lowest_result..=100);
                                let resist_factor = 1.0 - (target.resistance_to(current_effect[2]) as f64 / 100.0);
                                let target_roll = 100 - (current_effect[1].parse::<i64>()? as f64 * resist_factor) as i64;
                                if roll > target_roll {
                                    let status = current_effect[0]
                                        .replace("@", ":")
                                        .parse::<StrifeStatus>()?;
                                    push_status(&target.id, &[status]);
                                    append(current_effect[3]
                                        .replace("%USER%", attacker.name.as_str())
                                        .replace("%TARGET%", target.name.as_str()));
                                }
                            }
                        }
                    }

                    damage += flat_damage;
                }

                damage = damage.max(0); // No, failing to beat their defense does not heal them
                target.health -= damage; // This CAN go negative, if it does that's handled in the end of turn area.
                target.damage_taken += damage;
                attacker.damage_dealt += damage;
                // Since a round of combat represents extended fighting, the felled strifer still gets to make their own attacks
            }

            strifers[i] = attacker;
            strifers[t] = target;
        }
    }

    // Final iteration to apply post effects
    for i in 0..strifers.len() {
        let (left, strifer, right) = strifers.split_out_mut(i);

        // Commit End-of-Turn effects to strifer statuses field
        if let Some(statuses) = updated_status.get(&strifer.id) {
            strifer.statuses = statuses.clone();
        }

        // Reset bonus actions
        strifer.used_bonus_action = false;

        let mut new_status: Vec<StrifeStatus> = vec![];

        {
            let mut statuses = std::mem::take(&mut strifer.statuses);
            statuses.retain_mut(|status| {
                let mut removed = false;

                match &status.status_type {
                    StrifeStatusType::Timestop => {
                        // If the afflicted strifer is going to be frozen next turn, they can't take a consumable action either.
                        if status.duration.is_none_or(|d| d != 1) {
                            strifer.used_bonus_action = true;
                        }
                    }
                    StrifeStatusType::Paralyzed(severity) => {
                        let min_roll = 1 + (strifer.luck as f64 / 5.0).floor() as i64;
                        let save = rand::random_range(min_roll..=100) +
                            // Life resistance has a chance of aiding with paralysis
                            strifer.resistance_to("Life");

                        if save < *severity {
                            append(format!("{}'s paralysis prevents them from acting between rounds!", strifer.name));
                            strifer.used_bonus_action = true;
                        }
                    }
                    StrifeStatusType::Frozen => {
                        let min_roll = 1 + (strifer.luck as f64 / 8.0) as i64;
                        let save = rand::random_range(min_roll..=100);
                        if save > 80 || status.duration.is_some_and(|d| d == 1) {
                            append(format!("{} thawed out!", strifer.name));
                            removed = true;
                        } else {
                            strifer.used_bonus_action = true;
                            append(format!("{} is frozen solid!", strifer.name));
                        }
                    }
                    StrifeStatusType::Hopeless => {
                        // Hopeless gets a roll to disappear if the strifer manages to deal basic damage
                        let save = rand::random_range(1..=100);
                        if strifer.damage_dealt > 0 && save > 50 {
                            removed = true;
                            append(format!("{} has managed to regain hope!", strifer.name));
                        }
                    }
                    StrifeStatusType::Poison(severity) => { // `severity` is in thousandths of max health debited per round.
                        // Attempt to save first
                        let save = rand::random_range(1..=100) +
                            strifer.resistance_to("Doom");

                        if save >= 95 {
                            removed = true;
                            append(format!("{} manages to fight off the poison!", strifer.name));
                        } else {
                            let damage = ((severity * strifer.max_health) as f64 / 1000.0).floor() as i64;
                            strifer.health -= damage;
                            append(format!("{} loses some health to poison!", strifer.name));
                        }
                    }
                    StrifeStatusType::Bleeding(severity) => { // Severity is in thousandths of max health/power debited per round.
                        // Attempt to save first
                        let save = rand::random_range(1..=100) +
                            strifer.resistance_to("Blood");

                        if save >= 80 {
                            removed = true;
                            append(format!("{} manages to staunch a wound!", strifer.name));
                        } else {
                            let damage = ((severity * strifer.max_health) as f64 / 1000.0).floor() as i64;
                            strifer.health -= damage;

                            let power_loss = ((severity * strifer.max_power) as f64 / 1000.0).floor() as i64;
                            strifer.power -= power_loss;

                            append(format!("{} loses some blood or blood analogue!", strifer.name));
                        }
                    }
                    StrifeStatusType::Burning(damage) => { // Does <damage> damage every round.
                        // Attempt to save first
                        let save = rand::random_range(1..=100) +
                            (strifer.resistance_to("Rage") / 2);

                        if save > 75 {
                            removed = true;
                            append(format!("{} stops, drops, and rolls around a bit, extinguishing itself somewhat.", strifer.name));
                        } else {
                            let distracted_chance = rand::random_range(1..=100) +
                                strifer.resistance_to("Mind");

                            if distracted_chance < ((*damage as f64 / strifer.health as f64) * 100.0) as i64 {
                                append(format!("{} is on fire and panicking!", strifer.name));
                                new_status.push(StrifeStatus::new(StrifeStatusType::Distracted));
                            } else {
                                append(format!("{} is on fire! Oh no!", strifer.name));
                            }

                            strifer.health -= damage;
                        }
                    }
                    StrifeStatusType::Irradiated(severity) => { // Does <severity> damage and drains <severity> power every round.
                        // No save for radiation poisoning once you have it
                        strifer.health -= severity;
                        strifer.power -= severity;
                    }
                    StrifeStatusType::Delayed(s) => {
                        if status.duration.is_some_and(|d| d == 1) {
                            new_status.push(*s.clone());
                        }
                    }
                    StrifeStatusType::SelfInflict { status, chance, resist, message } => {
                        let save = rand::random_range(1..=100) +
                            strifer.resistance_to(resist.as_str());
                        if save <= *chance as i64 {
                            new_status.push(*status.clone());
                            append(format!("{}{}", strifer.name, message));
                        }
                    }
                    StrifeStatusType::Unlucky => {
                        let min_roll = 1 + (strifer.luck as f64 / 10.0).floor() as i64; // -200 or more means you can't throw off this debuff. hehehehehe.
                        let misfortune = rand::random_range(min_roll..=100);
                        if misfortune == 1 { // Natural 1. Oh dear!
                            if strifer.owner_id.is_none() { // NPC
                                append(format!("{} spontaneously winks out of existence. How very unfortunate!", strifer.name));
                                new_status.push(StrifeStatus::permanent(StrifeStatusType::Pinata));
                                strifer.power = 0;
                                strifer.health = 1;
                            } else { // PC
                                append(format!("{} has been unfortuitously felled!", strifer.name));
                                strifer.health = -888888;
                            }
                        } else if misfortune <= 2 { // This becomes a lot more likely if the target has negative luck
                            append(format!("{} is struck by lightning!", strifer.name));
                            new_status.extend_from_slice(&[
                                StrifeStatus::with_duration(StrifeStatusType::Disoriented, 2),
                                StrifeStatus::new(StrifeStatusType::Knockdown),
                                StrifeStatus::with_duration(StrifeStatusType::Burning(8888), 8)
                            ]);
                            strifer.health -= 88888;
                        } else if misfortune <= 10 {
                            append(format!("The ground gives way beneath {} and it plummets, shaking it up pretty badly when it lands.", strifer.name));
                            new_status.push(StrifeStatus::with_duration(StrifeStatusType::Knockdown, 2));
                            strifer.health -= 8888;
                            strifer.power -= 888;
                        } else if misfortune <= 20 {
                            append(format!("A meteor hits {}. What are the chances?", strifer.name));
                            strifer.health -= 10000;
                        } else if misfortune <= 50 {
                            append(format!("{} trips and falls. Hilarious!", strifer.name));
                            new_status.push(StrifeStatus::new(StrifeStatusType::Enraged));
                            strifer.health -= (strifer.health as f64 / 100.0).floor() as i64;
                            strifer.power -= (strifer.power as f64 / 100.0).floor() as i64;
                        } else if misfortune <= 80 {
                            // No misfortune this round
                        } else { // Managed to throw off the unlucky effect
                            append(format!("{} appears less unlucky. This concept is just as visually nebulous as the idea that it appeared unlucky in the first place.", strifer.name));
                            removed = true;
                        }
                    }
                    StrifeStatusType::Regeneration(quantity) => {
                        strifer.health += quantity;
                    }
                    StrifeStatusType::Energized(quantity) => {
                        strifer.energy += quantity;
                    }
                    StrifeStatusType::Recovery(quantity) => {
                        strifer.power += quantity; // Note that if this goes over maxpower it'll be brought back down.
                        // So this isn't really a power boost.
                    }
                    StrifeStatusType::BoostDrain(amount) => {
                        left.iter_mut().chain(right.iter_mut())
                            .filter(|s| s.side != strifer.side && !s.bonuses.is_empty())
                            .for_each(|s| {
                                let mut offense_drain = *amount;
                                let mut defense_drain = *amount;

                                s.bonuses.retain_mut(|bonus| {
                                    if offense_drain + defense_drain <= 0 {
                                        return true;
                                    }

                                    match bonus.bonus_type {
                                        StrifeBonusType::Power => {
                                            if bonus.value > offense_drain.min(defense_drain) {
                                                bonus.value -= offense_drain.min(defense_drain);
                                                offense_drain -= offense_drain.min(defense_drain);
                                                defense_drain -= offense_drain.min(defense_drain);
                                                true
                                            } else { // Reduction removes this power boost, don't retain it.
                                                offense_drain -= bonus.value;
                                                defense_drain -= bonus.value;
                                                false
                                            }
                                        }
                                        StrifeBonusType::Offense => {
                                            if bonus.value > offense_drain {
                                                bonus.value -= offense_drain;
                                                offense_drain = 0;
                                                true
                                            } else {
                                                offense_drain -= bonus.value;
                                                false
                                            }
                                        }
                                        StrifeBonusType::Defense => {
                                            if bonus.value > defense_drain {
                                                bonus.value -= defense_drain;
                                                defense_drain = 0;
                                                true
                                            } else {
                                                defense_drain -= bonus.value;
                                                false
                                            }
                                        }
                                        _ => true
                                    }
                                });
                            });
                    }
                    StrifeStatusType::PowerLoss(loss) => {
                        strifer.power -= loss;
                        removed = true;
                    }
                    StrifeStatusType::PowerGain(gain) => {
                        strifer.bonuses.push(StrifeBonus::permanent(StrifeBonusType::Power, *gain));
                        removed = true;
                    }
                    _ => {}
                }

                if status.duration.is_some_and(|d| d == 1) {
                    removed = true; // Duration 1: This status just expired
                }

                if !removed {
                    if let Some(duration) = status.duration.as_mut() {
                        *duration -= 1;
                    }
                }

                !removed
            });
            statuses.extend(new_status);
            strifer.statuses = statuses;
        }

        strifer.bonuses.retain_mut(|b| {
            if let Some(duration) = b.duration.as_mut() {
                if *duration == 1 {
                    return false
                }

                *duration -= 1;
            }

            true
        });
    }

    Ok(Html("A whole mess of strifing takes place.<br />"))
}

trait SliceExt<T> {
    fn split_out_mut(&mut self, i: usize) -> (&mut [T], &mut T, &mut [T]);
}

impl<T> SliceExt<T> for [T] {
    fn split_out_mut(&mut self, i: usize) -> (&mut [T], &mut T, &mut [T]) {
        assert!(i < self.len());

        let (left, right) = self.split_at_mut(i);
        let (x, right) = right.split_first_mut().unwrap();
        (left, x, right)
    }
}