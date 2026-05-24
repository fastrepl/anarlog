import { describe, expect, it } from "vitest";

import {
  assertCanonicalTemplateSections,
  parseStoredTemplateSections,
  parseStoredTemplateTargets,
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

  it("normalizes legacy string arrays from SQLite", () => {
    expect(
      parseStoredTemplateSections('["Updates","Feedback"]', "template-1"),
    ).toEqual([
      { title: "Updates", description: "" },
      { title: "Feedback", description: "" },
    ]);
  });

  it("fills missing descriptions on stored section objects", () => {
    expect(
      parseStoredTemplateSections('[{"title":"Updates"}]', "template-1"),
    ).toEqual([{ title: "Updates", description: "" }]);
  });

  it("preserves blank draft sections from SQLite", () => {
    expect(
      parseStoredTemplateSections(
        '[{"title":"","description":""}]',
        "template-1",
      ),
    ).toEqual([{ title: "", description: "" }]);
  });

  it("preserves described draft sections with blank titles from SQLite", () => {
    expect(
      parseStoredTemplateSections(
        '[{"title":"","description":"Capture decisions"}]',
        "template-1",
      ),
    ).toEqual([{ title: "", description: "Capture decisions" }]);
  });

  it("returns empty sections for invalid stored JSON", () => {
    expect(parseStoredTemplateSections("{", "template-1")).toEqual([]);
  });
});

describe("parseStoredTemplateTargets", () => {
  it("normalizes a legacy single-string target", () => {
    expect(parseStoredTemplateTargets('"Manager"', "template-1")).toEqual([
      "Manager",
    ]);
  });

  it("returns undefined for invalid stored target JSON", () => {
    expect(parseStoredTemplateTargets("{", "template-1")).toBeUndefined();
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

  it("keeps blank draft rows before saving", () => {
    expect(
      assertCanonicalTemplateSections(
        [{ title: "", description: "" }],
        "template form",
      ),
    ).toEqual([{ title: "", description: "" }]);
  });

  it("keeps described draft rows with blank section names", () => {
    expect(
      assertCanonicalTemplateSections(
        [{ title: "", description: "Capture decisions" }],
        "template form",
      ),
    ).toEqual([{ title: "", description: "Capture decisions" }]);
  });
});
