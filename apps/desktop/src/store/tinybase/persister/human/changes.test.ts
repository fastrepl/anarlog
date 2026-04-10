import { describe, expect, test } from "vitest";

import { parseHumanIdFromPath } from "./changes";

describe("parseHumanIdFromPath", () => {
  describe("relative paths (from notify events)", () => {
    test("parses id from valid path", () => {
      expect(parseHumanIdFromPath("humans/person-123.md")).toBe("person-123");
    });

    test("parses uuid from path", () => {
      expect(
        parseHumanIdFromPath("humans/550e8400-e29b-41d4-a716-446655440000.md"),
      ).toBe("550e8400-e29b-41d4-a716-446655440000");
    });

    test("parses id with special characters in name", () => {
      expect(parseHumanIdFromPath("humans/john-doe_2024.md")).toBe(
        "john-doe_2024",
      );
    });
  });

  describe("edge cases", () => {
    test("returns null for non-markdown file", () => {
      expect(parseHumanIdFromPath("humans/person-123.json")).toBeNull();
    });

    test("returns null for wrong directory", () => {
      expect(parseHumanIdFromPath("organizations/org-123.md")).toBeNull();
    });

    test("returns null for path without filename", () => {
      expect(parseHumanIdFromPath("humans/")).toBeNull();
    });

    test("returns null for directory name only", () => {
      expect(parseHumanIdFromPath("humans")).toBeNull();
    });

    test("returns null for empty path", () => {
      expect(parseHumanIdFromPath("")).toBeNull();
    });
  });

  describe("absolute paths (defensive handling)", () => {
    test("parses id from absolute path", () => {
      expect(parseHumanIdFromPath("/data/char/humans/person-123.md")).toBe(
        "person-123",
      );
    });
  });
});
