import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  analyticsEventFireAndForget: vi.fn(() => Promise.resolve()),
  execute: vi.fn(),
  executeTransaction: vi.fn(
    (_statements: Array<{ sql: string; params: unknown[] }>) =>
      Promise.resolve([1]),
  ),
}));

vi.mock("@hypr/plugin-analytics", () => ({
  commands: { eventFireAndForget: mocks.analyticsEventFireAndForget },
}));

vi.mock("@hypr/plugin-fs-sync", () => ({
  commands: {
    deleteSessionFolder: vi.fn(() =>
      Promise.resolve({ status: "ok", data: null }),
    ),
  },
}));

vi.mock("~/db", () => ({
  executeTransaction: mocks.executeTransaction,
  liveQueryClient: { execute: mocks.execute },
}));

import {
  addSessionParticipant,
  buildSessionTombstoneStatements,
  createSession,
  deleteEnhancedNote,
  getOrCreateSessionForEventId,
  isSessionEmpty,
  loadSessionEvent,
  removeSessionParticipant,
  restoreDeletedSession,
  softDeleteSession,
  updateEnhancedNoteContent,
  updateSession,
} from "./queries";

const event = {
  id: "event-1",
  tracking_id_event: "external-event-1",
  calendar_id: "calendar-1",
  title: "Planning",
  started_at: "2026-07-10T09:00:00.000Z",
  ended_at: "2026-07-10T10:00:00.000Z",
  location: "Room 1",
  meeting_link: "https://meet.example/1",
  description: "Plan",
  recurrence_series_id: "series-1",
  has_recurrence_rules: 1,
  is_all_day: 0,
  provider: "google",
  participants_json: JSON.stringify([
    { name: "Alice", email: "alice@example.com" },
  ]),
};

