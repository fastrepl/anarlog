INSERT INTO synced_preferences (id, workspace_id, value_json, updated_at)
SELECT
  settings.id,
  json_extract(binding.value_json, '$.workspace_id'),
  settings.value_json,
  settings.updated_at
FROM app_settings AS settings
JOIN app_settings AS binding ON binding.id = 'cloudsync_workspace_binding'
WHERE settings.id IN ('theme', 'app_icon', 'week_start')
  AND json_type(binding.value_json, '$.workspace_id') = 'text'
  AND json_extract(binding.value_json, '$.workspace_id') <> ''
ON CONFLICT(id) DO NOTHING;
