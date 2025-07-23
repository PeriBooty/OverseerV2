use crate::broadcast::BroadcastMessage;
use crate::error::Result;
use crate::routes::character::Character;
use super::ActionSubmission;
use axum::response::IntoResponse;
use axum::{Extension, Form};
use sqlx::MySqlPool;
use tokio::sync::broadcast::Sender;

pub async fn strife_select(
    mut character: Character,
    Extension(db): Extension<MySqlPool>,
    Extension(sse): Extension<Sender<BroadcastMessage>>,
    Form(form): Form<ActionSubmission>,
) -> Result<impl IntoResponse> {

    for (strifer_id, action) in &form.actions {
        sqlx::query!(
            "UPDATE Strifers SET lastactive = ?, lastpassive = ? WHERE ID = ?",
            action.active.clone(),
            action.passive.clone(),
            strifer_id
        ).execute(&db).await?;

        if character.strife.id == *strifer_id {
            character.strife.last_active = action.active.clone();
            character.strife.last_passive = action.passive.clone();
        }

        sse.send(BroadcastMessage::StrifeActionsUpdate {
            strifer_id: *strifer_id,
            strifer_actions_string: format!("{}/{}", action.active, action.passive),
        })?;
    }

    Ok("")
}