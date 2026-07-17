import { describe, expect, it } from "vitest";

import { getAppLinkDisplayParts, getAppLinkLabel, parseAppLinkUrl } from ".";

describe("app link parsing", () => {
  it("parses Linear document URLs", () => {
    const attrs = parseAppLinkUrl(
      "https://linear.app/fastrepl-inc/document/real-world-use-cases-8dcac4144e38",
    );

    expect(attrs).toMatchObject({
      provider: "linear",
      kind: "document",
      workspace: "fastrepl-inc",
      resourceId: "real-world-use-cases-8dcac4144e38",
      resourceTitle: "Real world use cases",
    });
    expect(getAppLinkDisplayParts(attrs!).subline).toBe(
      "Document: Real world use cases",
    );
  });

  it("parses Linear issue URLs", () => {
    const attrs = parseAppLinkUrl(
      "https://linear.app/fastrepl-inc/issue/ANLG-53/storage-model",
    );

    expect(attrs).toMatchObject({
      provider: "linear",
      kind: "issue",
      workspace: "fastrepl-inc",
      resourceId: "ANLG-53",
      resourceTitle: "Storage model",
    });
    expect(getAppLinkLabel(attrs!)).toBe("fastrepl-inc Issue ANLG-53");
  });

  it("parses unknown Linear routes without labeling them as workspaces", () => {
    const attrs = parseAppLinkUrl(
      "https://linear.app/fastrepl-inc/inbox/assigned-to-me",
    );

    expect(attrs).toMatchObject({
      provider: "linear",
      kind: "route",
      workspace: "fastrepl-inc",
      resourceId: "inbox/assigned-to-me",
      resourceTitle: "Inbox / Assigned to me",
    });
    expect(getAppLinkDisplayParts(attrs!)).toEqual({
      header: "fastrepl-inc",
      subline: "Route: Inbox / Assigned to me",
    });
  });

  it("parses Notion page URLs", () => {
    const attrs = parseAppLinkUrl(
      "https://www.notion.so/Product-Plan-0123456789abcdef0123456789abcdef",
    );

    expect(attrs).toMatchObject({
      provider: "notion",
      kind: "page",
      resourceId: "0123456789abcdef0123456789abcdef",
      resourceTitle: "Product Plan",
    });
    expect(getAppLinkDisplayParts(attrs!)).toEqual({
      header: "Notion",
      subline: "Page: Product Plan",
    });
  });

  it("parses Google workspace URLs", () => {
    const attrs = parseAppLinkUrl(
      "https://docs.google.com/spreadsheets/d/1a2b3c4d5e6f/edit",
    );

    expect(attrs).toMatchObject({
      provider: "google",
      kind: "spreadsheet",
      resourceId: "1a2b3c4d5e6f",
    });
    expect(getAppLinkDisplayParts(attrs!)).toEqual({
      header: "Google Sheets",
      subline: "Spreadsheet",
    });
  });

  it("parses Google Forms public URLs", () => {
    const attrs = parseAppLinkUrl(
      "https://docs.google.com/forms/d/e/1FAIpQLSc12345/viewform",
    );

    expect(attrs).toMatchObject({
      provider: "google",
      kind: "form",
      resourceId: "1FAIpQLSc12345",
    });
  });

  it("parses Figma design URLs", () => {
    const attrs = parseAppLinkUrl(
      "https://www.figma.com/design/abc123/Product-Roadmap?node-id=1-2",
    );

    expect(attrs).toMatchObject({
      provider: "figma",
      kind: "design",
      resourceId: "abc123",
      resourceTitle: "Product Roadmap",
    });
    expect(getAppLinkDisplayParts(attrs!)).toEqual({
      header: "Figma",
      subline: "Design file: Product Roadmap",
    });
  });

  it("parses Jira issue URLs", () => {
    const attrs = parseAppLinkUrl(
      "https://fastrepl.atlassian.net/browse/ANLG-5540",
    );

    expect(attrs).toMatchObject({
      provider: "atlassian",
      kind: "jira_issue",
      workspace: "fastrepl",
      resourceId: "ANLG-5540",
    });
    expect(getAppLinkLabel(attrs!)).toBe("fastrepl Jira ANLG-5540");
  });

  it("parses Confluence page URLs", () => {
    const attrs = parseAppLinkUrl(
      "https://fastrepl.atlassian.net/wiki/spaces/ENG/pages/123456/Product+Plan",
    );

    expect(attrs).toMatchObject({
      provider: "atlassian",
      kind: "confluence_page",
      workspace: "fastrepl",
      resourceId: "123456",
      resourceTitle: "Product Plan",
    });
    expect(getAppLinkDisplayParts(attrs!)).toEqual({
      header: "fastrepl",
      subline: "Confluence: Product Plan",
    });
  });

  it.each([
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
  ])("parses %s", (_, url, expectedAttrs, expectedLabel) => {
    const attrs = parseAppLinkUrl(url);

    expect(attrs).toMatchObject(expectedAttrs);
    expect(getAppLinkLabel(attrs!)).toBe(expectedLabel);
  });
});
