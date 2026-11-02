UPDATE session_attachments
SET cloud_sync_enabled = 0,
    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
WHERE source_type = 'session_audio'
  AND cloud_sync_enabled = 1;
