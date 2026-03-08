import { describe, expect, it } from "vitest";

import {
  createEnhanceValidator,
  normalizeForComparison,
} from "./enhance-validator";

describe("normalizeForComparison", () => {
  it("lowercases and replaces & with and", () => {
    expect(normalizeForComparison("Status & Upload")).toBe("status and upload");
  });

  it("strips punctuation and collapses whitespace", () => {
    expect(normalizeForComparison("  Hello,  World!  ")).toBe("hello world");
  });

  it("handles empty string", () => {
    expect(normalizeForComparison("")).toBe("");
  });
});

describe("createEnhanceValidator", () => {
  const template = {
    title: "",
    description: null,
    sections: [
      { title: "Data File Status and Upload Testing", description: null },
      { title: "API Integration Issues", description: null },
    ],
  };

  describe("h1 prefix requirement", () => {
    it("rejects text with no # at all", () => {
      const v = createEnhanceValidator(template);
      expect(v("Here is the summary")).toEqual({
        valid: false,
        feedback: "Output must start with a markdown h1 heading (# Title).",
      });
    });

    it("rejects h2 headings", () => {
      const v = createEnhanceValidator(template);
      expect(v("## Data File Status")).toEqual({
        valid: false,
        feedback: "Output must start with a markdown h1 heading (# Title).",
      });
    });
  });

  describe("section heading match", () => {
    it("accepts exact match", () => {
      const v = createEnhanceValidator(template);
      expect(v("# Data File Status and Upload Testing")).toEqual({
        valid: true,
      });
    });

    it("accepts partial streaming text that is a prefix of expected", () => {
      const v = createEnhanceValidator(template);
      expect(v("# Data File")).toEqual({ valid: true });
    });

    it("accepts fuzzy match (& vs and)", () => {
      const v = createEnhanceValidator(template);
      expect(v("# Data File Status & Upload Testing")).toEqual({ valid: true });
    });

    it("accepts fuzzy match (case difference)", () => {
      const v = createEnhanceValidator(template);
      expect(v("# data file status and upload testing")).toEqual({
        valid: true,
      });
    });

    it("rejects completely different heading", () => {
      const v = createEnhanceValidator(template);
      const result = v("# Something Entirely Different");
      expect(result.valid).toBe(false);
    });
  });

  describe("preamble stripping", () => {
    it("accepts output with preamble text before heading", () => {
      const v = createEnhanceValidator(template);
      expect(
        v("Here is the summary:\n# Data File Status and Upload Testing"),
      ).toEqual({
        valid: true,
      });
    });

    it("accepts preamble with correct heading for null template", () => {
      const v = createEnhanceValidator(null);
      expect(v("Sure, here you go:\n# Any Heading")).toEqual({ valid: true });
    });
  });

  describe("skip heading match", () => {
    it("skips heading match when template is null", () => {
      const v = createEnhanceValidator(null);
      expect(v("# Any Heading Works")).toEqual({ valid: true });
    });

    it("skips heading match when sections are empty", () => {
      const empty = { title: "", description: null, sections: [] };
      const v = createEnhanceValidator(empty);
      expect(v("# Any Heading Works")).toEqual({ valid: true });
    });
  });
});
