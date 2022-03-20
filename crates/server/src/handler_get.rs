use json_pointer::JsonPointer;
use poem::{
    error::{BadRequest, InternalServerError},
    handler,
    web::{Data, Path},
    Result,
};
use serde_json::Value;

use crate::{state::State, utils::normalize_path};

#[handler]
pub(crate) async fn handler_get(state: Data<&State>, path: Path<String>) -> Result<String> {
    let path = normalize_path(&path);
    tracing::debug!(path = path.as_str(), "get");

    let path = path.parse::<JsonPointer>().map_err(BadRequest)?;
    let locked_state = state.locked_state.read();
    let value = locked_state.mdb.get(&path).unwrap_or(&Value::Null);
    let value_str = serde_json::to_string(&value).map_err(InternalServerError)?;

    Ok(value_str)
}
