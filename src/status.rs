use crate::error::Error;
use include_lines::include_lines;
use itertools::Itertools;
use rand::seq::IndexedRandom;
use std::fmt::{Display, Formatter};
use std::str::FromStr;
use strum::{EnumDiscriminants, EnumIs};

const GLITCH_STATUSES: &[&str] = &include_lines!("glitches.txt");

pub fn generate_glitch_string() -> String {
    let horrible_mess = || -> String {
        const CHAR_ARRAY: &[&str] = &["a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k", "l", "m", "n", "o", "p", "q", "r", "s", "t", "u", "v", "w", "x", "y", "z",
                                      "A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "K", "L", "M", "N", "O", "P", "Q", "R", "S", "T", "U", "V", "W", "X", "Y", "Z",
                                      "!", "@", "#", "$", "%", "^", "&", "*", "+", "=", "?", "/", "\\", " "];

        let what_is_it = rand::random_range(1..4);
        let rand = CHAR_ARRAY.choose(&mut rand::rng()).unwrap();
        let mut result = String::new();

        for _ in (1..=rand::random_range(5..15)).rev() {
            if what_is_it == 1 {
                result.push_str(rand);
            } else {
                result.push_str(CHAR_ARRAY.choose(&mut rand::rng()).unwrap());
            }
        }

        result
    };

    let mut status = GLITCH_STATUSES
        .choose(&mut rand::rng())
        .unwrap_or(&"We're not very creative :/")
        .to_string();

    while status.contains("GLITCH") {
        status = status.replacen("GLITCH", horrible_mess().as_str(), 1);
    }

    status
}

#[derive(Debug, Clone, EnumDiscriminants, EnumIs)]
#[strum_discriminants(vis())]
pub enum StrifeStatusType {
    Timestop,
    Hopeless,
    Knockdown,
    WateryGel,
    Poison(i64),
    Bleeding(i64),
    Disoriented,
    Distracted,
    Enraged,
    Mellow,
    Glitched,
    Charmed(u8),
    Delayed(Box<StrifeStatus>),
    Pinata,
    Unlucky,
    HasEffect(String),
    HasResist(String),
    Inheritance,
    Isolated,
    Burning(i64),
    Unstuck,
    Regeneration(i64),
    Energized(i64),
    Recovery(i64),
    BoostDrain(i64),
    Frozen,
    Shrunk,
    Paralyzed(i64),
    Irradiated(i64),
    SelfInflict {
        status: Box<StrifeStatus>,
        chance: i8,
        resist: String,
        message: String,
    },
    LuckBoost(i64),
    CantAttack,
    CantDefend,
    Fury,
    PowerLoss(i64),
    PowerGain(i64),
    #[allow(dead_code)]
    Unknown(String, Option<String>)
}

#[derive(Debug, Clone)]
pub struct StrifeStatus {
    pub status_type: StrifeStatusType,
    pub duration: Option<i64>
}

impl StrifeStatus {
    pub fn new(status_type: StrifeStatusType) -> Self {
        StrifeStatus {
            status_type,
            duration: Some(1) // Last 1 round by default
        }
    }

    pub fn permanent(status_type: StrifeStatusType) -> Self {
        StrifeStatus {
            status_type,
            duration: None
        }
    }

    pub fn with_duration(status_type: StrifeStatusType, duration: i64) -> Self {
        StrifeStatus {
            status_type,
            duration: Some(duration)
        }
    }
}

impl FromStr for StrifeStatus {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let status_elements = s.split(":").collect_vec();
        if status_elements.len() < 2 {
            return Err(Error::TupleParse("tuple destructuring yielded incorrect element count".to_string()));
        }

        let name = status_elements[0];
        let duration = match status_elements[1].parse::<i64>().ok() {
            Some(duration) if duration != 0 => Some(duration),
            _ => None
        };
        let value = if status_elements.len() > 2 {
            Some(status_elements[2])
        } else {
            None
        };

        use StrifeStatusType::*;

