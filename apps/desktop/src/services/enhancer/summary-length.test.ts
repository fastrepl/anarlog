import { describe, expect, it } from "vitest";

import {
  constrainSummaryLength,
  countNormalizedCharacters,
  countTranscriptWordCharacters,
  getSummaryLengthPolicy,
} from "./summary-length";

describe("summary length policy", () => {
  it("counts transcript characters across languages without relying on spaces", () => {
    expect(
      countTranscriptWordCharacters([
        { words: [{ text: "이번" }, { text: "회의는" }, { text: "짧음" }] },
      ]),
    ).toBe(9);
  });

  it("caps short transcripts at two sections and sizes the output budget", () => {
    const policy = getSummaryLengthPolicy([
      {
        startedAt: null,
        endedAt: null,
        segments: [{ speaker: "John", text: "a".repeat(200) }],
      },
    ]);

    expect(policy).toEqual({
      transcriptCharacters: 200,
      maxCharacters: 200,
      maxSections: 2,
    });
  });

  it("keeps long transcripts on the normal section limit", () => {
    const policy = getSummaryLengthPolicy([
      {
        startedAt: null,
        endedAt: null,
        segments: [{ speaker: "John", text: "a".repeat(10_000) }],
      },
    ]);

    expect(policy).toMatchObject({
      transcriptCharacters: 10_000,
      maxCharacters: 10_000,
      maxSections: null,
    });
  });

  it("keeps no more than two sections or the transcript character count", () => {
    const markdown = `# First

- ${"a".repeat(100)}

# Second

- ${"b".repeat(100)}

# Third

- ${"c".repeat(100)}`;
    const result = constrainSummaryLength(markdown, {
      transcriptCharacters: 160,
      maxCharacters: 160,
      maxSections: 2,
    });

    expect(result).toContain("# First");
    expect(result).toContain("# Second");
    expect(result).not.toContain("# Third");
    expect(countNormalizedCharacters(result)).toBeLessThanOrEqual(160);
  });
});
