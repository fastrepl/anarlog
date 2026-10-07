import { beforeEach, describe, expect, it, vi } from "vitest";

import { trackPendingSoftDelete } from "./pending-soft-deletes";

const mocks = vi.hoisted(() => ({
  analyticsEventFireAndForget: vi.fn(() => Promise.resolve()),
  execute: vi.fn(),
  executeTransaction: vi.fn(
    (_statements: Array<{ sql: string; params: unknown[] }>) =>
      Promise.resolve([1]),
  ),
  createSession: vi.fn(() =>
    Promise.resolve({ status: "ok", data: "session-1" }),
  ),
  createSessionForEvent: vi.fn(() =>
    Promise.resolve({
      status: "ok",
      data: { session_id: "session-created", created: true },
    }),
  ),
  softDeleteSession: vi.fn(() =>
    Promise.resolve({
      status: "ok" as const,
      data: { id: "session-1", title: "Planning" } as {
        id: string;
        title: string;
      } | null,
    }),
  ),
  restoreDeletedSession: vi.fn(() =>
    Promise.resolve({ status: "ok", data: "restored" }),
  ),
  addSessionParticipant: vi.fn(() =>
    Promise.resolve({ status: "ok", data: null }),
  ),
  removeSessionParticipant: vi.fn(() =>
    Promise.resolve({ status: "ok", data: null }),
  ),
  persistChatSessionProposal: vi.fn(() =>
    Promise.resolve({ status: "ok", data: null }),
  ),
  setSessionProposalStatus: vi.fn(() =>
    Promise.resolve({ status: "ok", data: null }),
  ),
}));

vi.mock("@anlg/plugin-analytics", () => ({
  commands: { eventFireAndForget: mocks.analyticsEventFireAndForget },
}));

vi.mock("@anlg/plugin-session", () => ({
  commands: {
    createSession: mocks.createSession,
    createSessionForEvent: mocks.createSessionForEvent,
    softDeleteSession: mocks.softDeleteSession,
    restoreDeletedSession: mocks.restoreDeletedSession,
    addSessionParticipant: mocks.addSessionParticipant,
    removeSessionParticipant: mocks.removeSessionParticipant,
    persistChatSessionProposal: mocks.persistChatSessionProposal,
    setSessionProposalStatus: mocks.setSessionProposalStatus,
  },
}));