describe("session SQLite operations", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.useRealTimers();
  });

  it("loads embedded event metadata from the canonical session", async () => {
    mocks.execute.mockResolvedValueOnce([
      {
        event_json: JSON.stringify({
          tracking_id: "event-1",
          calendar_id: "calendar-1",
          title: "Planning",
        }),
      },
    ]);

    await expect(loadSessionEvent("session-1")).resolves.toMatchObject({
      tracking_id: "event-1",
      calendar_id: "calendar-1",
      title: "Planning",
    });
  });

  it("returns the existing note for an event without writing", async () => {
    mocks.execute
      .mockResolvedValueOnce([event])
      .mockResolvedValueOnce([{ id: "session-existing" }]);

    await expect(getOrCreateSessionForEventId("event-1")).resolves.toBe(
      "session-existing",
    );
    expect(mocks.executeTransaction).not.toHaveBeenCalled();
  });

  it("commits title and raw note changes in one ordered transaction", async () => {
    mocks.executeTransaction.mockResolvedValueOnce([1, 1]);

    await updateSession("session-1", {
      title: "Updated title",
      raw_md: '{"type":"doc"}',
    });

    const statements = mocks.executeTransaction.mock.calls[0][0] as Array<{
      sql: string;
      params: unknown[];
    }>;
    expect(statements).toHaveLength(2);
    expect(statements[0].sql).toContain("UPDATE sessions");
    expect(statements[0].params).toContain("Updated title");
    expect(statements[1].sql).toContain("session_documents");
    expect(statements[1].params).toContain('{"type":"doc"}');
  });

  it("creates a session with its initial event and note content atomically", async () => {
    await createSession("Welcome", "user-1", {
      event_json: '{"tracking_id":"welcome"}',
      raw_md: '{"type":"doc"}',
    });

    const statements = mocks.executeTransaction.mock.calls[0][0] as Array<{
      sql: string;
      params: unknown[];
    }>;
    expect(statements[0].sql).toContain("event_json");
    expect(statements[0].sql).toContain("cloudsync_workspace_binding");
    expect(statements[0].sql).toContain("NULLIF((");
    expect(statements[0].sql).not.toContain("COALESCE((");
    expect(statements[0].params).toContain('{"tracking_id":"welcome"}');
    expect(statements[1].sql).toContain("session_documents");
    expect(statements[1].sql).toContain("workspace_id");
    expect(statements[1].sql).toContain("FROM sessions");
    expect(statements[1].params).toContain('{"type":"doc"}');
  });

  it("derives the default self identity from the bound workspace", async () => {
    await createSession("Local note");

    const statements = mocks.executeTransaction.mock.calls[0][0] as Array<{
      sql: string;
      params: unknown[];
    }>;
    expect(statements[0].sql).toContain("NULLIF(NULLIF(?, '')");
    expect(statements[0].sql).toContain("00000000-0000-0000-0000-000000000000");
    expect(statements[2].sql).toContain("SELECT session.owner_user_id");
    expect(statements[2].params).not.toContain(
      "00000000-0000-0000-0000-000000000000",
    );
    expect(statements[3].sql).toContain("session.owner_user_id");
    expect(statements[3].params).not.toContain(
      "00000000-0000-0000-0000-000000000000",
    );
  });

  it("links a human to a session without creating duplicate active mappings", async () => {
    await addSessionParticipant("session-1", "human-1");

    const statements = mocks.executeTransaction.mock.calls[0][0];
    expect(statements[0].sql).toContain("source = 'excluded'");
    expect(statements[0].sql).toContain("? <> 'auto'");
    expect(statements[1].sql).toContain("INSERT INTO session_participants");
    expect(statements[1].sql).toContain("session.workspace_id");
    expect(statements[1].sql).toContain("NOT EXISTS");
    expect(statements[1].params).toContain("session-1");
    expect(statements[1].params).toContain("human-1");
    expect(statements[1].params).toContain("manual");
  });

  it("excludes auto participants and tombstones manual participants", async () => {
    await removeSessionParticipant("mapping-1");

    const statement = mocks.executeTransaction.mock.calls[0][0][0];
    expect(statement.sql).toContain("source = 'auto'");
    expect(statement.sql).toContain("THEN 'excluded'");
    expect(statement.sql).toContain("deleted_at = CASE");
    expect(statement.params).toContain("mapping-1");
  });

  it("commits enhanced note content and the derived session title together", async () => {
    mocks.executeTransaction.mockResolvedValueOnce([1, 1]);

    await updateEnhancedNoteContent(
      "enhanced-note-1",
      "session-1",
      '{"type":"doc"}',
      "Edited title",
    );

    const statements = mocks.executeTransaction.mock.calls[0][0] as Array<{
      sql: string;
      params: unknown[];
    }>;
    expect(statements).toHaveLength(2);
    expect(statements[0].sql).toContain("UPDATE session_documents");
    expect(statements[0].params).toContain("enhanced-note-1");
    expect(statements[0].params).toContain('{"type":"doc"}');
    expect(statements[1].sql).toContain("UPDATE sessions");
    expect(statements[1].params).toContain("session-1");
    expect(statements[1].params).toContain("Edited title");
  });

  it("soft-deletes an enhanced note instead of removing its data", async () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-07-10T12:00:00.000Z"));
    mocks.executeTransaction.mockResolvedValueOnce([1]);

    await deleteEnhancedNote("enhanced-note-1");

    const statements = mocks.executeTransaction.mock.calls[0][0] as Array<{
      sql: string;
      params: unknown[];
    }>;
    expect(statements).toHaveLength(1);
    expect(statements[0].sql).toContain("UPDATE session_documents");
    expect(statements[0].sql).toContain("deleted_at IS NULL");
    expect(statements[0].sql).not.toContain("DELETE FROM");
    expect(statements[0].params).toEqual([
      "2026-07-10T12:00:00.000Z",
      "2026-07-10T12:00:00.000Z",
      "enhanced-note-1",
    ]);
  });

  it("creates an event note with an in-transaction deduplication predicate", async () => {
    mocks.execute
      .mockResolvedValueOnce([event])
      .mockResolvedValueOnce([])
      .mockResolvedValueOnce([])
      .mockResolvedValueOnce([{ id: "session-created" }]);
    mocks.executeTransaction.mockResolvedValueOnce([1]);

    await expect(getOrCreateSessionForEventId("event-1")).resolves.toBe(
      "session-created",
    );

    const statements = mocks.executeTransaction.mock.calls[0][0] as Array<{
      sql: string;
      params: unknown[];
    }>;
    expect(statements[0].sql).toContain("WHERE NOT EXISTS");
    expect(statements[0].params).toContain("external-event-1");
    expect(
      statements.some((statement) => statement.sql.includes("humans")),
    ).toBe(true);
    expect(
      statements.some((statement) =>
        statement.sql.includes("session_participants"),
      ),
    ).toBe(true);
  });

  it("does not wait for analytics before returning a newly created event note", async () => {
    mocks.analyticsEventFireAndForget.mockImplementationOnce(
      () => new Promise<never>(() => {}),
    );
    mocks.execute
      .mockResolvedValueOnce([event])
      .mockResolvedValueOnce([])
      .mockResolvedValueOnce([])
      .mockResolvedValueOnce([{ id: "session-created" }]);
    mocks.executeTransaction.mockResolvedValueOnce([1]);

    await expect(getOrCreateSessionForEventId("event-1")).resolves.toBe(
      "session-created",
    );
    expect(mocks.analyticsEventFireAndForget).toHaveBeenCalledWith({
      event: "note_created",
      has_event_id: true,
    });
  }, 1_000);

  it("tombstones the session and every owned child with one timestamp", async () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-07-10T12:00:00.000Z"));
    mocks.execute.mockResolvedValueOnce([
      { id: "session-1", title: "Planning" },
    ]);
    mocks.executeTransaction.mockResolvedValueOnce([1, 1, 1, 1, 1, 1, 1, 1]);

    const deleted = await softDeleteSession("session-1");

    expect(deleted).toEqual({
      session: { id: "session-1", title: "Planning" },
      tombstone: "2026-07-10T12:00:00.000Z",
      deletedAt: Date.parse("2026-07-10T12:00:00.000Z"),
    });
    const statements = mocks.executeTransaction.mock.calls[0][0] as Array<{
      sql: string;
      params: unknown[];
    }>;
    expect(statements).toHaveLength(8);
    expect(
      statements.every((statement) =>
        statement.sql.includes("deleted_at IS NULL"),
      ),
    ).toBe(true);
    expect(
      statements.every((statement) =>
        statement.params.includes("2026-07-10T12:00:00.000Z"),
      ),
    ).toBe(true);
  });

  it("does not register a deletion when another window won the tombstone", async () => {
    mocks.execute.mockResolvedValueOnce([
      { id: "session-1", title: "Planning" },
    ]);
    mocks.executeTransaction.mockResolvedValueOnce([0, 0, 0, 0, 0, 0, 0, 0]);

    await expect(softDeleteSession("session-1")).resolves.toBeNull();
  });

  it("recognizes a blank SQLite session", async () => {
    mocks.execute.mockResolvedValueOnce([
      {
        title: "",
        event_json: "",
        note_body: JSON.stringify({
          type: "doc",
          content: [{ type: "paragraph" }],
        }),
        note_body_format: "prosemirror_json",
        transcript_count: 0,
        enhanced_note_count: 0,
        manual_participant_count: 0,
        tag_count: 0,
      },
    ]);

    await expect(isSessionEmpty("session-1")).resolves.toBe(true);
  });

  it.each([
    ["title", { title: "Named note", event_json: "" }],
    ["note body", { note_body: "Written content" }],
    ["transcript", { transcript_count: 1 }],
    ["enhanced note", { enhanced_note_count: 1 }],
    ["manual participant", { manual_participant_count: 1 }],
    ["tag", { tag_count: 1 }],
  ])("keeps a session with %s data", async (_label, overrides) => {
    mocks.execute.mockResolvedValueOnce([
      {
        title: "",
        event_json: "event",
        note_body: "",
        note_body_format: "prosemirror_json",
        transcript_count: 0,
        enhanced_note_count: 0,
        manual_participant_count: 0,
        tag_count: 0,
        ...overrides,
      },
    ]);

    await expect(isSessionEmpty("session-1")).resolves.toBe(false);
  });

  it("restores only rows carrying the deletion's exact tombstone", async () => {
    mocks.executeTransaction.mockResolvedValueOnce([1, 1, 1, 1, 1, 1, 1, 1]);
    await restoreDeletedSession({
      session: { id: "session-1", title: "Planning" },
      tombstone: "2026-07-10T12:00:00.000Z",
      deletedAt: 1,
    });

    const statements = mocks.executeTransaction.mock.calls[0][0] as Array<{
      sql: string;
      params: unknown[];
    }>;
    expect(statements).toHaveLength(8);
    expect(
      statements.every((statement) => statement.sql.includes("deleted_at = ?")),
    ).toBe(true);
    expect(statements.every((statement) => statement.params[0] === null)).toBe(
      true,
    );
  });

  it("covers all session-owned tables in the tombstone transaction", () => {
    const sql = buildSessionTombstoneStatements(
      "session-1",
      "2026-07-10T12:00:00.000Z",
    )
      .map((statement) => statement.sql)
      .join("\n");

    for (const table of [
      "sessions",
      "session_documents",
      "transcripts",
      "session_participants",
      "session_tags",
      "action_items",
      "session_attachments",
      "entity_mentions",
    ]) {
      expect(sql).toContain(table);
    }
  });
});
