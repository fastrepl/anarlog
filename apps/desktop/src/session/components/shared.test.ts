import { renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { computeCurrentNoteTab } from "./compute-note-tab";
import {
  hasStoredNoteContent,
  useCanShowTranscript,
  useCurrentNoteTab,
} from "./shared";

import type { Tab } from "~/store/zustand/tabs/schema";

const hoisted = vi.hoisted(() => ({
  batchError: null as string | null,
  enhancedNoteIds: ["note-1"] as string[],
  finalizingBySession: {} as Record<string, unknown>,
  hasTranscript: false,
  liveSegments: [] as unknown[],
  liveSessionId: null as string | null,
  sessionMode: "inactive",
}));

vi.mock("~/stt/contexts", () => ({
  useListener: (
    selector: (state: {
      batch: Record<string, { error: string | null } | undefined>;
      live: {
        sessionId: string | null;
        finalizingBySession: Record<string, unknown>;
      };
      liveSegments: unknown[];
      getSessionMode: () => string;
    }) => unknown,
  ) =>
    selector({
      batch: { "session-1": { error: hoisted.batchError } },
      live: {
        sessionId: hoisted.liveSessionId,
        finalizingBySession: hoisted.finalizingBySession,
      },
      liveSegments: hoisted.liveSegments,
      getSessionMode: () => hoisted.sessionMode,
    }),
}));

vi.mock("~/stt/utils", () => ({
  parseTranscriptWords: () =>
    hoisted.hasTranscript ? [{ text: "Hello" }] : [],
}));

vi.mock("~/store/tinybase/store/main", () => ({
  INDEXES: {
    enhancedNotesBySession: "enhancedNotesBySession",
    transcriptBySession: "transcriptBySession",
  },
  STORE_ID: "main",
  UI: {
    useCell: () => "",
    useSliceRowIds: (indexId: string) => {
      if (indexId === "enhancedNotesBySession") {
        return hoisted.enhancedNoteIds;
      }

      if (indexId === "transcriptBySession") {
        return ["transcript-1"];
      }

      return [];
    },
    useStore: () => ({
      addRowListener: vi.fn(() => "listener-1"),
      delListener: vi.fn(),
    }),
  },
}));

describe("useCurrentNoteTab", () => {
  const tab = {
    type: "sessions",
    id: "session-1",
    state: { view: { type: "transcript" } },
  } as Extract<Tab, { type: "sessions" }>;

  beforeEach(() => {
    hoisted.batchError = null;
    hoisted.enhancedNoteIds = ["note-1"];
    hoisted.finalizingBySession = {};
    hoisted.hasTranscript = false;
    hoisted.liveSegments = [];
    hoisted.liveSessionId = null;
    hoisted.sessionMode = "inactive";
  });

  it("keeps the transcript view available when saved audio exists", () => {
    const { result } = renderHook(() =>
      useCurrentNoteTab(tab, { audioExists: true }),
    );

    expect(result.current).toEqual({ type: "transcript" });
  });

  it("normalizes the transcript view when audio and transcript rows are missing", () => {
    const { result } = renderHook(() => useCurrentNoteTab(tab));

    expect(result.current).toEqual({ type: "raw" });
  });

  it("normalizes active transcript view when no transcript evidence exists", () => {
    hoisted.sessionMode = "active";
    hoisted.liveSessionId = "session-1";

    const { result } = renderHook(() => useCurrentNoteTab(tab));

    expect(result.current).toEqual({ type: "raw" });
  });

  it("normalizes active transcript view when only in-progress audio exists", () => {
    hoisted.sessionMode = "active";
    hoisted.liveSessionId = "session-1";

    const { result } = renderHook(() =>
      useCurrentNoteTab(tab, { audioExists: true }),
    );

    expect(result.current).toEqual({ type: "raw" });
  });
});

describe("useCanShowTranscript", () => {
  beforeEach(() => {
    hoisted.batchError = null;
    hoisted.finalizingBySession = {};
    hoisted.hasTranscript = false;
    hoisted.liveSegments = [];
    hoisted.liveSessionId = null;
    hoisted.sessionMode = "inactive";
  });

  it("shows transcript evidence for live segments owned by the session", () => {
    hoisted.liveSessionId = "session-1";
    hoisted.liveSegments = [{ id: "segment-1" }];

    const { result } = renderHook(() => useCanShowTranscript("session-1"));

    expect(result.current).toBe(true);
  });

  it("ignores live segments from another active session while finalizing", () => {
    hoisted.finalizingBySession = { "session-1": { startedAt: 1 } };
    hoisted.liveSessionId = "session-2";
    hoisted.liveSegments = [{ id: "segment-1" }];
    hoisted.sessionMode = "finalizing";

    const { result } = renderHook(() => useCanShowTranscript("session-1"));

    expect(result.current).toBe(false);
  });
});

describe("hasStoredNoteContent", () => {
  it("returns false for empty stored note values", () => {
    expect(hasStoredNoteContent("")).toBe(false);
    expect(
      hasStoredNoteContent(
        JSON.stringify({
          type: "doc",
          content: [{ type: "paragraph" }],
        }),
      ),
    ).toBe(false);
  });

  it("returns true for markdown and ProseMirror JSON text content", () => {
    expect(hasStoredNoteContent("Meeting notes")).toBe(true);
    expect(
      hasStoredNoteContent(
        JSON.stringify({
          type: "doc",
          content: [
            {
              type: "paragraph",
              content: [{ type: "text", text: "Meeting notes" }],
            },
          ],
        }),
      ),
    ).toBe(true);
  });
});

describe("computeCurrentNoteTab", () => {
  describe("when listening is active", () => {
    it("preserves enhanced view", () => {
      const result = computeCurrentNoteTab(
        { type: "enhanced", id: "note-1" },
        true,
        "note-1",
        false,
      );
      expect(result).toEqual({ type: "enhanced", id: "note-1" });
    });

    it("preserves raw view", () => {
      const result = computeCurrentNoteTab({ type: "raw" }, true, "note-1");
      expect(result).toEqual({ type: "raw" });
    });

    it("preserves transcript view when transcript can show", () => {
      const result = computeCurrentNoteTab(
        { type: "transcript" },
        true,
        "note-1",
        true,
      );
      expect(result).toEqual({ type: "transcript" });
    });

    it("normalizes transcript view when transcript cannot show", () => {
      const result = computeCurrentNoteTab(
        { type: "transcript" },
        true,
        "note-1",
        false,
      );
      expect(result).toEqual({ type: "raw" });
    });

    it("returns raw view when no persisted view", () => {
      const result = computeCurrentNoteTab(null, true, "note-1");
      expect(result).toEqual({ type: "raw" });
    });
  });

  describe("when not listening", () => {
    it("respects persisted enhanced view", () => {
      const result = computeCurrentNoteTab(
        { type: "enhanced", id: "note-1" },
        false,
        "note-1",
        false,
      );
      expect(result).toEqual({ type: "enhanced", id: "note-1" });
    });

    it("respects persisted raw view", () => {
      const result = computeCurrentNoteTab({ type: "raw" }, false, "note-1");
      expect(result).toEqual({ type: "raw" });
    });

    it("respects persisted transcript view", () => {
      const result = computeCurrentNoteTab(
        { type: "transcript" },
        false,
        "note-1",
        true,
      );
      expect(result).toEqual({ type: "transcript" });
    });

    it("normalizes persisted transcript view before transcript content exists", () => {
      const result = computeCurrentNoteTab(
        { type: "transcript" },
        false,
        "note-1",
        false,
      );
      expect(result).toEqual({ type: "raw" });
    });

    it("normalizes persisted attachments view to raw", () => {
      const result = computeCurrentNoteTab(
        { type: "attachments" },
        false,
        "note-1",
        false,
      );
      expect(result).toEqual({ type: "raw" });
    });

    it("normalizes persisted enhanced view when no enhanced notes exist", () => {
      const result = computeCurrentNoteTab(
        { type: "enhanced", id: "note-1" },
        false,
        undefined,
        false,
      );
      expect(result).toEqual({ type: "raw" });
    });

    it("defaults to enhanced view when available and no persisted view", () => {
      const result = computeCurrentNoteTab(null, false, "note-1");
      expect(result).toEqual({ type: "enhanced", id: "note-1" });
    });

    it("defaults to raw when no enhanced notes and no persisted view", () => {
      const result = computeCurrentNoteTab(null, false, undefined);
      expect(result).toEqual({ type: "raw" });
    });
  });
});
