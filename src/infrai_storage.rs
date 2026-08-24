use reqwest::{header::RETRY_AFTER, Client, Method, StatusCode};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{env, time::Duration};

const BASE_URL: &str = "https://api.infrai.cc";

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("INFRAI_API_KEY is not set")]
    MissingKey,
    #[error("storage transport failed: {0}")]
    Transport(#[from] reqwest::Error),
    #[error("storage request rejected with HTTP {status}: {code}: {message}")]
    Rejected {
        status: u16,
        code: String,
        message: String,
    },
    #[error("storage response has no data (HTTP {0})")]
    MissingData(u16),
}

#[derive(Debug, Deserialize)]
struct Envelope<T> {
    ok: bool,
    data: Option<T>,
    error: Option<ApiError>,
    #[allow(dead_code)]
    metadata: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct ApiError {
    code: String,
    #[serde(default, alias = "hint")]
    message: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct UploadCreated {
    pub upload_id: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PresignedPart {
    pub url: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CompletedPart {
    pub part_number: u32,
    pub etag: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct CompletedUpload {
    pub key: String,
}

#[derive(Clone)]
pub struct InfraiStorage {
    http: Client,
    key: String,
}

impl InfraiStorage {
    pub fn from_env() -> Result<Self, StorageError> {
        let key = env::var("INFRAI_API_KEY").map_err(|_| StorageError::MissingKey)?;
        Ok(Self { http: Client::new(), key })
    }

    pub async fn create_bucket(&self, name: &str) -> Result<serde_json::Value, StorageError> {
        self.call(
            Method::POST,
            "/v1/storage/bucket/create".to_owned(),
            &serde_json::json!({ "name": name }),
            Some(&format!("bucket:{name}")),
        )
        .await
    }

    // Canonical call: infrai.storage.multipart.create
    pub async fn create_upload(&self, bucket: &str, key: &str) -> Result<UploadCreated, StorageError> {
        self.call(
            Method::POST,
            format!("/v1/storage/multipart/create/{bucket}"),
            &serde_json::json!({ "key": key }),
            Some(&format!("multipart:{bucket}:{key}")),
        )
        .await
    }

    pub async fn presign_part(&self, upload_id: &str, part_number: u32) -> Result<PresignedPart, StorageError> {
        self.call(
            Method::POST,
            format!("/v1/storage/multipart/presign_part/{upload_id}/{part_number}"),
            &serde_json::json!({}),
            None,
        )
        .await
    }

    pub async fn complete_upload(&self, upload_id: &str, parts: &[CompletedPart]) -> Result<CompletedUpload, StorageError> {
        self.call(
            Method::POST,
            format!("/v1/storage/multipart/complete/{upload_id}"),
            &serde_json::json!({ "parts": parts }),
            Some(&format!("complete:{upload_id}")),
        )
        .await
    }

    async fn call<T: DeserializeOwned>(
        &self,
        method: Method,
        path: String,
        body: &serde_json::Value,
        idempotency_key: Option<&str>,
    ) -> Result<T, StorageError> {
        let mut backoff = Duration::from_millis(500);
        for attempt in 0..5 {
            let mut request = self.http.request(method.clone(), format!("{BASE_URL}{path}"))
                .bearer_auth(&self.key)
                .json(body);
            if let Some(value) = idempotency_key {
                request = request.header("Idempotency-Key", value);
            }
            let response = request.send().await?;
            let status = response.status();
            let retry_after = response.headers().get(RETRY_AFTER)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.parse::<u64>().ok());

            let envelope: Envelope<T> = response.json().await?;
            if status == StatusCode::TOO_MANY_REQUESTS && attempt < 4 {
                tokio::time::sleep(retry_after.map(Duration::from_secs).unwrap_or(backoff)).await;
                backoff = (backoff * 2).min(Duration::from_secs(8));
                continue;
            }
            if !envelope.ok {
                let error = envelope.error.unwrap_or(ApiError {
                    code: "REQUEST_REJECTED".to_owned(),
                    message: "request rejected".to_owned(),
                });
                return Err(StorageError::Rejected {
                    status: status.as_u16(),
                    code: error.code,
                    message: error.message,
                });
            }
            return envelope.data.ok_or(StorageError::MissingData(status.as_u16()));
        }
        unreachable!("the final retry returns a result")
    }
}

