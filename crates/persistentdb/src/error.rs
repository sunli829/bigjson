#[derive(Debug, thiserror::Error)]
pub enum PersistentDbError {
    #[error("block file is full")]
    BlockFileIsFull,
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    MemDB(#[from] memdb::MemDbError),
}
