use super::*;

pub(super) const SNAPSHOT_BOOTSTRAP_MIN_GAP: u64 = 1024;
pub(crate) const SNAPSHOT_PUBLISH_MIN_DELTA: u64 = 2000;
const MAX_SNAPSHOT_DOWNLOAD_BYTES: usize = anlg_e2ee::MAX_SEALED_SNAPSHOT_BYTES;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SnapshotResponse {
    snapshot: Option<SnapshotDescriptor>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SnapshotDescriptor {
    snapshot_sequence: u64,
    format_version: u16,
    payload_bytes: u64,
    payload_sha256: String,
    #[allow(dead_code)]
    record_count: u32,
    #[allow(dead_code)]
    key_id: String,
    #[allow(dead_code)]
    head_sequence: u64,
    #[allow(dead_code)]
    cloud_authority_after: Option<u64>,
    download_url: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PrepareSnapshotUploadRequest<'a> {
    snapshot_sequence: u64,
    payload_bytes: u64,
    payload_sha256: &'a str,
    record_count: u32,
    key_id: &'a str,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PrepareSnapshotUploadResponse {
    object_key: String,
    upload_url: String,
    #[allow(dead_code)]
    token: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CommitSnapshotRequest<'a> {
    snapshot_sequence: u64,
    object_key: &'a str,
    format_version: u16,
    payload_bytes: u64,
    payload_sha256: &'a str,
    record_count: u32,
    key_id: &'a str,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CommitSnapshotResponse {
    #[allow(dead_code)]
    committed: bool,
    #[allow(dead_code)]
    snapshot_sequence: u64,
}

impl E2eeWitnessClient {
    pub(super) fn snapshot_endpoint(&self) -> reqwest::Url {
        let mut endpoint = self.endpoint.clone();
        endpoint
            .path_segments_mut()
            .expect("validated HTTP URL")
            .push("snapshot");
        endpoint
    }

    pub(super) fn snapshot_upload_endpoint(&self) -> reqwest::Url {
        let mut endpoint = self.endpoint.clone();
        endpoint
            .path_segments_mut()
            .expect("validated HTTP URL")
            .extend(["snapshot", "upload"]);
        endpoint
    }

    pub(super) fn snapshot_commit_endpoint(&self) -> reqwest::Url {
        let mut endpoint = self.endpoint.clone();
        endpoint
            .path_segments_mut()
            .expect("validated HTTP URL")
            .extend(["snapshot", "commit"]);
        endpoint
    }

    pub(super) async fn try_bootstrap_from_snapshot_cancellable(
        &self,
        pool: &sqlx::SqlitePool,
        keyring: &anlg_e2ee::WorkspaceKeyring,
        cancellation: &E2eeWitnessCancellation,
    ) -> io::Result<Option<u64>> {
        let cursor = witness_cursor_cancellable(pool, &self.workspace_id, cancellation).await?;
        let endpoint = self.snapshot_endpoint();
        let response = self
            .send_with_rate_limit_retry(
                || {
                    self.client
                        .get(endpoint.clone())
                        .bearer_auth(&self.access_token)
                },
                cancellation,
            )
            .await?;
        let status = response.status();
        let bytes = cancellation.run_network(read_bounded(response)).await??;
        if status == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !status.is_success() {
            return Err(io::Error::other(format!(
                "E2EE snapshot read was rejected with status {status}"
            )));
        }
        let descriptor: SnapshotResponse = serde_json::from_slice(&bytes)
            .map_err(|_| invalid_data("Invalid E2EE snapshot response"))?;
        let Some(desc) = descriptor.snapshot else {
            return Ok(None);
        };
        if desc.format_version != anlg_e2ee::SNAPSHOT_FORMAT_VERSION
            || desc.snapshot_sequence <= cursor
        {
            return Ok(None);
        }

        let download_response = cancellation
            .run_network(self.client.get(&desc.download_url).send())
            .await?
            .map_err(transport_error)?;
        if !download_response.status().is_success() {
            return Err(io::Error::other(format!(
                "E2EE snapshot download was rejected with status {}",
                download_response.status()
            )));
        }
        let snapshot_bytes = cancellation
            .run_network(read_bounded_with_limit(
                download_response,
                MAX_SNAPSHOT_DOWNLOAD_BYTES,
            ))
            .await??;
        if snapshot_bytes.len() as u64 != desc.payload_bytes
            || anlg_e2ee::bytes_sha256(&snapshot_bytes) != desc.payload_sha256
        {
            return Err(invalid_data("E2EE snapshot checksum or size mismatch"));
        }

        let payload = anlg_e2ee::open_snapshot(
            keyring,
            &self.workspace_id,
            desc.snapshot_sequence,
            &snapshot_bytes,
        )
        .map_err(|_| invalid_data("Invalid encrypted E2EE snapshot"))?;

        anlg_db_app::import_e2ee_workspace_snapshot_cancellable(pool, keyring, &payload, || {
            cancellation.is_cancelled()
        })
        .await
        .map_err(replica_error)?;
        Ok(Some(desc.snapshot_sequence))
    }

    pub async fn maybe_publish_snapshot_cancellable(
        &self,
        pool: &sqlx::SqlitePool,
        keyring: &anlg_e2ee::WorkspaceKeyring,
        min_delta_events: u64,
        last_published_sequence: Option<u64>,
        cancellation: &E2eeWitnessCancellation,
    ) -> io::Result<Option<u64>> {
        let cursor = witness_cursor_cancellable(pool, &self.workspace_id, cancellation).await?;
        if cursor == 0
            || cursor.saturating_sub(last_published_sequence.unwrap_or(0)) < min_delta_events
        {
            return Ok(None);
        }
        if last_published_sequence.is_none() {
            let endpoint = self.snapshot_endpoint();
            let response = self
                .send_with_rate_limit_retry(
                    || {
                        self.client
                            .get(endpoint.clone())
                            .bearer_auth(&self.access_token)
                    },
                    cancellation,
                )
                .await?;
            let status = response.status();
            let bytes = cancellation.run_network(read_bounded(response)).await??;
            if status == reqwest::StatusCode::NOT_FOUND {
                return Ok(None);
            }
            if !status.is_success() {
                return Err(io::Error::other(format!(
                    "E2EE snapshot status check was rejected with status {status}"
                )));
            }
            let descriptor: SnapshotResponse = serde_json::from_slice(&bytes)
                .map_err(|_| invalid_data("Invalid E2EE snapshot response"))?;
            if let Some(desc) = descriptor.snapshot
                && cursor.saturating_sub(desc.snapshot_sequence) < min_delta_events
            {
                return Ok(Some(desc.snapshot_sequence));
            }
        }
        let Some(payload) = anlg_db_app::export_e2ee_workspace_snapshot(pool, &self.workspace_id)
            .await
            .map_err(replica_error)?
        else {
            return Ok(None);
        };
        let active_key = keyring.active();
        let sealed = match anlg_e2ee::seal_snapshot(active_key, &payload) {
            Ok(bytes) if bytes.len() <= MAX_SNAPSHOT_DOWNLOAD_BYTES => bytes,
            Ok(bytes) => {
                tracing::warn!(
                    workspace_id = self.workspace_id,
                    snapshot_sequence = payload.snapshot_sequence,
                    sealed_bytes = bytes.len(),
                    max_bytes = MAX_SNAPSHOT_DOWNLOAD_BYTES,
                    "skipping E2EE snapshot publication because sealed payload exceeds download limit"
                );
                return Ok(None);
            }
            Err(anlg_e2ee::Error::InvalidPayload) => {
                tracing::warn!(
                    workspace_id = self.workspace_id,
                    snapshot_sequence = payload.snapshot_sequence,
                    "skipping E2EE snapshot publication because plaintext exceeds snapshot limit"
                );
                return Ok(None);
            }
            Err(_) => return Err(invalid_data("Could not encrypt E2EE snapshot")),
        };
        let payload_bytes =
            u64::try_from(sealed.len()).map_err(|_| invalid_data("Snapshot size overflow"))?;
        let payload_sha256 = anlg_e2ee::bytes_sha256(&sealed);
        let record_count = u32::try_from(payload.records.len())
            .map_err(|_| invalid_data("Snapshot record count overflow"))?;

        let prepare_endpoint = self.snapshot_upload_endpoint();
        let response = self
            .send_with_rate_limit_retry(
                || {
                    self.client
                        .post(prepare_endpoint.clone())
                        .bearer_auth(&self.access_token)
                        .json(&PrepareSnapshotUploadRequest {
                            snapshot_sequence: payload.snapshot_sequence,
                            payload_bytes,
                            payload_sha256: &payload_sha256,
                            record_count,
                            key_id: active_key.key_id(),
                        })
                },
                cancellation,
            )
            .await?;
        let status = response.status();
        let bytes = cancellation.run_network(read_bounded(response)).await??;
        if status == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !status.is_success() {
            return Err(io::Error::other(format!(
                "E2EE snapshot upload preparation was rejected with status {status}"
            )));
        }
        let upload: PrepareSnapshotUploadResponse = serde_json::from_slice(&bytes)
            .map_err(|_| invalid_data("Invalid E2EE snapshot upload preparation response"))?;

        let put_response = cancellation
            .run_network(
                self.client
                    .put(&upload.upload_url)
                    .header(reqwest::header::CONTENT_TYPE, "application/octet-stream")
                    .body(sealed)
                    .send(),
            )
            .await?
            .map_err(transport_error)?;
        let put_status = put_response.status();
        let _ = cancellation.run_network(read_bounded(put_response)).await?;
        if !put_status.is_success() {
            return Err(io::Error::other(format!(
                "E2EE snapshot upload failed with status {put_status}"
            )));
        }

        let commit_endpoint = self.snapshot_commit_endpoint();
        let commit_response = self
            .send_with_rate_limit_retry(
                || {
                    self.client
                        .post(commit_endpoint.clone())
                        .bearer_auth(&self.access_token)
                        .json(&CommitSnapshotRequest {
                            snapshot_sequence: payload.snapshot_sequence,
                            object_key: &upload.object_key,
                            format_version: anlg_e2ee::SNAPSHOT_FORMAT_VERSION,
                            payload_bytes,
                            payload_sha256: &payload_sha256,
                            record_count,
                            key_id: active_key.key_id(),
                        })
                },
                cancellation,
            )
            .await?;
        let commit_status = commit_response.status();
        let commit_bytes = cancellation
            .run_network(read_bounded(commit_response))
            .await??;
        if !commit_status.is_success() {
            return Err(io::Error::other(format!(
                "E2EE snapshot commit was rejected with status {commit_status}"
            )));
        }
        let _committed: CommitSnapshotResponse = serde_json::from_slice(&commit_bytes)
            .map_err(|_| invalid_data("Invalid E2EE snapshot commit response"))?;
        Ok(Some(payload.snapshot_sequence))
    }
}

async fn read_bounded_with_limit(
    response: reqwest::Response,
    max_bytes: usize,
) -> io::Result<Vec<u8>> {
    if response
        .content_length()
        .is_some_and(|length| length > max_bytes as u64)
    {
        return Err(invalid_data("E2EE snapshot response is too large"));
    }
    let mut stream = response.bytes_stream();
    let mut bytes = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(transport_error)?;
        if bytes.len().saturating_add(chunk.len()) > max_bytes {
            return Err(invalid_data("E2EE snapshot response exceeded maximum size"));
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}
