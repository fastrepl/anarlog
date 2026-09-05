import { describe, expect, it } from "vitest";

import { type AppLinkAttrs, getAppLinkDisplayParts, getAppLinkLabel } from ".";

describe("saved app link display", () => {
  it("renders Linear document URLs", () => {
    const attrs: AppLinkAttrs = {
      url: "https://linear.app/fastrepl-inc/document/real-world-use-cases-8dcac4144e38",
      provider: "linear",
      kind: "document",
      workspace: "fastrepl-inc",
      resourceId: "real-world-use-cases-8dcac4144e38",
      resourceTitle: "Real world use cases",
    };
    expect(getAppLinkDisplayParts(attrs).subline).toBe(
      "Document: Real world use cases",
    );
  });

  it("renders Linear issue URLs", () => {
    const attrs: AppLinkAttrs = {
      url: "https://linear.app/fastrepl-inc/issue/ANLG-53/storage-model",
      provider: "linear",
      kind: "issue",
      workspace: "fastrepl-inc",
      resourceId: "ANLG-53",
      resourceTitle: "Storage model",
    };
    expect(getAppLinkLabel(attrs)).toBe("fastrepl-inc Issue ANLG-53");
  });

  it("renders unknown Linear routes without labeling them as workspaces", () => {
    const attrs: AppLinkAttrs = {
      url: "https://linear.app/fastrepl-inc/inbox/assigned-to-me",
      provider: "linear",
      kind: "route",
      workspace: "fastrepl-inc",
      resourceId: "inbox/assigned-to-me",
      resourceTitle: "Inbox / Assigned to me",
    };
    expect(getAppLinkDisplayParts(attrs)).toEqual({
      header: "fastrepl-inc",
      subline: "Route: Inbox / Assigned to me",
    });
  });

  it("renders Notion page URLs", () => {
    const attrs: AppLinkAttrs = {
      url: "https://www.notion.so/Product-Plan-0123456789abcdef0123456789abcdef",
      provider: "notion",
      kind: "page",
      resourceId: "0123456789abcdef0123456789abcdef",
      resourceTitle: "Product Plan",
    };
    expect(getAppLinkDisplayParts(attrs)).toEqual({
      header: "Notion",
      subline: "Page: Product Plan",
    });
  });

  it("renders Google workspace URLs", () => {
    const attrs: AppLinkAttrs = {
      url: "https://docs.google.com/spreadsheets/d/1a2b3c4d5e6f/edit",
      provider: "google",
      kind: "spreadsheet",
      resourceId: "1a2b3c4d5e6f",
    };
    expect(getAppLinkDisplayParts(attrs)).toEqual({
      header: "Google Sheets",
      subline: "Spreadsheet",
    });
  });

  it("renders Google Forms public URLs", () => {
    const attrs: AppLinkAttrs = {
      url: "https://docs.google.com/forms/d/e/1FAIpQLSc12345/viewform",
      provider: "google",
      kind: "form",
      resourceId: "1FAIpQLSc12345",
    };
    expect(getAppLinkDisplayParts(attrs)).toEqual({
      header: "Google Forms",
      subline: "Form",
    });
  });

  it("renders Figma design URLs", () => {
    const attrs: AppLinkAttrs = {
      url: "https://www.figma.com/design/abc123/Product-Roadmap?node-id=1-2",
      provider: "figma",
      kind: "design",
      resourceId: "abc123",
      resourceTitle: "Product Roadmap",
    };
    expect(getAppLinkDisplayParts(attrs)).toEqual({
      header: "Figma",
      subline: "Design file: Product Roadmap",
    });
  });

  it("renders Jira issue URLs", () => {
    const attrs: AppLinkAttrs = {
      url: "https://fastrepl.atlassian.net/browse/ANLG-5540",
      provider: "atlassian",
      kind: "jira_issue",
      workspace: "fastrepl",
      resourceId: "ANLG-5540",
    };
    expect(getAppLinkLabel(attrs)).toBe("fastrepl Jira ANLG-5540");
  });

  it("renders Confluence page URLs", () => {
    const attrs: AppLinkAttrs = {
      url: "https://fastrepl.atlassian.net/wiki/spaces/ENG/pages/123456/Product+Plan",
      provider: "atlassian",
      kind: "confluence_page",
      workspace: "fastrepl",
      resourceId: "123456",
      resourceTitle: "Product Plan",
    };
    expect(getAppLinkDisplayParts(attrs)).toEqual({
      header: "fastrepl",
      subline: "Confluence: Product Plan",
    });
  });

  it.each<[string, string, Omit<import("./work").WorkAttrs, "url">, string]>([
    [
      "Asana task URLs",
      "https://app.asana.com/0/1200000000000000/1200000000000001/f",
      {
        provider: "asana",
        kind: "task",
        resourceId: "1200000000000001",
      },
      "Asana Task 1200000000000001",
    ],
    [
      "Trello card URLs",
      "https://trello.com/c/a1b2c3d4/product-roadmap",
      {
        provider: "trello",
        kind: "card",
        resourceId: "a1b2c3d4",
        resourceTitle: "product roadmap",
      },
      "Trello Card: product roadmap",
    ],
    [
      "Airtable view URLs",
      "https://airtable.com/appBase123/tblTable123/viwView123",
      {
        provider: "airtable",
        kind: "view",
        workspace: "appBase123",
        resourceId: "viwView123",
      },
      "Airtable View",
    ],
    [
      "Miro board URLs",
      "https://miro.com/app/board/uXjVKwz123=/",
      {
        provider: "miro",
        kind: "board",
        resourceId: "uXjVKwz123=",
      },
      "Miro Board",
    ],
    [
      "Loom share URLs",
      "https://www.loom.com/share/abcdef1234567890",
      {
        provider: "loom",
        kind: "video",
        resourceId: "abcdef1234567890",
      },
      "Loom Video",
    ],
    [
      "Dropbox file URLs",
      "https://www.dropbox.com/scl/fi/abc123/Product-Plan.pdf?dl=0",
      {
        provider: "dropbox",
        kind: "file",
        resourceId: "abc123",
        resourceTitle: "Product Plan",
      },
      "Dropbox File: Product Plan",
    ],
    [
      "Zoom meeting URLs",
      "https://fastrepl.zoom.us/j/1234567890",
      {
        provider: "zoom",
        kind: "meeting",
        resourceId: "1234567890",
        workspace: "fastrepl",
      },
      "Zoom Meeting 1234567890",
    ],
    [
      "Calendly event URLs",
      "https://calendly.com/john/product-demo",
      {
        provider: "calendly",
        kind: "event",
        workspace: "john",
        resourceId: "product-demo",
        resourceTitle: "product demo",
      },
      "Calendly Event: product demo",
    ],
  ])("renders %s", (_, url, expectedAttrs, expectedLabel) => {
    const attrs = { url, ...expectedAttrs };
    expect(getAppLinkLabel(attrs)).toBe(expectedLabel);
  });
});
