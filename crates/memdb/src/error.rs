use json_pointer::JsonPointer;

#[derive(Debug, thiserror::Error)]
pub enum MemDbError {
    #[error("path not found: {path}")]
    PathNotFound { path: JsonPointer },
    #[error("invalid index: {index}")]
    InvalidIndex { path: JsonPointer, index: String },
    #[error("not a container: {path}")]
    NotAContainer { path: JsonPointer },
    #[error("empty path")]
    EmptyPath,
    #[error("test failed")]
    TestFailed,
}
