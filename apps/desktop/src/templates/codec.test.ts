import { describe, expect, it } from "vitest";

import {
  assertCanonicalTemplateSections,
  parseStoredTemplateSections,
  parseWebTemplates,
} from "./codec";

describe("parseStoredTemplateSections", () => {
  it("parses canonical JSON text from SQLite", () => {
    expect(
      parseStoredTemplateSections(
        '[{"title":"Updates","description":"What changed"}]',
        "template-1",
      ),
    ).toEqual([{ title: "Updates", description: "What changed" }]);
  });

  it("rejects legacy string arrays from SQLite", () => {
    expect(() =>
      parseStoredTemplateSections('["Updates","Feedback"]', "template-1"),
    ).toThrow(/template template-1 sections_json/);
  });
});

describe("parseWebTemplates", () => {
  it("parses canonical web templates", () => {
    expect(
      parseWebTemplates([
        {
          slug: "one-on-one-meeting",
          title: "1:1 Meeting",
          description: "For structured one-on-one meetings",
          category: "Management",
          targets: ["Manager", "Team Lead"],
          sections: [
            { title: "Updates", description: "What changed?" },
            { title: "Feedback" },
          ],
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
          { title: "Updates", description: "What changed?" },
          { title: "Feedback", description: "" },
        ],
      },
    ]);
  });

  it("drops malformed web templates instead of repairing them", () => {
    expect(
      parseWebTemplates([
        {
          slug: "broken",
          title: "Broken Template",
          description: "",
          category: "",
          targets: ["Manager"],
          sections: ["Updates", "Feedback"],
        },
      ]),
    ).toEqual([]);
  });
});

describe("assertCanonicalTemplateSections", () => {
  it("rejects section entries that are not objects", () => {
    expect(() =>
      assertCanonicalTemplateSections(["Manager"], "enhance render"),
    ).toThrow(/enhance render/);
  });
});
