import { describe, expect, it, vi } from "vitest";

import {
  appendTagLineToMarkdown,
  extractEnhanceTagNames,
  upsertSessionTags,
} from "./summary-tags";

function createEnhanceArgs(
  overrides: Partial<Parameters<typeof extractEnhanceTagNames>[1]> = {},
): Parameters<typeof extractEnhanceTagNames>[1] {
  return {
    language: "en",
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

  it("upserts tag rows and session mappings", () => {
    const store = {
      getValue: vi.fn().mockReturnValue("user-1"),
      setRow: vi.fn(),
    } as any;

    upsertSessionTags(store, "session-1", ["#Launch", "launch", "prep"]);

    expect(store.setRow).toHaveBeenCalledWith("tags", "launch", {
      user_id: "user-1",
      name: "launch",
    });
    expect(store.setRow).toHaveBeenCalledWith(
      "mapping_tag_session",
      "session-1:prep",
      {
        user_id: "user-1",
        tag_id: "prep",
        session_id: "session-1",
      },
    );
  });
});