        let status_type = match name {
            "TIMESTOP" => Timestop,
            "HOPELESS" => Hopeless,
            "KNOCKDOWN" => Knockdown,
            "WATERYGEL" => WateryGel,
            "POISON" => Poison(value
                .ok_or(Error::TupleParse("Attempting to construct Poison effect with no severity value".to_string()))?
                .parse()?),
            "BLEEDING" => Bleeding(value
                .ok_or(Error::TupleParse("Attempting to construct Bleeding effect with no severity value".to_string()))?
                .parse()?),
            "DISORIENTED" => Disoriented,
            "DISTRACTED" => Distracted,
            "ENRAGED" => Enraged,
            "MELLOW" => Mellow,
            "GLITCHED" => Glitched,
            "CHARMED" => Charmed(value
                .ok_or(Error::TupleParse("Attempting to construct Charmed effect with no side value".to_string()))?
                .parse()?),
            "DELAYED" => Delayed(Box::new(value
                .ok_or(Error::TupleParse("Attempting to construct Delayed effect with no status to be delayed".to_string()))?
                .replace("@", ":").replace("&", "@")
                .parse()?)),
            "PINATA" => Pinata,
            "UNLUCKY" => Unlucky,
            "HASEFFECT" => {
                let effect = value
                    .ok_or(Error::TupleParse("Attempting to construct HasEffect with no effect".to_string()))?
                    .split("@").collect::<Vec<&str>>();
                HasEffect(effect[0].to_string())
            },
            "HASRESIST" => {
                let effect = value
                    .ok_or(Error::TupleParse("Attempting to construct HasResist with no effect".to_string()))?
                    .split("@").collect::<Vec<&str>>();
                HasResist(effect[0].to_string())
            },
            "INHERITANCE" => Inheritance,
            "ISOLATED" => Isolated,
            "BURNING" => Burning(value
                .ok_or(Error::TupleParse("Attempting to construct Burning effect with no severity amount".to_string()))?
                .parse()?),
            "UNSTUCK" => Unstuck,
            "REGENERATION" => Regeneration(value
                .ok_or(Error::TupleParse("Attempting to construct Regeneration effect with no regeneration quantity".to_string()))?
                .parse()?),
            "ENERGIZED" => Energized(value
                .ok_or(Error::TupleParse("Attempting to construct Energized effect with no energized quantity".to_string()))?
                .parse()?),
            "RECOVERY" => Recovery(value
                .ok_or(Error::TupleParse("Attempting to construct Recovery effect with no recovery amount".to_string()))?
                .parse()?),
            "BOOSTDRAIN" => BoostDrain(value
                .ok_or(Error::TupleParse("Attempting to construct BoostDrain effect with no drain amount".to_string()))?
                .parse()?),
            "FROZEN" => Frozen,
            "SHRUNK" => Shrunk,
            "PARALYZED" => Paralyzed(value
                .ok_or(Error::TupleParse("Attempting to construct Paralyzed effect with no severity value".to_string()))?
                .parse()?),
            "IRRADIATED" => Irradiated(value
                .ok_or(Error::TupleParse("Attempting to construct Irradiated effect with no severity value".to_string()))?
                .parse()?),
            "SELFINFLICT" => {
                if status_elements.len() < 6 {
                    return Err(Error::TupleParse("Attempted to construct SelfInflict with invalid number of arguments".to_string()));
                }

                let status = status_elements[2]
                    .replace("@", ":")
                    .replace("&", "@")
                    .parse::<StrifeStatus>()?;

                let chance = status_elements[3]
                    .parse::<i8>()?;

                SelfInflict {
                    status: Box::new(status),
                    chance,
                    resist: status_elements[4].to_string(),
                    message: status_elements[5].to_string(),
                }
            },
            "LUCKBOOST" => LuckBoost(value
                .ok_or(Error::TupleParse("Attempting to construct LuckBoost with no boost percentage".to_string()))?
                .parse()?),
            "CANTATTACK" => CantAttack,
            "CANTDEFEND" => CantDefend,
            "FURY" => Fury,
            "POWERLOSS" => PowerLoss(value
                .ok_or(Error::TupleParse("Attempting to construct PowerLoss effect with no amount".to_string()))?
                .parse()?),
            "POWERGAIN" => PowerGain(value
                .ok_or(Error::TupleParse("Attempting to construct PowerGain effect with no amount".to_string()))?
                .parse()?),
            _ => Unknown(name.to_string(), value.map(str::to_string)),
        };

        Ok(StrifeStatus { status_type, duration })
    }
}

