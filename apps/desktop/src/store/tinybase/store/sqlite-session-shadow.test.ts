import { describe, expect, it, vi } from "vitest";

vi.mock("~/db", () => ({ db: {} }));

import {
  fromSqliteSessionRow,
  toSqliteSessionDocumentRow,
  toSqliteSessionRow,
} from "./sqlite-session-shadow";

describe("toSqliteSessionRow", () => {
  it("preserves sidebar metadata and extracts calendar timing", () => {
    const row = toSqliteSessionRow("session-1", {
      user_id: "user-1",
      created_at: "2026-07-10T01:00:00Z",
      folder_id: "work/project",
      title: "Planning",
      raw_md: "{}",
      event_json: JSON.stringify({
        tracking_id: "event-1",
        calendar_id: "calendar-1",
        title: "Planning",
        started_at: "2026-07-10T02:00:00Z",
        ended_at: "2026-07-10T03:00:00Z",
        is_all_day: false,
        has_recurrence_rules: true,
        recurrence_series_id: "series-1",
      }),
    });

    expect(row).toMatchObject({
      id: "session-1",
      ownerUserId: "user-1",
      title: "Planning",
      createdAt: "2026-07-10T01:00:00Z",
      startedAt: "2026-07-10T02:00:00Z",
      endedAt: "2026-07-10T03:00:00Z",
      externalEventId: "event-1",
      seriesId: "series-1",
      folderPath: "work/project",
      deletedAt: null,
    });
  });

  it("stores the raw note in the canonical session document", () => {
    expect(
      toSqliteSessionDocumentRow("session-1", {
        user_id: "user-1",
        created_at: "2026-07-10T01:00:00Z",
        raw_md: '{"type":"doc","content":[]}',
      }),
    ).toMatchObject({
      id: "session-1",
      sessionId: "session-1",
      kind: "note",
      bodyFormat: "prosemirror_json",
      body: '{"type":"doc","content":[]}',
      createdBy: "user-1",
    });
  });

  it("hydrates imported Markdown as ProseMirror JSON", () => {
    const row = fromSqliteSessionRow({
      id: "session-1",
      owner_user_id: "user-1",
      title: "Planning",
      created_at: "2026-07-10T01:00:00Z",
      event_json: "",
      folder_path: "",
      raw_md: "# Notes",
      raw_body_format: "markdown",
    });

    expect(JSON.parse(row.raw_md as string)).toMatchObject({ type: "doc" });
  });
});
