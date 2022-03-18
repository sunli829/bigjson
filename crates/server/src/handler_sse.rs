use std::sync::Arc;

use json_patch::JsonPatch;
use json_pointer::JsonPointer;
use poem::{
    error::BadRequest,
    handler,
    web::{
        sse::{Event, SSE},
        Data, Path,
    },
    Result,
};
use serde_json::Value;
use tokio_stream::StreamExt;

use crate::{state::State, utils::normalize_path};

#[handler]
pub(crate) async fn handler_sse(state: Data<&State>, path: Path<String>) -> Result<SSE> {
    let path = normalize_path(&path);
    tracing::debug!(path = path.as_str(), "subscribe");

    let path = path.parse::<JsonPointer>().map_err(BadRequest)?;
    let mut locked_state = state.locked_state.write();
    let value = locked_state.mdb.get(&path).cloned().unwrap_or(Value::Null);

    let receiver = locked_state
        .subscriptions
        .entry(path)
        .or_insert_with(|| {
            let (sender, _) = tokio::sync::broadcast::channel(64);
            sender
        })
        .subscribe();

    let first_patch: Arc<[JsonPatch]> = vec![JsonPatch::Add {
        path: JsonPointer::root(),
        value,
    }]
    .into();
    let stream = tokio_stream::once(Ok(first_patch))
        .chain(tokio_stream::wrappers::BroadcastStream::new(receiver))
        .take_while(|res| res.is_ok())
        .map(Result::unwrap)
        .map(|patch| Event::message(serde_json::to_string(&*patch).unwrap()).event_type("patch"));

    Ok(SSE::new(stream))
}
