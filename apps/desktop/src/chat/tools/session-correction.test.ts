import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { json2md, md2json, parseJsonContent } from "@hypr/editor/markdown";

const mocks = vi.hoisted(() => ({
  applySessionContentCorrections: vi.fn(),
  loadSessionContentSnapshot: vi.fn(),
  updateSettingValue: vi.fn(),
}));

vi.mock("~/session/content-mutations", () => ({
  applySessionContentCorrections: mocks.applySessionContentCorrections,
}));

vi.mock("~/session/content-queries", () => ({
  loadSessionContentSnapshot: mocks.loadSessionContentSnapshot,
}));

vi.mock("~/settings/queries", () => ({
  updateSettingValue: mocks.updateSettingValue,
}));

import {
  buildApplySessionCorrectionTool,
  sessionCorrectionTestInternals,
} from "./session-correction";

function summary(markdown: string, id = "note-1", title = "Summary") {
  const content = JSON.stringify(md2json(markdown));
  return {
    id,
    title,
    markdown,
    content,
    contentFormat: "prosemirror_json",
    templateId: "",
    position: 0,
  };
}

function transcript({
  words,
  memo,
}: {
  words: Array<Record<string, unknown>>;
  memo: string;
}) {
  return {
    id: "transcript-1",
    started_at: 0,
    ended_at: 400,
    memo,
    wordsJson: JSON.stringify(words),
    words,
    speaker_hints: [],
  };
}

function snapshot({
  notes = [],
  transcripts = [],
  sessionId = "session-1",
}: {
  notes?: ReturnType<typeof summary>[];
  transcripts?: ReturnType<typeof transcript>[];
  sessionId?: string;
} = {}) {
  return {
    sessionId,
    title: "Planning",
    createdAt: "2026-07-10T09:00:00.000Z",
    event: null,
    eventId: null,
    rawMarkdown: "",
    enhancedNotes: notes,
    transcripts,
    participants: [],
  };
}

function buildTool({
  sessionId = "session-1",
  enhancedNoteId,
}: {
  sessionId?: string;
  enhancedNoteId?: string;
} = {}) {
  return buildApplySessionCorrectionTool({
    getSessionId: () => sessionId,
    getEnhancedNoteId: () => enhancedNoteId,
  });
}

