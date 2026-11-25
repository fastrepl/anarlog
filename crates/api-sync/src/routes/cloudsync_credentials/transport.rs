use axum::http::HeaderMap;

/// Desktop advertises the transports it can accept from `/sync/token`.
pub(in crate::routes) const CLOUDSYNC_TRANSPORTS_HEADER: &str = "x-anarlog-cloudsync-transports";

/// `replica` shipped in desktop 1.4.24, five releases before the accepted-state
/// outbox (1.4.29), so it cannot prove a build keeps unsent edits through
/// hydration or reacts to an update-required response. Builds that do both
/// advertise this second value, and it is the only capability that earns
/// credentials from `/sync/token` and `/sync/replica/credentials`.
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
