import { describe, expect, it } from "vitest";

import {
  getBrowserMeetingPlatform,
  getMeetingPlatformNameForMicApp,
} from "./meeting-apps";

describe("meeting app platform names", () => {
  it("only promotes explicitly classified meeting apps", () => {
    expect(getMeetingPlatformNameForMicApp({ id: "zoom", name: "Zoom" })).toBe(
      "Zoom",
    );
    expect(
      getMeetingPlatformNameForMicApp({
        id: "com.apple.FaceTime",
        name: "FaceTime",
      }),
    ).toBeNull();
  });

  it("detects X Calls for a browser mic app with an X call link", () => {
    const chrome = { id: "com.google.Chrome", name: "Google Chrome" };

    expect(
      getBrowserMeetingPlatform([chrome], {
        id: "event-1",
        title: "Call",
        meetingLink: "https://call.x.com/1A2b3C4d5E",
        participantNames: [],
      })?.displayName,
    ).toBe("X Calls");

    expect(
      getBrowserMeetingPlatform([chrome], {
        id: "event-2",
        title: "Call",
        location: "https://x.com/anarlog",
        participantNames: [],
      }),
    ).toBeNull();
  });
});
