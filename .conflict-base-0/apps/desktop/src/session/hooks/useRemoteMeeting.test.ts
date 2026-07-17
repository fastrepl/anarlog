import { describe, expect, test } from "vitest";

import { detectMeetingType, getRemoteMeeting } from "./useRemoteMeeting";

describe("remote meeting detection", () => {
  test("detects Cal.com video links", () => {
    expect(
      detectMeetingType("https://app.cal.com/video/d713v9w1d2krBptPtwUAnJ"),
    ).toBe("cal-com");
  });

  test("keeps regular Cal.com booking links out of join controls", () => {
    expect(detectMeetingType("https://cal.com/john/intro")).toBeNull();
    expect(detectMeetingType("https://app.cal.com/john/intro")).toBeNull();
  });

  test("returns the remote meeting payload for recognized links", () => {
    expect(
      getRemoteMeeting("https://app.cal.com/video/d713v9w1d2krBptPtwUAnJ"),
    ).toEqual({
      type: "cal-com",
      url: "https://app.cal.com/video/d713v9w1d2krBptPtwUAnJ",
    });
  });
});
