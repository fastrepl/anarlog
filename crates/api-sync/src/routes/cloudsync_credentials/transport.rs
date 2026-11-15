use axum::http::HeaderMap;
use serde::Deserialize;

use super::SUPABASE_REQUEST_TIMEOUT;
use crate::{
    config::CloudsyncTransport,
    error::{Result, SyncError},
    state::ReplicaState,
};

/// Desktop advertises the transports it can accept from `/sync/token`. Older
/// builds omit the header and always receive sqlite-sync credentials, so a
/// server-side flip never strands a client that cannot run the replica path.
pub(in crate::routes) const CLOUDSYNC_TRANSPORTS_HEADER: &str = "x-anarlog-cloudsync-transports";

/// `replica` shipped in desktop 1.4.24, five releases before the accepted-state
/// outbox (1.4.29), so it cannot prove a build keeps unsent edits through
/// hydration or reacts to an update-required response. Builds that do both
/// advertise this second value, and it is the only capability that still
/// earns credentials from `/sync/token` once SQLite Cloud stops issuing tokens.
pub(in crate::routes) const ACCEPTED_REPLICA_TRANSPORT: &str = "accepted_replica";

fn client_advertises_transport(headers: &HeaderMap, expected: &str) -> bool {
    headers
        .get_all(CLOUDSYNC_TRANSPORTS_HEADER)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(','))
        .any(|transport| transport.trim() == expected)
}

pub(super) fn client_accepts_replica_transport(headers: &HeaderMap) -> bool {
    client_advertises_transport(headers, "replica")
}

pub(super) fn client_runs_accepted_replica(headers: &HeaderMap) -> bool {
    client_advertises_transport(headers, ACCEPTED_REPLICA_TRANSPORT)
}

#[derive(Deserialize)]
struct SyncTransportOverrideRow {
    transport: String,
}

pub(super) async fn resolve_desktop_transport(
    state: &ReplicaState,
    account_user_id: &str,
    default: CloudsyncTransport,
) -> Result<CloudsyncTransport> {
    let response = state
        .client
        .get(format!(
            "{}/rest/v1/sync_transport_overrides",
            state.config.supabase_url
        ))
        .header("apikey", &state.config.supabase_service_role_key)
        .bearer_auth(&state.config.supabase_service_role_key)
        .query(&[
            ("user_id", format!("eq.{account_user_id}")),
            ("select", "transport".to_string()),
            ("limit", "1".to_string()),
        ])
        .timeout(SUPABASE_REQUEST_TIMEOUT)
        .send()
        .await
        .map_err(|error| {
            tracing::warn!(%error, "Supabase sync transport override request failed");
            SyncError::Upstream
        })?;
    if !response.status().is_success() {
        tracing::warn!(
            status = %response.status(),
            "Supabase sync transport override request was rejected"
        );
        return Err(SyncError::Upstream);
    }
    let rows: Vec<SyncTransportOverrideRow> = response.json().await.map_err(|error| {
        tracing::warn!(%error, "Supabase sync transport override response was invalid");
        SyncError::Upstream
    })?;
    match rows.first().map(|row| row.transport.as_str()) {
        None => Ok(default),
        Some("replica") => Ok(CloudsyncTransport::Replica),
        Some("sqlite_sync") => Ok(CloudsyncTransport::SqliteSync),
        Some(other) => {
            tracing::warn!(transport = other, "unknown sync transport override");
            Err(SyncError::Upstream)
        }
    }
}

#[cfg(test)]
mod tests {
    use axum::http::HeaderValue;

    use super::*;

    #[test]
    fn accepts_replica_only_when_advertised() {
        let mut headers = HeaderMap::new();
        assert!(!client_accepts_replica_transport(&headers));

        headers.insert(
            CLOUDSYNC_TRANSPORTS_HEADER,
            HeaderValue::from_static("sqlite_sync"),
        );
        assert!(!client_accepts_replica_transport(&headers));

        headers.insert(
            CLOUDSYNC_TRANSPORTS_HEADER,
            HeaderValue::from_static("sqlite_sync, replica"),
        );
        assert!(client_accepts_replica_transport(&headers));
        assert!(!client_runs_accepted_replica(&headers));

        headers.insert(
            CLOUDSYNC_TRANSPORTS_HEADER,
            HeaderValue::from_static("replica, accepted_replica"),
        );
        assert!(client_accepts_replica_transport(&headers));
        assert!(client_runs_accepted_replica(&headers));
    }
}
