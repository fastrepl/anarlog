import { describe, expect, it } from "vitest";

import { getMeetingPlatformNameForMicApp } from "./meeting-apps";

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
});