describe("session correction chat tool", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.spyOn(console, "error").mockImplementation(() => {});
    mocks.applySessionContentCorrections.mockResolvedValue(undefined);
    mocks.updateSettingValue.mockImplementation(async (_key, update) =>
      update("[]"),
    );
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("plans exact summary replacements without mutating the snapshot", () => {
    const note = summary("Discussed X roadmap.");

    const plan = sessionCorrectionTestInternals.planSummaryCorrections({
      notes: [note],
      oldText: "X roadmap",
      newText: "Y roadmap",
    });

    expect(plan.changes).toEqual([
      {
        enhancedNoteId: "note-1",
        title: "Summary",
        replacements: 1,
      },
    ]);
    expect(json2md(parseJsonContent(plan.updates[0]?.nextContent))).toContain(
      "Y roadmap",
    );
    expect(note.markdown).toContain("X roadmap");
  });

  it("plans transcript word and memo corrections together", () => {
    const source = transcript({
      words: [
        { id: "w1", text: "It", start_ms: 0, end_ms: 100, channel: 0 },
        { id: "w2", text: "is", start_ms: 100, end_ms: 200, channel: 0 },
        { id: "w3", text: "X", start_ms: 200, end_ms: 300, channel: 0 },
      ],
      memo: "Speaker 1: It is X",
    });

    const plan = sessionCorrectionTestInternals.planTranscriptCorrections({
      transcripts: [source] as any,
      oldText: "X",
      newText: "Y",
    });

    expect(plan.changes).toEqual([
      {
        transcriptId: "transcript-1",
        wordReplacements: 1,
        memoReplacements: 1,
      },
    ]);
    expect(JSON.parse(plan.updates[0]!.nextWordsJson)).toMatchObject([
      { text: "It" },
      { text: "is" },
      { text: "Y" },
    ]);
    expect(plan.updates[0]?.nextMemo).toBe("Speaker 1: It is Y");
    expect(source.memo).toBe("Speaker 1: It is X");
  });

  it("updates every repeated transcript phrase", () => {
    const plan = sessionCorrectionTestInternals.planTranscriptCorrections({
      transcripts: [
        transcript({
          words: [
            { id: "w1", text: "X", start_ms: 0, end_ms: 100, channel: 0 },
            { id: "w2", text: "then", start_ms: 100, end_ms: 200, channel: 0 },
            { id: "w3", text: "X", start_ms: 200, end_ms: 300, channel: 0 },
          ],
          memo: "Speaker 1: X then X",
        }),
      ] as any,
      oldText: "X",
      newText: "Y",
    });

    expect(plan.changes[0]).toMatchObject({
      wordReplacements: 2,
      memoReplacements: 2,
    });
    expect(plan.updates[0]?.nextMemo).toBe("Speaker 1: Y then Y");
  });

  it("does not plan a partial transcript row update", () => {
    const plan = sessionCorrectionTestInternals.planTranscriptCorrections({
      transcripts: [
        transcript({
          words: [
            { id: "w1", text: "X", start_ms: 0, end_ms: 100, channel: 0 },
          ],
          memo: "Speaker 1: no correction here",
        }),
      ] as any,
      oldText: "X",
      newText: "Y",
    });

    expect(plan).toEqual({ changes: [], updates: [] });
  });

  it("does not remove transcript words for blank replacement text", () => {
    const plan = sessionCorrectionTestInternals.planTranscriptCorrections({
      transcripts: [
        transcript({
          words: [
            { id: "w1", text: "X", start_ms: 0, end_ms: 100, channel: 0 },
          ],
          memo: "Speaker 1: X",
        }),
      ] as any,
      oldText: "X",
      newText: "   ",
    });

    expect(plan).toEqual({ changes: [], updates: [] });
  });

  it("can replace transcript phrases with a different word count", () => {
    const result = sessionCorrectionTestInternals.replaceTranscriptWords(
      [
        { id: "w1", text: "not", start_ms: 0, end_ms: 100, channel: 0 },
        { id: "w2", text: "X", start_ms: 100, end_ms: 300, channel: 0 },
      ],
      "not X",
      "Y instead",
    );

    expect(result.count).toBe(1);
    expect(result.words).toMatchObject([
      { id: "w1", text: "Y", start_ms: 0, end_ms: 150 },
      { id: "w1:correction:1", text: "instead", start_ms: 150, end_ms: 300 },
    ]);
  });

  it("commits summary and transcript corrections before dictionary terms", async () => {
    mocks.loadSessionContentSnapshot.mockResolvedValue(
      snapshot({
        notes: [
          summary("Sam (from Airborne Brothers) liked the OpenWorld concept."),
        ],
        transcripts: [
          transcript({
            words: [
              { id: "w1", text: "sam", start_ms: 0, end_ms: 100, channel: 0 },
              {
                id: "w2",
                text: "from",
                start_ms: 100,
                end_ms: 200,
                channel: 0,
              },
              {
                id: "w3",
                text: "Airborne",
                start_ms: 200,
                end_ms: 300,
                channel: 0,
              },
              {
                id: "w4",
                text: "Brothers,",
                start_ms: 300,
                end_ms: 400,
                channel: 0,
              },
            ],
            memo: "Speaker 1: sam from Airborne Brothers, liked it.",
          }),
        ],
      }),
    );
    let persistedDictionary = "";
    mocks.updateSettingValue.mockImplementation(async (_key, update) => {
      persistedDictionary = update(JSON.stringify(["Anarlog"]));
      return persistedDictionary;
    });

    const result = await (
      buildTool({ enhancedNoteId: "note-1" }) as any
    ).execute({
      oldText: "Sam (from Airborne Brothers)",
      newText: "Tim from Erebor",
      dictionaryTerms: ["Erebor"],
    });

    expect(result).toMatchObject({
      status: "applied",
      summaryChanges: [{ enhancedNoteId: "note-1", replacements: 1 }],
      transcriptChanges: [
        {
          transcriptId: "transcript-1",
          wordReplacements: 1,
          memoReplacements: 1,
        },
      ],
      dictionaryChanges: { addedTerms: ["Erebor"] },
    });
    expect(mocks.applySessionContentCorrections).toHaveBeenCalledWith(
      expect.objectContaining({
        sessionId: "session-1",
        summaries: [expect.objectContaining({ id: "note-1" })],
        transcripts: [expect.objectContaining({ id: "transcript-1" })],
      }),
    );
    expect(
      mocks.applySessionContentCorrections.mock.invocationCallOrder[0],
    ).toBeLessThan(mocks.updateSettingValue.mock.invocationCallOrder[0]);
    expect(persistedDictionary).toBe(JSON.stringify(["Anarlog", "Erebor"]));
  });

  it("reports partial success when a requested target does not match", async () => {
    mocks.loadSessionContentSnapshot.mockResolvedValue(
      snapshot({ notes: [summary("Discussed X roadmap.")] }),
    );

    const result = await (buildTool() as any).execute({
      oldText: "X roadmap",
      newText: "Y roadmap",
    });

    expect(result).toMatchObject({
      status: "partial",
      message:
        "Applied correction where matched, but no matching transcript text was found.",
      summaryChanges: [{ enhancedNoteId: "note-1", replacements: 1 }],
      transcriptChanges: [],
    });
  });

  it("returns an explicit error for a summary id outside the session", async () => {
    mocks.loadSessionContentSnapshot.mockResolvedValue(
      snapshot({ notes: [summary("Discussed X roadmap.")] }),
    );

    const result = await (buildTool() as any).execute({
      target: "summary",
      enhancedNoteId: "missing-note",
      oldText: "X roadmap",
      newText: "Y roadmap",
    });

    expect(result).toEqual({
      status: "error",
      message: "The requested summary does not belong to the target session.",
      sessionId: "session-1",
    });
    expect(mocks.applySessionContentCorrections).not.toHaveBeenCalled();
  });

  it("still corrects the transcript when the default target has an invalid summary id", async () => {
    mocks.loadSessionContentSnapshot.mockResolvedValue(
      snapshot({
        notes: [summary("No correction here.")],
        transcripts: [
          transcript({
            words: [
              { id: "w1", text: "X", start_ms: 0, end_ms: 100, channel: 0 },
            ],
            memo: "Speaker 1: X",
          }),
        ],
      }),
    );

    const result = await (buildTool() as any).execute({
      enhancedNoteId: "missing-note",
      oldText: "X",
      newText: "Y",
    });

    expect(result).toMatchObject({
      status: "applied",
      summaryChanges: [],
      transcriptChanges: [{ transcriptId: "transcript-1" }],
    });
  });

  it("defaults summary correction to the active enhanced note", async () => {
    mocks.loadSessionContentSnapshot.mockResolvedValue(
      snapshot({
        notes: [
          summary("Discussed X roadmap.", "note-1", "Summary"),
          summary("Discussed X roadmap.", "note-2", "Other"),
        ],
      }),
    );

    const result = await (
      buildTool({ enhancedNoteId: "note-1" }) as any
    ).execute({
      target: "summary",
      oldText: "X roadmap",
      newText: "Y roadmap",
    });

    expect(result.summaryChanges).toEqual([
      {
        enhancedNoteId: "note-1",
        title: "Summary",
        replacements: 1,
      },
    ]);
    expect(
      mocks.applySessionContentCorrections.mock.calls[0][0].summaries,
    ).toEqual([expect.objectContaining({ id: "note-1" })]);
  });

  it("does not use the active summary for an explicit session", async () => {
    mocks.loadSessionContentSnapshot.mockResolvedValue(
      snapshot({
        sessionId: "session-2",
        notes: [summary("Discussed X roadmap.", "note-2", "Target")],
      }),
    );

    const result = await (
      buildTool({ enhancedNoteId: "note-1" }) as any
    ).execute({
      sessionId: "session-2",
      target: "summary",
      oldText: "X roadmap",
      newText: "Y roadmap",
    });

    expect(result).toMatchObject({
      status: "applied",
      sessionId: "session-2",
      summaryChanges: [{ enhancedNoteId: "note-2", title: "Target" }],
    });
  });

  it("reports a stale transaction instead of saving dictionary terms", async () => {
    mocks.loadSessionContentSnapshot.mockResolvedValue(
      snapshot({ notes: [summary("Discussed X roadmap.")] }),
    );
    mocks.applySessionContentCorrections.mockRejectedValueOnce(
      new Error("expected 1 row"),
    );

    const result = await (buildTool() as any).execute({
      target: "summary",
      oldText: "X roadmap",
      newText: "Y roadmap",
      dictionaryTerms: ["Y"],
    });

    expect(result).toEqual({
      status: "error",
      message:
        "The note changed before the correction could be committed. Read the note and retry.",
      sessionId: "session-1",
    });
    expect(mocks.updateSettingValue).not.toHaveBeenCalled();
  });

  it("does not report a durable correction as failed when dictionary storage fails", async () => {
    mocks.loadSessionContentSnapshot.mockResolvedValue(
      snapshot({ notes: [summary("Discussed X roadmap.")] }),
    );
    mocks.updateSettingValue.mockRejectedValueOnce(new Error("settings busy"));

    const result = await (buildTool() as any).execute({
      target: "summary",
      oldText: "X roadmap",
      newText: "Y roadmap",
      dictionaryTerms: ["Y roadmap"],
    });

    expect(result).toMatchObject({
      status: "applied",
      message:
        "The correction was applied, but dictionary terms could not be saved.",
      summaryChanges: [{ enhancedNoteId: "note-1" }],
      dictionaryChanges: { addedTerms: [] },
    });
  });
});
