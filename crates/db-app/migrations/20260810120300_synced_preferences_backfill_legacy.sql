-- Devices that never wrote theme/app_icon/week_start directly still carry the
-- values inside the imported legacy documents, which reads treat as live
-- settings. Mirror the read fallback order (settings document, then main
-- values document) so those preferences also start syncing without a user
-- edit. Direct rows won via the previous backfill's insert; DO NOTHING keeps
-- that precedence.
INSERT INTO synced_preferences (id, workspace_id, value_json, updated_at)
SELECT
  keys.id,
  json_extract(binding.value_json, '$.workspace_id'),
  json_quote(CASE
    WHEN json_type(legacy_settings.value_json, '$.general.' || keys.id) = 'text'
      THEN json_extract(legacy_settings.value_json, '$.general.' || keys.id)
    ELSE json_extract(legacy_main.value_json, '$.' || keys.id)
  END),
  CASE
    WHEN json_type(legacy_settings.value_json, '$.general.' || keys.id) = 'text'
      THEN legacy_settings.updated_at
    ELSE legacy_main.updated_at
  END
FROM (
  SELECT 'theme' AS id
  UNION ALL
  SELECT 'app_icon'
  UNION ALL
  SELECT 'week_start'
) AS keys
JOIN app_settings AS binding ON binding.id = 'cloudsync_workspace_binding'
LEFT JOIN app_settings AS legacy_settings ON legacy_settings.id = 'legacy_settings_document'
LEFT JOIN app_settings AS legacy_main ON legacy_main.id = 'legacy_main_values_document'
WHERE json_type(binding.value_json, '$.workspace_id') = 'text'
  AND json_extract(binding.value_json, '$.workspace_id') <> ''
  AND (
    json_type(legacy_settings.value_json, '$.general.' || keys.id) = 'text'
    OR json_type(legacy_main.value_json, '$.' || keys.id) = 'text'
  )
ON CONFLICT(id) DO NOTHING;
