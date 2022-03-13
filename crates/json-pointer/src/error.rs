#[derive(Debug, thiserror::Error, Copy, Clone, Eq, PartialEq)]
#[error("invalid json pointer")]
pub struct ParseJsonPointerError;
