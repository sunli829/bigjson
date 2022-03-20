#[derive(Debug, thiserror::Error)]
pub enum BigJsonClientError {
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Http(#[from] reqwest::Error),
    #[error(transparent)]
    SSE(#[from] sse_codec::Error),
    #[error("unknown event: `{event}`")]
    UnknownEvent { event: String },
    #[error("test failed")]
    TestFailed,
}
