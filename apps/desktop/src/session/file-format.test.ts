import { describe, expect, test } from "vitest";

import {
  parseSessionMarkdown,
  renderSessionMarkdown,
  SESSION_MARKDOWN_SCHEMA_VERSION,
  type SessionMarkdownDocument,
} from "./file-format";

const baseDocument = {
  schemaVersion: SESSION_MARKDOWN_SCHEMA_VERSION,
  id: "session-1",
  createdAt: "2026-05-19T00:00:00.000Z",
  updatedAt: "2026-05-19T00:05:00.000Z",
  title: "Weekly sync",
  folderId: "work",
  eventId: "event-1",
  event: { tracking_id: "event-1", title: "Weekly sync" },
  participants: [
    {
      person_id: "person-1",
      legacy_human_id: "human-1",
      name: "Jane Doe",
      email: "jane@example.com",
      source: "manual",
    },
  ],
  tags: ["work", "sync"],
  notes: "Raw notes.\n\n- follow up",
  summary: "Summary text.",
  transcript: "Jane: Hello.\nSam: Hi.",
} satisfies SessionMarkdownDocument;

describe("session file format", () => {
  test("roundtrips the canonical session markdown shape", () => {
    const markdown = renderSessionMarkdown(baseDocument);

    expect(parseSessionMarkdown(markdown)).toEqual(baseDocument);
  });

  test("parses unsectioned body as notes for hand-authored compatibility", () => {
    const parsed = parseSessionMarkdown(`---
schema_version: 1
id: "session-1"
created_at: "2026-05-19T00:00:00.000Z"
title: "Loose note"
---
Loose markdown body.
`);

    expect(parsed.notes).toBe("Loose markdown body.");
    expect(parsed.summary).toBe("");
    expect(parsed.transcript).toBe("");
  });

  test("keeps unknown headings inside the active section", () => {
    const parsed = parseSessionMarkdown(`---
schema_version: 1
id: "session-1"
created_at: "2026-05-19T00:00:00.000Z"
---

# Notes

## Nested heading

Body.

# Summary

Done.
`);

    expect(parsed.notes).toBe("## Nested heading\n\nBody.");
    expect(parsed.summary).toBe("Done.");
  });

  test("requires stable identity metadata", () => {
    expect(() =>
      parseSessionMarkdown(`---
created_at: "2026-05-19T00:00:00.000Z"
---
Body
`),
    ).toThrow("session_markdown_missing_id");
  });
});
