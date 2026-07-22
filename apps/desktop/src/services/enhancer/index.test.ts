import type { LanguageModel } from "ai";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { EnhancerService } from ".";

const mocks = vi.hoisted(() => ({
  analyticsEvent: vi.fn().mockResolvedValue(undefined),
  loadSessionContentSnapshot: vi.fn(),
  ensureSummaryDocument: vi.fn(),
  replaceSummaryDocumentTemplate: vi.fn().mockResolvedValue(undefined),
  updateSummaryDocumentTitleIfCurrent: vi.fn().mockResolvedValue(undefined),
  getTemplateById: vi.fn().mockResolvedValue(null),
  listenerSubscribe: vi.fn(),
}));

vi.mock("@hypr/plugin-analytics", () => ({
  commands: { event: mocks.analyticsEvent },
}));

vi.mock("~/session/content-queries", () => ({
  loadSessionContentSnapshot: mocks.loadSessionContentSnapshot,
}));

vi.mock("./storage", () => ({
  ensureSummaryDocument: mocks.ensureSummaryDocument,
  replaceSummaryDocumentTemplate: mocks.replaceSummaryDocumentTemplate,
  updateSummaryDocumentTitleIfCurrent:
    mocks.updateSummaryDocumentTitleIfCurrent,
}));

vi.mock("~/templates/queries", () => ({
  getTemplateById: mocks.getTemplateById,
}));

vi.mock("~/store/zustand/listener/instance", () => ({
  listenerStore: {
    subscribe: mocks.listenerSubscribe,
  },
}));

function createNote(overrides: Record<string, any> = {}): any {
  return {
    id: "note-1",
    title: "Summary",
    markdown: "",
    content: "",
    contentFormat: "prosemirror_json",
    templateId: "",
    position: 1,
    ...overrides,
  };
}

function createSnapshot({
  notes = [],
  wordCount = 0,
}: {
  notes?: any[];
  wordCount?: number;
} = {}) {
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
    enhancedNotes: notes,
    transcripts:
      wordCount > 0
        ? [
            {
              id: "transcript-1",
              started_at: 0,
              ended_at: 1,
              memo: "",
              wordsJson: "[]",
              words: Array.from({ length: wordCount }, (_, index) => ({
                id: `word-${index}`,
                text: "word",
                start_ms: index,
                end_ms: index + 1,
              })),
              speaker_hints: [],
            },
          ]
        : [],
    participants: [],
  };
}

function createMockAITaskStore(
  getTaskState: (taskId: string) => unknown = () => undefined,
) {
  const generate = vi.fn().mockResolvedValue(undefined);
  const reset = vi.fn();
  const store = {
    getState: vi.fn(() => ({
      generate,
      reset,
      getState: vi.fn(getTaskState),
    })),
  } as unknown as ConstructorParameters<
    typeof EnhancerService
  >[0]["aiTaskStore"];

  return {
    generate,
    reset,
    store,
  };
}

function createDeps(
  overrides: Partial<ConstructorParameters<typeof EnhancerService>[0]> = {},
): ConstructorParameters<typeof EnhancerService>[0] {
  return {
    aiTaskStore: createMockAITaskStore().store,
    getModel: () => ({}) as LanguageModel,
    getLLMConn: () => ({ providerId: "test", modelId: "test-model" }),
    getSelectedTemplateId: () => undefined,
    ...overrides,
  };
}

