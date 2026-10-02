import { describe, expect, it } from "vitest";

import {
  createEnhanceValidator,
  normalizeForComparison,
} from "./enhance-validator";

describe("normalizeForComparison", () => {
  it("normalizes ampersands, punctuation, case, and whitespace", () => {
    expect(normalizeForComparison("Status & Upload")).toBe("status and upload");
    expect(normalizeForComparison("  Hello,  World!  ")).toBe("hello world");
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

  it.each([
    "# Data File Status and Upload Testing",
    "# Data File",
    "# data file status & upload testing",
    "Here is the summary:\n# Data File Status and Upload Testing",
  ])("accepts valid heading input: %s", (input) => {
    expect(createEnhanceValidator(template)(input)).toEqual({ valid: true });
  });

  it.each([
    "Here is the summary",
    "## Data File Status",
    "# Something Entirely Different",
  ])("rejects invalid heading input: %s", (input) => {
    expect(createEnhanceValidator(template)(input).valid).toBe(false);
  });

  it.each([
    {
      template: null,
      input: "Sure, here you go:\n# Any Heading",
    },
    {
      template: { title: "", description: null, sections: [] },
      input: "# Any Heading Works",
    },
  ])(
    "accepts heading input without section matching: $input",
    ({ template: inputTemplate, input }) => {
      expect(createEnhanceValidator(inputTemplate)(input)).toEqual({
        valid: true,
      });
    },
  );

  it("allows custom instructions to replace heading and template format requirements", () => {
    const validator = createEnhanceValidator(template, {
      overrideTemplateFormatting: true,
    });

    expect(validator("Decisions\n- Ship it")).toEqual({ valid: true });
    expect(validator("# Custom Heading")).toEqual({ valid: true });
  });
});
