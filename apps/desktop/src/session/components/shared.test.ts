import { renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { computeCurrentNoteTab } from "@anlg/utils/session";

import {
  hasStoredNoteContent,
  useCanShowTranscript,
  useCurrentNoteHasContent,
  useCurrentNoteTab,
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

  it("keeps the transcript view while listening even before transcript evidence arrives", () => {
    hoisted.sessionMode = "active";
    hoisted.liveSessionId = "session-1";

    const { result } = renderHook(() => useCurrentNoteTab(tab));

    expect(result.current).toEqual({ type: "transcript" });
  });

  it("keeps the transcript view while listening when only in-progress audio exists", () => {
    hoisted.sessionMode = "active";
    hoisted.liveSessionId = "session-1";

    const { result } = renderHook(() =>
      useCurrentNoteTab(tab, { audioExists: true }),
    );

    expect(result.current).toEqual({ type: "transcript" });
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
  const enhanced = { type: "enhanced", id: "note-1" } as const;
  const raw = { type: "raw" } as const;
  const transcript = { type: "transcript" } as const;

  it.each<
    [
      string,
      Parameters<typeof computeCurrentNoteTab>,
      ReturnType<typeof computeCurrentNoteTab>,
    ]
  >([
    [
      "keeps enhanced while listening",
      [enhanced, true, ["note-1"], false],
      enhanced,
    ],
    [
      "keeps transcript while listening",
      [transcript, true, ["note-1"], true],
      transcript,
    ],
    ["defaults to raw while listening", [null, true, ["note-1"]], raw],
    ["keeps a persisted raw view", [raw, false, ["note-1"]], raw],
    [
      "keeps a persisted transcript view",
      [transcript, false, ["note-1"], true],
      transcript,
    ],
    [
      "normalizes transcript before content exists",
      [transcript, false, ["note-1"], false],
      raw,
    ],
    [
      "normalizes transcript while listening without evidence",
      [transcript, true, ["note-1"], false],
      raw,
    ],
    [
      "normalizes the attachments view",
      [{ type: "attachments" }, false, ["note-1"], false],
      raw,
    ],
    [
      "normalizes enhanced without summaries",
      [enhanced, false, [], false],
      raw,
    ],
    [
      "defaults to the summary when available",
      [null, false, ["note-1"]],
      enhanced,
    ],
    ["defaults to raw without summaries", [null, false, []], raw],
    [
      "falls back to the migrated summary for a stale id",
      [{ type: "enhanced", id: "legacy-summary" }, false, ["sqlite-summary"]],
      { type: "enhanced", id: "sqlite-summary" },
    ],
  ])("%s", (_label, args, expected) => {
    expect(computeCurrentNoteTab(...args)).toEqual(expected);
  });
});

vi.mock("~/shared-notes/event-keys", () => ({
  useSessionSharedNoteIds: () => [],
}));
