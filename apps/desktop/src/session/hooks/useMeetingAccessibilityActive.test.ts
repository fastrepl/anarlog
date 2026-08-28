import { describe, expect, it } from "vitest";

import { inspectionShowsActiveMeeting } from "./useMeetingAccessibilityActive";

const activeInspection = {
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

  it("fails closed for incomplete, ambiguous, or unscoped captures", () => {
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
});
