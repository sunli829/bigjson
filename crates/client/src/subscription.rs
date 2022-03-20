use std::{
    pin::Pin,
    task::{Context, Poll},
};

use futures_util::{Stream, StreamExt};
use serde_json::Value;
use sse_codec::Event;

use crate::{json_patch::JsonPatch, BigJsonClientError};

pub enum SubscriptionEvent {
    Value { value: Value },
    Patch { patch: Vec<JsonPatch> },
}

pub struct SubscriptionStream {
    stream: Pin<Box<dyn Stream<Item = Result<Event, sse_codec::Error>> + Send>>,
}

impl SubscriptionStream {
    pub(crate) fn new(
        stream: Pin<Box<dyn Stream<Item = Result<Event, sse_codec::Error>> + Send>>,
    ) -> Self {
        Self { stream }
    }
}

impl Stream for SubscriptionStream {
    type Item = Result<SubscriptionEvent, BigJsonClientError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = &mut *self;

        loop {
            match this.stream.poll_next_unpin(cx)? {
                Poll::Ready(Some(Event::Message { event, data, .. })) => {
                    return match event.as_str() {
                        "value" => {
                            let value = match serde_json::from_str(&data) {
                                Ok(value) => value,
                                Err(err) => return Poll::Ready(Some(Err(err.into()))),
                            };
                            Poll::Ready(Some(Ok(SubscriptionEvent::Value { value })))
                        }
                        "patch" => {
                            let patch = match serde_json::from_str(&data) {
                                Ok(value) => value,
                                Err(err) => return Poll::Ready(Some(Err(err.into()))),
                            };
                            Poll::Ready(Some(Ok(SubscriptionEvent::Patch { patch })))
                        }
                        _ => Poll::Ready(Some(Err(BigJsonClientError::UnknownEvent { event }))),
                    }
                }
                Poll::Ready(Some(Event::Retry { .. })) => {}
                Poll::Ready(None) => return Poll::Ready(None),
                Poll::Pending => return Poll::Pending,
            }
        }
    }
}
