import { describe, expect, test } from "vitest";

import { parseOrganizationIdFromPath } from "./changes";

describe("parseOrganizationIdFromPath", () => {
  describe("relative paths (from notify events)", () => {
    test("parses id from valid path", () => {
      expect(parseOrganizationIdFromPath("organizations/acme-corp.md")).toBe(
        "acme-corp",
      );
    });

    test("parses uuid from path", () => {
      expect(
        parseOrganizationIdFromPath(
          "organizations/550e8400-e29b-41d4-a716-446655440000.md",
        ),
      ).toBe("550e8400-e29b-41d4-a716-446655440000");
    });

    test("parses id with special characters in name", () => {
      expect(
        parseOrganizationIdFromPath("organizations/acme_corp-2024.md"),
      ).toBe("acme_corp-2024");
    });
  });

  describe("edge cases", () => {
    test("returns null for non-markdown file", () => {
      expect(
        parseOrganizationIdFromPath("organizations/acme-corp.json"),
      ).toBeNull();
    });

    test("returns null for wrong directory", () => {
      expect(parseOrganizationIdFromPath("humans/person-123.md")).toBeNull();
    });

    test("returns null for path without filename", () => {
      expect(parseOrganizationIdFromPath("organizations/")).toBeNull();
    });

    test("returns null for directory name only", () => {
      expect(parseOrganizationIdFromPath("organizations")).toBeNull();
    });

    test("returns null for empty path", () => {
      expect(parseOrganizationIdFromPath("")).toBeNull();
    });
  });

  describe("absolute paths (defensive handling)", () => {
    test("parses id from absolute path", () => {
      expect(
        parseOrganizationIdFromPath(
          "/data/char/organizations/acme-corp.md",
        ),
      ).toBe("acme-corp");
    });
  });
});
