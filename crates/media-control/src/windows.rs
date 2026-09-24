pub(super) async fn pause_playback() -> Result<(), crate::Error> {
    use windows::Media::Control::GlobalSystemMediaTransportControlsSessionManager;

    let manager = GlobalSystemMediaTransportControlsSessionManager::RequestAsync()
        .and_then(|operation| operation.get())
        .map_err(crate::Error::new)?;
    let sessions = manager.GetSessions().map_err(crate::Error::new)?;
    for session in &sessions {
        if let Err(error) = session
            .TryPauseAsync()
            .and_then(|operation| operation.get())
        {
            tracing::warn!(%error, "media_pause_session_failed");
        }
    }
    Ok(())
}
