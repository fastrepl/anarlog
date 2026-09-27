import { describe, expect, it } from "vitest";

import {
  constrainSummaryLength,
  countNormalizedCharacters,
  countTranscriptWordCharacters,
  formatSummaryLengthModeGuidance,
  formatSummaryLengthGuidance,
  getSummaryLengthPolicy,
  normalizeSummaryLengthMode,
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
      mode: "detailed",
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
      maxCharacters: 30_000,
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

  it("reduces guidance budgets for balanced and crisp modes while keeping the hard cap", () => {
    const transcripts = [
      {
        startedAt: null,
        endedAt: null,
        segments: [{ speaker: "John", text: "a".repeat(10_000) }],
      },
    ];

    expect(getSummaryLengthPolicy(transcripts, "detailed")).toMatchObject({
      maxCharacters: 10_000,
      guidance: { maxCharacters: 10_000, minSections: 3, maxSections: 6 },
    });
    expect(getSummaryLengthPolicy(transcripts, "balanced")).toMatchObject({
      maxCharacters: 10_000,
      guidance: { maxCharacters: 5_000, minSections: 2, maxSections: 3 },
    });
    expect(getSummaryLengthPolicy(transcripts, "crisp")).toMatchObject({
      maxCharacters: 10_000,
      guidance: { maxCharacters: 2_500, minSections: 1, maxSections: 2 },
    });
  });

  it("keeps at least two guided sections for crisp summaries", () => {
    const policy = getSummaryLengthPolicy(
      [
        {
          startedAt: null,
          endedAt: null,
          segments: [{ speaker: "John", text: "a".repeat(1_000) }],
        },
      ],
      "crisp",
    );

    expect(policy?.guidance).toMatchObject({ minSections: 1, maxSections: 2 });
    expect(formatSummaryLengthGuidance(policy)).toContain("1 to 2 sections");
  });

  it("floors the guidance budget at 150 characters per template section", () => {
    const policy = getSummaryLengthPolicy(
      [
        {
          startedAt: null,
          endedAt: null,
          segments: [{ speaker: "John", text: "a".repeat(160) }],
        },
      ],
      "crisp",
      true,
      12,
    );

    expect(policy?.guidance?.maxCharacters).toBe(1_800);
  });

  it.each([
    ["crisp", 7_500, "about half the length of a balanced summary"],
    ["balanced", 15_000, "the baseline length"],
    ["detailed", 30_000, "about twice the length of a balanced summary"],
  ] as const)(
    "scales %s guidance to %s characters relative to the transcript",
    (mode, expected, description) => {
      const policy = getSummaryLengthPolicy(
        [
          {
            startedAt: null,
            endedAt: null,
            segments: [{ speaker: "John", text: "a".repeat(30_000) }],
          },
        ],
        mode,
      );

      expect(policy?.guidance?.maxCharacters).toBe(expected);
      expect(formatSummaryLengthGuidance(policy)).toContain(
        `Summary length mode "${mode}" is ${description}.`,
      );
    },
  );

  it("keeps every template section under the length budget", () => {
    const policy = getSummaryLengthPolicy(
      [
        {
          startedAt: null,
          endedAt: null,
          segments: [{ speaker: "John", text: "a".repeat(10_000) }],
        },
      ],
      "detailed",
      true,
    );

    const guidance = formatSummaryLengthGuidance(policy, {
      hasTemplateSections: true,
    });

    expect(guidance).toContain("Summary length:");
    expect(guidance).toContain(
      "Keep every requested template section and stay under 10000 characters overall.",
    );
    expect(guidance).not.toContain("sections and stay under");
    expect(guidance).not.toMatch(/\d to \d sections|exactly \d+ section/);
  });

  it("keeps detailed as the default and explicitly requests full context", () => {
    expect(normalizeSummaryLengthMode(undefined)).toBe("detailed");
    expect(normalizeSummaryLengthMode("unsupported")).toBe("detailed");
    expect(normalizeSummaryLengthMode("crisp")).toBe("crisp");
    expect(formatSummaryLengthModeGuidance("detailed", false)).toContain(
      "every material topic",
    );
  });

  it.each(["crisp", "balanced", "detailed"] as const)(
    "keeps %s guidance independent of presentation",
    (mode) => {
      for (const hasTemplate of [false, true]) {
        const guidance = formatSummaryLengthModeGuidance(mode, hasTemplate);
        expect(guidance).not.toMatch(
          /bullet|list item|# Next Steps|never put prose/,
        );
      }
      expect(formatSummaryLengthModeGuidance(mode, true)).toContain(
        "Preserve every requested template section",
      );
    },
  );

  it("preserves custom sections while retaining the length budget", () => {
    const transcripts = [
      {
        startedAt: null,
        endedAt: null,
        segments: [{ speaker: "John", text: "a".repeat(636) }],
      },
    ];
    const custom = getSummaryLengthPolicy(transcripts, "detailed", true);
    const standard = getSummaryLengthPolicy(transcripts, "detailed");
    expect(custom?.maxSections).toBeNull();
    expect(standard?.maxSections).toBe(2);
    expect(custom?.maxCharacters).toBe(standard?.maxCharacters);
    const guidance = formatSummaryLengthGuidance(custom, {
      customFormat: true,
    });
    expect(guidance).toContain("Keep the requested structure");
    expect(guidance).toContain("under 636 characters");
    expect(guidance).not.toContain("1 to 2 sections");
  });

  it("keeps no more than two sections or the transcript character count", () => {
    const markdown = `# First

- ${"a".repeat(40)}

# Second

- ${"b".repeat(40)}

# Third

- ${"c".repeat(100)}`;
    const result = constrainSummaryLength(markdown, {
      mode: "detailed",
      transcriptCharacters: 160,
      maxCharacters: 160,
      maxSections: 2,
    });

    expect(result).toContain("# First");
    expect(result).toContain("# Second");
    expect(result).not.toContain("# Third");
    expect(countNormalizedCharacters(result)).toBeLessThanOrEqual(160);
  });

  it("never truncates a summary in the middle of a sentence", () => {
    const result = constrainSummaryLength(
      `# Decision

- The team approved the launch. This additional explanation does not fit within the summary limit.

# Follow-up`,
      {
        mode: "detailed",
        transcriptCharacters: 60,
        maxCharacters: 60,
        maxSections: null,
      },
    );

    expect(result).toBe("# Decision\n\n- The team approved the launch.");
    expect(countNormalizedCharacters(result)).toBeLessThanOrEqual(60);
  });

  it("keeps period-less bullets at a word boundary", () => {
    const result = constrainSummaryLength(
      `# Decision

- alpha beta gamma delta epsilon zeta`,
      {
        mode: "detailed",
        transcriptCharacters: 30,
        maxCharacters: 30,
        maxSections: null,
      },
    );

    expect(result).toBe("# Decision\n\n- alpha beta");
    expect(countNormalizedCharacters(result)).toBeLessThanOrEqual(30);
  });
});
