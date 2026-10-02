import { describe, expect, it } from "vitest";

import {
  assertCanonicalTemplateSections,
  parseStoredTemplateSections,
  parseStoredTemplateTargets,
  parseWebTemplates,
} from "./codec";
import { DEFAULT_TEMPLATE_ICON } from "./template-icon";

describe("parseStoredTemplateSections", () => {
  it.each([
    [
      "canonical JSON text",
      '[{"title":"Updates","description":"What changed"}]',
      [{ title: "Updates", description: "What changed" }],
    ],
    [
      "legacy string arrays",
      '["Updates","Feedback"]',
      [
        { title: "Updates", description: "" },
        { title: "Feedback", description: "" },
      ],
    ],
    [
      "missing descriptions on stored objects",
      '[{"title":"Updates"}]',
      [{ title: "Updates", description: "" }],
    ],
    [
      "blank draft sections",
      '[{"title":"","description":""},{"title":"","description":"Capture decisions"}]',
      [
        { title: "", description: "" },
        { title: "", description: "Capture decisions" },
      ],
    ],
    ["invalid stored JSON", "{", []],
  ])("normalizes stored sections: %s", (_name, raw, expected) => {
    expect(parseStoredTemplateSections(raw, "template-1")).toEqual(expected);
  });
});

describe("parseStoredTemplateTargets", () => {
  it("normalizes stored targets and rejects invalid JSON", () => {
    expect(parseStoredTemplateTargets('"Manager"', "template-1")).toEqual([
      "Manager",
    ]);
    expect(parseStoredTemplateTargets("{", "template-1")).toBeUndefined();
  });
});

describe("parseWebTemplates", () => {
  it("parses canonical web templates and their icons", () => {
    expect(
      parseWebTemplates([
        {
          title: "Launch",
          icon: { type: "emoji", value: "🚀" },
          sections: [],
        },
      ])[0]?.icon,
    ).toEqual({ type: "emoji", value: "🚀" });
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
        icon: DEFAULT_TEMPLATE_ICON,
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

  it("keeps draft rows before saving", () => {
    expect(
      assertCanonicalTemplateSections(
        [
          { title: "", description: "" },
          { title: "", description: "Capture decisions" },
        ],
        "template form",
      ),
    ).toEqual([
      { title: "", description: "" },
      { title: "", description: "Capture decisions" },
    ]);
  });
});
