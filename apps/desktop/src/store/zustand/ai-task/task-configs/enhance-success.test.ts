import type { LanguageModel } from "ai";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { json2md } from "@hypr/editor/markdown";

import type { TaskConfig } from ".";
import { enhanceSuccess, runEnhanceSuccess } from "./enhance-success";

import { MIN_SUMMARY_CHARACTERS } from "~/services/enhancer/summary-length";
import { useLiveTitle } from "~/store/zustand/live-title";

const mocks = vi.hoisted(() => ({
  beginCloudsyncActivity: vi.fn().mockResolvedValue(undefined),
  endCloudsyncActivity: vi.fn().mockResolvedValue(undefined),
  loadSessionContentSnapshot: vi.fn(),
  persistGeneratedEnhancedNote: vi.fn().mockResolvedValue(undefined),
  persistGeneratedTitle: vi.fn().mockResolvedValue(true),
  inferAutomaticSpeakerAssignments: vi.fn().mockResolvedValue([]),
}));

vi.mock("@hypr/plugin-db", async (importOriginal) => ({
  ...(await importOriginal<typeof import("@hypr/plugin-db")>()),
  beginCloudsyncActivity: mocks.beginCloudsyncActivity,
  endCloudsyncActivity: mocks.endCloudsyncActivity,
}));

vi.mock("~/session/content-queries", () => ({
  loadSessionContentSnapshot: mocks.loadSessionContentSnapshot,
}));

vi.mock("~/session/content-mutations", () => ({
  persistGeneratedEnhancedNote: mocks.persistGeneratedEnhancedNote,
}));

vi.mock("~/services/enhancer/speaker-attribution", () => ({
  inferAutomaticSpeakerAssignments: mocks.inferAutomaticSpeakerAssignments,
}));

vi.mock("./title-success", async (importOriginal) => ({
  ...(await importOriginal<typeof import("./title-success")>()),
  persistGeneratedTitle: mocks.persistGeneratedTitle,
}));

type EnhanceSuccessParams = Parameters<
  NonNullable<TaskConfig<"enhance">["onSuccess"]>
>[0];

function createSnapshot(title = "") {
  return {
    sessionId: "session-1",
    ownerUserId: "user-1",
    title,
    createdAt: "2026-07-10T00:00:00.000Z",
    event: null,
    eventId: null,
    rawNoteId: "session-1",
    rawContent: "",
    rawContentFormat: "prosemirror_json",
    rawMarkdown: "",
    enhancedNotes: [
      {
        id: "note-1",
        title: "",
        markdown: "",
        content: "old content",
        contentFormat: "markdown",
        templateId: "",
        position: 0,
      },
    ],
    transcripts: [],
    participants: [],
  };
}

function createTransformedArgs(): EnhanceSuccessParams["transformedArgs"] {
  return {
    language: "en",
    promptOverride: "",
    session: {
      title: "Weekly Review",
      startedAt: null,
      endedAt: null,
      event: null,
    },
    participants: [],
    template: null,
    preMeetingMemo: "",
    postMeetingMemo: "",
    transcripts: [],
    imageContext: [],
  };
}

function createParams(
  overrides: Partial<EnhanceSuccessParams> = {},
): EnhanceSuccessParams {
  return {
    taskId: "note-1-enhance",
    text: "# Summary\n\n- Point",
    model: {} as LanguageModel,
    args: {
      sessionId: "session-1",
      enhancedNoteId: "note-1",
      templateId: undefined,
    },
    transformedArgs: createTransformedArgs(),
    signal: new AbortController().signal,
    startTask: vi.fn().mockResolvedValue(undefined),
    getTaskState: vi.fn().mockReturnValue(undefined),
    ...overrides,
  };
}

