-- Automations: an enabled workflow with a successful run, a draft that needs setup and failed
-- last time, a blank draft, and the Markdown export starter enabled with a folder. Settings
-- store the workflow list as a JSON string, as shipping does.
INSERT INTO
  app_settings (id, value_json)
VALUES
  (
    'automation_workflows',
    json_quote(
      json_array(
        json_object(
          'id',
          'wf-standup',
          'title',
          'Post standup recaps',
          'enabled',
          json('true'),
          'trigger',
          'note_enhanced',
          'steps',
          json_array(
            json_object(
              'id',
              'st-slack',
              'type',
              'slack_recap',
              'target',
              json_object('id', 'C024BE91L', 'name', 'eng-standups')
            ),
            json_object(
              'id',
              'st-linear',
              'type',
              'linear_issues',
              'target',
              json_object('id', 'team-eng', 'name', 'Engineering')
            )
          ),
          'lastRun',
          json_object(
            'at',
            strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-3 hours'),
            'status',
            'success',
            'detail',
            '#eng-standups · 2 issues in Engineering'
          ),
          'processedSessionIds',
          json_array(),
          'chatGroupId',
          json('null')
        ),
        json_object(
          'id',
          'wf-archive',
          'title',
          'Archive client calls',
          'enabled',
          json('false'),
          'trigger',
          'meeting_completed',
          'steps',
          json_array(
            json_object(
              'id',
              'st-markdown',
              'type',
              'markdown_export',
              'directory',
              '/Users/alex/Documents/Client calls',
              'options',
              json_object(
                'include_memo',
                json('true'),
                'include_summary',
                json('true'),
                'include_transcript',
                json('false'),
                'include_action_items',
                json('true'),
                'filename',
                '{date} {title}',
                'include_id_suffix',
                json('false')
              )
            ),
            json_object(
              'id',
              'st-notion',
              'type',
              'notion_update',
              'target',
              json('null')
            )
          ),
          'lastRun',
          json_object(
            'at',
            strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-2 days'),
            'status',
            'error',
            'detail',
            'choose a Notion page first'
          ),
          'processedSessionIds',
          json_array(),
          'chatGroupId',
          json('null')
        ),
        json_object(
          'id',
          'wf-blank',
          'title',
          '',
          'enabled',
          json('false'),
          'trigger',
          'note_enhanced',
          'steps',
          json_array(),
          'lastRun',
          json('null'),
          'processedSessionIds',
          json_array(),
          'chatGroupId',
          json('null')
        )
      ) || ''
    )
  ),
  ('automation_markdown_export_enabled', 'true'),
  (
    'automation_markdown_export_directory',
    '"/Users/alex/Documents/Meetings"'
  ),
  (
    'automation_markdown_export_last_run',
    json_quote(
      json_object(
        'at',
        strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-1 day'),
        'status',
        'success',
        'detail',
        '/Users/alex/Documents/Meetings/Weekly product sync.md'
      ) || ''
    )
  );
