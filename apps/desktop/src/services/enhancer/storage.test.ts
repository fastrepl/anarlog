import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  executeTransaction: vi.fn().mockResolvedValue([1]),
  loadSessionContentSnapshot: vi.fn(),
  enqueueDatabaseWrite: vi.fn((_key: string, write: () => Promise<unknown>) =>
    write(),
  ),
}));

vi.mock("~/db", () => ({
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
  ensureSummaryDocument,
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