describe("enhanceSuccess.onSuccess", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.beginCloudsyncActivity.mockReset().mockResolvedValue(undefined);
    mocks.endCloudsyncActivity.mockReset().mockResolvedValue(undefined);
    useLiveTitle.setState({ titles: {} });
    mocks.loadSessionContentSnapshot.mockResolvedValue(createSnapshot());
    mocks.persistGeneratedEnhancedNote.mockResolvedValue(undefined);
    mocks.persistGeneratedTitle.mockResolvedValue(true);
    mocks.inferAutomaticSpeakerAssignments.mockReset().mockResolvedValue([]);
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("persists generated content and tags through one guarded SQLite write", async () => {
    const params = createParams({
      text: "# Summary\n\nDiscussed #Launch.",
      transformedArgs: {
        ...createTransformedArgs(),
        preMeetingMemo: "Prep #prep #Launch",
      },
    });

    await enhanceSuccess.onSuccess?.(params);

    expect(mocks.persistGeneratedEnhancedNote).toHaveBeenCalledWith({
      sessionId: "session-1",
      ownerUserId: "user-1",
      note: {
        id: "note-1",
        currentContent: "old content",
        currentContentFormat: "markdown",
        nextContent: expect.any(String),
      },
      tagNames: ["launch", "prep"],
    });
    const content =
      mocks.persistGeneratedEnhancedNote.mock.calls[0][0].note.nextContent;
    expect(json2md(JSON.parse(content)).trim()).toBe(
      "# Summary\n\nDiscussed #Launch.\n\n#launch #prep",
    );
    const cloudsyncLeaseKey = mocks.beginCloudsyncActivity.mock.calls[0]?.[1];
    expect(cloudsyncLeaseKey).toMatch(/^note-1-enhance:/);
    expect(mocks.beginCloudsyncActivity).toHaveBeenCalledWith(
      "enhance",
      cloudsyncLeaseKey,
    );
    expect(mocks.beginCloudsyncActivity).toHaveBeenCalledBefore(
      mocks.persistGeneratedEnhancedNote,
    );
    expect(
      mocks.beginCloudsyncActivity.mock.invocationCallOrder[0],
    ).toBeLessThan(
      mocks.loadSessionContentSnapshot.mock.invocationCallOrder[1],
    );
    expect(mocks.persistGeneratedEnhancedNote).toHaveBeenCalledBefore(
      mocks.endCloudsyncActivity,
    );
    expect(mocks.endCloudsyncActivity).toHaveBeenCalledWith(
      "enhance",
      cloudsyncLeaseKey,
    );
  });

  it("reports persistence only after generated content is saved", async () => {
    const onPersisted = vi.fn();

    await runEnhanceSuccess({
      ...createParams(),
      onPersisted,
    });

    expect(mocks.persistGeneratedEnhancedNote).toHaveBeenCalledOnce();
    expect(onPersisted).toHaveBeenCalledOnce();
    expect(mocks.persistGeneratedEnhancedNote).toHaveBeenCalledBefore(
      onPersisted,
    );
  });

  it("does not report persistence for empty generated content", async () => {
    const onPersisted = vi.fn();

    await runEnhanceSuccess({
      ...createParams({ text: "" }),
      onPersisted,
    });

    expect(mocks.persistGeneratedEnhancedNote).not.toHaveBeenCalled();
    expect(onPersisted).not.toHaveBeenCalled();
  });

  it("uses the durable auto-enhance baseline instead of a later user edit", async () => {
    const snapshot = createSnapshot("Meeting title");
    snapshot.enhancedNotes[0].content = "user edit";
    snapshot.enhancedNotes[0].contentFormat = "prosemirror_json";
    mocks.loadSessionContentSnapshot.mockResolvedValue(snapshot);

    await enhanceSuccess.onSuccess?.(
      createParams({
        args: {
          sessionId: "session-1",
          enhancedNoteId: "note-1",
          templateId: undefined,
          pendingAutoEnhance: {
            generation: "generation-1",
            expectedBody: "old content",
            expectedContentFormat: "markdown",
          },
        },
      }),
    );

    expect(mocks.persistGeneratedEnhancedNote).toHaveBeenCalledWith(
      expect.objectContaining({
        note: expect.objectContaining({
          currentContent: "old content",
          currentContentFormat: "markdown",
        }),
        pendingAutoEnhance: {
          generation: "generation-1",
          expectedBody: "old content",
          expectedContentFormat: "markdown",
        },
      }),
    );
  });

  it("persists inferred speaker hints with the generated note", async () => {
    const transcriptSpeakerHints = [
      {
        id: "transcript-1",
        currentWordsJson: "words",
        currentSpeakerHintsJson: "[]",
        nextSpeakerHintsJson: '[{"type":"automatic_speaker_assignment"}]',
        expectedParticipantHumanIdsJson: '["human-1","human-2"]',
      },
    ];
    mocks.inferAutomaticSpeakerAssignments.mockResolvedValueOnce(
      transcriptSpeakerHints,
    );

    await enhanceSuccess.onSuccess?.(createParams());

    expect(mocks.inferAutomaticSpeakerAssignments).toHaveBeenCalledWith(
      expect.objectContaining({
        generatedSummary: "# Summary\n\n- Point",
        snapshot: expect.objectContaining({ sessionId: "session-1" }),
      }),
    );
    expect(mocks.inferAutomaticSpeakerAssignments).toHaveBeenCalledBefore(
      mocks.beginCloudsyncActivity,
    );
    expect(mocks.persistGeneratedEnhancedNote).toHaveBeenCalledWith(
      expect.objectContaining({ transcriptSpeakerHints }),
    );
  });

  it("still persists the summary when speaker attribution fails", async () => {
    const consoleError = vi
      .spyOn(console, "error")
      .mockImplementation(() => {});
    mocks.inferAutomaticSpeakerAssignments.mockRejectedValueOnce(
      new Error("unsupported structured output"),
    );

    await expect(
      enhanceSuccess.onSuccess?.(createParams()),
    ).resolves.toBeUndefined();

    expect(mocks.persistGeneratedEnhancedNote).toHaveBeenCalledOnce();
    expect(consoleError).toHaveBeenCalledWith(
      "[enhance] failed to identify transcript speakers",
      expect.any(Error),
    );
    consoleError.mockRestore();
  });

  it("waits for a generated title, saves the note, then persists the title", async () => {
    const startTask = vi.fn().mockImplementation(async (_taskId, config) => {
      config.onComplete?.("Generated title");
    });

    await enhanceSuccess.onSuccess?.(createParams({ startTask }));

    expect(startTask).toHaveBeenCalledWith(
      "session-1-title",
      expect.objectContaining({
        taskType: "title",
        args: expect.objectContaining({ skipPersist: true }),
      }),
    );
    expect(mocks.persistGeneratedEnhancedNote).toHaveBeenCalledBefore(
      mocks.persistGeneratedTitle,
    );
    expect(startTask).toHaveBeenCalledBefore(mocks.beginCloudsyncActivity);
    expect(mocks.persistGeneratedTitle).toHaveBeenCalledBefore(
      mocks.endCloudsyncActivity,
    );
    const content =
      mocks.persistGeneratedEnhancedNote.mock.calls[0][0].note.nextContent;
    expect(json2md(JSON.parse(content)).trim()).toBe(
      "# Generated title\n\n# Summary\n\n- Point",
    );
    expect(mocks.persistGeneratedTitle).toHaveBeenCalledWith({
      text: "Generated title",
      args: { sessionId: "session-1" },
    });
  });

  it("uses an existing title without starting title generation", async () => {
    mocks.loadSessionContentSnapshot.mockResolvedValue(
      createSnapshot("Existing title"),
    );
    const params = createParams();

    await enhanceSuccess.onSuccess?.(params);

    expect(params.startTask).not.toHaveBeenCalled();
    expect(mocks.persistGeneratedTitle).not.toHaveBeenCalled();
    const content =
      mocks.persistGeneratedEnhancedNote.mock.calls[0][0].note.nextContent;
    expect(json2md(JSON.parse(content)).trim()).toBe(
      "# Existing title\n\n# Summary\n\n- Point",
    );
  });

  it("persists a short summary and tags within the transcript length and section cap", async () => {
    mocks.loadSessionContentSnapshot.mockResolvedValue(
      createSnapshot("Meeting title"),
    );
    const transformedArgs = createTransformedArgs();
    transformedArgs.preMeetingMemo = "Follow up with #launch";
    transformedArgs.transcripts = [
      {
        startedAt: null,
        endedAt: null,
        segments: [{ speaker: "John", text: "x".repeat(160) }],
      },
    ];

    await enhanceSuccess.onSuccess?.(
      createParams({
        text: `# First

- ${"a".repeat(100)}

# Second

- ${"b".repeat(100)}

# Third

- ${"c".repeat(100)}`,
        transformedArgs,
      }),
    );

    const content =
      mocks.persistGeneratedEnhancedNote.mock.calls[0][0].note.nextContent;
    const markdown = json2md(JSON.parse(content)).trim();
    expect(markdown).toContain("# First");
    expect(markdown).toContain("# Second");
    expect(markdown).not.toContain("# Third");
    expect(markdown).toContain("#launch");
    expect(
      Array.from(markdown.replace(/\s+/gu, " ")).length,
    ).toBeLessThanOrEqual(MIN_SUMMARY_CHARACTERS);
  });

  it("does not claim success when the guarded SQLite write fails", async () => {
    mocks.persistGeneratedEnhancedNote.mockRejectedValueOnce(
      new Error("stale summary"),
    );

    await expect(
      enhanceSuccess.onSuccess?.(
        createParams({
          getTaskState: vi.fn().mockReturnValue({
            taskType: "title",
            status: "generating",
            streamedText: "",
            abortController: null,
          }),
        }),
      ),
    ).rejects.toThrow("stale summary");
    expect(mocks.persistGeneratedTitle).not.toHaveBeenCalled();
    expect(mocks.endCloudsyncActivity).toHaveBeenCalledWith(
      "enhance",
      mocks.beginCloudsyncActivity.mock.calls[0]?.[1],
    );
  });

  it("does not persist until the enhance activity lease is acquired", async () => {
    let acquireActivity: (() => void) | undefined;
    mocks.beginCloudsyncActivity.mockReturnValueOnce(
      new Promise<void>((resolve) => {
        acquireActivity = resolve;
      }),
    );

    const persistence = enhanceSuccess.onSuccess?.(createParams());
    await vi.waitFor(() =>
      expect(mocks.beginCloudsyncActivity).toHaveBeenCalledOnce(),
    );
    expect(mocks.loadSessionContentSnapshot).toHaveBeenCalledOnce();
    expect(mocks.persistGeneratedEnhancedNote).not.toHaveBeenCalled();

    acquireActivity?.();
    await persistence;

    expect(mocks.loadSessionContentSnapshot).toHaveBeenCalledTimes(2);
    expect(mocks.persistGeneratedEnhancedNote).toHaveBeenCalledOnce();
    expect(mocks.endCloudsyncActivity).toHaveBeenCalledOnce();
  });

  it("keeps the retryable summary untouched when activity acquisition fails", async () => {
    const acquisitionError = new Error("cloudsync busy");
    mocks.beginCloudsyncActivity.mockRejectedValueOnce(acquisitionError);

    await expect(
      enhanceSuccess.onSuccess?.(
        createParams({
          args: {
            sessionId: "session-1",
            enhancedNoteId: "note-1",
            templateId: undefined,
            pendingAutoEnhance: {
              generation: "generation-1",
              expectedBody: "old content",
              expectedContentFormat: "markdown",
            },
          },
        }),
      ),
    ).rejects.toBe(acquisitionError);

    expect(mocks.persistGeneratedEnhancedNote).not.toHaveBeenCalled();
    expect(mocks.persistGeneratedTitle).not.toHaveBeenCalled();
    expect(mocks.loadSessionContentSnapshot).toHaveBeenCalledOnce();
    expect(mocks.endCloudsyncActivity).not.toHaveBeenCalled();
  });

  it("retries a transient activity release without rerunning persistence", async () => {
    mocks.endCloudsyncActivity
      .mockRejectedValueOnce(new Error("bridge busy"))
      .mockResolvedValueOnce(undefined);

    await enhanceSuccess.onSuccess?.(createParams());

    expect(mocks.persistGeneratedEnhancedNote).toHaveBeenCalledOnce();
    expect(mocks.endCloudsyncActivity).toHaveBeenCalledTimes(2);
  });

  it("keeps a persisted summary successful while release retries in the background", async () => {
    vi.useFakeTimers();
    mocks.endCloudsyncActivity
      .mockRejectedValueOnce(new Error("failure 1"))
      .mockRejectedValueOnce(new Error("failure 2"))
      .mockRejectedValueOnce(new Error("failure 3"))
      .mockRejectedValueOnce(new Error("failure 4"))
      .mockResolvedValueOnce(undefined);
    const consoleError = vi
      .spyOn(console, "error")
      .mockImplementation(() => {});

    const success = enhanceSuccess.onSuccess?.(createParams());
    await vi.advanceTimersByTimeAsync(400);
    await expect(success).resolves.toBeUndefined();

    expect(mocks.persistGeneratedEnhancedNote).toHaveBeenCalledOnce();
    expect(mocks.endCloudsyncActivity).toHaveBeenCalledTimes(3);

    await vi.advanceTimersByTimeAsync(5_000);
    expect(mocks.endCloudsyncActivity).toHaveBeenCalledTimes(4);
    expect(mocks.persistGeneratedEnhancedNote).toHaveBeenCalledOnce();

    await vi.advanceTimersByTimeAsync(5_000);
    expect(mocks.endCloudsyncActivity).toHaveBeenCalledTimes(5);
    expect(mocks.persistGeneratedEnhancedNote).toHaveBeenCalledOnce();
    consoleError.mockRestore();
  });

  it("retries a transient database lock while saving generated content", async () => {
    vi.useFakeTimers();
    mocks.persistGeneratedEnhancedNote
      .mockRejectedValueOnce(new Error("database is locked"))
      .mockResolvedValueOnce(undefined);

    const persist = enhanceSuccess.onSuccess?.(createParams());
    await vi.advanceTimersByTimeAsync(100);
    await persist;

    expect(mocks.persistGeneratedEnhancedNote).toHaveBeenCalledTimes(2);
  });

  it("does not retry persistence after cancellation during a database lock", async () => {
    vi.useFakeTimers();
    const abortController = new AbortController();
    mocks.persistGeneratedEnhancedNote.mockImplementationOnce(async () => {
      abortController.abort();
      throw new Error("database is locked");
    });

    const persist = enhanceSuccess.onSuccess?.(
      createParams({ signal: abortController.signal }),
    );
    await vi.advanceTimersByTimeAsync(100);
    await persist;

    expect(mocks.persistGeneratedEnhancedNote).toHaveBeenCalledOnce();
  });

  it("does not save after cancellation during title generation", async () => {
    const abortController = new AbortController();
    const onPersisted = vi.fn();
    const startTask = vi.fn().mockImplementation(async (_taskId, config) => {
      abortController.abort();
      config.onComplete?.("Generated title");
    });

    await runEnhanceSuccess({
      ...createParams({ signal: abortController.signal, startTask }),
      onPersisted,
    });

    expect(mocks.persistGeneratedEnhancedNote).not.toHaveBeenCalled();
    expect(onPersisted).not.toHaveBeenCalled();
  });

  it("rejects persistence when the target summary disappeared", async () => {
    const snapshot = createSnapshot("Existing title");
    snapshot.enhancedNotes = [];
    mocks.loadSessionContentSnapshot.mockResolvedValue(snapshot);

    await expect(enhanceSuccess.onSuccess?.(createParams())).rejects.toThrow(
      "Summary note-1 no longer exists",
    );
  });

  it("does not generate a title while a live title edit exists", async () => {
    useLiveTitle.getState().setTitle("session-1", "Draft title");
    const params = createParams();

    await enhanceSuccess.onSuccess?.(params);

    expect(params.startTask).not.toHaveBeenCalled();
    expect(mocks.persistGeneratedTitle).not.toHaveBeenCalled();
  });
});
