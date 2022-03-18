use json_patch::JsonPatch;
use json_pointer::JsonPointer;
use poem::{
    error::BadRequest,
    handler,
    web::{Data, Json, Path},
    Result,
};

use crate::{state::State, subscription_patch::publish, utils::normalize_path};

#[handler]
pub(crate) async fn handler_patch(
    state: Data<&State>,
    prefix: Path<String>,
    mut patch: Json<Vec<JsonPatch>>,
) -> Result<()> {
    let prefix = normalize_path(&prefix);
    tracing::debug!(prefix = prefix.as_str(), patch_count = patch.len(), "patch");

    let prefix = prefix.parse::<JsonPointer>().map_err(BadRequest)?;
    let mut locked_state = state.locked_state.write();
    let prefix = if !prefix.as_ref().is_empty() {
        Some(prefix)
    } else {
        None
    };

    locked_state
        .mdb
        .patch(prefix.as_ref(), patch.0.clone())
        .map_err(BadRequest)?;
    publish(
        &locked_state.mdb,
        &locked_state.subscriptions,
        prefix.as_ref(),
        &patch,
    );
    if let Some(patch_sender) = &state.patch_sender {
        let _ = patch_sender.send((prefix, patch.0));
    }
    Ok(())
}
