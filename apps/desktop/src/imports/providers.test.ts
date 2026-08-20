import { describe, expect, it } from "vitest";

import {
  detectMeetingImportProviders,
  MEETING_IMPORT_PROVIDERS,
} from "./providers";

describe("meeting import providers", () => {
  it("keeps every researched provider in the catalog", () => {
    expect(MEETING_IMPORT_PROVIDERS).toHaveLength(31);
    expect(
      new Set(MEETING_IMPORT_PROVIDERS.map((provider) => provider.id)).size,
    ).toBe(MEETING_IMPORT_PROVIDERS.length);
  });

  it("enables direct OAuth imports for MCP providers and Nango meeting sources", () => {
    expect(
      MEETING_IMPORT_PROVIDERS.filter((provider) => provider.directImport).map(
        (provider) => provider.id,
      ),
    ).toEqual([
      "granola",
      "circleback",
      "fireflies",
      "krisp",
      "fathom",
      "read-ai",
      "notion",
      "fellow",
      "tactiq",
      "jiminny",
      "plaud",
      "pocket",
      "zoom",
      "microsoft-teams",
      "google-meet",
      "webex",
    ]);
    expect(
      MEETING_IMPORT_PROVIDERS.find((provider) => provider.id === "plaud"),
    ).toMatchObject({
      directImport: "cli",
    });
    expect(
      MEETING_IMPORT_PROVIDERS.find((provider) => provider.id === "pocket"),
    ).toMatchObject({
      directImport: "mcp-oauth",
      helpUrl: "https://docs.heypocketai.com/docs",
    });
    expect(
      MEETING_IMPORT_PROVIDERS.find((provider) => provider.id === "zoom"),
    ).toMatchObject({
      directImport: "nango-oauth",
      nangoIntegrationId: "zoom",
    });
    expect(
      MEETING_IMPORT_PROVIDERS.filter(
        (provider) => provider.directImport === "nango-oauth",
      ).map((provider) => provider.nangoIntegrationId),
    ).toEqual([
      "fathom",
      "notion",
      "zoom",
      "microsoft-teams",
      "google-meet",
      "webex",
    ]);
  });

  it("detects exact native names and bundle identifiers", () => {
    const providers = detectMeetingImportProviders([
      { id: "com.granola.app", name: "Granola" },
      { id: "ai.plaud.desktop.plaud", name: "Plaud Desktop" },
      { id: "com.microsoft.teams2", name: "Microsoft Teams" },
      { id: "com.openvisionengineering.pocket-desktop-app", name: "Pocket" },
    ]);

    expect(providers.map((provider) => provider.id)).toEqual([
      "granola",
      "plaud",
      "pocket",
      "microsoft-teams",
      "google-meet",
    ]);
    expect(providers.map((provider) => provider.installedAppId)).toEqual([
      "com.granola.app",
      "ai.plaud.desktop.plaud",
      "com.openvisionengineering.pocket-desktop-app",
      "com.microsoft.teams2",
      "google-meet",
    ]);
  });

  it("detects Plaud and Pocket desktop apps from Windows display names", () => {
    const providers = detectMeetingImportProviders([
      { id: "windows:hklm:Plaud Desktop", name: "Plaud Desktop" },
      { id: "windows:hkcu:Pocket", name: "Pocket Desktop" },
    ]);

    expect(providers.map((provider) => provider.id)).toEqual([
      "plaud",
      "pocket",
      "google-meet",
    ]);
  });

  it("does not treat Pocket Casts as Pocket", () => {
    expect(
      detectMeetingImportProviders([
        { id: "com.electron.pocket-casts", name: "Pocket Casts" },
      ]).map((provider) => provider.id),
    ).toEqual(["google-meet"]);
  });

  it("does not accept bundle identifier prefixes", () => {
    expect(
      detectMeetingImportProviders([
        { id: "com.granola.app.helper", name: "Something Else" },
      ]).map((provider) => provider.id),
    ).toEqual(["google-meet"]);
  });

  it("does not infer extension-only products from a browser", () => {
    expect(
      detectMeetingImportProviders([
        { id: "com.google.Chrome", name: "Google Chrome" },
      ]).map((provider) => provider.id),
    ).toEqual(["google-meet"]);
  });
});
