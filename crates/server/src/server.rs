use std::{
    future::Future,
    io::Result as IoResult,
    sync::Arc,
    time::{Duration, Instant},
};

use crossbeam::channel::Receiver;
use json_patch::JsonPatch;
use json_pointer::JsonPointer;
use memdb::MemDb;
use parking_lot::RwLock;
use persistentdb::{PersistentDb, PersistentDbError};
use poem::{
    endpoint::make_sync,
    get,
    listener::TcpListener,
    middleware::{NormalizePath, TrailingSlash},
    EndpointExt, Route, Server,
};

use crate::{
    handler_delete::handler_delete,
    handler_get::handler_get,
    handler_patch::handler_patch,
    handler_post::handler_post,
    handler_put::handler_put,
    handler_sse::handler_sse,
    handler_ws::handler_ws,
    state::{LockedState, State},
    ServerConfig,
};

pub fn create_server(
    config: ServerConfig,
) -> Result<impl Future<Output = IoResult<()>>, PersistentDbError> {
    let (mdb, tx) = if let Some(data_dir) = &config.data_dir {
        let pdb = PersistentDb::open(data_dir)?;
        let memdb = pdb.create_memdb()?;
        let (tx, rx) = crossbeam::channel::unbounded();
        pdb.compact();
        std::thread::spawn(move || sync_loop(rx, pdb));
        (memdb, Some(tx))
    } else {
        (MemDb::default(), None)
    };

    let routes = Route::new()
        .nest(
            "/data",
            Route::new().at(
                "/*path",
                get(handler_get)
                    .post(handler_post)
                    .put(handler_put)
                    .patch(handler_patch)
                    .delete(handler_delete),
            ),
        )
        .nest("/sse", Route::new().at("/*path", handler_sse))
        .at("/ws", get(handler_ws))
        .at("/health", get(make_sync(|_| "OK")))
        .with(NormalizePath::new(TrailingSlash::Trim))
        .data(State {
            locked_state: Arc::new(RwLock::new(LockedState {
                mdb,
                subscriptions: Default::default(),
            })),
            patch_sender: tx,
        });

    tracing::info!(bind = config.bind.as_str(), "listening");
    let server = Server::new(TcpListener::bind(config.bind));
    Ok(server.run(routes))
}

fn sync_loop(rx: Receiver<(Option<JsonPointer>, Vec<JsonPatch>)>, mut pdb: PersistentDb) {
    let mut prev_compact_at = Instant::now();
    let compact_interval = Duration::from_secs(60 * 30);

    for (prefix, patch) in rx {
        loop {
            match pdb.append(prefix.as_ref(), &patch, false) {
                Ok(()) => break,
                Err(err) => {
                    tracing::error!(error = %err, "failed to write data");
                    std::thread::sleep(Duration::from_secs(5));
                }
            }
        }

        if Instant::now() - prev_compact_at > compact_interval {
            pdb.compact();
            prev_compact_at = Instant::now();
        }
    }
}
