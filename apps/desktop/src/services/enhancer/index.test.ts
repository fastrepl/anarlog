import type { LanguageModel } from "ai";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { EnhancerService } from ".";

import { listenerStore } from "~/store/zustand/listener/instance";

vi.mock("@hypr/plugin-analytics", () => ({
  commands: {
    event: vi.fn().mockResolvedValue(undefined),
  },
}));

vi.mock("~/store/zustand/listener/instance", () => ({
  listenerStore: {
    getState: vi.fn().mockReturnValue({
      live: { status: "inactive", sessionId: null },
      batch: {},
    }),
    subscribe: vi.fn().mockReturnValue(() => {}),
  },
}));

type Tables = Record<string, Record<string, Record<string, any>>>;

function createTables(data?: {
  transcripts?: Record<string, { session_id: string; words: string }>;
  enhanced_notes?: Record<string, { session_id: string; template_id?: string }>;
  sessions?: Record<string, { title: string }>;
}): Tables {
  return {
    transcripts: data?.transcripts ?? {},
    enhanced_notes: data?.enhanced_notes ?? {},
    sessions: data?.sessions ?? {},
    templates: {},
  };
}

function createMockStore(tables: Tables) {
  return {
    getCell: vi.fn((table: string, rowId: string, cellId: string) => {
      return tables[table]?.[rowId]?.[cellId];
    }),
    getValue: vi.fn((valueId: string) => {
      if (valueId === "user_id") return "user-1";
      return undefined;
    }),
    setRow: vi.fn((table: string, rowId: string, row: Record<string, any>) => {
      if (!tables[table]) tables[table] = {};
      tables[table][rowId] = row;
    }),
    setPartialRow: vi.fn(),
  } as any;
}

function createMockIndexes(tables: Tables) {
  return {
    getSliceRowIds: vi.fn((indexId: string, sliceId: string) => {
      if (indexId === "transcriptBySession") {
        return Object.keys(tables.transcripts ?? {}).filter(
          (id) => tables.transcripts[id]?.session_id === sliceId,
        );
      }
      if (indexId === "enhancedNotesBySession") {
        return Object.keys(tables.enhanced_notes ?? {}).filter(
          (id) => tables.enhanced_notes[id]?.session_id === sliceId,
        );
      }
      return [];
    }),
  };
}

function createMockAITaskStore() {
  const generatingTasks = new Set<string>();
  return {
    getState: vi.fn().mockReturnValue({
      generate: vi.fn().mockImplementation((taskId: string) => {
        generatingTasks.add(taskId);
        return Promise.resolve();
      }),
      getState: vi.fn().mockImplementation((taskId: string) => {
        if (generatingTasks.has(taskId)) {
          return { status: "generating" };
        }
        return undefined;
      }),
    }),
  };
}

function createDeps(
  overrides?: Partial<ConstructorParameters<typeof EnhancerService>[0]>,
) {
  const tables = createTables();
  return {
    mainStore: createMockStore(tables),
    indexes: createMockIndexes(tables),
    aiTaskStore: createMockAITaskStore(),
    getModel: () => ({}) as LanguageModel,
    getLLMConn: () => ({ providerId: "test", modelId: "test-model" }),
    getSelectedTemplateId: () => undefined,
    ...overrides,
  };
}

