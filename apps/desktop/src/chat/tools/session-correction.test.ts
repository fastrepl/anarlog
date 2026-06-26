import { describe, expect, it, vi } from "vitest";

import { md2json } from "@hypr/editor/markdown";

import {
  buildApplySessionCorrectionTool,
  sessionCorrectionTestInternals,
} from "./session-correction";

function createStore(tables: Record<string, Record<string, any>>) {
  return {
    getCell: vi.fn((table: string, rowId: string, cellId: string) => {
      return tables[table]?.[rowId]?.[cellId];
    }),
    setCell: vi.fn(
      (table: string, rowId: string, cellId: string, value: unknown) => {
        tables[table][rowId][cellId] = value;
      },
    ),
    setPartialRow: vi.fn(
      (table: string, rowId: string, partial: Record<string, unknown>) => {
        tables[table][rowId] = {
          ...tables[table][rowId],
          ...partial,
        };
      },
    ),
  } as any;
}

function createIndexes(tables: Record<string, Record<string, any>>) {
  return {
    getSliceRowIds: vi.fn((indexId: string, sessionId: string) => {
      if (indexId === "enhancedNotesBySession") {
        return Object.keys(tables.enhanced_notes ?? {}).filter(
          (id) => tables.enhanced_notes[id]?.session_id === sessionId,
        );
      }

      if (indexId === "transcriptBySession") {
        return Object.keys(tables.transcripts ?? {}).filter(
          (id) => tables.transcripts[id]?.session_id === sessionId,
        );
      }

      return [];
    }),
  } as any;
}

function summaryContent(markdown: string) {
  return JSON.stringify(md2json(markdown));
}

