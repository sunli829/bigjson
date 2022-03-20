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

    let first_item = Event::message(serde_json::to_string(&value).unwrap()).event_type("value");
    let stream = tokio_stream::once(first_item).chain(
        tokio_stream::wrappers::BroadcastStream::new(receiver)
            .take_while(|res| res.is_ok())
            .map(Result::unwrap)
            .map(|patch| {
                Event::message(serde_json::to_string(&*patch).unwrap()).event_type("patch")
            }),
    );
    Ok(SSE::new(stream))
}
