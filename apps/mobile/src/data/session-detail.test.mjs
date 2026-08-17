import assert from "node:assert/strict";
import test from "node:test";

import { mapSessionDetailRows } from "./session-detail.ts";

const baseRow = {
  id: "session-1",
  title: "Weekly planning",
  created_at: "2026-08-17T00:00:00.000Z",
  raw_body: JSON.stringify({
    type: "doc",
    content: [
      {
        type: "heading",
        attrs: { level: 1 },
        content: [{ type: "text", text: "Weekly planning" }],
      },
      { type: "paragraph", content: [{ type: "text", text: "My note" }] },
    ],
  }),
  raw_body_format: "prosemirror_json",
  summary_id: "summary-1",
  summary_title: "Key decisions",
  summary_body: "# Generated summary\n\nShip the mobile timeline.",
  summary_body_format: "markdown",
};

test("maps a canonical summary alongside the editable note", () => {
  assert.deepEqual(mapSessionDetailRows([baseRow]), {
    id: "session-1",
    title: "Weekly planning",
    createdAt: "2026-08-17T00:00:00.000Z",
    noteText: "My note",
    bodyFormat: "prosemirror_json",
    plainEditable: true,
    summary: {
      title: "Key decisions",
      text: "Ship the mobile timeline.",
    },
  });
});

test("keeps a meeting without a generated summary as an empty note surface", () => {
  assert.deepEqual(
    mapSessionDetailRows([
      {
        ...baseRow,
        raw_body: "",
        summary_id: "",
        summary_title: "",
        summary_body: "",
        summary_body_format: "prosemirror_json",
      },
    ]),
    {
      id: "session-1",
      title: "Weekly planning",
      createdAt: "2026-08-17T00:00:00.000Z",
      noteText: "",
      bodyFormat: "prosemirror_json",
      plainEditable: true,
      summary: null,
    },
  );
});

test("does not render an empty synced summary document", () => {
  const detail = mapSessionDetailRows([
    {
      ...baseRow,
      summary_title: "",
      summary_body: "",
      summary_body_format: "prosemirror_json",
    },
  ]);

  assert.equal(detail?.summary, null);
});

test("uses the body heading when a synced summary has no stored title", () => {
  const detail = mapSessionDetailRows([
    {
      ...baseRow,
      summary_title: "",
      summary_body: "# Decisions\n\nUse hosted live transcription.",
    },
  ]);

  assert.deepEqual(detail?.summary, {
    title: "Decisions",
    text: "Use hosted live transcription.",
  });
});
