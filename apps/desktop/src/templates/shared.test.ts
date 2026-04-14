import { describe, expect, it } from "vitest";

import { normalizeTemplateSections, normalizeWebTemplates } from "./shared";

describe("normalizeTemplateSections", () => {
  it("converts legacy string sections into TemplateSection objects", () => {
    expect(normalizeTemplateSections(["Wins", "Risks"])).toEqual([
      { title: "Wins", description: "" },
      { title: "Risks", description: "" },
    ]);
  });

  it("parses stringified section payloads", () => {
    expect(
      normalizeTemplateSections(
        '[{"title":"Summary","description":"What changed"}]',
      ),
    ).toEqual([{ title: "Summary", description: "What changed" }]);
  });
});

describe("normalizeWebTemplates", () => {
  it("normalizes malformed template payloads from the web API", () => {
    expect(
      normalizeWebTemplates([
        {
          slug: "one-on-one-meeting",
          title: "1:1 Meeting",
          description: "For structured one-on-one meetings",
          category: "Management",
          targets: '["Manager","Team Lead"]',
          sections: '["Updates","Feedback"]',
        },
      ]),
    ).toEqual([
      {
        slug: "one-on-one-meeting",
        title: "1:1 Meeting",
        description: "For structured one-on-one meetings",
        category: "Management",
        targets: ["Manager", "Team Lead"],
        sections: [
          { title: "Updates", description: "" },
          { title: "Feedback", description: "" },
        ],
      },
    ]);
  });
});
