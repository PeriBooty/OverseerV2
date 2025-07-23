pub mod leader;
pub mod abscond;
pub mod select;
mod resolve;

pub use leader::*;
pub use abscond::*;
pub use select::*;

use std::collections::HashMap;
use std::fmt::Formatter;
use askama::Template;
use axum::Extension;
use axum::response::IntoResponse;
use itertools::Itertools;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde::de::{MapAccess, Unexpected, Visitor};
use serde::ser::SerializeMap;
use sqlx::MySqlPool;
use crate::error::{Error, Result};
use crate::routes::character::{Character, Strifer};
use crate::routes::HtmlTemplate;

pub async fn strife_display(character: Character, Extension(db): Extension<MySqlPool>) -> Result<impl IntoResponse> {
    let main_strifer = character.strife.clone();
    let db = &db;

    let strifers = if let Some(strife_id) = main_strifer.strife_id {
        Strifer::from_strife_id(strife_id, db).await?
    } else { Vec::new() };

    let (player_side, fraymotif_strifer) = if main_strifer.strife_id.is_none() {
        (-1, None)
    } else {
        let mut player_side: i8 = -1;
        let mut fraymotif_strifer = None;
        for strifer in &strifers {
            if strifer.fetch_owner(db).await?.is_some_and(|o| o.id == character.id) {
                player_side = strifer.side;
                if !strifer.current_motif.is_empty() {
                    fraymotif_strifer = Some(strifer);
                }
            }
        }

        (player_side, fraymotif_strifer)
    };

    let fraymotif_message = match fraymotif_strifer {
        None => None,
        Some(strifer) => {
            let template = StrifeFraymotifTemplate {
                fraymotif: strifer.current_motif.clone(),
                fraymotif_name: strifer.current_motif_name.clone(),
                strifer_name: strifer.name.clone(),
            };

            Some(template)
        }
    };

    let chumroll = Character::from_session(character.session, db).await?;

    let chain = {
        let mut chain: HashMap<i64, bool> = HashMap::new();
        let mut mates: HashMap<i64, &Character> = HashMap::new();

        for chum in &chumroll {
            chain.insert(chum.id, false);
            mates.insert(chum.id, chum);
        }

        if character.gates_cleared >= 1 {
            chain.insert(character.id, true);
        }

        if character.gates_cleared >= 2 {
            let mut current_chum = &character;
            let mut no_break = true;
            let mut minus_2_row: Option<&Character> = None;
            let mut minus_1_row: Option<&Character> = None;

            while current_chum.server_id.is_some_and(|s| s != character.id) && no_break {
                let server_id = current_chum.server_id.unwrap();
                let minus_3_row = minus_2_row;
                minus_2_row = minus_1_row;
                minus_1_row = Some(&current_chum);

                current_chum = mates.get(&server_id).ok_or(Error::CharacterNotFound(server_id))?;
                no_break =
                    current_chum.gates_cleared >= 6 && minus_3_row.is_some_and(|c| c.gates_cleared >= 6)
                        || current_chum.gates_cleared >= 4 && minus_2_row.is_some_and(|r| r.gates_cleared >= 4)
                        || current_chum.gates_cleared >= 2 && minus_1_row.is_some_and(|c| c.gates_cleared >= 2);

                if no_break {
                    if let Some(c) = chain.get_mut(&current_chum.id) {
                        *c = true;
                    }
                }
            }
        }

        chain
    };

    let dream_enemies: Vec<(String, i64)> = if character.dreaming_status == "Awake" { Vec::new() } else {
        sqlx::query!(r#"SELECT basename as base_name, basepower as base_power FROM Enemy_Types WHERE appearson = ? ORDER BY base_power ASC"#, character.dreaming_status)
            .map(|r| (r.base_name, r.base_power))
            .fetch_all(db).await?
    };

    let allies: Vec<(i64, String)> = {
        chumroll.iter()
            .filter(|c| c.id != character.id)
            .map(|c| &c.strife)
            .filter(|s| s.strife_id.is_some())
            .map(|s| (s.strife_id.unwrap(), s.name.clone()))
            .collect()
    };

    let background = if character.dreaming_status == "Awake" {
        "".to_string()
    } else {
        character
            .dreamer
            .clone()
            .ok_or(Error::ShouldHaveDreamer(character.id))?
    };

    let potential_leaders = if !main_strifer.is_leader { Vec::new() } else {
        strifers
            .iter()
            .filter(|s| s.aspect.is_some() && s.side == main_strifer.side && s.id != main_strifer.id)
            .map(|s| s.clone())
            .collect()
    };

    let strife_commands = StrifeCommandsTemplate {
        main_strifer: main_strifer.clone(),
        potential_leaders,
        actions: StrifeActionsTemplate {
            character: character.clone(),
            main_strifer: main_strifer.clone(),
            strifers: strifers.clone(),
        }
    };

    let strifers = StrifersTemplate {
        strifers,
        player_side
    };

    Ok(HtmlTemplate(StrifeDisplayTemplate {
        character,
        main_strifer,
        strifers,
        background,
        announcements: vec![],
        fraymotif_message,
        chain,
        chumroll,
        allies,
        dream_enemies,
        strife_commands
    }))
}

#[derive(Template)]
#[template(path = "strife_display.html.jinja")]
pub struct StrifeDisplayTemplate {
    pub character: Character,
    pub main_strifer: Strifer,
    pub background: String,
    pub announcements: Vec<String>,
    pub fraymotif_message: Option<StrifeFraymotifTemplate>,
    pub chain: HashMap<i64, bool>,
    pub chumroll: Vec<Character>,
    pub allies: Vec<(i64, String)>,
    pub dream_enemies: Vec<(String, i64)>,
    pub strife_commands: StrifeCommandsTemplate,
    pub strifers: StrifersTemplate,
}

#[derive(Template)]
#[template(path = "partial/strife-fraymotif.html.jinja")]
pub struct StrifeFraymotifTemplate {
    pub fraymotif: String,
    pub fraymotif_name: String,
    pub strifer_name: String,
}

#[derive(Template)]
#[template(path = "partial/strife-strifers.html.jinja")]
pub struct StrifersTemplate {
    pub strifers: Vec<Strifer>,
    pub player_side: i8
}

#[derive(Template)]
#[template(path = "partial/strife-actions.html.jinja")]
pub struct StrifeActionsTemplate {
    pub character: Character,
    pub main_strifer: Strifer,
    pub strifers: Vec<Strifer>,
}

#[derive(Clone)]
pub struct Action {
    pub active: String,
    pub passive: String,
}

#[derive(Clone)]
pub struct ActionSubmission {
    pub actions: HashMap<i64, Action>
}

impl Serialize for ActionSubmission {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer
    {
        let mut map = serializer.serialize_map(Some(self.actions.len()))?;
        for (k, v) in &self.actions {
            map.serialize_entry(format!("{}active", k).as_str(), &v.active)?;
            map.serialize_entry(format!("{}passive", k).as_str(), &v.passive)?;
        }
        map.end()
    }
}

impl<'de> Deserialize<'de> for ActionSubmission {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>
    {
        deserializer.deserialize_map(ActionSubmissionVisitor)
    }
}

struct ActionSubmissionVisitor;

impl<'de> Visitor<'de> for ActionSubmissionVisitor {
    type Value = ActionSubmission;

    fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
        formatter.write_str("a map of actions formatted as { \"<strifer_id>active\": \"string\", \"<strifer_id>passive\": \"string\", ... }")
    }

    fn visit_map<A>(self, mut map: A) -> std::result::Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        struct PartialAction {
            active: Option<String>,
            passive: Option<String>,
        }

        let mut partial_actions = HashMap::new();

        while let Some((key, value)) = map.next_entry::<String, String>()? {
            if !key.contains(':') {
                continue; // We PROBABLY don't care about this field.
            }

            let [strifer_id, action_type] = key.split(':').collect_array::<2>()
                .ok_or(serde::de::Error::unknown_field(key.as_str(), &["<strifer_id>:active", "<strifer_id>:passive"]))?;

            let strifer_id = strifer_id.parse::<i64>().map_err(|_err|
                serde::de::Error::invalid_value(Unexpected::Str(strifer_id), &self))?;

            let action = partial_actions.entry(strifer_id)
                .or_insert(PartialAction { passive: None, active: None });

            match action_type {
                "active" => action.active = Some(value),
                "passive" => action.passive = Some(value),
                _ => return Err(serde::de::Error::unknown_field(key.as_str(), &["<strifer_id>:active", "<strifer_id>:passive"]))
            };
        }

        let mut actions = HashMap::new();
        for (strifer_id, action) in partial_actions {
            actions.insert(strifer_id, match (action.active, action.passive) {
                (Some(active), Some(passive)) => Action { active, passive },
                (Some(_), None) => return Err(serde::de::Error::missing_field("<strife_id>:passive")),
                (None, Some(_)) => return Err(serde::de::Error::missing_field("<strifer_id>:active")),
                (None, None) => unreachable!(), // Should never reach here
            });
        }

        Ok(ActionSubmission { actions })
    }
}