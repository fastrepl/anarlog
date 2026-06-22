import { describe, expect, it } from "vitest";

import {
  computeCurrentNoteTab,
  getPersistedNoteTabView,
} from "./compute-note-tab";
import { hasStoredNoteContent } from "./shared";

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

  it("returns true for markdown and tiptap text content", () => {
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

describe("getPersistedNoteTabView", () => {
  it("stores raw when summary is selected during a live session", () => {
    expect(
      getPersistedNoteTabView({ type: "enhanced", id: "note-1" }, true),
    ).toEqual({ type: "raw" });
  });

  it("preserves enhanced views outside live sessions", () => {
    expect(
      getPersistedNoteTabView({ type: "enhanced", id: "note-1" }, false),
    ).toEqual({ type: "enhanced", id: "note-1" });
  });
});

describe("computeCurrentNoteTab", () => {
  describe("when listening is active", () => {
    it("returns raw view when current view is enhanced", () => {
      const result = computeCurrentNoteTab(
        { type: "enhanced", id: "note-1" },
        true,
        "note-1",
      );
      expect(result).toEqual({ type: "raw" });
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
      );
      expect(result).toEqual({ type: "transcript" });
    });

    it("normalizes non-primary enhanced view to primary enhanced view", () => {
      const result = computeCurrentNoteTab(
        { type: "enhanced", id: "note-2" },
        false,
        "note-1",
      );
      expect(result).toEqual({ type: "enhanced", id: "note-1" });
    });

    it("normalizes persisted attachments view to raw", () => {
      const result = computeCurrentNoteTab(
        { type: "attachments" },
        false,
        "note-1",
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