describe("session correction chat tool internals", () => {
  it("applies exact summary corrections to matching enhanced notes", () => {
    const tables = {
      enhanced_notes: {
        "note-1": {
          session_id: "session-1",
          title: "Summary",
          content: summaryContent("Discussed X roadmap."),
        },
        "note-2": {
          session_id: "session-1",
          title: "Tasks",
          content: summaryContent("No correction here."),
        },
      },
    };
    const store = createStore(tables);
    const indexes = createIndexes(tables);

    const changes = sessionCorrectionTestInternals.applySummaryCorrection({
      store,
      indexes,
      sessionId: "session-1",
      oldText: "X roadmap",
      newText: "Y roadmap",
    });

    expect(changes).toEqual([
      {
        enhancedNoteId: "note-1",
        title: "Summary",
        replacements: 1,
      },
    ]);
    expect(tables.enhanced_notes["note-1"].content).toContain("Y roadmap");
    expect(tables.enhanced_notes["note-2"].content).toContain(
      "No correction here.",
    );
  });

  it("updates transcript words and memo markdown for exact corrections", () => {
    const tables = {
      transcripts: {
        "transcript-1": {
          session_id: "session-1",
          words: JSON.stringify([
            { id: "w1", text: "It", start_ms: 0, end_ms: 100, channel: 0 },
            { id: "w2", text: "is", start_ms: 100, end_ms: 200, channel: 0 },
            { id: "w3", text: "X", start_ms: 200, end_ms: 300, channel: 0 },
          ]),
          memo_md: "Speaker 1: It is X",
        },
      },
    };
    const store = createStore(tables);
    const indexes = createIndexes(tables);

    const changes = sessionCorrectionTestInternals.applyTranscriptCorrection({
      store,
      indexes,
      sessionId: "session-1",
      oldText: "X",
      newText: "Y",
    });

    expect(changes).toEqual([
      {
        transcriptId: "transcript-1",
        wordReplacements: 1,
        memoReplacements: 1,
      },
    ]);
    expect(JSON.parse(tables.transcripts["transcript-1"].words)).toMatchObject([
      { text: "It" },
      { text: "is" },
      { text: "Y" },
    ]);
    expect(tables.transcripts["transcript-1"].memo_md).toBe(
      "Speaker 1: It is Y",
    );
  });

  it("updates every matching transcript word span when memo has repeated text", () => {
    const tables = {
      transcripts: {
        "transcript-1": {
          session_id: "session-1",
          words: JSON.stringify([
            { id: "w1", text: "X", start_ms: 0, end_ms: 100, channel: 0 },
            { id: "w2", text: "then", start_ms: 100, end_ms: 200, channel: 0 },
            { id: "w3", text: "X", start_ms: 200, end_ms: 300, channel: 0 },
          ]),
          memo_md: "Speaker 1: X then X",
        },
      },
    };
    const store = createStore(tables);
    const indexes = createIndexes(tables);

    const changes = sessionCorrectionTestInternals.applyTranscriptCorrection({
      store,
      indexes,
      sessionId: "session-1",
      oldText: "X",
      newText: "Y",
    });

    expect(changes).toEqual([
      {
        transcriptId: "transcript-1",
        wordReplacements: 2,
        memoReplacements: 2,
      },
    ]);
    expect(JSON.parse(tables.transcripts["transcript-1"].words)).toMatchObject([
      { text: "Y" },
      { text: "then" },
      { text: "Y" },
    ]);
    expect(tables.transcripts["transcript-1"].memo_md).toBe(
      "Speaker 1: Y then Y",
    );
  });

  it("does not partially update transcript rows when words and memo do not both match", () => {
    const tables = {
      transcripts: {
        "transcript-1": {
          session_id: "session-1",
          words: JSON.stringify([
            { id: "w1", text: "X", start_ms: 0, end_ms: 100, channel: 0 },
          ]),
          memo_md: "Speaker 1: no correction here",
        },
      },
    };
    const store = createStore(tables);
    const indexes = createIndexes(tables);

    const changes = sessionCorrectionTestInternals.applyTranscriptCorrection({
      store,
      indexes,
      sessionId: "session-1",
      oldText: "X",
      newText: "Y",
    });

    expect(changes).toEqual([]);
    expect(store.setCell).not.toHaveBeenCalled();
    expect(JSON.parse(tables.transcripts["transcript-1"].words)).toMatchObject([
      { text: "X" },
    ]);
    expect(tables.transcripts["transcript-1"].memo_md).toBe(
      "Speaker 1: no correction here",
    );
  });

  it("does not remove transcript words for blank replacement text", () => {
    const tables = {
      transcripts: {
        "transcript-1": {
          session_id: "session-1",
          words: JSON.stringify([
            { id: "w1", text: "X", start_ms: 0, end_ms: 100, channel: 0 },
          ]),
          memo_md: "Speaker 1: X",
        },
      },
    };
    const store = createStore(tables);
    const indexes = createIndexes(tables);

    const changes = sessionCorrectionTestInternals.applyTranscriptCorrection({
      store,
      indexes,
      sessionId: "session-1",
      oldText: "X",
      newText: "   ",
    });

    expect(changes).toEqual([]);
    expect(store.setCell).not.toHaveBeenCalled();
    expect(JSON.parse(tables.transcripts["transcript-1"].words)).toMatchObject([
      { text: "X" },
    ]);
    expect(tables.transcripts["transcript-1"].memo_md).toBe("Speaker 1: X");
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

  it("returns an explicit error for a summary id outside the session", async () => {
    const tables = {
      enhanced_notes: {
        "note-1": {
          session_id: "session-1",
          title: "Summary",
          content: summaryContent("Discussed X roadmap."),
        },
      },
      transcripts: {},
    };
    const store = createStore(tables);
    const indexes = createIndexes(tables);
    const tool = buildApplySessionCorrectionTool({
      getStore: () => store,
      getIndexes: () => indexes,
      getSessionId: () => "session-1",
      getEnhancedNoteId: () => undefined,
    });

    const result = await (tool as any).execute({
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
    expect(store.setPartialRow).not.toHaveBeenCalled();
  });

  it("still applies transcript correction when an invalid summary id is provided for the default target", async () => {
    const tables = {
      enhanced_notes: {
        "note-1": {
          session_id: "session-1",
          title: "Summary",
          content: summaryContent("No correction here."),
        },
      },
      transcripts: {
        "transcript-1": {
          session_id: "session-1",
          words: JSON.stringify([
            { id: "w1", text: "X", start_ms: 0, end_ms: 100, channel: 0 },
          ]),
          memo_md: "Speaker 1: X",
        },
      },
    };
    const store = createStore(tables);
    const indexes = createIndexes(tables);
    const tool = buildApplySessionCorrectionTool({
      getStore: () => store,
      getIndexes: () => indexes,
      getSessionId: () => "session-1",
      getEnhancedNoteId: () => undefined,
    });

    const result = await (tool as any).execute({
      enhancedNoteId: "missing-note",
      oldText: "X",
      newText: "Y",
    });

    expect(result).toMatchObject({
      status: "applied",
      sessionId: "session-1",
      summaryChanges: [],
      transcriptChanges: [
        {
          transcriptId: "transcript-1",
          wordReplacements: 1,
          memoReplacements: 1,
        },
      ],
    });
    expect(JSON.parse(tables.transcripts["transcript-1"].words)).toMatchObject([
      { text: "Y" },
    ]);
    expect(tables.transcripts["transcript-1"].memo_md).toBe("Speaker 1: Y");
  });

  it("defaults summary correction to the active enhanced note", async () => {
    const tables = {
      enhanced_notes: {
        "note-1": {
          session_id: "session-1",
          title: "Summary",
          content: summaryContent("Discussed X roadmap."),
        },
        "note-2": {
          session_id: "session-1",
          title: "Other",
          content: summaryContent("Discussed X roadmap."),
        },
      },
      transcripts: {},
    };
    const store = createStore(tables);
    const indexes = createIndexes(tables);
    const tool = buildApplySessionCorrectionTool({
      getStore: () => store,
      getIndexes: () => indexes,
      getSessionId: () => "session-1",
      getEnhancedNoteId: () => "note-1",
    });

    const result = await (tool as any).execute({
      target: "summary",
      oldText: "X roadmap",
      newText: "Y roadmap",
    });

    expect(result).toMatchObject({
      status: "applied",
      summaryChanges: [
        {
          enhancedNoteId: "note-1",
          title: "Summary",
          replacements: 1,
        },
      ],
    });
    expect(tables.enhanced_notes["note-1"].content).toContain("Y roadmap");
    expect(tables.enhanced_notes["note-2"].content).toContain("X roadmap");
  });

  it("does not use the active enhanced note for explicit session corrections", async () => {
    const tables = {
      enhanced_notes: {
        "note-1": {
          session_id: "session-1",
          title: "Current",
          content: summaryContent("Discussed X roadmap."),
        },
        "note-2": {
          session_id: "session-2",
          title: "Target",
          content: summaryContent("Discussed X roadmap."),
        },
      },
      transcripts: {},
    };
    const store = createStore(tables);
    const indexes = createIndexes(tables);
    const tool = buildApplySessionCorrectionTool({
      getStore: () => store,
      getIndexes: () => indexes,
      getSessionId: () => "session-1",
      getEnhancedNoteId: () => "note-1",
    });

    const result = await (tool as any).execute({
      sessionId: "session-2",
      target: "summary",
      oldText: "X roadmap",
      newText: "Y roadmap",
    });

    expect(result).toMatchObject({
      status: "applied",
      sessionId: "session-2",
      summaryChanges: [
        {
          enhancedNoteId: "note-2",
          title: "Target",
          replacements: 1,
        },
      ],
    });
    expect(tables.enhanced_notes["note-1"].content).toContain("X roadmap");
    expect(tables.enhanced_notes["note-2"].content).toContain("Y roadmap");
  });
});