vi.mock("@anlg/plugin-fs-sync", () => ({
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
  applySessionProposal,
  declineSessionProposal,
  deleteEnhancedNote,
  getOrCreateSessionForEventId,
  isSessionDeleted,
  isSessionEmpty,
  loadSessionEvent,
  restoreDeletedSession,
  softDeleteSession,
  updateEnhancedNoteContent,
  updateSession,
} from "./queries";

const event = {
  id: "event-1",
  participants_json: JSON.stringify([
    { name: "Alice", email: "alice@example.com" },
  ]),
};

type SqlStatement = { sql: string; params: unknown[] };

// Reads the persisted note metadata back out of a transaction without
// pinning statement counts or positions.
function readWrittenMetadata(statements: SqlStatement[]): unknown {
  const metadataWrite = statements.find((statement) =>
    statement.sql.includes("generation_metadata_json = ?"),
  );
  if (!metadataWrite) {
    throw new Error("expected a generation_metadata_json write");
  }
  return JSON.parse(metadataWrite.params[0] as string);
}

function writesDocument(statements: SqlStatement[]): boolean {
  return statements.some((statement) =>
    statement.sql.includes("INSERT INTO session_documents"),
  );
}

function writesMetadata(statements: SqlStatement[]): boolean {
  return statements.some((statement) =>
    statement.sql.includes("generation_metadata_json"),
  );
}

describe("session SQLite operations", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.useRealTimers();
    mocks.createSessionForEvent.mockResolvedValue({
      status: "ok",
      data: { session_id: "session-created", created: true },
    });
    mocks.softDeleteSession.mockResolvedValue({
      status: "ok",
      data: { id: "session-1", title: "Planning" },
    });
    mocks.restoreDeletedSession.mockResolvedValue({
      status: "ok",
      data: "restored",
    });
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

  it("attaches parsed calendar participants to an existing event note", async () => {
    mocks.execute.mockResolvedValueOnce([event]);
    mocks.createSessionForEvent.mockResolvedValueOnce({
      status: "ok",
      data: { session_id: "session-existing", created: false },
    });

    await expect(getOrCreateSessionForEventId("event-1")).resolves.toBe(
      "session-existing",
    );

    expect(mocks.createSessionForEvent).toHaveBeenCalledWith(
      expect.objectContaining({
        event_id: "event-1",
        participants: [expect.objectContaining({ email: "alice@example.com" })],
      }),
    );
    expect(mocks.analyticsEventFireAndForget).not.toHaveBeenCalled();
  });

  it("does not attach malformed or self calendar participants", async () => {
    mocks.execute.mockResolvedValueOnce([
      {
        ...event,
        participants_json: JSON.stringify([
          null,
          { name: 42, email: "invalid@example.com" },
          {
            name: "John",
            email: "john@example.com",
            is_current_user: true,
          },
          { name: "Artem", email: "artem@example.com" },
          { name: "Artem Dupe", email: "ARTEM@example.com" },
        ]),
      },
    ]);

    await expect(getOrCreateSessionForEventId("event-1")).resolves.toBe(
      "session-created",
    );

    expect(mocks.createSessionForEvent).toHaveBeenCalledWith(
      expect.objectContaining({
        participants: [expect.objectContaining({ email: "artem@example.com" })],
      }),
    );
  });

  it("commits title and raw note changes in one ordered transaction", async () => {
    mocks.executeTransaction.mockResolvedValueOnce([1, 1]);
    mocks.execute.mockResolvedValueOnce([
      {
        generation_metadata_json: JSON.stringify({
          headings: { key: "", updatedAt: "2026-10-01T00:00:00.000Z" },
        }),
        updated_at: "2026-10-01T00:00:00.000Z",
      },
    ]);

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

  it("persists a note lock flag on the session row", async () => {
    await updateSession("session-1", { locked: true });

    const statements = mocks.executeTransaction.mock.calls[0][0] as Array<{
      sql: string;
      params: unknown[];
    }>;
    expect(statements).toHaveLength(1);
    expect(statements[0].sql).toContain("locked = ?");
    expect(statements[0].params).toContain(1);
  });

  it("stores a memo template without clearing it on later edits", async () => {
    await updateSession("session-1", {
      raw_md: '{"type":"doc"}',
      raw_template_id: "template-1",
    });

    const templateStatement = mocks.executeTransaction.mock.calls[0][0][0];
    expect(templateStatement.sql).toContain(
      "template_id = excluded.template_id",
    );
    expect(templateStatement.params).toContain("template-1");

    mocks.executeTransaction.mockClear();
    await updateSession("session-1", { raw_md: '{"type":"doc"}' });

    const editStatement = mocks.executeTransaction.mock.calls[0][0][0];
    expect(editStatement.sql).not.toContain(
      "template_id = excluded.template_id",
    );
  });

  it("stores an applied template snapshot alongside other note metadata", async () => {
    mocks.execute.mockResolvedValueOnce([
      {
        generation_metadata_json: JSON.stringify({
          appliedTemplate: {
            templateId: "template-old",
            sections: [{ title: "Old", description: "" }],
          },
          otherKey: "kept",
        }),
      },
    ]);

    await updateSession("session-1", {
      raw_md: '{"type":"doc"}',
      raw_template_id: "template-1",
      raw_template_snapshot: {
        templateId: "template-1",
        sections: ["Updates"],
      },
    });

    const statements = mocks.executeTransaction.mock
      .calls[0][0] as SqlStatement[];
    expect(writesDocument(statements)).toBe(true);
    expect(readWrittenMetadata(statements)).toEqual({
      appliedTemplate: {
        templateId: "template-1",
        sections: ["Updates"],
      },
      otherKey: "kept",
      headings: { key: "", updatedAt: expect.any(String) },
    });
  });

  it("stamps untracked memo headings with the previous write time", async () => {
    const body = JSON.stringify({
      type: "doc",
      content: [
        {
          type: "heading",
          attrs: { level: 2 },
          content: [{ type: "text", text: "Updates" }],
        },
      ],
    });
    mocks.execute.mockResolvedValueOnce([
      {
        generation_metadata_json: JSON.stringify({
          appliedTemplate: {
            templateId: "template-1",
            sections: ["Updates"],
          },
        }),
        updated_at: "2026-10-01T00:00:00.000Z",
        body,
        body_format: "prosemirror_json",
      },
    ]);

    await updateSession("session-1", { raw_md: body });

    const statements = mocks.executeTransaction.mock
      .calls[0][0] as SqlStatement[];
    const metadata = readWrittenMetadata(statements) as {
      appliedTemplate: unknown;
      headings: { key: string; updatedAt: string };
    };
    expect(metadata.appliedTemplate).toEqual({
      templateId: "template-1",
      sections: ["Updates"],
    });
    expect(metadata.headings).toEqual({
      key: "Updates",
      updatedAt: "2026-10-01T00:00:00.000Z",
    });
  });

  it("stamps a real heading edit on a legacy memo with the current time", async () => {
    mocks.execute.mockResolvedValueOnce([
      {
        generation_metadata_json: JSON.stringify({ otherKey: "kept" }),
        updated_at: "2026-10-01T00:00:00.000Z",
        body: JSON.stringify({
          type: "doc",
          content: [
            {
              type: "heading",
              attrs: { level: 2 },
              content: [{ type: "text", text: "To-dos" }],
            },
          ],
        }),
        body_format: "prosemirror_json",
      },
    ]);

    await updateSession("session-1", {
      raw_md: JSON.stringify({
        type: "doc",
        content: [
          {
            type: "heading",
            attrs: { level: 2 },
            content: [{ type: "text", text: "To do" }],
          },
        ],
      }),
    });

    const statements = mocks.executeTransaction.mock
      .calls[0][0] as SqlStatement[];
    const metadata = readWrittenMetadata(statements) as {
      headings: { key: string; updatedAt: string };
    };
    expect(metadata.headings.key).toBe("To do");
    expect(
      Date.parse(metadata.headings.updatedAt) >
        Date.parse("2026-10-01T00:00:00.000Z"),
    ).toBe(true);
  });

  it("compares legacy markdown bodies when stamping headings", async () => {
    const body = JSON.stringify({
      type: "doc",
      content: [
        {
          type: "heading",
          attrs: { level: 2 },
          content: [{ type: "text", text: "Updates" }],
        },
      ],
    });
    mocks.execute.mockResolvedValueOnce([
      {
        generation_metadata_json: JSON.stringify({ otherKey: "kept" }),
        updated_at: "2026-10-01T00:00:00.000Z",
        body: "## Updates\n",
        body_format: "markdown",
      },
    ]);

    await updateSession("session-1", { raw_md: body });

    const statements = mocks.executeTransaction.mock
      .calls[0][0] as SqlStatement[];
    const metadata = readWrittenMetadata(statements) as {
      headings: { key: string; updatedAt: string };
    };
    expect(metadata.headings).toEqual({
      key: "Updates",
      updatedAt: "2026-10-01T00:00:00.000Z",
    });
  });

  it("restamps memo headings only when they change", async () => {
    const body = JSON.stringify({
      type: "doc",
      content: [
        {
          type: "heading",
          attrs: { level: 2 },
          content: [{ type: "text", text: "Updates" }],
        },
        {
          type: "heading",
          attrs: { level: 2 },
          content: [{ type: "text", text: "Next Steps" }],
        },
      ],
    });
    mocks.execute.mockResolvedValueOnce([
      {
        generation_metadata_json: JSON.stringify({
          headings: { key: "Updates", updatedAt: "2026-10-01T00:00:00.000Z" },
        }),
        updated_at: "2026-10-01T00:00:00.000Z",
      },
    ]);

    await updateSession("session-1", { raw_md: body });

    const restamp = mocks.executeTransaction.mock.calls[0][0] as SqlStatement[];
    const metadata = readWrittenMetadata(restamp) as {
      headings: { key: string; updatedAt: string };
    };
    expect(metadata.headings.key).toBe("Updates\nNext Steps");
    expect(
      Date.parse(metadata.headings.updatedAt) >=
        Date.parse("2026-10-01T00:00:00.000Z"),
    ).toBe(true);

    mocks.executeTransaction.mockClear();
    mocks.execute.mockResolvedValueOnce([
      {
        generation_metadata_json: JSON.stringify({
          headings: {
            key: "Updates\nNext Steps",
            updatedAt: metadata.headings.updatedAt,
          },
        }),
        updated_at: metadata.headings.updatedAt,
      },
    ]);

    await updateSession("session-1", { raw_md: body });

    const unchanged = mocks.executeTransaction.mock
      .calls[0][0] as SqlStatement[];
    expect(writesMetadata(unchanged)).toBe(false);
    expect(writesDocument(unchanged)).toBe(true);
  });

  it("skips heading tracking for unparseable memo bodies", async () => {
    mocks.execute.mockResolvedValueOnce([
      {
        generation_metadata_json: JSON.stringify({ otherKey: "kept" }),
        updated_at: "2026-10-01T00:00:00.000Z",
      },
    ]);

    await updateSession("session-1", { raw_md: "not json" });

    const statements = mocks.executeTransaction.mock
      .calls[0][0] as SqlStatement[];
    expect(writesMetadata(statements)).toBe(false);
    expect(writesDocument(statements)).toBe(true);
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
    expect(statements).toHaveLength(3);
    expect(statements[0].sql).toContain("UPDATE session_documents");
    expect(statements[0].params).toContain("enhanced-note-1");
    expect(statements[0].params).toContain('{"type":"doc"}');
    expect(statements[1].sql).toContain("DELETE FROM app_settings");
    expect(statements[1].sql).toContain("$.noteId");
    expect(statements[1].sql).toContain("$.body");
    expect(statements[1].params).toEqual([
      "auto_enhance_pending:session-1",
      "enhanced-note-1",
      '{"type":"doc"}',
    ]);
    expect(statements[2].sql).toContain("UPDATE sessions");
    expect(statements[2].params).toContain("session-1");
    expect(statements[2].params).toContain("Edited title");
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

  it("does not wait for analytics before returning a newly created event note", async () => {
    mocks.analyticsEventFireAndForget.mockImplementationOnce(
      () => new Promise<never>(() => {}),
    );
    mocks.execute.mockResolvedValueOnce([event]);

    await expect(getOrCreateSessionForEventId("event-1")).resolves.toBe(
      "session-created",
    );
    expect(mocks.analyticsEventFireAndForget).toHaveBeenCalledWith({
      event: "note_created",
      has_event_id: true,
    });
  }, 1_000);

  it("tombstones the session and returns its identity", async () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-07-10T12:00:00.000Z"));

    const deleted = await softDeleteSession("session-1");

    expect(deleted).toEqual({
      session: { id: "session-1", title: "Planning" },
      tombstone: "2026-07-10T12:00:00.000Z",
      deletedAt: Date.parse("2026-07-10T12:00:00.000Z"),
    });
  });

  it("does not register a deletion when another window won the tombstone", async () => {
    mocks.softDeleteSession.mockResolvedValueOnce({ status: "ok", data: null });

    await expect(softDeleteSession("session-1")).resolves.toBeNull();
  });

  it("reports whether a session has been deleted", async () => {
    mocks.execute
      .mockResolvedValueOnce([{ id: "session-1" }])
      .mockResolvedValueOnce([]);

    await expect(isSessionDeleted("session-1")).resolves.toBe(false);
    await expect(isSessionDeleted("session-1")).resolves.toBe(true);
  });

  it("waits for an in-flight soft delete before checking the session", async () => {
    let finishDelete = () => {};
    const deleteWrite = new Promise<void>((resolve) => {
      finishDelete = resolve;
    });
    trackPendingSoftDelete("pending-session", deleteWrite);

    const deleted = isSessionDeleted("pending-session");
    await Promise.resolve();
    expect(mocks.execute).not.toHaveBeenCalled();

    mocks.execute.mockResolvedValueOnce([]);
    finishDelete();
    await expect(deleted).resolves.toBe(true);
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
        meeting_chat_count: 0,
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
    ["captured meeting chat", { meeting_chat_count: 1 }],
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
        meeting_chat_count: 0,
        manual_participant_count: 0,
        tag_count: 0,
        ...overrides,
      },
    ]);

    await expect(isSessionEmpty("session-1")).resolves.toBe(false);
  });

  it("retries a restore until the session is deleted and throws if it never is", async () => {
    vi.useFakeTimers();
    const deleted = {
      session: { id: "session-1", title: "Planning" },
      tombstone: "2026-07-10T12:00:00.000Z",
      deletedAt: 1,
    };

    mocks.restoreDeletedSession
      .mockResolvedValueOnce({ status: "ok", data: "not_deleted" })
      .mockResolvedValueOnce({ status: "ok", data: "restored" });

    const restored = restoreDeletedSession(deleted);
    await vi.advanceTimersByTimeAsync(100);
    await expect(restored).resolves.toBeUndefined();

    mocks.restoreDeletedSession.mockResolvedValue({
      status: "ok",
      data: "not_deleted",
    });
    const stuck = restoreDeletedSession(deleted);
    const stuckExpectation = expect(stuck).rejects.toThrow(
      "Session session-1 was never soft-deleted",
    );
    await vi.advanceTimersByTimeAsync(1_000);
    await stuckExpectation;
  });

  it("applies a pending summary proposal and marks it applied", async () => {
    mocks.execute
      .mockResolvedValueOnce([
        {
          id: "proposal-1",
          session_id: "session-1",
          kind: "summary_replace",
          target_id: "summary-1",
          base_updated_at: "2026-08-26T00:00:00Z",
          current_markdown: "Current",
          proposed_markdown: "Proposed",
          status: "pending",
          source: "cli",
          created_at: "2026-08-26T00:00:00Z",
          updated_at: "2026-08-26T00:00:00Z",
        },
      ])
      .mockResolvedValueOnce([{ updated_at: "2026-08-26T00:00:00Z" }]);

    await applySessionProposal("proposal-1");

    const writes = mocks.executeTransaction.mock.calls.map(
      (call) => call[0] as Array<{ sql: string; params: unknown[] }>,
    );
    expect(writes[0][0].sql).toContain("UPDATE session_documents");
    expect(mocks.setSessionProposalStatus).toHaveBeenCalledWith({
      proposal_id: "proposal-1",
      status: "applied",
    });
  });

  it("rejects a stale proposal instead of writing the meeting", async () => {
    mocks.execute
      .mockResolvedValueOnce([
        {
          id: "proposal-1",
          session_id: "session-1",
          kind: "memo_replace",
          target_id: "session-1",
          base_updated_at: "2026-08-26T00:00:00Z",
          current_markdown: "Current",
          proposed_markdown: "Proposed",
          status: "pending",
          source: "mcp",
          created_at: "2026-08-26T00:00:00Z",
          updated_at: "2026-08-26T00:00:00Z",
        },
      ])
      .mockResolvedValueOnce([{ updated_at: "2026-08-26T01:00:00Z" }]);

    await expect(applySessionProposal("proposal-1")).rejects.toThrow(
      "This proposal is stale. The meeting changed after it was created.",
    );
    expect(mocks.executeTransaction).not.toHaveBeenCalled();
    expect(mocks.setSessionProposalStatus).not.toHaveBeenCalled();
  });

  it("declines only pending proposals", async () => {
    mocks.execute.mockResolvedValueOnce([
      {
        id: "proposal-1",
        session_id: "session-1",
        kind: "summary_replace",
        target_id: "summary-1",
        base_updated_at: "2026-08-26T00:00:00Z",
        current_markdown: "Current",
        proposed_markdown: "Proposed",
        status: "pending",
        source: "cli",
        created_at: "2026-08-26T00:00:00Z",
        updated_at: "2026-08-26T00:00:00Z",
      },
    ]);

    await declineSessionProposal("proposal-1");

    expect(mocks.setSessionProposalStatus).toHaveBeenCalledWith({
      proposal_id: "proposal-1",
      status: "declined",
    });
  });
});
