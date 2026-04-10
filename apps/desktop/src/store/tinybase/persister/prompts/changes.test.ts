import { describe, expect, test } from "vitest";

import { parsePromptIdFromPath } from "./changes";

describe("parsePromptIdFromPath", () => {
  describe("relative paths (from notify events)", () => {
    test("parses id from valid path", () => {
      expect(parsePromptIdFromPath("prompts/my-prompt.md")).toBe("my-prompt");
    });

    test("parses uuid from path", () => {
      expect(
        parsePromptIdFromPath(
          "prompts/550e8400-e29b-41d4-a716-446655440000.md",
        ),
      ).toBe("550e8400-e29b-41d4-a716-446655440000");
    });

    test("parses id with special characters in name", () => {
      expect(parsePromptIdFromPath("prompts/meeting-notes_v2.md")).toBe(
        "meeting-notes_v2",
      );
    });
  });

  describe("edge cases", () => {
    test("returns null for non-markdown file", () => {
      expect(parsePromptIdFromPath("prompts/my-prompt.json")).toBeNull();
    });

    test("returns null for wrong directory", () => {
      expect(parsePromptIdFromPath("humans/person-123.md")).toBeNull();
    });

    test("returns null for path without filename", () => {
      expect(parsePromptIdFromPath("prompts/")).toBeNull();
    });

    test("returns null for directory name only", () => {
      expect(parsePromptIdFromPath("prompts")).toBeNull();
    });

    test("returns null for empty path", () => {
      expect(parsePromptIdFromPath("")).toBeNull();
    });
  });

  describe("absolute paths (defensive handling)", () => {
    test("parses id from absolute path", () => {
      expect(parsePromptIdFromPath("/data/char/prompts/my-prompt.md")).toBe(
        "my-prompt",
      );
    });
  });
});
