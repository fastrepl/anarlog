import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  loadSessionContentSnapshot: vi.fn(),
  executeTransaction: vi.fn(),
  liveQueryExecute: vi.fn(),
  audioExist: vi.fn(),
  audioCopy: vi.fn(),
  catalogLocalSessionAudio: vi.fn(),
  deleteSessionAudio: vi.fn(),
  live: {
    sessionId: null as string | null,
    status: "inactive" as string,
    finalizingBySession: {} as Record<string, unknown>,
    batchTranscriptionPendingBySession: {} as Record<string, boolean>,
    postStopProcessingBySession: {} as Record<string, boolean>,
  },
}));

vi.mock("./content-queries", () => ({
  loadSessionContentSnapshot: mocks.loadSessionContentSnapshot,
}));

vi.mock("./attachments", () => ({
  catalogLocalSessionAudio: mocks.catalogLocalSessionAudio,
  deleteSessionAudio: mocks.deleteSessionAudio,
}));

vi.mock("~/db", () => ({
  executeTransaction: mocks.executeTransaction,
  liveQueryClient: {
    execute: mocks.liveQueryExecute,
  },
}));

vi.mock("@anlg/plugin-fs-sync", () => ({
  commands: {
    audioExist: mocks.audioExist,
    audioCopy: mocks.audioCopy,
  },
}));

vi.mock("~/store/zustand/listener/instance", () => ({
  listenerStore: {
    getState: () => ({ live: mocks.live }),
  },
}));

import { moveSessionContents } from "./move-contents";

function snapshot({
  sessionId,
  title,
  rawMarkdown = "",
  rawContent = "",
  transcripts = 0,
  summaries = 0,
}: {
  sessionId: string;
  title: string;
  rawMarkdown?: string;
  rawContent?: string;
  transcripts?: number;
  summaries?: number;
}) {
  return {
    sessionId,
    ownerUserId: "user-1",
    title,
    createdAt: "2026-08-19T12:00:00.000Z",
    event: null,
    eventId: null,
    rawNoteId: sessionId,
    rawTemplateId: "",
    rawContent,
    rawContentFormat: "prosemirror_json",
    rawMarkdown,
    enhancedNotes: Array.from({ length: summaries }, (_, index) => ({
      id: `summary-${index}`,
      title: "Summary",
      markdown: "Notes",
      content: "{}",
      contentFormat: "prosemirror_json",
      templateId: "",
      position: index,
    })),
    transcripts: Array.from({ length: transcripts }, (_, index) => ({
      id: `transcript-${index}`,
      started_at: 0,
      ended_at: 1000,
      memo: "hello",
      wordsJson: "[]",
      speakerHintsJson: "[]",
      words: [],
      speaker_hints: [],
    })),
    participants: [],
  };
}

describe("moveSessionContents", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.live.sessionId = null;
    mocks.live.status = "inactive";
    mocks.live.finalizingBySession = {};
    mocks.live.batchTranscriptionPendingBySession = {};
    mocks.live.postStopProcessingBySession = {};
    mocks.executeTransaction.mockResolvedValue([1, 1, 1, 0, 0]);
    mocks.liveQueryExecute.mockResolvedValue([{ action_item_count: 2 }]);
    mocks.audioExist.mockImplementation(async (sessionId: string) => ({
      status: "ok",
      data: sessionId === "source",
    }));
    mocks.audioCopy.mockResolvedValue({ status: "ok", data: true });
    mocks.catalogLocalSessionAudio.mockResolvedValue(undefined);
    mocks.deleteSessionAudio.mockResolvedValue(true);
    mocks.loadSessionContentSnapshot.mockImplementation(
      async (sessionId: string) => {
        if (sessionId === "source") {
          return snapshot({
            sessionId: "source",
            title: "Standup",
            rawMarkdown: "Wrong place",
            rawContent: '{"type":"doc"}',
            transcripts: 1,
            summaries: 1,
          });
        }
        if (sessionId === "target") {
          return snapshot({
            sessionId: "target",
            title: "Board",
          });
        }
        return null;
      },
    );
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("moves recording, transcript, summary, notes, and action items", async () => {
    await expect(
      moveSessionContents({
        sourceSessionId: "source",
        targetSessionId: "target",
      }),
    ).resolves.toEqual({
      status: "moved",
      sourceMeetingId: "source",
      targetMeetingId: "target",
      sourceTitle: "Standup",
      targetTitle: "Board",
      moved: {
        recording: true,
        transcripts: 1,
        summaries: 1,
        notes: true,
        actionItems: 2,
      },
    });

    expect(mocks.audioCopy).toHaveBeenCalledWith("source", "target");
    expect(mocks.catalogLocalSessionAudio).toHaveBeenCalledWith("target");
    expect(mocks.deleteSessionAudio).toHaveBeenCalledWith(
      "source",
      expect.any(Function),
    );

    const statements = mocks.executeTransaction.mock.calls[0][0];
    expect(statements[0].sql).toContain("UPDATE transcripts");
    expect(statements[0].params).toEqual([
      "target",
      1,
      "session-audio:source",
      "session-audio:target",
      expect.any(String),
      "source",
    ]);
    expect(statements[1].sql).toContain(
      "kind IN ('summary', 'template_output')",
    );
    expect(statements[2].sql).toContain("UPDATE action_items");
    expect(statements[5].params[0]).toBe('{"type":"doc"}');
    expect(statements[5].params[2]).toBe("target");
    expect(statements[6].params[2]).toBe("source");
  });

  it("refuses to overwrite a target that already has a transcript", async () => {
    mocks.audioExist.mockResolvedValue({ status: "ok", data: false });
    mocks.loadSessionContentSnapshot.mockImplementation(
      async (sessionId: string) => {
        if (sessionId === "source") {
          return snapshot({
            sessionId: "source",
            title: "Standup",
            transcripts: 1,
          });
        }
        return snapshot({
          sessionId: "target",
          title: "Board",
          transcripts: 1,
        });
      },
    );

    await expect(
      moveSessionContents({
        sourceSessionId: "source",
        targetSessionId: "target",
      }),
    ).resolves.toMatchObject({
      status: "error",
      message: expect.stringContaining("already has a recording or transcript"),
    });
    expect(mocks.audioCopy).not.toHaveBeenCalled();
    expect(mocks.executeTransaction).not.toHaveBeenCalled();
  });

  it("refuses while either meeting is still recording", async () => {
    mocks.live.sessionId = "source";
    mocks.live.status = "active";

    await expect(
      moveSessionContents({
        sourceSessionId: "source",
        targetSessionId: "target",
      }),
    ).resolves.toMatchObject({
      status: "error",
      message: expect.stringContaining("recording and transcription finish"),
    });
    expect(mocks.audioCopy).not.toHaveBeenCalled();
  });

  it("rolls back a copied recording if the database write fails", async () => {
    vi.spyOn(console, "error").mockImplementation(() => {});
    mocks.executeTransaction.mockRejectedValue(new Error("busy"));

    await expect(
      moveSessionContents({
        sourceSessionId: "source",
        targetSessionId: "target",
      }),
    ).resolves.toMatchObject({
      status: "error",
      message: "The move could not be completed. Nothing was changed.",
    });
    expect(mocks.deleteSessionAudio).toHaveBeenCalledWith(
      "target",
      expect.any(Function),
    );
  });
});