impl Display for StrifeStatus {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match &self.status_type {
            StrifeStatusType::Timestop => write!(f, "Timestop: This strifer is frozen in time.")?,
            StrifeStatusType::Hopeless => write!(f, "Hopeless: This strifer does not believe in itself.")?,
            StrifeStatusType::Knockdown => write!(f, "Knocked over: This strifer needs to get back on its feet or feet analogue.")?,
            StrifeStatusType::WateryGel => write!(f, "Watery health gel: This strifer's health vial is currently easier to dislodge.")?,
            StrifeStatusType::Poison(_) => write!(f, "Poisoned: This strifer is suffering from poison.")?,
            StrifeStatusType::Bleeding(_) => write!(f, "Bleeding: This strifer has a nasty-looking wound.")?,
            StrifeStatusType::Disoriented => write!(f, "Disoriented: This strifer looks kind of dazed and isn't cooperating effectively.")?,
            StrifeStatusType::Distracted => write!(f, "Distracted: This strifer has been distracted by a distaction you mean distraction.")?,
            StrifeStatusType::Enraged => write!(f, "Enraged: This strifer is behaving unusually recklessly.")?,
            StrifeStatusType::Mellow => write!(f, "Mellowed Out: This strifer seem unusually laid back for someone involved in a fight to the death.")?,
            StrifeStatusType::Glitched => write!(f, "Glitched Out: {}", generate_glitch_string())?,
            StrifeStatusType::Charmed(_) => write!(f, "Charmed: This strifer is fighting for a different side!")?,
            StrifeStatusType::Delayed(_) => write!(f, "???: This strifer is going to be affected by an unknown status effect in...")?,
            StrifeStatusType::Pinata => write!(f, "This strifer has been replaced by a small replica of itself. Taped to the replica is a note saying \"Pinata. Enjoy! -The Management\"")?,
            StrifeStatusType::Unlucky => write!(f, "Unlucky: This strifer appears unlucky. Huh? What does unlucky look like? How should I know?")?,
            StrifeStatusType::HasEffect(effect) => write!(f, "Empowered: This strifer possesses the {} on-hit effect.", effect)?,
            StrifeStatusType::HasResist(effect) => write!(f, "Resistant: This strifer is being artificially granted {} resistance.", effect)?,
            StrifeStatusType::Inheritance => write!(f, "Attuned: This strifer has focused on allowing their Aspect to permeate them and will reap the benefits this round.")?,
            StrifeStatusType::Isolated => write!(f, "Isolated: This strifer is unable to fight with their allies at present.")?,
            StrifeStatusType::Burning(_) => write!(f, "On Fire: Do you really need any further explanation?")?,
            StrifeStatusType::Unstuck => write!(f, "Unstuck: This strifer's fakeness attribute is fluctuating wildly!")?,
            StrifeStatusType::Regeneration(_) => write!(f, "Regeneration: This strifer is regaining health every round.")?,
            StrifeStatusType::Energized(_) => write!(f, "Energized: This strifer is regaining energy every round.")?,
            StrifeStatusType::Recovery(_) => write!(f, "Power recovery: If their power is reduced below maximum, this strifer will regain some of their lost power every round.")?,
            StrifeStatusType::BoostDrain(_) => write!(f, "Boost drain: This strifer's opponents, if boosted, will see their power boosts degrade every turn.")?,
            StrifeStatusType::Frozen => write!(f, "Frozen: This strifer is frozen solid!")?,
            StrifeStatusType::Shrunk => write!(f, "Shrunk: This strifer has been reduced in size.")?,
            StrifeStatusType::Paralyzed(_) => write!(f, "Paralyzed: This strifer may not be able to act.")?,
            StrifeStatusType::Irradiated(_) => write!(f, "Irradiated: This strifer has been afflicted with radiation.")?,
            StrifeStatusType::SelfInflict { status, .. } => write!(f, "Afflicted?: This strifer is being hit with the {:?} status effect at random.", StrifeStatusTypeDiscriminants::from(&status.status_type))?,
            StrifeStatusType::LuckBoost(percentage) => write!(f, "Luck Boosted: This strifer is {}% luckier! This has a very distinct visual cue that I'm not going to tell you about.", percentage)?,
            StrifeStatusType::Unknown(name, _) => write!(f, "No message for status {}. This is probably a bug, please submit a report!", name)?,
            _ => {},
        };

        write!(f, " Duration: ")?;
        if let Some(duration) = self.duration {
            write!(f, "{} turn(s).", duration)?;
        } else {
            write!(f, "Entire strife.")?;
        }
        write!(f, "<br />")?;

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum StrifeBonusType {
    Power,
    Offense,
    Defense,
    Aggrieve,
    Aggress,
    Assail,
    Assault,
    Abuse,
    Accuse,
    Abjure,
    Abstain,
    Unknown,
}

impl StrifeBonusType {
    pub fn key(&self) -> &'static str {
        match self {
            StrifeBonusType::Power => "POWER",
            StrifeBonusType::Offense => "OFFENSE",
            StrifeBonusType::Defense => "DEFENSE",
            StrifeBonusType::Aggrieve => "AGGRIEVE",
            StrifeBonusType::Aggress => "AGGRESS",
            StrifeBonusType::Assail => "ASSAIL",
            StrifeBonusType::Assault => "ASSAULT",
            StrifeBonusType::Abuse => "ABUSE",
            StrifeBonusType::Accuse => "ACCUSE",
            StrifeBonusType::Abjure => "ABJURE",
            StrifeBonusType::Abstain => "ABSTAIN",
            StrifeBonusType::Unknown => "UNKNOWN"
        }
    }
}

