use std::{collections::HashMap, sync::Arc};

use futures_util::{stream::SplitSink, Sink, SinkExt, StreamExt};
use json_patch::JsonPatch;
use json_pointer::JsonPointer;
use poem::{
    handler,
    web::{
        websocket::{Message, WebSocket, WebSocketStream},
        Data,
    },
    IntoResponse,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::{
    broadcast::error::RecvError as BroadcastRecvError, mpsc, mpsc::UnboundedSender, oneshot,
};

use crate::{state::State, subscription_patch::publish};

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum ClientRequest {
    Subscribe {
        id: String,
        path: JsonPointer,
    },
    Unsubscribe {
        id: String,
    },
    Get {
        id: String,
        path: JsonPointer,
    },
    Patch {
        id: String,
        prefix: Option<JsonPointer>,
        patch: Vec<JsonPatch>,
    },
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum ServerResponse<'a> {
    Patch {
        id: &'a str,
        patch: &'a [JsonPatch],
    },
    Complete {
        id: &'a str,
    },
    Response {
        id: &'a str,
        #[serde(skip_serializing_if = "Option::is_none")]
        value: Option<&'a Value>,
    },
    Error {
        id: &'a str,
        message: &'a str,
    },
}

type PatchSender = UnboundedSender<Result<(Arc<str>, Arc<[JsonPatch]>), BroadcastRecvError>>;

struct ClientState {
    state: State,
    subscriptions: HashMap<Arc<str>, oneshot::Sender<()>>,
    sink: SplitSink<WebSocketStream, Message>,
    patch_tx: PatchSender,
}

#[handler]
pub(crate) async fn handler_ws(state: Data<&State>, ws: WebSocket) -> impl IntoResponse {
    let state = state.0.clone();

    ws.protocols(["bigjson"])
        .on_upgrade(move |socket| async move {
            let (sink, mut stream) = socket.split();
            let (patch_tx, mut patch_rx) = mpsc::unbounded_channel();
            let mut client_state = ClientState {
                state,
                subscriptions: HashMap::new(),
                sink,
                patch_tx,
            };

            loop {
                tokio::select! {
                    item = stream.next() => {
                        match item {
                            Some(Ok(msg)) => {
                                match serde_json::from_slice::<ClientRequest>(msg.as_bytes()) {
                                    Ok(req) => handle_client_request(&mut client_state, req).await,
                                    Err(_) => {
                                        // bad request
                                        break;
                                    }
                                }
                            }
                            None | Some(Err(_)) => {
                                // client closed
                                break;
                            }
                        }
                    }
                    item = patch_rx.recv() => {
                        match item {
                            Some(Ok((id, patch))) => {
                                if !client_state.subscriptions.contains_key(&id) {
                                    continue;
                                }

                                if send_response(
                                    &mut client_state.sink,
                                    ServerResponse::Patch {
                                        id: &id,
                                        patch: &patch,
                                    },
                                )
                                .await.is_err() {
                                    // client closed
                                    break;
                                }
                            }
                            Some(Err(_)) => {
                                // client to slow
                                break;
                            }
                            None => {
                                // unreachable error
                                break;
                            }
                        }
                    }
                }
            }

            for mut cancel_tx in client_state.subscriptions.into_values() {
                let _ = cancel_tx.send(());
            }
        })
}

async fn send_response<T: Sink<Message> + Unpin>(
    mut sink: T,
    resp: ServerResponse<'_>,
) -> Result<(), T::Error> {
    let data = Message::text(serde_json::to_string(&resp).unwrap());
    sink.send(data).await
}

async fn handle_client_request_subscribe(
    client_state: &mut ClientState,
    id: String,
    path: JsonPointer,
) {
    if client_state.subscriptions.contains_key(&*id) {
        let _ = send_response(
            &mut client_state.sink,
            ServerResponse::Error {
                id: &id,
                message: &format!("duplicate operation id: '{}'", id),
            },
        )
        .await;
        return;
    }

    let (id, value, mut receiver, patch_tx, cancel_tx, mut cancel_rx) = {
        let mut locked_state = client_state.state.locked_state.write();
        let value = locked_state.mdb.get(&path).cloned().unwrap_or(Value::Null);
        let receiver = locked_state
            .subscriptions
            .entry(path)
            .or_insert_with(|| {
                let (sender, _) = tokio::sync::broadcast::channel(64);
                sender
            })
            .subscribe();
        let patch_tx = client_state.patch_tx.clone();
        let (cancel_tx, cancel_rx) = oneshot::channel();
        let id: Arc<str> = id.into();
        (id, value, receiver, patch_tx, cancel_tx, cancel_rx)
    };

    client_state.subscriptions.insert(id.clone(), cancel_tx);
    let _ = send_response(
        &mut client_state.sink,
        ServerResponse::Patch {
            id: &id,
            patch: &[JsonPatch::Add {
                path: JsonPointer::root(),
                value,
            }],
        },
    )
    .await;

    tokio::spawn(async move {
        loop {
            tokio::select! {
                res = receiver.recv() => {
                    if patch_tx.send(res.map(|patch| (id.clone(), patch))).is_err() {
                        break;
                    }
                }
                _ = &mut cancel_rx => break,
            }
        }
    });
}

async fn handle_client_request_unsubscribe(client_state: &mut ClientState, id: String) {
    let cancel_tx = match client_state.subscriptions.remove(&*id) {
        Some(cancel_tx) => cancel_tx,
        None => {
            let _ = send_response(
                &mut client_state.sink,
                ServerResponse::Error {
                    id: &id,
                    message: &format!("operation id does not exists: '{}'", id),
                },
            )
            .await;
            return;
        }
    };

    let _ = cancel_tx.send(());
    let _ = send_response(&mut client_state.sink, ServerResponse::Complete { id: &id }).await;
}

async fn handle_client_request_get(client_state: &mut ClientState, id: String, path: JsonPointer) {
    let value = {
        let locked_state = client_state.state.locked_state.read();
        locked_state.mdb.get(path).cloned().unwrap_or_default()
    };
    let _ = send_response(
        &mut client_state.sink,
        ServerResponse::Response {
            id: &id,
            value: Some(&value),
        },
    )
    .await;
}

async fn handle_client_request_patch(
    client_state: &mut ClientState,
    id: String,
    prefix: Option<JsonPointer>,
    patch: Vec<JsonPatch>,
) {
    let res = {
        let mut locked_state = client_state.state.locked_state.write();
        match locked_state.mdb.patch(prefix.as_ref(), patch.clone()) {
            Ok(()) => {
                publish(
                    &locked_state.mdb,
                    &locked_state.subscriptions,
                    prefix.as_ref(),
                    &patch,
                );
                if let Some(patch_sender) = &client_state.state.patch_sender {
                    let _ = patch_sender.send((prefix, patch));
                }
                Ok(())
            }
            Err(err) => Err(err),
        }
    };

    match res {
        Ok(()) => {
            let _ = send_response(
                &mut client_state.sink,
                ServerResponse::Response {
                    id: &id,
                    value: None,
                },
            )
            .await;
        }
        Err(err) => {
            let _ = send_response(
                &mut client_state.sink,
                ServerResponse::Error {
                    id: &id,
                    message: &err.to_string(),
                },
            )
            .await;
        }
    }
}

async fn handle_client_request(client_state: &mut ClientState, req: ClientRequest) {
    match req {
        ClientRequest::Subscribe { id, path } => {
            handle_client_request_subscribe(client_state, id, path).await
        }
        ClientRequest::Unsubscribe { id } => {
            handle_client_request_unsubscribe(client_state, id).await
        }
        ClientRequest::Get { id, path } => handle_client_request_get(client_state, id, path).await,
        ClientRequest::Patch { id, prefix, patch } => {
            handle_client_request_patch(client_state, id, prefix, patch).await
        }
    }
}
