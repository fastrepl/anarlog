-- App-link chips in a memo: a GitHub issue (open) and pull request (merged) that open the Task
-- tab, a commit, and Notion, Slack and Linear links. Dated last so earlier timeline captures
-- keep their rows.
INSERT INTO
  sessions (
    id,
    title,
    created_at,
    started_at,
    ended_at,
    folder_path
  )
VALUES
  (
    's-links',
    'Release checklist',
    strftime('%Y-%m-%dT%H:%M:%SZ', 'now', '-90 days'),
    '',
    '',
    ''
  );
INSERT INTO
  session_documents (
    id,
    session_id,
    kind,
    title,
    body_format,
    body,
    sort_order
  )
VALUES
  (
    's-links',
    's-links',
    'note',
    '',
    'prosemirror_json',
    '{"type":"doc","content":[
    {"type":"paragraph","content":[{"type":"text","text":"Ship the calendar fix after "},
      {"type":"appLink","attrs":{"provider":"github","kind":"issue","url":"https://github.com/acme/app/issues/42","owner":"acme","repo":"app","number":42}},
      {"type":"text","text":" and "},
      {"type":"appLink","attrs":{"provider":"github","kind":"pull_request","url":"https://github.com/acme/app/pull/43","owner":"acme","repo":"app","number":43}},
      {"type":"text","text":" land."}]},
    {"type":"bulletList","content":[
      {"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"Hotfix "},
        {"type":"appLink","attrs":{"provider":"github","kind":"commit","url":"https://github.com/acme/app/commit/9f3c2ab41d","owner":"acme","repo":"app","subId":"9f3c2ab41d"}}]}]},
      {"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"Notes in "},
        {"type":"appLink","attrs":{"provider":"notion","kind":"page","url":"https://www.notion.so/acme/Release-notes","workspace":"Acme","resourceTitle":"Release notes"}}]}]},
      {"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"Reported in "},
        {"type":"appLink","attrs":{"provider":"slack","kind":"message","url":"https://acme.slack.com/archives/C042/p1700000000","workspace":"acme","channelId":"support","messageTs":"1700000000"}}]}]},
      {"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"Tracked as "},
        {"type":"appLink","attrs":{"provider":"linear","kind":"issue","url":"https://linear.app/acme/issue/ENG-128","workspace":"acme","resourceId":"ENG-128"}}]}]}]}]}',
    0
  );
