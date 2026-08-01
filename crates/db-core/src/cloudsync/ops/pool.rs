use sqlx::Sqlite;
use sqlx::pool::PoolConnection;

pub(super) async fn close_pool_connections(
    connections: Vec<PoolConnection<Sqlite>>,
) -> Result<(), anlg_cloudsync::Error> {
    let mut first_error = None;
    for connection in connections {
        if let Err(error) = connection.close().await
            && first_error.is_none()
        {
            first_error = Some(error.into());
        }
    }

    first_error.map_or(Ok(()), Err)
}

pub(super) async fn return_pool_connections(mut connections: Vec<PoolConnection<Sqlite>>) {
    for connection in &mut connections {
        connection.return_to_pool().await;
    }
}
