import { describe, expect, it } from "vitest";

import {
  constrainSummaryLength,
  countNormalizedCharacters,
  countTranscriptWordCharacters,
  formatSummaryLengthGuidance,
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
      maxCharacters: 320,
      maxSections: 2,
      guidance: {
        maxCharacters: 320,
        minSections: 1,
        maxSections: 2,
      },
    });
  });

  it("scales the guided section range with the transcript size", () => {
    const policyFor = (characters: number) =>
      getSummaryLengthPolicy([
        {
          startedAt: null,
          endedAt: null,
          segments: [{ speaker: "John", text: "a".repeat(characters) }],
        },
      ])?.guidance;

    expect(policyFor(636)).toEqual({
      maxCharacters: 636,
      minSections: 1,
      maxSections: 2,
    });
    expect(policyFor(6_000)).toEqual({
      maxCharacters: 6_000,
      minSections: 2,
      maxSections: 4,
    });
    expect(policyFor(30_000)).toEqual({
      maxCharacters: 6_000,
      minSections: 5,
      maxSections: 8,
    });
  });

  it("renders proportional length guidance for the prompt", () => {
    const policy = getSummaryLengthPolicy([
      {
        startedAt: null,
        endedAt: null,
        segments: [{ speaker: "John", text: "a".repeat(636) }],
      },
    ]);

    const guidance = formatSummaryLengthGuidance(policy);

    expect(guidance).toContain("about 636 characters");
    expect(guidance).toContain("1 to 2 sections");
    expect(guidance).toContain("under 636 characters");
    expect(formatSummaryLengthGuidance(null)).toBeNull();
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
