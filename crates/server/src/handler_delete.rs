use json_patch::JsonPatch;
use json_pointer::JsonPointer;
use poem::{
    error::BadRequest,
    handler,
    web::{Data, Path},
    Result,
};

use crate::{state::State, subscription_patch::publish, utils::normalize_path};

#[handler]
pub(crate) async fn handler_delete(state: Data<&State>, path: Path<String>) -> Result<()> {
    let path = normalize_path(&path);
    tracing::debug!(path = path.as_str(), "delete");

    let mut locked_state = state.locked_state.write();
    let path = path.parse::<JsonPointer>().map_err(BadRequest)?;
    let patch = vec![JsonPatch::Remove { path }];

    locked_state
        .mdb
        .patch(None, patch.clone())
        .map_err(BadRequest)?;
    publish(&locked_state.mdb, &locked_state.subscriptions, None, &patch);
    if let Some(patch_sender) = &state.patch_sender {
        let _ = patch_sender.send((None, patch));
    }
    Ok(())
}
