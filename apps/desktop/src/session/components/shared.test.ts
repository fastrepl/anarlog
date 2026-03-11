import { describe, expect, it } from "vitest";

import { computeCurrentNoteTab } from "./compute-note-tab";

describe("computeCurrentNoteTab", () => {
  describe("when listening is active", () => {
    it("returns raw view when current view is enhanced", () => {
      const result = computeCurrentNoteTab(
        { type: "enhanced", id: "note-1" },
        true,
        true,
        ["note-1"],
      );
      expect(result).toEqual({ type: "raw" });
    });

    it("preserves raw view", () => {
      const result = computeCurrentNoteTab({ type: "raw" }, true, true, [
        "note-1",
      ]);
      expect(result).toEqual({ type: "raw" });
    });

    it("preserves transcript view", () => {
      const result = computeCurrentNoteTab({ type: "transcript" }, true, true, [
        "note-1",
      ]);
      expect(result).toEqual({ type: "transcript" });
    });

    it("returns raw view when no persisted view", () => {
      const result = computeCurrentNoteTab(null, true, true, ["note-1"]);
      expect(result).toEqual({ type: "raw" });
    });
  });

  describe("when not listening", () => {
    it("respects persisted enhanced view", () => {
      const result = computeCurrentNoteTab(
        { type: "enhanced", id: "note-1" },
        false,
        true,
        ["note-1"],
      );
      expect(result).toEqual({ type: "enhanced", id: "note-1" });
    });

    it("respects persisted raw view", () => {
      const result = computeCurrentNoteTab({ type: "raw" }, false, true, [
        "note-1",
      ]);
      expect(result).toEqual({ type: "raw" });
    });

    it("respects persisted transcript view", () => {
      const result = computeCurrentNoteTab(
        { type: "transcript" },
        false,
        true,
        ["note-1"],
      );
      expect(result).toEqual({ type: "transcript" });
    });

    it("defaults to enhanced view when available and no persisted view", () => {
      const result = computeCurrentNoteTab(null, false, true, ["note-1"]);
      expect(result).toEqual({ type: "enhanced", id: "note-1" });
    });

    it("defaults to raw when no enhanced notes and no persisted view", () => {
      const result = computeCurrentNoteTab(null, false, false, []);
      expect(result).toEqual({ type: "raw" });
    });

    it("falls back to raw when transcript view is no longer available", () => {
      const result = computeCurrentNoteTab(
        { type: "transcript" },
        false,
        false,
        [],
      );
      expect(result).toEqual({ type: "raw" });
    });
  });
});
