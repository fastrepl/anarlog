import assert from "node:assert/strict";
import test from "node:test";

import { mapSessionDetailRows as mapRows } from "./session-detail.ts";

function mapSessionDetailRows(rows) {
  return mapRows(
    rows.map((row) => ({
      ...row,
      summary_documents_json: JSON.stringify(
        row.summary_documents ??
          (row.summary_id
            ? [
                {
                  id: row.summary_id,
                  title: row.summary_title,
                  body: row.summary_body,
                  body_format: row.summary_body_format,
                  kind: "summary",
                  template_id: "",
                  sort_order: 0,
                },
              ]
            : []),
      ),
    })),
  );
}

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

  const emptySyncedSummary = mapSessionDetailRows([
    {
      ...baseRow,
      summary_title: "",
      summary_body: "",
      summary_body_format: "prosemirror_json",
    },
  ]);
  assert.equal(emptySyncedSummary?.summary, null);
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

test("a named desktop placeholder with only the meeting title is not a finished summary", () => {
  const detail = mapSessionDetailRows([
    {
      ...baseRow,
      summary_body_format: "prosemirror_json",
      summary_body: JSON.stringify({
        type: "doc",
        content: [
          {
            type: "heading",
            attrs: { level: 1 },
            content: [{ type: "text", text: baseRow.title }],
          },
          { type: "paragraph" },
        ],
      }),
    },
  ]);
  assert.equal(detail.summary, null);
  const populatedTemplate = mapSessionDetailRows([
    {
      ...baseRow,
      summary_documents: [
        {
          id: "placeholder",
          kind: "summary",
          template_id: "",
          sort_order: 0,
          title: "Summary",
          body_format: "prosemirror_json",
          body: JSON.stringify({
            type: "doc",
            content: [
              {
                type: "heading",
                attrs: { level: 1 },
                content: [{ type: "text", text: baseRow.title }],
              },
            ],
          }),
        },
        {
          id: "template",
          kind: "template_output",
          template_id: "template-1",
          sort_order: 1,
          title: "Decisions",
          body_format: "markdown",
          body: "Ship the release.",
        },
      ],
    },
  ]);
  assert.deepEqual(populatedTemplate.summary, {
    title: "Decisions",
    text: "Ship the release.",
  });
});
