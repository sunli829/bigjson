use futures_util::TryStreamExt;
use reqwest::{Client, StatusCode};
use serde::{de::DeserializeOwned, Serialize};
use std::io::ErrorKind;
use tokio_util::compat::TokioAsyncReadCompatExt;

use crate::{json_patch::JsonPatch, Batch, BigJsonClientError, SubscriptionStream};

pub struct BigJsonClient {
    server_url: String,
    client: Client,
}

impl BigJsonClient {
    pub fn new(server_url: impl Into<String>) -> Self {
        Self {
            server_url: server_url.into(),
            client: Client::new(),
        }
    }

    pub async fn get<T: DeserializeOwned>(
        &self,
        path: impl AsRef<str>,
    ) -> Result<T, BigJsonClientError> {
        Ok(self
            .client
            .get(format!("{}/data{}", self.server_url, path.as_ref()))
            .send()
            .await?
            .error_for_status()?
            .json::<T>()
            .await?)
    }

    pub async fn add<T: Serialize>(
        &self,
        path: impl AsRef<str>,
        value: &T,
    ) -> Result<(), BigJsonClientError> {
        self.client
            .post(format!("{}/data{}", self.server_url, path.as_ref()))
            .json(&value)
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    pub async fn replace<T: Serialize>(
        &self,
        path: impl AsRef<str>,
        value: &T,
    ) -> Result<(), BigJsonClientError> {
        self.client
            .put(format!("{}/data{}", self.server_url, path.as_ref()))
            .json(&value)
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    pub async fn remove(&self, path: impl AsRef<str>) -> Result<(), BigJsonClientError> {
        self.client
            .delete(format!("{}/data{}", self.server_url, path.as_ref()))
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    pub async fn move_to<T: Serialize>(
        &self,
        from: impl Into<String>,
        path: impl Into<String>,
    ) -> Result<(), BigJsonClientError> {
        self.client
            .patch(format!("{}/data", self.server_url))
            .json(&[JsonPatch::Move {
                from: from.into(),
                path: path.into(),
            }])
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    pub async fn copy_to<T: Serialize>(
        &self,
        from: impl Into<String>,
        path: impl Into<String>,
    ) -> Result<(), BigJsonClientError> {
        self.client
            .patch(format!("{}/data", self.server_url))
            .json(&[JsonPatch::Copy {
                from: from.into(),
                path: path.into(),
            }])
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    pub async fn batch(&self, batch: Batch) -> Result<(), BigJsonClientError> {
        let resp = self
            .client
            .patch(format!("{}/data", self.server_url))
            .json(&batch.res?)
            .send()
            .await?;
        if resp.status() == StatusCode::PRECONDITION_FAILED {
            return Err(BigJsonClientError::TestFailed);
        }
        resp.error_for_status()?;
        Ok(())
    }

    pub async fn subscribe(
        &self,
        path: impl AsRef<str>,
    ) -> Result<SubscriptionStream, BigJsonClientError> {
        let resp = self
            .client
            .get(format!("{}/sse{}", self.server_url, path.as_ref()))
            .send()
            .await?
            .error_for_status()?;
        let stream = sse_codec::decode_stream(
            tokio_util::io::StreamReader::new(
                resp.bytes_stream()
                    .map_err(|err| std::io::Error::new(ErrorKind::Other, err.to_string())),
            )
            .compat(),
        );
        Ok(SubscriptionStream::new(Box::pin(stream)))
    }
}
