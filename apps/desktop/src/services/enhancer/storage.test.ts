import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { resolve } from "node:path";
import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  defaultSummaryDocumentId,
  resolveSummaryDocument,
  visibleSummaryDocuments,
} from "@anlg/utils/session";

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
  discardPendingAutoEnhanceJob,
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
    expect(statement.params).toEqual([
      "new-note",
      "template_output",
      "template-2",
      5,
      expect.any(String),
      expect.any(String),
      "session-1",
      "session-1",
      "template-2",
      0,
    ]);
    expect(statement.expectedRowsAffected).toBe(1);
  });

  it("marks an existing empty summary as pending auto-enhance", async () => {
    await ensurePendingAutoEnhanceDocument("session-1", "template-1");

    const statement = mocks.executeTransaction.mock.calls[0][0][0];
    expect(statement.params).toEqual([
      "auto_enhance_pending:session-1",
      '{"noteId":"existing-note","body":"","bodyFormat":"prosemirror_json","generation":"new-note"}',
      expect.any(String),
    ]);
    expect(statement.expectedRowsAffected).toBe(1);
  });

  it("discards only the exact failed auto-enhance generation", async () => {
    await discardPendingAutoEnhanceJob({
      sessionId: "session-1",
      noteId: "existing-note",
      templateId: "template-1",
      expectedBody: "",
      expectedContentFormat: "prosemirror_json",
      generation: "generation-1",
    });

    expect(mocks.enqueueDatabaseWrite).toHaveBeenCalledWith(
      "session:session-1",
      expect.any(Function),
    );
    const statement = mocks.executeTransaction.mock.calls[0][0][0];
    expect(statement.params).toEqual([
      "auto_enhance_pending:session-1",
      "existing-note",
      "generation-1",
      "",
      "prosemirror_json",
    ]);
  });

  it("persists the auto-enhance marker with a new empty summary", async () => {
    await ensurePendingAutoEnhanceDocument("session-1", "template-2");

    const statements = mocks.executeTransaction.mock.calls[0][0];
    expect(statements).toHaveLength(2);
    expect(statements[1].params).toEqual([
      "auto_enhance_pending:session-1",
      '{"noteId":"new-note","body":"","bodyFormat":"prosemirror_json","generation":"new-note"}',
      expect.any(String),
    ]);
  });

  it("maps durable pending auto-enhance rows to jobs", async () => {
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
    expect(statement.params).toEqual([
      "One-on-one",
      expect.any(String),
      "existing-note",
      "session-1",
      "template-1",
      "Summary",
    ]);
  });
  it("offline default creation converges and late summary content keeps its original document", async () => {
    const { DatabaseSync } = createRequire(import.meta.url)(
      "node:sqlite",
    ) as typeof import("node:sqlite");
    const devices = [
      new DatabaseSync(":memory:"),
      new DatabaseSync(":memory:"),
    ];
    let database = devices[0];
    const rows = () =>
      database
        .prepare(
          "SELECT * FROM session_documents WHERE kind IN ('summary', 'template_output') AND deleted_at IS NULL",
        )
        .all() as Array<{
        id: string;
        body: string;
        kind: string;
        template_id: string;
        title: string;
        sort_order: number;
        created_at: string;
        updated_at: string;
        generation_metadata_json: string;
      }>;
    try {
      for (const device of devices) {
        device.exec(
          readFileSync(
            resolve(
              process.cwd(),
              "../../crates/db-app/migrations/20260710223922_canonical_data_model.sql",
            ),
            "utf8",
          ),
        );
        device.exec(
          "INSERT INTO sessions (id, workspace_id) VALUES ('session-1', 'workspace-1')",
        );
      }
      mocks.execute.mockImplementation(
        async (sql: string, params: unknown[] = []) =>
          database
            .prepare(sql)
            .all(...(params as import("node:sqlite").SQLInputValue[])),
      );
      mocks.loadSessionContentSnapshot.mockImplementation(async () => ({
        ...createSnapshot(),
        enhancedNotes: rows().map((row) => ({
          id: row.id,
          content: row.body,
          contentFormat: "prosemirror_json",
          templateId: row.template_id,
          position: row.sort_order,
          title: row.title,
          markdown: row.body,
        })),
      }));
      mocks.executeTransaction.mockImplementation(async (statements) =>
        statements.map(({ sql, params }: { sql: string; params: unknown[] }) =>
          Number(
            database
              .prepare(sql)
              .run(...(params as import("node:sqlite").SQLInputValue[]))
              .changes,
          ),
        ),
      );
      const desktop = await ensureSummaryDocument("session-1", "template-1");
      database = devices[1];
      // Mobile uses this same identity, even when desktop selected a template.
      database
        .prepare(
          "INSERT INTO session_documents (id, session_id, kind, body) VALUES (?, 'session-1', 'summary', 'Mobile summary')",
        )
        .run(defaultSummaryDocumentId("session-1"));
      expect(desktop.id).toBe(rows()[0].id);
      expect((await ensureSummaryDocument("session-1")).id).toBe(desktop.id);
      database = devices[0];
      database.exec(
        "INSERT INTO session_documents (id, session_id, kind, title, body, created_at, updated_at) VALUES ('legacy', 'session-1', 'summary', 'Summary', 'Original desktop summary', 'old', 'old'); INSERT INTO session_documents (id, session_id, kind, title, body, created_at, updated_at) VALUES ('placeholder', 'session-1', 'summary', 'Summary', '', 'new', 'new')",
      );
      const documents = rows();
      expect(
        visibleSummaryDocuments(documents)
          .map((row) => row.id)
          .sort(),
      ).toEqual(["legacy", desktop.id]);
      expect(resolveSummaryDocument(documents)?.id).toBe("legacy");
      expect(rows()).toHaveLength(3);
      database.exec(
        "UPDATE session_documents SET body = 'Late synced edit', updated_at = 'later' WHERE id = 'placeholder'",
      );
      expect(visibleSummaryDocuments(rows())).toHaveLength(3);
      await ensureSummaryDocument("session-1", "template-2");
      expect(
        rows().filter((row) => row.kind === "template_output"),
      ).toHaveLength(2);
      database.exec(
        "UPDATE session_documents SET deleted_at = 'deleted', updated_at = 'deleted'",
      );
      expect((await ensureSummaryDocument("session-1", "template-1")).id).toBe(
        desktop.id,
      );
      expect(rows()).toHaveLength(1);
    } finally {
      devices.forEach((device) => device.close());
    }
  });
});
