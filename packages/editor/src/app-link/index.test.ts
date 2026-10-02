import { describe, expect, it } from "vitest";

import { type AppLinkAttrs, getAppLinkDisplayParts, getAppLinkLabel } from ".";

type AppLinkTestCase = {
  name: string;
  url: string;
  attrs: Omit<AppLinkAttrs, "url">;
  expected:
    | { kind: "label"; value: string }
    | {
        kind: "display";
        value: Partial<ReturnType<typeof getAppLinkDisplayParts>>;
        exact: boolean;
      };
};

const appLinkCases: AppLinkTestCase[] = [
  {
    name: "Linear document URLs",
    url: "https://linear.app/fastrepl-inc/document/real-world-use-cases-8dcac4144e38",
    attrs: {
      provider: "linear",
      kind: "document",
      workspace: "fastrepl-inc",
      resourceId: "real-world-use-cases-8dcac4144e38",
      resourceTitle: "Real world use cases",
    },
    expected: {
      kind: "display",
      value: { subline: "Document: Real world use cases" },
      exact: false,
    },
  },
  {
    name: "Linear issue URLs",
    url: "https://linear.app/fastrepl-inc/issue/ANLG-53/storage-model",
    attrs: {
      provider: "linear",
      kind: "issue",
      workspace: "fastrepl-inc",
      resourceId: "ANLG-53",
      resourceTitle: "Storage model",
    },
    expected: { kind: "label", value: "fastrepl-inc Issue ANLG-53" },
  },
  {
    name: "unknown Linear routes without labeling them as workspaces",
    url: "https://linear.app/fastrepl-inc/inbox/assigned-to-me",
    attrs: {
      provider: "linear",
      kind: "route",
      workspace: "fastrepl-inc",
      resourceId: "inbox/assigned-to-me",
      resourceTitle: "Inbox / Assigned to me",
    },
    expected: {
      kind: "display",
      value: {
        header: "fastrepl-inc",
        subline: "Route: Inbox / Assigned to me",
      },
      exact: true,
    },
  },
  {
    name: "Notion page URLs",
    url: "https://www.notion.so/Product-Plan-0123456789abcdef0123456789abcdef",
    attrs: {
      provider: "notion",
      kind: "page",
      resourceId: "0123456789abcdef0123456789abcdef",
      resourceTitle: "Product Plan",
    },
    expected: {
      kind: "display",
      value: { header: "Notion", subline: "Page: Product Plan" },
      exact: true,
    },
  },
  {
    name: "Google workspace URLs",
    url: "https://docs.google.com/spreadsheets/d/1a2b3c4d5e6f/edit",
    attrs: {
      provider: "google",
      kind: "spreadsheet",
      resourceId: "1a2b3c4d5e6f",
    },
    expected: {
      kind: "display",
      value: { header: "Google Sheets", subline: "Spreadsheet" },
      exact: true,
    },
  },
  {
    name: "Google Forms public URLs",
    url: "https://docs.google.com/forms/d/e/1FAIpQLSc12345/viewform",
    attrs: {
      provider: "google",
      kind: "form",
      resourceId: "1FAIpQLSc12345",
    },
    expected: {
      kind: "display",
      value: { header: "Google Forms", subline: "Form" },
      exact: true,
    },
  },
  {
    name: "Figma design URLs",
    url: "https://www.figma.com/design/abc123/Product-Roadmap?node-id=1-2",
    attrs: {
      provider: "figma",
      kind: "design",
      resourceId: "abc123",
      resourceTitle: "Product Roadmap",
    },
    expected: {
      kind: "display",
      value: { header: "Figma", subline: "Design file: Product Roadmap" },
      exact: true,
    },
  },
  {
    name: "Jira issue URLs",
    url: "https://fastrepl.atlassian.net/browse/ANLG-5540",
    attrs: {
      provider: "atlassian",
      kind: "jira_issue",
      workspace: "fastrepl",
      resourceId: "ANLG-5540",
    },
    expected: { kind: "label", value: "fastrepl Jira ANLG-5540" },
  },
  {
    name: "Confluence page URLs",
    url: "https://fastrepl.atlassian.net/wiki/spaces/ENG/pages/123456/Product+Plan",
    attrs: {
      provider: "atlassian",
      kind: "confluence_page",
      workspace: "fastrepl",
      resourceId: "123456",
      resourceTitle: "Product Plan",
    },
    expected: {
      kind: "display",
      value: { header: "fastrepl", subline: "Confluence: Product Plan" },
      exact: true,
    },
  },
  {
    name: "Asana task URLs",
    url: "https://app.asana.com/0/1200000000000000/1200000000000001/f",
    attrs: {
      provider: "asana",
      kind: "task",
      resourceId: "1200000000000001",
    },
    expected: { kind: "label", value: "Asana Task 1200000000000001" },
  },
  {
    name: "Trello card URLs",
    url: "https://trello.com/c/a1b2c3d4/product-roadmap",
    attrs: {
      provider: "trello",
      kind: "card",
      resourceId: "a1b2c3d4",
      resourceTitle: "product roadmap",
    },
    expected: { kind: "label", value: "Trello Card: product roadmap" },
  },
  {
    name: "Airtable view URLs",
    url: "https://airtable.com/appBase123/tblTable123/viwView123",
    attrs: {
      provider: "airtable",
      kind: "view",
      workspace: "appBase123",
      resourceId: "viwView123",
    },
    expected: { kind: "label", value: "Airtable View" },
  },
  {
    name: "Miro board URLs",
    url: "https://miro.com/app/board/uXjVKwz123=/",
    attrs: {
      provider: "miro",
      kind: "board",
      resourceId: "uXjVKwz123=",
    },
    expected: { kind: "label", value: "Miro Board" },
  },
  {
    name: "Loom share URLs",
    url: "https://www.loom.com/share/abcdef1234567890",
    attrs: {
      provider: "loom",
      kind: "video",
      resourceId: "abcdef1234567890",
    },
    expected: { kind: "label", value: "Loom Video" },
  },
  {
    name: "Dropbox file URLs",
    url: "https://www.dropbox.com/scl/fi/abc123/Product-Plan.pdf?dl=0",
    attrs: {
      provider: "dropbox",
      kind: "file",
      resourceId: "abc123",
      resourceTitle: "Product Plan",
    },
    expected: { kind: "label", value: "Dropbox File: Product Plan" },
  },
  {
    name: "Zoom meeting URLs",
    url: "https://fastrepl.zoom.us/j/1234567890",
    attrs: {
      provider: "zoom",
      kind: "meeting",
      resourceId: "1234567890",
      workspace: "fastrepl",
    },
    expected: { kind: "label", value: "Zoom Meeting 1234567890" },
  },
  {
    name: "Calendly event URLs",
    url: "https://calendly.com/john/product-demo",
    attrs: {
      provider: "calendly",
      kind: "event",
      workspace: "john",
      resourceId: "product-demo",
      resourceTitle: "product demo",
    },
    expected: { kind: "label", value: "Calendly Event: product demo" },
  },
];

describe("saved app link display", () => {
  it.each(appLinkCases)("renders $name", ({ url, attrs, expected }) => {
    const appLinkAttrs = { url, ...attrs };

    if (expected.kind === "label") {
      expect(getAppLinkLabel(appLinkAttrs)).toBe(expected.value);
    } else if (expected.exact) {
      expect(getAppLinkDisplayParts(appLinkAttrs)).toEqual(expected.value);
    } else {
      expect(getAppLinkDisplayParts(appLinkAttrs)).toMatchObject(
        expected.value,
      );
    }
  });
});
