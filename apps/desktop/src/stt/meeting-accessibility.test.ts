import { describe, expect, it } from "vitest";

import {
  inspectionsShowActiveMeetingForApps,
  inspectionShowsActiveMeeting,
} from "./meeting-accessibility";

const activeInspection = {
  activeCall: true,
  app: { id: "com.google.Chrome", name: "Google Chrome" },
  pid: 123,
  platform: "googleMeet" as const,
  surface: "web" as const,
  accessibilityTrusted: true,
  windowTitle: "Meet - abc-defg-hij",
  warnings: [],
};

describe("meeting accessibility activity", () => {
  it("accepts one trusted, validated active meeting", () => {
    expect(inspectionShowsActiveMeeting(activeInspection)).toBe(true);
  });

  it("accepts active native calls validated by platform fallbacks", () => {
    for (const [platform, app] of [
      ["discord", { id: "com.discordapp.Discord", name: "Discord" }],
      [
        "microsoftTeams",
        { id: "com.microsoft.teams2", name: "Microsoft Teams" },
      ],
    ] as const) {
      expect(
        inspectionShowsActiveMeeting({
          ...activeInspection,
          app,
          platform,
          surface: "native",
        }),
      ).toBe(true);
    }
  });

  it("fails closed for incomplete, ambiguous, or unscoped captures", () => {
    expect(
      inspectionShowsActiveMeeting({
        ...activeInspection,
        activeCall: false,
      }),
    ).toBe(false);
    expect(
      inspectionShowsActiveMeeting({
        ...activeInspection,
        warnings: ["AX snapshot was incomplete"],
      }),
    ).toBe(false);
    expect(
      inspectionShowsActiveMeeting({
        ...activeInspection,
        warnings: ["meeting window scope was ambiguous"],
      }),
    ).toBe(false);
    expect(
      inspectionShowsActiveMeeting({
        ...activeInspection,
        windowTitle: null,
      }),
    ).toBe(false);
  });

  it("only accepts active meetings from the expected trigger app", () => {
    expect(
      inspectionsShowActiveMeetingForApps(
        [activeInspection],
        ["com.google.Chrome"],
      ),
    ).toBe(true);
    expect(
      inspectionsShowActiveMeetingForApps([activeInspection], ["us.zoom.xos"]),
    ).toBe(false);
  });
});
