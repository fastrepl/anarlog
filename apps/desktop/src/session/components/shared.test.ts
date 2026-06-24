import { renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { computeCurrentNoteTab } from "./compute-note-tab";
import { hasStoredNoteContent, useCurrentNoteTab } from "./shared";

import type { Tab } from "~/store/zustand/tabs/schema";

const hoisted = vi.hoisted(() => ({
  batchError: null as string | null,
  enhancedNoteIds: ["note-1"] as string[],
  hasTranscript: false,
  sessionMode: "inactive",
}));

vi.mock("~/stt/contexts", () => ({
  useListener: (
    selector: (state: {
      batch: Record<string, { error: string | null } | undefined>;
      getSessionMode: () => string;
    }) => unknown,
  ) =>
    selector({
      batch: { "session-1": { error: hoisted.batchError } },
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
    useStore: () => ({}),
    useTable: () => ({}),
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
    hoisted.hasTranscript = false;
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

    it("preserves transcript view", () => {
      const result = computeCurrentNoteTab(
        { type: "transcript" },
        true,
        "note-1",
        false,
      );
      expect(result).toEqual({ type: "transcript" });
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
