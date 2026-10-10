import { describe, expect, it } from "vitest";

import {
  countNormalizedCharacters,
  countTranscriptWordCharacters,
  formatSummaryLengthGuidance,
  type SummaryLengthPolicy,
} from "./summary-length";

const detailedPolicy: SummaryLengthPolicy = {
  transcript_characters: 636,
  guidance: {
    max_characters: 636,
    min_sections: 1,
    max_sections: 2,
  },
};

describe("summary length guidance", () => {
  it("counts transcript characters across languages without relying on spaces", () => {
    expect(
      countTranscriptWordCharacters([
        { words: [{ text: "이번" }, { text: "회의는" }, { text: "짧음" }] },
      ]),
    ).toBe(9);
    expect(countNormalizedCharacters("😀  meeting\tnotes")).toBe(15);
  });

  it("renders proportional length guidance for the prompt", () => {
    const guidance = formatSummaryLengthGuidance(detailedPolicy);

    expect(guidance).toContain("about 636 characters");
    expect(guidance).toContain("1 to 2 sections");
    expect(guidance).toContain("under 636 characters");
    expect(formatSummaryLengthGuidance(null)).toBeNull();
  });

  it("keeps every template section under the length budget", () => {
    const guidance = formatSummaryLengthGuidance(
      {
        ...detailedPolicy,
        transcript_characters: 10_000,
        guidance: {
          max_characters: 10_000,
          min_sections: 3,
          max_sections: 6,
        },
      },
      { hasTemplateSections: true },
    );

    expect(guidance).toContain("Summary length:");
    expect(guidance).toContain(
      "Keep every requested template section and stay under 10000 characters overall.",
    );
    expect(guidance).not.toContain("sections and stay under");
    expect(guidance).not.toMatch(/\d to \d sections|exactly \d+ section/);
  });

  it("keeps custom-format guidance independent of section count", () => {
    expect(
      formatSummaryLengthGuidance(detailedPolicy, { customFormat: true }),
    ).toContain("Keep the requested structure");
    expect(
      formatSummaryLengthGuidance(detailedPolicy, { customFormat: true }),
    ).toContain("under 636 characters");
  });
});
