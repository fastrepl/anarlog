use super::*;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Receipt {
    sequence: u64,
    record_id: String,
    payload_hash: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Acceptance {
    initialized_at: String,
    head_sequence: u64,
    cloud_authority_after: u64,
    mutation_id: String,
    receipts: Vec<Receipt>,
}

impl E2eeWitnessClient {
    pub(super) fn accepted_endpoint(&self) -> reqwest::Url {
        let mut endpoint = self.endpoint.clone();
        endpoint
            .path_segments_mut()
            .expect("validated HTTP URL")
            .push("accepted");
        endpoint
    }

    pub(super) async fn configure_authority(
        &self,
        pool: &sqlx::SqlitePool,
        page: &ReadPage,
    ) -> io::Result<()> {
        if page.accepted {
            if page.head_sequence
                < anlg_db_app::e2ee_cloud_accepted_head(pool, &self.workspace_id)
                    .await
                    .map_err(replica_error)?
            {
                return Err(rollback_error());
            }
            anlg_db_app::configure_e2ee_cloud_authority(
                pool,
                &self.workspace_id,
                page.cloud_authority_after,
            )
            .await
            .map_err(replica_error)
        } else if anlg_db_app::e2ee_cloud_authority_enabled(pool, &self.workspace_id)
            .await
            .map_err(replica_error)?
        {
            Err(invalid_data(
                "Cloud-authoritative sync cannot fall back to a legacy server",
            ))
        } else {
            Ok(())
        }
    }

    pub(super) async fn publish_accepted(
        &self,
        pool: &sqlx::SqlitePool,
        keyring: &anlg_e2ee::WorkspaceKeyring,
        initialize: bool,
        cancellation: &E2eeWitnessCancellation,
    ) -> io::Result<()> {
        let mut conflicts = 0;
        loop {
            cancellation.check()?;
            anlg_db_app::rebase_e2ee_cloud_documents(pool, &self.workspace_id, keyring)
                .await
                .map_err(replica_error)?;
            cancellation.check()?;
            let Some(batch) =
                anlg_db_app::pending_e2ee_cloud_batch(pool, &self.workspace_id, initialize)
                    .await
                    .map_err(replica_error)?
            else {
                return Ok(());
            };
            cancellation.check()?;
            anlg_db_app::preserve_e2ee_cloud_conflicts(pool, &self.workspace_id, keyring, &batch)
                .await
                .map_err(replica_error)?;
            cancellation.check()?;
            let response = self
                .send_with_rate_limit_retry(
                    || {
                        self.client
                            .post(self.accepted_endpoint())
                            .bearer_auth(&self.access_token)
                            .json(&batch)
                    },
                    cancellation,
                )
                .await?;
            let status = response.status();
            let bytes = cancellation.run_network(read_bounded(response)).await??;
            if status == reqwest::StatusCode::CONFLICT
                && serde_json::from_slice::<serde_json::Value>(&bytes)
                    .is_ok_and(|v| v["error"]["code"] == "e2ee_replica_base_changed")
            {
                anlg_db_app::reject_e2ee_cloud_batch(pool, &self.workspace_id, &batch.mutation_id)
                    .await
                    .map_err(replica_error)?;
                self.refresh_keyring_cancellable(pool, keyring, cancellation)
                    .await?;
                conflicts += 1;
                if conflicts >= 3 {
                    return Err(io::Error::other(
                        "Cloud state kept changing; pending edits will retry",
                    ));
                }
                continue;
            }
            if !status.is_success() {
                return Err(io::Error::other(format!(
                    "Cloud acceptance was rejected with status {status}"
                )));
            }
            let receipt: Acceptance = serde_json::from_slice(&bytes)
                .map_err(|_| invalid_data("Invalid cloud acceptance receipt"))?;
            if receipt.initialized_at.is_empty()
                || receipt.mutation_id != batch.mutation_id
                || receipt.head_sequence <= batch.base_sequence
                || receipt.cloud_authority_after >= receipt.head_sequence
                || receipt.receipts.len() != batch.events.len()
                || receipt
                    .receipts
                    .windows(2)
                    .any(|p| p[0].sequence >= p[1].sequence)
                || receipt
                    .receipts
                    .last()
                    .is_none_or(|r| r.sequence != receipt.head_sequence)
                || batch.events.iter().any(|e| {
                    receipt
                        .receipts
                        .iter()
                        .filter(|r| {
                            r.record_id == e.record_id
                                && r.payload_hash == e.payload_hash
                                && r.sequence > batch.base_sequence
                        })
                        .count()
                        != 1
                })
            {
                return Err(invalid_data(
                    "Cloud acceptance receipt does not match the pending batch",
                ));
            }
            cancellation.check()?;
            anlg_db_app::configure_e2ee_cloud_authority(
                pool,
                &self.workspace_id,
                Some(receipt.cloud_authority_after),
            )
            .await
            .map_err(replica_error)?;
            let events = receipt
                .receipts
                .into_iter()
                .map(|r| {
                    let event = batch
                        .events
                        .iter()
                        .find(|e| e.record_id == r.record_id)
                        .expect("validated receipt");
                    anlg_db_app::E2eeWitnessEvent {
                        sequence: r.sequence,
                        record_id: r.record_id,
                        workspace_id: self.workspace_id.clone(),
                        payload_hash: r.payload_hash,
                        payload: event.payload.clone(),
                    }
                })
                .collect::<Vec<_>>();
            anlg_db_app::merge_e2ee_witness_events_with_keyring_cancellable(
                pool,
                keyring,
                &self.workspace_id,
                &events,
                || cancellation.is_cancelled(),
            )
            .await
            .map_err(replica_error)?;
            cancellation.check()?;
            anlg_db_app::acknowledge_e2ee_cloud_batch(pool, &self.workspace_id, &batch)
                .await
                .map_err(replica_error)?;
            self.refresh_keyring_cancellable(pool, keyring, cancellation)
                .await?;
        }
    }
}