describe("EnhancerService", () => {
  let snapshot: ReturnType<typeof createSnapshot>;
  let listener: ((state: any) => void) | undefined;
  let unsubscribe: ReturnType<typeof vi.fn>;

  beforeEach(() => {
    vi.clearAllMocks();
    snapshot = createSnapshot();
    unsubscribe = vi.fn();
    mocks.listenerSubscribe.mockImplementation((callback) => {
      listener = callback;
      return unsubscribe;
    });
    mocks.loadSessionContentSnapshot.mockImplementation(async () => snapshot);
    mocks.ensureSummaryDocument.mockImplementation(
      async (_sessionId: string, templateId?: string) => {
        const normalizedTemplateId = templateId ?? "";
        const existing = snapshot.enhancedNotes.find(
          (note) => note.templateId === normalizedTemplateId,
        );
        if (existing) return existing;

        const note = createNote({
          id: `note-${snapshot.enhancedNotes.length + 1}`,
          templateId: normalizedTemplateId,
          position: snapshot.enhancedNotes.length + 1,
        });
        snapshot.enhancedNotes.push(note);
        return note;
      },
    );
    mocks.replaceSummaryDocumentTemplate.mockResolvedValue(undefined);
    mocks.updateSummaryDocumentTitleIfCurrent.mockResolvedValue(undefined);
    mocks.getTemplateById.mockResolvedValue(null);
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("returns no_model without touching session storage", async () => {
    const service = new EnhancerService(createDeps({ getModel: () => null }));

    await expect(service.enhance("session-1")).resolves.toEqual({
      type: "no_model",
    });
    expect(mocks.loadSessionContentSnapshot).not.toHaveBeenCalled();
  });

  it("creates the SQLite summary before starting generation", async () => {
    const ai = createMockAITaskStore();
    const service = new EnhancerService(createDeps({ aiTaskStore: ai.store }));

    const result = await service.enhance("session-1");

    expect(result).toEqual({ type: "started", noteId: "note-1" });
    expect(mocks.ensureSummaryDocument).toHaveBeenCalledWith(
      "session-1",
      undefined,
    );
    expect(mocks.ensureSummaryDocument).toHaveBeenCalledBefore(ai.generate);
    expect(ai.generate).toHaveBeenCalledWith("note-1-enhance", {
      model: expect.any(Object),
      taskType: "enhance",
      args: {
        sessionId: "session-1",
        enhancedNoteId: "note-1",
        templateId: undefined,
      },
    });
  });

  it("reuses a matching summary and its stored auto-enhance template", async () => {
    snapshot = createSnapshot({
      notes: [createNote({ id: "existing", templateId: "one-on-one" })],
    });
    const ai = createMockAITaskStore();
    const service = new EnhancerService(createDeps({ aiTaskStore: ai.store }));

    const result = await service.enhance("session-1", { isAuto: true });

    expect(result).toEqual({ type: "started", noteId: "existing" });
    expect(mocks.ensureSummaryDocument).not.toHaveBeenCalled();
    expect(ai.generate).toHaveBeenCalledWith(
      "existing-enhance",
      expect.objectContaining({
        args: expect.objectContaining({ templateId: "one-on-one" }),
      }),
    );
  });

  it("returns already_active while the note task is generating", async () => {
    snapshot = createSnapshot({ notes: [createNote()] });
    const ai = createMockAITaskStore(() => ({ status: "generating" }));
    const service = new EnhancerService(createDeps({ aiTaskStore: ai.store }));

    await expect(service.enhance("session-1")).resolves.toEqual({
      type: "already_active",
      noteId: "note-1",
    });
    expect(ai.generate).not.toHaveBeenCalled();
  });

  it("does not rerun a successful task with durable summary content", async () => {
    snapshot = createSnapshot({
      notes: [
        createNote({
          content:
            '{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"Saved"}]}]}',
        }),
      ],
    });
    const ai = createMockAITaskStore(() => ({ status: "success" }));
    const service = new EnhancerService(createDeps({ aiTaskStore: ai.store }));

    await expect(service.enhance("session-1")).resolves.toMatchObject({
      type: "already_active",
    });
    expect(ai.generate).not.toHaveBeenCalled();
  });

  it("reruns a successful task whose summary is still empty", async () => {
    snapshot = createSnapshot({ notes: [createNote()] });
    const ai = createMockAITaskStore(() => ({ status: "success" }));
    const service = new EnhancerService(createDeps({ aiTaskStore: ai.store }));

    await expect(service.enhance("session-1")).resolves.toMatchObject({
      type: "started",
    });
    expect(ai.generate).toHaveBeenCalledOnce();
  });

  it("replaces a target note before generating with the selected template", async () => {
    snapshot = createSnapshot({ notes: [createNote({ content: "Old" })] });
    const ai = createMockAITaskStore();
    const service = new EnhancerService(createDeps({ aiTaskStore: ai.store }));

    const result = await service.enhance("session-1", {
      targetNoteId: "note-1",
      templateId: "template-1",
      templateTitle: "Customer review",
    });

    expect(result).toEqual({ type: "started", noteId: "note-1" });
    expect(mocks.replaceSummaryDocumentTemplate).toHaveBeenCalledWith({
      sessionId: "session-1",
      noteId: "note-1",
      templateId: "template-1",
      title: "Customer review",
    });
    expect(mocks.replaceSummaryDocumentTemplate).toHaveBeenCalledBefore(
      ai.generate,
    );
  });

  it("lets an explicit null template override the selected default", async () => {
    snapshot = createSnapshot({ notes: [createNote({ templateId: "old" })] });
    const service = new EnhancerService(
      createDeps({ getSelectedTemplateId: () => "default-template" }),
    );

    await service.enhance("session-1", {
      targetNoteId: "note-1",
      templateId: null,
    });

    expect(mocks.replaceSummaryDocumentTemplate).toHaveBeenCalledWith(
      expect.objectContaining({ templateId: undefined }),
    );
  });

  it("does not queue auto-enhance when a durable summary exists", async () => {
    snapshot = createSnapshot({
      notes: [createNote({ content: "Saved summary" })],
      wordCount: 10,
    });
    const service = new EnhancerService(createDeps());
    const queueSpy = vi.spyOn(service, "queueAutoEnhance");

    await expect(
      service.queueAutoEnhanceIfSummaryEmpty("session-1"),
    ).resolves.toEqual({ type: "summary_exists", noteId: "note-1" });
    expect(queueSpy).not.toHaveBeenCalled();
  });

  it("creates a visible empty summary when a short transcript cannot enhance", async () => {
    snapshot = createSnapshot({ wordCount: 2 });
    const service = new EnhancerService(createDeps());
    const queueSpy = vi
      .spyOn(service, "queueAutoEnhance")
      .mockImplementation(() => {});

    await expect(
      service.queueAutoEnhanceIfSummaryEmpty("session-1"),
    ).resolves.toEqual({ type: "queued" });
    expect(mocks.ensureSummaryDocument).toHaveBeenCalledWith(
      "session-1",
      undefined,
    );
    expect(queueSpy).toHaveBeenCalledWith("session-1");
  });

  it("computes eligibility from canonical transcript words", async () => {
    const service = new EnhancerService(createDeps());

    snapshot = createSnapshot();
    await expect(service.checkEligibility("session-1")).resolves.toMatchObject({
      eligible: false,
      reason: "No transcript recorded",
    });
    snapshot = createSnapshot({ wordCount: 4 });
    await expect(service.checkEligibility("session-1")).resolves.toMatchObject({
      eligible: false,
      wordCount: 4,
    });
    snapshot = createSnapshot({ wordCount: 5 });
    await expect(service.checkEligibility("session-1")).resolves.toMatchObject({
      eligible: false,
      characterCount: 24,
      reason: "Transcript too short to summarize (24/160 characters minimum)",
      wordCount: 5,
    });
    snapshot = createSnapshot({ wordCount: 40 });
    await expect(service.checkEligibility("session-1")).resolves.toEqual({
      eligible: true,
      characterCount: 199,
      wordCount: 40,
    });
  });

  it("resets every canonical summary task", async () => {
    snapshot = createSnapshot({
      notes: [createNote({ id: "one" }), createNote({ id: "two" })],
    });
    const ai = createMockAITaskStore();
    const service = new EnhancerService(createDeps({ aiTaskStore: ai.store }));

    await service.resetEnhanceTasks("session-1");

    expect(ai.reset).toHaveBeenCalledWith("one-enhance");
    expect(ai.reset).toHaveBeenCalledWith("two-enhance");
  });

  it("deduplicates eligible auto-enhance requests", async () => {
    snapshot = createSnapshot({ wordCount: 40 });
    const ai = createMockAITaskStore();
    const service = new EnhancerService(createDeps({ aiTaskStore: ai.store }));

    service.queueAutoEnhance("session-1");
    service.queueAutoEnhance("session-1");

    await vi.waitFor(() => expect(ai.generate).toHaveBeenCalledOnce());
  });

  it("emits no-model and started auto-enhance outcomes", async () => {
    snapshot = createSnapshot({ wordCount: 40 });
    const noModelService = new EnhancerService(
      createDeps({ getModel: () => null }),
    );
    const noModelEvent = vi.fn();
    noModelService.on(noModelEvent);

    noModelService.queueAutoEnhance("session-1");
    await vi.waitFor(() =>
      expect(noModelEvent).toHaveBeenCalledWith({
        type: "auto-enhance-no-model",
        sessionId: "session-1",
      }),
    );

    const startedService = new EnhancerService(createDeps());
    const startedEvent = vi.fn();
    startedService.on(startedEvent);
    startedService.queueAutoEnhance("session-1");
    await vi.waitFor(() =>
      expect(startedEvent).toHaveBeenCalledWith({
        type: "auto-enhance-started",
        sessionId: "session-1",
        noteId: "note-1",
      }),
    );
  });

  it("retries short transcripts and eventually emits the skip reason", async () => {
    vi.useFakeTimers();
    snapshot = createSnapshot({ wordCount: 1 });
    const service = new EnhancerService(createDeps());
    const event = vi.fn();
    service.on(event);

    service.queueAutoEnhance("session-1");
    await vi.advanceTimersByTimeAsync(10_500);

    expect(event).toHaveBeenCalledWith({
      type: "auto-enhance-skipped",
      sessionId: "session-1",
      reason: "Not enough words recorded (1/5 minimum)",
      reasonCode: "transcript_too_short",
    });
  });

  it("cancels a pending retry when the session becomes active", async () => {
    vi.useFakeTimers();
    snapshot = createSnapshot({ wordCount: 1 });
    const service = new EnhancerService(createDeps());
    const event = vi.fn();
    service.on(event);
    service.start();

    service.queueAutoEnhance("session-1");
    await vi.advanceTimersByTimeAsync(100);
    listener?.({ live: { status: "active", sessionId: "session-1" } });
    await vi.advanceTimersByTimeAsync(20_000);

    expect(event).not.toHaveBeenCalled();
  });

  it("hydrates placeholder template titles without overwriting newer metadata", async () => {
    snapshot = createSnapshot({
      notes: [createNote({ templateId: "template-1", title: "Summary" })],
    });
    mocks.getTemplateById.mockResolvedValue({ title: "One-on-one" });
    const service = new EnhancerService(createDeps());

    await service.ensureNote("session-1", "template-1");

    await vi.waitFor(() =>
      expect(mocks.updateSummaryDocumentTitleIfCurrent).toHaveBeenCalledWith({
        sessionId: "session-1",
        noteId: "note-1",
        templateId: "template-1",
        currentTitle: "Summary",
        nextTitle: "One-on-one",
      }),
    );
  });

  it("disposes listener subscriptions and pending timers", async () => {
    vi.useFakeTimers();
    snapshot = createSnapshot({ wordCount: 1 });
    const service = new EnhancerService(createDeps());
    service.start();
    service.queueAutoEnhance("session-1");
    await vi.advanceTimersByTimeAsync(100);

    service.dispose();
    await vi.advanceTimersByTimeAsync(20_000);

    expect(unsubscribe).toHaveBeenCalledOnce();
  });
});
