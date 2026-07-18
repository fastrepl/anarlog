import { describe, expect, it } from "vitest";

import {
  appendTagLineToMarkdown,
  extractEnhanceTagNames,
} from "./summary-tags";

function createEnhanceArgs(
  overrides: Partial<Parameters<typeof extractEnhanceTagNames>[1]> = {},
): Parameters<typeof extractEnhanceTagNames>[1] {
  return {
    language: "en",
    promptOverride: "",
    session: {
      title: "Weekly Review",
      startedAt: null,
      endedAt: null,
      event: null,
    },
    participants: [],
    template: null,
    preMeetingMemo: "",
    postMeetingMemo: "",
    transcripts: [],
    imageContext: [],
    ...overrides,
  };
}

describe("summary tags", () => {
  it("extracts unique hashtags from summary, memos, and template content", () => {
    const tags = extractEnhanceTagNames(
      "# Summary\n\nDiscussed #Launch and issue #123.",
      createEnhanceArgs({
        preMeetingMemo: "Prep #prep #launch",
        postMeetingMemo: "Next #follow-up",
        template: {
          title: "Template #customer",
          description: null,
          sections: [
            {
              title: "Actions",
              description: "Use #owners",
            },
          ],
        },
      }),
    );

    expect(tags).toEqual(["launch", "prep", "follow-up", "customer", "owners"]);
  });

  it("appends tags at the bottom without duplicating existing trailing tags", () => {
    expect(
      appendTagLineToMarkdown("Body\n\n#old #tags", ["old", "tags", "new"]),
    ).toBe("Body\n\n#old #tags #new");
  });
});
