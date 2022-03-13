use std::{collections::HashMap, sync::Arc};

use crossbeam::channel::Sender;
use json_patch::JsonPatch;
use json_pointer::JsonPointer;
use memdb::MemDb;
use parking_lot::RwLock;
use tokio::sync::broadcast::Sender as BroadcastSender;

pub(crate) type SubscriptionHashMap = HashMap<JsonPointer, BroadcastSender<Arc<[JsonPatch]>>>;

pub(crate) struct LockedState {
    pub(crate) mdb: MemDb,
    pub(crate) subscriptions: SubscriptionHashMap,
}

#[derive(Clone)]
pub(crate) struct State {
    pub(crate) locked_state: Arc<RwLock<LockedState>>,
    pub(crate) patch_sender: Option<Sender<(Option<JsonPointer>, Vec<JsonPatch>)>>,
}
