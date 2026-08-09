import { describe, expect, it } from "vitest";

import {
  detectMeetingImportProviders,
  MEETING_IMPORT_PROVIDERS,
} from "./providers";

describe("meeting import providers", () => {
  it("keeps every researched provider in the catalog", () => {
    expect(MEETING_IMPORT_PROVIDERS).toHaveLength(30);
    expect(
      new Set(MEETING_IMPORT_PROVIDERS.map((provider) => provider.id)).size,
    ).toBe(MEETING_IMPORT_PROVIDERS.length);
  });

  it("enables direct OAuth imports for every provider with a public MCP server", () => {
    expect(
      MEETING_IMPORT_PROVIDERS.filter((provider) => provider.directImport).map(
        (provider) => provider.id,
      ),
    ).toEqual([
      "granola",
      "circleback",
      "fireflies",
      "krisp",
      "read-ai",
      "fellow",
      "tactiq",
      "jiminny",
    ]);
  });

  it("detects exact native names and bundle identifiers", () => {
    const providers = detectMeetingImportProviders([
      { id: "com.granola.app", name: "Granola" },
      { id: "com.microsoft.teams2", name: "Microsoft Teams" },
    ]);

    expect(providers.map((provider) => provider.id)).toEqual([
      "granola",
      "microsoft-teams",
    ]);
    expect(providers.map((provider) => provider.installedAppId)).toEqual([
      "com.granola.app",
      "com.microsoft.teams2",
    ]);
  });

  it("does not accept bundle identifier prefixes", () => {
    expect(
      detectMeetingImportProviders([
        { id: "com.granola.app.helper", name: "Something Else" },
      ]),
    ).toEqual([]);
  });

  it("does not infer extension-only products from a browser", () => {
    expect(
      detectMeetingImportProviders([
        { id: "com.google.Chrome", name: "Google Chrome" },
      ]),
    ).toEqual([]);
  });
});
