use json_patch::JsonPatch;
use json_pointer::JsonPointer;
use poem::{
    error::BadRequest,
    handler,
    web::{Data, Json, Path},
    Result,
};
use serde_json::Value;

use crate::{state::State, subscription_patch::publish, utils::normalize_path};

#[handler]
pub(crate) fn handler_put(
    state: Data<&State>,
    path: Path<String>,
    value: Json<Value>,
) -> Result<()> {
    let path = normalize_path(&path);
    tracing::debug!(path = path.as_str(), "post");

    let path = path.parse::<JsonPointer>().map_err(BadRequest)?;
    let mut locked_state = state.locked_state.write();
    let patch = vec![JsonPatch::Replace {
        path,
        value: value.0,
    }];
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
