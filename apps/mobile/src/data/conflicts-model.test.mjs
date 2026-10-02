import assert from "node:assert/strict";
import test from "node:test";

import {
  bodyFormatFromBody,
  buildVersionHistory,
  mapConflictRows,
  previewFromBody,
} from "./conflicts-model.ts";

const doc = JSON.stringify({
  type: "doc",
  content: [
    {
      type: "heading",
      attrs: { level: 1 },
      content: [{ type: "text", text: "Weekly planning" }],
    },
    {
      type: "paragraph",
      content: [
        { type: "text", text: "Ship the " },
        { type: "text", text: "timeline", marks: [{ type: "bold" }] },
      ],
    },
    {
      type: "bulletList",
      content: [
        {
          type: "listItem",
          content: [
            { type: "paragraph", content: [{ type: "text", text: "First" }] },
          ],
        },
        {
          type: "listItem",
          content: [
            { type: "paragraph", content: [{ type: "text", text: "Second" }] },
          ],
        },
      ],
    },
  ],
});

test("previews bodies across formats and detects their format", () => {
  for (const [body, format, expected] of [
    [doc, "prosemirror_json", "Weekly planning Ship the timeline First Second"],
    [
      "# Weekly planning\n\nShip the   timeline\n",
      "markdown",
      "Weekly planning Ship the timeline",
    ],
    ["just text", "prosemirror_json", "just text"],
    ["", "prosemirror_json", ""],
  ]) {
    assert.equal(previewFromBody(body, format), expected);
  }

  const preview = previewFromBody("a".repeat(200), "markdown");
  assert.equal(preview.length, 121);
  assert.ok(preview.endsWith("…"));

  assert.equal(bodyFormatFromBody(doc), "prosemirror_json");
  assert.equal(bodyFormatFromBody("# Title\n\ntext"), "markdown");
});

test("maps conflict rows and drops values that are not text", () => {
  const conflicts = mapConflictRows([
    {
      id: "conflict-1",
      row_id: "session-1",
      table_name: "session_documents",
      field_name: "body",
      lost_side: "remote",
      edited_at_ms: Date.parse("2026-09-09T10:00:00.000Z"),
      value_json: JSON.stringify("# Other device\n\nTheir note"),
      created_at: "2026-09-09T11:00:00.000Z",
    },
    {
      id: "conflict-2",
      row_id: "session-1",
      table_name: "sessions",
      field_name: "title",
      lost_side: "local",
      edited_at_ms: null,
      value_json: JSON.stringify("Their title"),
      created_at: "2026-09-09T11:00:00.000Z",
    },
    {
      id: "conflict-3",
      row_id: "session-1",
      table_name: "sessions",
      field_name: "event_json",
      lost_side: "local",
      edited_at_ms: null,
      value_json: "null",
      created_at: "2026-09-09T11:00:00.000Z",
    },
  ]);

  assert.deepEqual(conflicts, [
    {
      id: "conflict-1",
      sessionId: "session-1",
      field: "body",
      lostSide: "remote",
      at: "2026-09-09T10:00:00.000Z",
      value: "# Other device\n\nTheir note",
      bodyFormat: "markdown",
    },
    {
      id: "conflict-2",
      sessionId: "session-1",
      field: "title",
      lostSide: "local",
      at: "2026-09-09T11:00:00.000Z",
      value: "Their title",
      bodyFormat: "markdown",
    },
  ]);
});

test("lists conflict copies and saved versions newest first", () => {
  const conflicts = mapConflictRows([
    {
      id: "conflict-1",
      row_id: "session-1",
      table_name: "session_documents",
      field_name: "body",
      lost_side: "remote",
      edited_at_ms: Date.parse("2026-09-09T12:00:00.000Z"),
      value_json: JSON.stringify("# Note\n\nTheir note"),
      created_at: "2026-09-09T12:30:00.000Z",
    },
    {
      id: "conflict-2",
      row_id: "session-1",
      table_name: "sessions",
      field_name: "title",
      lost_side: "remote",
      edited_at_ms: Date.parse("2026-09-09T13:00:00.000Z"),
      value_json: JSON.stringify("Their title"),
      created_at: "2026-09-09T13:00:00.000Z",
    },
  ]);

  const entries = buildVersionHistory(conflicts, [
    {
      id: "version-1",
      body: "# Note\n\nEarlier",
      body_format: "markdown",
      source: "local",
      created_at: "2026-09-09T09:00:00.000Z",
    },
    {
      id: "version-2",
      body: doc,
      body_format: "prosemirror_json",
      source: "sync",
      created_at: "2026-09-09T14:00:00.000Z",
    },
  ]);

  assert.deepEqual(
    entries.map((entry) => [entry.id, entry.label, entry.preview]),
    [
      ["version-2", "Synced", "Weekly planning Ship the timeline First Second"],
      ["conflict-1", "Other device", "Note Their note"],
      ["version-1", "This device", "Note Earlier"],
    ],
  );
  assert.equal(entries[1].conflictId, "conflict-1");
  assert.equal(entries[0].conflictId, null);
});
