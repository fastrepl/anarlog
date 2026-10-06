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

  test("detects X Calls links", () => {
    expect(detectMeetingType("https://call.x.com/1A2b3C4d5E")).toBe("x-call");
    expect(detectMeetingType("https://x.com/i/call/1A2b3C4d5E")).toBe("x-call");
  });

  test("keeps non-call X links out of join controls", () => {
    expect(detectMeetingType("https://call.x.com/")).toBeNull();
    expect(detectMeetingType("https://call.x.com/new")).toBeNull();
    expect(detectMeetingType("https://x.com/anarlog")).toBeNull();
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
