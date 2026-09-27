const MPRIS_PREFIX: &str = "org.mpris.MediaPlayer2.";
const MPRIS_PATH: &str = "/org/mpris/MediaPlayer2";
const MPRIS_PLAYER: &str = "org.mpris.MediaPlayer2.Player";

pub(super) async fn pause_playback() -> Result<(), crate::Error> {
    let connection = zbus::Connection::session()
        .await
        .map_err(crate::Error::new)?;
    let dbus = zbus::fdo::DBusProxy::new(&connection)
        .await
        .map_err(crate::Error::new)?;
    let names = dbus.list_names().await.map_err(crate::Error::new)?;

    for name in names.iter().filter(|name| name.starts_with(MPRIS_PREFIX)) {
        pause_player(&connection, name.as_str()).await;
    }
    Ok(())
}

async fn pause_player(connection: &zbus::Connection, name: &str) {
    let player = match zbus::Proxy::new(connection, name, MPRIS_PATH, MPRIS_PLAYER).await {
        Ok(player) => player,
        Err(error) => {
            tracing::warn!(%name, %error, "mpris_proxy_failed");
            return;
        }
    };
    let status: String = player
        .get_property("PlaybackStatus")
        .await
        .unwrap_or_default();
    if status != "Playing" {
        return;
    }
    if let Err(error) = player.call_method("Pause", &()).await {
        tracing::warn!(%name, %error, "mpris_pause_failed");
    }
}