#[derive(Debug, Clone)]
pub struct StrifeBonus {
    pub key: String,
    pub bonus_type: StrifeBonusType,
    pub duration: Option<i64>,
    pub value: i64
}

impl StrifeBonus {
    pub fn new(bonus_type: StrifeBonusType, value: i64) -> StrifeBonus {
        StrifeBonus {
            key: bonus_type.key().to_string(),
            bonus_type,
            duration: Some(1),
            value
        }
    }

    pub fn permanent(bonus_type: StrifeBonusType, value: i64) -> StrifeBonus {
        StrifeBonus {
            key: bonus_type.key().to_string(),
            bonus_type,
            duration: None,
            value
        }
    }

    pub fn with_duration(bonus_type: StrifeBonusType, duration: i64, value: i64) -> StrifeBonus {
        StrifeBonus {
            key: bonus_type.key().to_string(),
            bonus_type,
            duration: Some(duration),
            value
        }
    }
}

impl FromStr for StrifeBonus {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let [name, duration, value] = s.split(':').collect_array::<3>().ok_or(Error::TupleParse("tuple destructuring yielded incorrect element count".to_string()))?;

        let duration = match duration.parse::<i64>() {
            Ok(duration) if duration != 0 => Some(duration),
            _ => None
        };

        use StrifeBonusType::*;

        let bonus_type = match name {
            "POWER" => Power,
            "OFFENSE" => Offense,
            "DEFENSE" => Defense,
            "AGGRIEVE" => Aggrieve,
            "AGGRESS" => Aggress,
            "ASSAIL" => Assail,
            "ASSAULT" => Assault,
            "ABUSE" => Abuse,
            "ACCUSE" => Accuse,
            "ABJURE" => Abjure,
            "ABSTAIN" => Abstain,
            _ => Unknown
        };


        Ok(StrifeBonus { key: name.to_string(), bonus_type, duration, value: value.parse()? })
    }
}

impl Display for StrifeBonus {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match &self.bonus_type {
            StrifeBonusType::Power => write!(f, "Power boost.")?,
            StrifeBonusType::Offense => write!(f, "Offense boost.")?,
            StrifeBonusType::Defense => write!(f, "Defense boost.")?,
            StrifeBonusType::Aggrieve => write!(f, "Aggrieve boost.")?,
            StrifeBonusType::Aggress => write!(f, "Aggress boost.")?,
            StrifeBonusType::Assail => write!(f, "Assail boost.")?,
            StrifeBonusType::Assault => write!(f, "Assault boost.")?,
            StrifeBonusType::Abuse => write!(f, "Abuse boost.")?,
            StrifeBonusType::Accuse => write!(f, "Accuse boost.")?,
            StrifeBonusType::Abjure => write!(f, "Abjure boost.")?,
            StrifeBonusType::Abstain => write!(f, "Abstain boost.")?,
            StrifeBonusType::Unknown => write!(f, "No message for bonus {}. THis is probably a bug, please submit a report!", self.key.as_str())?,
        }

        write!(f, " Duration: ")?;
        if let Some(duration) = self.duration {
            write!(f, "{} turn(s).", duration)?;
        } else {
            write!(f, "Entire strife.")?;
        }
        write!(f, "<br />")?;

        Ok(())
    }
}

// trait StrExt {
//     #[allow(clippy::needless_lifetimes)]
//     fn replace_with<'a, P, F, S>(&'a self, pattern: P, replacer: F) -> Cow<'a, str>
//     where
//         P: Pattern,
//         F: FnMut(usize, usize, &'a str) -> S,
//         S: AsRef<str>;
// }
//
// impl StrExt for str {
//     #[allow(clippy::needless_lifetimes)]
//     fn replace_with<'a, P, F, S>(&'a self, pattern: P, mut replacer: F) -> Cow<'a, str>
//     where
//         P: Pattern,
//         F: FnMut(usize, usize, &'a str) -> S,
//         S: AsRef<str>
//     {
//         let mut result = String::new();
//         let mut lastpos = 0;
//
//         for (idx, (pos, substr)) in self.match_indices(pattern).enumerate() {
//             result.push_str(&self[lastpos..pos]);
//             lastpos = pos + substr.len();
//             let replacement = replacer(idx, pos, substr);
//             result.push_str(replacement.as_ref());
//         }
//
//         if lastpos == 0 {
//             Cow::Borrowed(self)
//         } else {
//             result.push_str(&self[lastpos..]);
//             Cow::Owned(result)
//         }
//     }
// }