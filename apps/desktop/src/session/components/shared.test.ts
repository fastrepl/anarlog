import { renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { computeCurrentNoteTab } from "./compute-note-tab";
import {
  hasStoredNoteContent,
  useCanShowTranscript,
  useCurrentNoteHasContent,
  useCurrentNoteTab,
  useListenButtonState,
} from "./shared";

import type { Tab } from "~/store/zustand/tabs/schema";

const hoisted = vi.hoisted(() => ({
  batchError: null as string | null,
  enhancedNoteIds: ["note-1"] as string[],
  finalizingBySession: {} as Record<string, unknown>,
  hasTranscript: false,
  rawMd: "",
  enhancedContent: "",
  liveSegments: [] as unknown[],
  liveLastError: null as string | null,
  liveLastErrorSessionId: null as string | null,
  liveLastErrorIsAudioRelated: false,
  liveSessionId: null as string | null,
  sessionMode: "inactive",
}));

vi.mock("~/stt/contexts", () => ({
  useListener: (
    selector: (state: {
      batch: Record<string, { error: string | null } | undefined>;
      live: {
        lastError: string | null;
        lastErrorSessionId: string | null;
        lastErrorIsAudioRelated: boolean;
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
        lastError: hoisted.liveLastError,
        lastErrorSessionId: hoisted.liveLastErrorSessionId,
        lastErrorIsAudioRelated: hoisted.liveLastErrorIsAudioRelated,
        sessionId: hoisted.liveSessionId,
        finalizingBySession: hoisted.finalizingBySession,
      },
      liveSegments: hoisted.liveSegments,
      getSessionMode: () => hoisted.sessionMode,
    }),
}));

vi.mock("~/session/queries", () => ({
  useEnhancedNote: () => ({ content: hoisted.enhancedContent }),
  useEnhancedNoteRecords: () => hoisted.enhancedNoteIds.map((id) => ({ id })),
  useSession: () => ({ raw_md: hoisted.rawMd }),
  useSessionHasTranscript: () => hoisted.hasTranscript,
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
    hoisted.rawMd = "";
    hoisted.enhancedContent = "";
    hoisted.liveSegments = [];
    hoisted.liveLastError = null;
    hoisted.liveLastErrorSessionId = null;
    hoisted.liveLastErrorIsAudioRelated = false;
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

  it("switches active transcript view to memos when no transcript evidence exists", () => {
    hoisted.sessionMode = "active";
    hoisted.liveSessionId = "session-1";

    const { result } = renderHook(() => useCurrentNoteTab(tab));

    expect(result.current).toEqual({ type: "raw" });
  });

  it("switches active transcript view to memos when only in-progress audio exists", () => {
    hoisted.sessionMode = "active";
    hoisted.liveSessionId = "session-1";

    const { result } = renderHook(() =>
      useCurrentNoteTab(tab, { audioExists: true }),
    );

    expect(result.current).toEqual({ type: "raw" });
  });
});

describe("useListenButtonState", () => {
  beforeEach(() => {
    hoisted.liveLastError = null;
    hoisted.liveLastErrorSessionId = null;
    hoisted.liveLastErrorIsAudioRelated = false;
    hoisted.sessionMode = "inactive";
  });

  it("routes capture failures to audio capability settings", () => {
    hoisted.liveLastError = "microphone unavailable";
    hoisted.liveLastErrorSessionId = "session-1";
    hoisted.liveLastErrorIsAudioRelated = true;

    const { result } = renderHook(() => useListenButtonState("session-1"));

    expect(result.current).toEqual({
      shouldRender: true,
      isDisabled: false,
      warningMessage: "Session failed: microphone unavailable",
      recoverySettingsTab: "permissions",
    });
  });

  it("does not route transcription failures to audio settings", () => {
    hoisted.liveLastError = "transcription connection closed";
    hoisted.liveLastErrorSessionId = "session-1";

    const { result } = renderHook(() => useListenButtonState("session-1"));

    expect(result.current).toEqual({
      shouldRender: true,
      isDisabled: false,
      warningMessage: "Session failed: transcription connection closed",
      recoverySettingsTab: null,
    });
  });

  it("does not show another session's capture failure", () => {
    hoisted.liveLastError = "microphone unavailable";
    hoisted.liveLastErrorSessionId = "session-2";
    hoisted.liveLastErrorIsAudioRelated = true;

    const { result } = renderHook(() => useListenButtonState("session-1"));

    expect(result.current).toEqual({
      shouldRender: true,
      isDisabled: false,
      warningMessage: "",
      recoverySettingsTab: null,
    });
  });

  it("does not offer audio configuration for batch progress", () => {
    hoisted.sessionMode = "running_batch";

    const { result } = renderHook(() => useListenButtonState("session-1"));

    expect(result.current).toEqual({
      shouldRender: true,
      isDisabled: true,
      warningMessage: "Batch transcription in progress.",
      recoverySettingsTab: null,
    });
  });
});

describe("useCurrentNoteHasContent", () => {
  beforeEach(() => {
    hoisted.hasTranscript = false;
    hoisted.rawMd = "";
    hoisted.enhancedContent = "";
  });

  it("reads raw note content from SQLite", () => {
    hoisted.rawMd = "Meeting notes";

    const { result } = renderHook(() =>
      useCurrentNoteHasContent("session-1", { type: "raw" }),
    );

    expect(result.current).toBe(true);
  });

  it("reads enhanced note content from SQLite", () => {
    hoisted.enhancedContent = "Summary";

    const { result } = renderHook(() =>
      useCurrentNoteHasContent("session-1", {
        type: "enhanced",
        id: "note-1",
      }),
    );

    expect(result.current).toBe(true);
  });

  it("reads transcript presence from SQLite", () => {
    hoisted.hasTranscript = true;

    const { result } = renderHook(() =>
      useCurrentNoteHasContent("session-1", { type: "transcript" }),
    );

    expect(result.current).toBe(true);
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

  it("shows the transcript while active before live segments arrive", () => {
    hoisted.liveSessionId = "session-1";
    hoisted.sessionMode = "active";

    const { result } = renderHook(() => useCanShowTranscript("session-1"));

    expect(result.current).toBe(true);
  });

  it("shows the transcript while finalizing", () => {
    hoisted.finalizingBySession = { "session-1": { startedAt: 1 } };
    hoisted.sessionMode = "finalizing";

    const { result } = renderHook(() => useCanShowTranscript("session-1"));

    expect(result.current).toBe(true);
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
        ["note-1"],
        false,
      );
      expect(result).toEqual({ type: "enhanced", id: "note-1" });
    });

    it("preserves raw view", () => {
      const result = computeCurrentNoteTab({ type: "raw" }, true, ["note-1"]);
      expect(result).toEqual({ type: "raw" });
    });

    it("switches transcript view to memos while listening", () => {
      const result = computeCurrentNoteTab(
        { type: "transcript" },
        true,
        ["note-1"],
        true,
      );
      expect(result).toEqual({ type: "raw" });
    });

    it("normalizes transcript view when transcript cannot show", () => {
      const result = computeCurrentNoteTab(
        { type: "transcript" },
        true,
        ["note-1"],
        false,
      );
      expect(result).toEqual({ type: "raw" });
    });

    it("returns raw view when no persisted view", () => {
      const result = computeCurrentNoteTab(null, true, ["note-1"]);
      expect(result).toEqual({ type: "raw" });
    });
  });

  describe("when not listening", () => {
    it("respects persisted enhanced view", () => {
      const result = computeCurrentNoteTab(
        { type: "enhanced", id: "note-1" },
        false,
        ["note-1"],
        false,
      );
      expect(result).toEqual({ type: "enhanced", id: "note-1" });
    });

    it("respects persisted raw view", () => {
      const result = computeCurrentNoteTab({ type: "raw" }, false, ["note-1"]);
      expect(result).toEqual({ type: "raw" });
    });

    it("respects persisted transcript view", () => {
      const result = computeCurrentNoteTab(
        { type: "transcript" },
        false,
        ["note-1"],
        true,
      );
      expect(result).toEqual({ type: "transcript" });
    });

    it("normalizes persisted transcript view before transcript content exists", () => {
      const result = computeCurrentNoteTab(
        { type: "transcript" },
        false,
        ["note-1"],
        false,
      );
      expect(result).toEqual({ type: "raw" });
    });

    it("normalizes persisted attachments view to raw", () => {
      const result = computeCurrentNoteTab(
        { type: "attachments" },
        false,
        ["note-1"],
        false,
      );
      expect(result).toEqual({ type: "raw" });
    });

    it("normalizes persisted enhanced view when no enhanced notes exist", () => {
      const result = computeCurrentNoteTab(
        { type: "enhanced", id: "note-1" },
        false,
        [],
        false,
      );
      expect(result).toEqual({ type: "raw" });
    });

    it("defaults to enhanced view when available and no persisted view", () => {
      const result = computeCurrentNoteTab(null, false, ["note-1"]);
      expect(result).toEqual({ type: "enhanced", id: "note-1" });
    });

    it("defaults to raw when no enhanced notes and no persisted view", () => {
      const result = computeCurrentNoteTab(null, false, []);
      expect(result).toEqual({ type: "raw" });
    });

    it("falls back to the migrated summary when the persisted summary id is stale", () => {
      const result = computeCurrentNoteTab(
        { type: "enhanced", id: "legacy-summary" },
        false,
        ["sqlite-summary"],
      );

      expect(result).toEqual({ type: "enhanced", id: "sqlite-summary" });
    });
  });
});
