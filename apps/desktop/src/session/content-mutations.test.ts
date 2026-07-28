import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  executeTransaction: vi.fn(
    (
      _statements: Array<{
        sql: string;
        params: unknown[];
        expectedRowsAffected?: number;
      }>,
    ) => Promise.resolve([1, 1]),
  ),
}));

vi.mock("~/db", () => ({
  executeTransaction: mocks.executeTransaction,
}));

import {
  applyGeneratedSessionTitle,
  applySessionContentCorrections,
  persistGeneratedEnhancedNote,
} from "./content-mutations";

describe("session content SQLite corrections", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("guards every summary and transcript update against stale content", async () => {
    await applySessionContentCorrections({
      sessionId: "session-1",
      summaries: [
        {
          id: "summary-1",
          currentContent: "old summary",
          currentContentFormat: "markdown",
          nextContent: '{"type":"doc"}',
        },
      ],
      transcripts: [
        {
          id: "transcript-1",
          currentWordsJson: '[{"text":"X"}]',
          currentMemo: "Speaker: X",
          nextWordsJson: '[{"text":"Y"}]',
          nextMemo: "Speaker: Y",
        },
      ],
    });

    const statements = mocks.executeTransaction.mock.calls[0][0];
    expect(statements).toHaveLength(2);
    expect(statements[0]).toMatchObject({ expectedRowsAffected: 1 });
    expect(statements[0].sql).toContain("body = ?");
    expect(statements[0].sql).toContain("body_format = ?");
    expect(statements[1]).toMatchObject({ expectedRowsAffected: 1 });
    expect(statements[1].sql).toContain("words_json = ?");
    expect(statements[1].sql).toContain("memo = ?");
  });

  it("saves generated content and deterministic tag rows atomically", async () => {
    await persistGeneratedEnhancedNote({
      sessionId: "session-1",
      ownerUserId: "user-1",
      note: {
        id: "summary-1",
        currentContent: "old summary",
        currentContentFormat: "markdown",
        nextContent: '{"type":"doc"}',
      },
      tagNames: ["launch", "launch", "prep"],
    });

    const statements = mocks.executeTransaction.mock.calls[0][0];
    expect(statements).toHaveLength(6);
    expect(statements[0]).toMatchObject({ expectedRowsAffected: 1 });
    expect(statements[0].sql).toContain("AND body = ?");
    expect(statements[0].sql).toContain("EXISTS");
    expect(statements[1].sql).toContain("DELETE FROM app_settings");
    expect(statements[1].sql).toContain("$.noteId");
    expect(statements[1].sql).toContain("$.body");
    expect(statements[1].params).toEqual([
      "auto_enhance_pending:session-1",
      "summary-1",
      "old summary",
    ]);
    expect(statements[2].sql).toContain("INSERT INTO tags");
    expect(statements[2].params[0]).toBe("launch");
    expect(statements[3].sql).toContain("INSERT INTO session_tags");
    expect(statements[3].params[0]).toBe("session-1:launch");
    expect(
      statements
        .filter((statement) => statement !== statements[1])
        .every((statement) => statement.expectedRowsAffected === 1),
    ).toBe(true);
  });

  it("requires the exact pending generation before saving auto-enhance output", async () => {
    await persistGeneratedEnhancedNote({
      sessionId: "session-1",
      ownerUserId: "user-1",
      note: {
        id: "summary-1",
        currentContent: "old summary",
        currentContentFormat: "markdown",
        nextContent: '{"type":"doc"}',
      },
      tagNames: [],
      pendingAutoEnhance: {
        generation: "generation-1",
        expectedBody: "old summary",
        expectedContentFormat: "markdown",
      },
    });

    const statements = mocks.executeTransaction.mock.calls[0][0];
    expect(statements).toHaveLength(2);
    expect(statements[0].sql).toContain("FROM app_settings AS pending");
    expect(statements[0].sql).toContain("$.generation");
    expect(statements[0].sql).toContain("$.bodyFormat");
    expect(statements[0].sql).toContain("session_documents.body =");
    expect(statements[0].params).toEqual([
      '{"type":"doc"}',
      expect.any(String),
      "summary-1",
      "session-1",
      "old summary",
      "markdown",
      "session-1",
      "auto_enhance_pending:session-1",
      "summary-1",
      "generation-1",
      "old summary",
      "markdown",
    ]);
    expect(statements[1]).toMatchObject({ expectedRowsAffected: 1 });
    expect(statements[1].params).toEqual([
      "auto_enhance_pending:session-1",
      "summary-1",
      "generation-1",
      "old summary",
      "markdown",
    ]);
  });

  it("guards automatic speaker hints atomically without failing the summary", async () => {
    mocks.executeTransaction
      .mockResolvedValueOnce([1, 1])
      .mockRejectedValueOnce(
        new Error("transaction statement 0 affected 0 rows; expected 1"),
      );
    const consoleWarn = vi.spyOn(console, "warn").mockImplementation(() => {});

    await expect(
      persistGeneratedEnhancedNote({
        sessionId: "session-1",
        ownerUserId: "user-1",
        note: {
          id: "summary-1",
          currentContent: "old summary",
          currentContentFormat: "markdown",
          nextContent: '{"type":"doc"}',
        },
        tagNames: [],
        transcriptSpeakerHints: [
          {
            id: "transcript-1",
            currentWordsJson: '[{"id":"word-1"}]',
            currentSpeakerHintsJson: "[]",
            nextSpeakerHintsJson: '[{"type":"automatic_speaker_assignment"}]',
            expectedParticipantHumanIdsJson: '["human-1","human-2"]',
          },
          {
            id: "transcript-2",
            currentWordsJson: '[{"id":"word-2"}]',
            currentSpeakerHintsJson: "[]",
            nextSpeakerHintsJson: '[{"type":"automatic_speaker_assignment"}]',
            expectedParticipantHumanIdsJson: '["human-1","human-2"]',
          },
        ],
      }),
    ).resolves.toBeUndefined();

    expect(mocks.executeTransaction).toHaveBeenCalledTimes(2);
    const summaryStatements = mocks.executeTransaction.mock.calls[0][0];
    expect(
      summaryStatements.some((statement) =>
        statement.sql.includes("UPDATE transcripts"),
      ),
    ).toBe(false);
    const transcriptStatements = mocks.executeTransaction.mock.calls[1][0];
    expect(transcriptStatements).toHaveLength(2);
    expect(
      transcriptStatements.every(
        (statement) => statement.expectedRowsAffected === 1,
      ),
    ).toBe(true);
    const transcriptStatement = transcriptStatements[0];
    expect(transcriptStatement?.sql).toContain("words_json = ?");
    expect(transcriptStatement?.sql).toContain("speaker_hints_json = ?");
    expect(transcriptStatement?.sql).toContain("session_participants");
    expect(transcriptStatement?.sql).toContain("json_each(?)");
    expect(transcriptStatement?.params).toEqual([
      '[{"type":"automatic_speaker_assignment"}]',
      expect.any(String),
      "transcript-1",
      "session-1",
      '[{"id":"word-1"}]',
      "[]",
      "session-1",
      "user-1",
      '["human-1","human-2"]',
      '["human-1","human-2"]',
      "session-1",
      "user-1",
    ]);
    expect(consoleWarn).toHaveBeenCalledOnce();
    consoleWarn.mockRestore();
  });

  it("rolls back a generated title when any document guard is stale", async () => {
    await applyGeneratedSessionTitle({
      sessionId: "session-1",
      currentTitle: "",
      nextTitle: "Planning",
      documents: [
        {
          id: "session-1",
          currentContent: "old note",
          currentContentFormat: "markdown",
          nextContent: '{"type":"doc"}',
        },
      ],
    });

    const statements = mocks.executeTransaction.mock.calls[0][0];
    expect(statements).toHaveLength(2);
    expect(statements[0].sql).toContain("AND title = ?");
    expect(statements[0]).toMatchObject({ expectedRowsAffected: 1 });
    expect(statements[1].sql).toContain("AND body = ?");
    expect(statements[1]).toMatchObject({ expectedRowsAffected: 1 });
  });
});