describe("EnhancerService", () => {
  describe("enhance()", () => {
    it("returns no_model when model is null", () => {
      const deps = createDeps({ getModel: () => null });
      const service = new EnhancerService(deps);

      const result = service.enhance("session-1");
      expect(result).toEqual({ type: "no_model" });
    });

    it("does not skip manual enhance when no transcript exists", () => {
      const deps = createDeps();
      const service = new EnhancerService(deps);

      const result = service.enhance("session-1");
      expect(result.type).toBe("started");
    });

    it("creates note and starts generation", () => {
      const tables = createTables();
      const store = createMockStore(tables);
      const aiTaskStore = createMockAITaskStore();
      const deps = createDeps({
        mainStore: store,
        indexes: createMockIndexes(tables),
        aiTaskStore,
      });
      const service = new EnhancerService(deps);

      const result = service.enhance("session-1");
      expect(result.type).toBe("started");
      expect(store.setRow).toHaveBeenCalledWith(
        "enhanced_notes",
        expect.any(String),
        expect.objectContaining({
          session_id: "session-1",
          title: "Summary",
        }),
      );
      expect(aiTaskStore.getState().generate).toHaveBeenCalled();
    });

    it("reuses existing note with same template", () => {
      const tables = createTables({
        enhanced_notes: {
          "existing-note": {
            session_id: "session-1",
            template_id: undefined as any,
          },
        },
      });
      const store = createMockStore(tables);
      const deps = createDeps({
        mainStore: store,
        indexes: createMockIndexes(tables),
      });
      const service = new EnhancerService(deps);

      const result = service.enhance("session-1");
      expect(result).toEqual({
        type: "started",
        noteId: "existing-note",
      });
      expect(store.setRow).not.toHaveBeenCalledWith(
        "enhanced_notes",
        expect.not.stringMatching("existing-note"),
        expect.anything(),
      );
    });

    it("returns already_active when task is generating", () => {
      const tables = createTables({
        enhanced_notes: {
          "note-1": {
            session_id: "session-1",
            template_id: undefined as any,
          },
        },
      });
      const aiTaskStore = createMockAITaskStore();
      aiTaskStore.getState.mockReturnValue({
        generate: vi.fn(),
        getState: vi.fn().mockReturnValue({ status: "generating" }),
      });
      const deps = createDeps({
        mainStore: createMockStore(tables),
        indexes: createMockIndexes(tables),
        aiTaskStore,
      });
      const service = new EnhancerService(deps);

      const result = service.enhance("session-1");
      expect(result).toEqual({ type: "already_active", noteId: "note-1" });
      expect(aiTaskStore.getState().generate).not.toHaveBeenCalled();
    });

    it("returns already_active when task has succeeded", () => {
      const tables = createTables({
        enhanced_notes: {
          "note-1": {
            session_id: "session-1",
            template_id: undefined as any,
          },
        },
      });
      const aiTaskStore = createMockAITaskStore();
      aiTaskStore.getState.mockReturnValue({
        generate: vi.fn(),
        getState: vi.fn().mockReturnValue({ status: "success" }),
      });
      const deps = createDeps({
        mainStore: createMockStore(tables),
        indexes: createMockIndexes(tables),
        aiTaskStore,
      });
      const service = new EnhancerService(deps);

      const result = service.enhance("session-1");
      expect(result).toEqual({ type: "already_active", noteId: "note-1" });
      expect(aiTaskStore.getState().generate).not.toHaveBeenCalled();
    });
  });

  describe("checkEligibility()", () => {
    it("returns not eligible when no transcript exists", () => {
      const deps = createDeps();
      const service = new EnhancerService(deps);

      const result = service.checkEligibility("session-1");
      expect(result.eligible).toBe(false);
    });

    it("returns not eligible when not enough words", () => {
      const tables = createTables({
        transcripts: {
          "t-1": {
            session_id: "session-1",
            words: JSON.stringify([{ text: "hi" }, { text: "there" }]),
          },
        },
      });
      const deps = createDeps({
        mainStore: createMockStore(tables),
        indexes: createMockIndexes(tables),
      });
      const service = new EnhancerService(deps);

      const result = service.checkEligibility("session-1");
      expect(result.eligible).toBe(false);
    });

    it("returns eligible when enough words", () => {
      const words = Array.from({ length: 10 }, (_, i) => ({
        text: `word${i}`,
      }));
      const tables = createTables({
        transcripts: {
          "t-1": {
            session_id: "session-1",
            words: JSON.stringify(words),
          },
        },
      });
      const deps = createDeps({
        mainStore: createMockStore(tables),
        indexes: createMockIndexes(tables),
      });
      const service = new EnhancerService(deps);

      const result = service.checkEligibility("session-1");
      expect(result.eligible).toBe(true);
    });
  });

  describe("deduplication", () => {
    it("auto-enhance does not run twice for same session", () => {
      const words = Array.from({ length: 10 }, (_, i) => ({
        text: `word${i}`,
      }));
      const tables = createTables({
        transcripts: {
          "t-1": {
            session_id: "session-1",
            words: JSON.stringify(words),
          },
        },
      });
      const aiTaskStore = createMockAITaskStore();
      const deps = createDeps({
        mainStore: createMockStore(tables),
        indexes: createMockIndexes(tables),
        aiTaskStore,
      });
      const service = new EnhancerService(deps);

      const result1 = service.enhance("session-1", { isAuto: true });
      expect(result1.type).toBe("started");

      const result2 = service.enhance("session-1", { isAuto: true });
      expect(result2.type).toBe("already_active");

      expect(aiTaskStore.getState().generate).toHaveBeenCalledTimes(1);
    });

    it("allows manual enhance even after auto-enhance", () => {
      const words = Array.from({ length: 10 }, (_, i) => ({
        text: `word${i}`,
      }));
      const tables = createTables({
        transcripts: {
          "t-1": {
            session_id: "session-1",
            words: JSON.stringify(words),
          },
        },
      });
      const aiTaskStore = createMockAITaskStore();
      const deps = createDeps({
        mainStore: createMockStore(tables),
        indexes: createMockIndexes(tables),
        aiTaskStore,
      });
      const service = new EnhancerService(deps);

      service.enhance("session-1", { isAuto: true });
      service.enhance("session-1", { templateId: "custom-template" });

      expect(aiTaskStore.getState().generate).toHaveBeenCalledTimes(2);
    });
  });

  describe("tryAutoEnhance", () => {
    it("emits auto-enhance-skipped when not eligible after max retries", () => {
      const tables = createTables();
      const deps = createDeps({
        mainStore: createMockStore(tables),
        indexes: createMockIndexes(tables),
      });
      const service = new EnhancerService(deps);
      const events: any[] = [];
      service.on((event) => events.push(event));

      (service as any).tryAutoEnhance("session-1", 20);

      expect(events).toContainEqual({
        type: "auto-enhance-skipped",
        sessionId: "session-1",
        reason: expect.any(String),
      });
    });

    it("clears activeAutoEnhance on skipped after max retries", () => {
      const tables = createTables();
      const deps = createDeps({
        mainStore: createMockStore(tables),
        indexes: createMockIndexes(tables),
      });
      const service = new EnhancerService(deps);

      (service as any).activeAutoEnhance.add("session-1");
      (service as any).tryAutoEnhance("session-1", 20);

      expect((service as any).activeAutoEnhance.has("session-1")).toBe(false);
    });

    it("retries when not eligible and under max attempts", () => {
      vi.useFakeTimers();
      const tables = createTables();
      const deps = createDeps({
        mainStore: createMockStore(tables),
        indexes: createMockIndexes(tables),
      });
      const service = new EnhancerService(deps);

      (service as any).tryAutoEnhance("session-1", 0);

      expect((service as any).pendingRetries.has("session-1")).toBe(true);
      vi.useRealTimers();
    });

    it("emits auto-enhance-no-model and does not retry when no model", () => {
      const words = Array.from({ length: 10 }, (_, i) => ({
        text: `word${i}`,
      }));
      const tables = createTables({
        transcripts: {
          "t-1": {
            session_id: "session-1",
            words: JSON.stringify(words),
          },
        },
      });
      const deps = createDeps({
        mainStore: createMockStore(tables),
        indexes: createMockIndexes(tables),
        getModel: () => null,
      });
      const service = new EnhancerService(deps);
      const events: any[] = [];
      service.on((event) => events.push(event));

      (service as any).activeAutoEnhance.add("session-1");
      (service as any).tryAutoEnhance("session-1", 0);

      expect(events).toContainEqual({
        type: "auto-enhance-no-model",
        sessionId: "session-1",
      });
      expect((service as any).activeAutoEnhance.has("session-1")).toBe(false);
      expect((service as any).pendingRetries.has("session-1")).toBe(false);
    });

    it("emits auto-enhance-started and clears activeAutoEnhance on success", () => {
      const words = Array.from({ length: 10 }, (_, i) => ({
        text: `word${i}`,
      }));
      const tables = createTables({
        transcripts: {
          "t-1": {
            session_id: "session-1",
            words: JSON.stringify(words),
          },
        },
      });
      const deps = createDeps({
        mainStore: createMockStore(tables),
        indexes: createMockIndexes(tables),
      });
      const service = new EnhancerService(deps);
      const events: any[] = [];
      service.on((event) => events.push(event));

      (service as any).activeAutoEnhance.add("session-1");
      (service as any).tryAutoEnhance("session-1", 0);

      expect(events).toContainEqual({
        type: "auto-enhance-started",
        sessionId: "session-1",
        noteId: expect.any(String),
      });
      expect((service as any).activeAutoEnhance.has("session-1")).toBe(false);
    });
  });

  describe("start() subscriber", () => {
    let subscriber: ((state: any) => void) | undefined;

    beforeEach(() => {
      vi.mocked(listenerStore.subscribe).mockImplementation((cb: any) => {
        subscriber = cb;
        return () => {
          subscriber = undefined;
        };
      });
    });

    afterEach(() => {
      subscriber = undefined;
      vi.mocked(listenerStore.subscribe).mockReturnValue(() => {});
    });

    function createEligibleDeps() {
      const words = Array.from({ length: 10 }, (_, i) => ({
        text: `word${i}`,
      }));
      const tables = createTables({
        transcripts: {
          "t-1": {
            session_id: "session-1",
            words: JSON.stringify(words),
          },
        },
      });
      return createDeps({
        mainStore: createMockStore(tables),
        indexes: createMockIndexes(tables),
      });
    }

    it("triggers auto-enhance when live session stops", () => {
      const deps = createEligibleDeps();
      const service = new EnhancerService(deps);
      const events: any[] = [];
      service.on((event) => events.push(event));
      service.start();

      subscriber?.({
        live: { status: "active", sessionId: "session-1" },
        batch: {},
      });
      subscriber?.({
        live: { status: "inactive", sessionId: null },
        batch: {},
      });

      expect(events).toContainEqual({
        type: "auto-enhance-started",
        sessionId: "session-1",
        noteId: expect.any(String),
      });
    });

    it("triggers auto-enhance when finalizing session stops", () => {
      const deps = createEligibleDeps();
      const service = new EnhancerService(deps);
      const events: any[] = [];
      service.on((event) => events.push(event));
      service.start();

      subscriber?.({
        live: { status: "finalizing", sessionId: "session-1" },
        batch: {},
      });
      subscriber?.({
        live: { status: "inactive", sessionId: null },
        batch: {},
      });

      expect(events).toContainEqual({
        type: "auto-enhance-started",
        sessionId: "session-1",
        noteId: expect.any(String),
      });
    });

    it("triggers auto-enhance when batch completes", () => {
      const deps = createEligibleDeps();
      const service = new EnhancerService(deps);
      const events: any[] = [];
      service.on((event) => events.push(event));
      service.start();

      subscriber?.({
        live: { status: "inactive", sessionId: null },
        batch: { "session-1": { percentage: 50 } },
      });
      subscriber?.({
        live: { status: "inactive", sessionId: null },
        batch: {},
      });

      expect(events).toContainEqual({
        type: "auto-enhance-started",
        sessionId: "session-1",
        noteId: expect.any(String),
      });
    });

    it("does not trigger auto-enhance when batch errors", () => {
      const deps = createEligibleDeps();
      const service = new EnhancerService(deps);
      const events: any[] = [];
      service.on((event) => events.push(event));
      service.start();

      subscriber?.({
        live: { status: "inactive", sessionId: null },
        batch: { "session-1": { percentage: 50 } },
      });
      subscriber?.({
        live: { status: "inactive", sessionId: null },
        batch: { "session-1": { percentage: 50, error: "failed" } },
      });

      expect(events).toHaveLength(0);
    });

    it("cancels retries when session becomes active", () => {
      vi.useFakeTimers();
      const deps = createDeps();
      const service = new EnhancerService(deps);
      service.start();

      service.queueAutoEnhance("session-1");
      expect((service as any).pendingRetries.has("session-1")).toBe(true);

      subscriber?.({
        live: { status: "active", sessionId: "session-1" },
        batch: {},
      });

      expect((service as any).pendingRetries.has("session-1")).toBe(false);
      expect((service as any).activeAutoEnhance.has("session-1")).toBe(false);
      vi.useRealTimers();
    });

    it("does not trigger on inactive-to-inactive", () => {
      const deps = createEligibleDeps();
      const service = new EnhancerService(deps);
      const events: any[] = [];
      service.on((event) => events.push(event));
      service.start();

      subscriber?.({
        live: { status: "inactive", sessionId: null },
        batch: {},
      });

      expect(events).toHaveLength(0);
    });
  });

  describe("dispose()", () => {
    it("cleans up subscriptions, timers, and singleton", () => {
      const deps = createDeps();
      const service = new EnhancerService(deps);
      service.start();

      (service as any).activeAutoEnhance.add("session-1");
      service.dispose();

      expect((service as any).activeAutoEnhance.size).toBe(0);
    });
  });
});
