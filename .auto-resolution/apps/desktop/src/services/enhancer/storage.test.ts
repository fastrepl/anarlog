import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  execute: vi.fn(),
  executeTransaction: vi.fn().mockResolvedValue([1]),
  loadSessionContentSnapshot: vi.fn(),
  enqueueDatabaseWrite: vi.fn((_key: string, write: () => Promise<unknown>) =>
    write(),
  ),
}));

vi.mock("~/db", () => ({
  liveQueryClient: { execute: mocks.execute },
  executeTransaction: mocks.executeTransaction,
}));

vi.mock("~/db/write-queue", () => ({
  enqueueDatabaseWrite: mocks.enqueueDatabaseWrite,
}));

vi.mock("~/session/content-queries", () => ({
  loadSessionContentSnapshot: mocks.loadSessionContentSnapshot,
}));

vi.mock("~/shared/utils", () => ({
  id: () => "new-note",
}));

import {
  ensurePendingAutoEnhanceDocument,
  ensureSummaryDocument,
  loadPendingAutoEnhanceJobs,
  replaceSummaryDocumentTemplate,
  updateSummaryDocumentTitleIfCurrent,
} from "./storage";

function createSnapshot() {
  return {
    sessionId: "session-1",
    ownerUserId: "user-1",
    title: "Planning",
    createdAt: "2026-07-10T00:00:00.000Z",
    event: null,
    eventId: null,
    rawNoteId: "session-1",
    rawContent: "",
    rawContentFormat: "prosemirror_json",
    rawMarkdown: "",
    enhancedNotes: [
      {
        id: "existing-note",
        title: "Summary",
        markdown: "",
        content: "",
        contentFormat: "prosemirror_json",
        templateId: "template-1",
        position: 4,
      },
    ],
    transcripts: [],
    participants: [],
  };
}

describe("enhancer SQLite storage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.execute.mockResolvedValue([]);
    mocks.executeTransaction.mockResolvedValue([1]);
    mocks.loadSessionContentSnapshot.mockResolvedValue(createSnapshot());
  });

  it("returns the existing note for the same template", async () => {
    await expect(
      ensureSummaryDocument("session-1", "template-1"),
    ).resolves.toMatchObject({ id: "existing-note" });
    expect(mocks.executeTransaction).not.toHaveBeenCalled();
  });

  it("serializes creation and inserts the next stable position", async () => {
    const result = await ensureSummaryDocument("session-1", "template-2");

    expect(result).toMatchObject({
      id: "new-note",
      templateId: "template-2",
      position: 5,
    });
    expect(mocks.enqueueDatabaseWrite).toHaveBeenCalledWith(
      "session:session-1",
      expect.any(Function),
    );
    const statement = mocks.executeTransaction.mock.calls[0][0][0];
    expect(statement.sql).toContain("INSERT INTO session_documents");
    expect(statement.sql).toContain("workspace_id");
    expect(statement.sql).toContain("?, workspace_id, id");
    expect(statement.params).toEqual([
      "new-note",
      "template_output",
      "template-2",
      5,
      expect.any(String),
      expect.any(String),
      "session-1",
    ]);
    expect(statement.expectedRowsAffected).toBe(1);
  });

  it("marks an existing empty summary as pending auto-enhance", async () => {
    await ensurePendingAutoEnhanceDocument("session-1", "template-1");

    const statement = mocks.executeTransaction.mock.calls[0][0][0];
    expect(statement.sql).toContain("INSERT INTO app_settings");
    expect(statement.params).toEqual([
      "auto_enhance_pending:session-1",
      '{"noteId":"existing-note","body":"","bodyFormat":"prosemirror_json","generation":"new-note"}',
      expect.any(String),
    ]);
    expect(statement.expectedRowsAffected).toBe(1);
  });

  it("persists the auto-enhance marker with a new empty summary", async () => {
    await ensurePendingAutoEnhanceDocument("session-1", "template-2");

    const statements = mocks.executeTransaction.mock.calls[0][0];
    expect(statements).toHaveLength(2);
    expect(statements[0].sql).toContain("INSERT INTO session_documents");
    expect(statements[1].sql).toContain("INSERT INTO app_settings");
    expect(statements[1].params).toEqual([
      "auto_enhance_pending:session-1",
      '{"noteId":"new-note","body":"","bodyFormat":"prosemirror_json","generation":"new-note"}',
      expect.any(String),
    ]);
  });

  it("loads only durable pending summaries with transcript words", async () => {
    mocks.execute.mockResolvedValue([
      {
        session_id: "session-1",
        note_id: "note-1",
        template_id: "",
        expected_body: "",
        expected_content_format: "prosemirror_json",
        generation: "generation-1",
      },
      {
        session_id: "session-2",
        note_id: "note-2",
        template_id: "template-2",
        expected_body: "Previous",
        expected_content_format: "markdown",
        generation: "generation-2",
      },
    ]);

    await expect(loadPendingAutoEnhanceJobs()).resolves.toEqual([
      {
        sessionId: "session-1",
        noteId: "note-1",
        templateId: "",
        expectedBody: "",
        expectedContentFormat: "prosemirror_json",
        generation: "generation-1",
      },
      {
        sessionId: "session-2",
        noteId: "note-2",
        templateId: "template-2",
        expectedBody: "Previous",
        expectedContentFormat: "markdown",
        generation: "generation-2",
      },
    ]);

    const [sql, params] = mocks.execute.mock.calls[0];
    expect(sql).toContain("FROM app_settings AS setting");
    expect(sql).toContain("document.body = json_extract");
    expect(sql).toContain("$.body");
    expect(sql).toContain("$.bodyFormat");
    expect(sql).toContain("$.generation");
    expect(sql).toContain("json_array_length(transcript.words_json) > 0");
    expect(params).toEqual([22, 22, "auto_enhance_pending:%"]);
  });

  it("does not create a summary for a deleted session", async () => {
    mocks.loadSessionContentSnapshot.mockResolvedValue(null);

    await expect(ensureSummaryDocument("missing")).rejects.toThrow(
      "Session missing no longer exists",
    );
    expect(mocks.executeTransaction).not.toHaveBeenCalled();
  });

  it("replaces a target summary through one checked update", async () => {
    await replaceSummaryDocumentTemplate({
      sessionId: "session-1",
      noteId: "existing-note",
      templateId: "template-2",
      title: "Customer review",
    });

    const statement = mocks.executeTransaction.mock.calls[0][0][0];
    expect(statement.sql).toContain("body = ''");
    expect(statement.params).toContain("template_output");
    expect(statement.params).toContain("Customer review");
    expect(statement.expectedRowsAffected).toBe(1);
  });

  it("hydrates a title only while template and placeholder title still match", async () => {
    await updateSummaryDocumentTitleIfCurrent({
      sessionId: "session-1",
      noteId: "existing-note",
      templateId: "template-1",
      currentTitle: "Summary",
      nextTitle: "One-on-one",
    });

    const statement = mocks.executeTransaction.mock.calls[0][0][0];
    expect(statement.sql).toContain("AND template_id = ?");
    expect(statement.sql).toContain("AND title = ?");
    expect(statement.params).toEqual([
      "One-on-one",
      expect.any(String),
      "existing-note",
      "session-1",
      "template-1",
      "Summary",
    ]);
  });
});
