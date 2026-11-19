import { beforeEach, describe, expect, test, vi } from "vitest";

import type { MeetingNoticeEvent } from "@anlg/plugin-detect";

import {
  MEETING_DISCLOSURE_MESSAGE,
  stopWatchingMeetingNotice,
  watchMeetingNotice,
} from "./meeting-disclosure";

const { listeners, unlistenMock, toastWarningMock } = vi.hoisted(() => ({
  listeners: [] as Array<(event: { payload: MeetingNoticeEvent }) => void>,
  unlistenMock: vi.fn(),
  toastWarningMock: vi.fn(),
}));

vi.mock("@anlg/plugin-detect", () => ({
  events: {
    meetingNoticeEvent: {
      listen: vi.fn(
        async (handler: (event: { payload: MeetingNoticeEvent }) => void) => {
          listeners.push(handler);
          return unlistenMock;
        },
      ),
    },
  },
}));

vi.mock("@anlg/ui/components/ui/toast", () => ({
  toast: { warning: toastWarningMock },
}));

function deliver(event: Partial<MeetingNoticeEvent>) {
  for (const listener of [...listeners]) {
    listener({
      payload: {
        sessionId: "session-1",
        captureId: "capture-1",
        outcome: "sent",
        reason: "",
        ...event,
      },
    });
  }
}

const settle = () => new Promise((resolve) => setTimeout(resolve, 0));

describe("the meeting chat notice outcome", () => {
  beforeEach(() => {
    listeners.length = 0;
    unlistenMock.mockReset();
    toastWarningMock.mockReset();
  });

  test("warns when this recording's notice was not sent", async () => {
    watchMeetingNotice("session-1", () => true);
    await settle();
    deliver({ sessionId: "session-2", outcome: "notSent" });
    expect(toastWarningMock).not.toHaveBeenCalled();
    deliver({ outcome: "notSent", reason: "permission_required" });
    expect(toastWarningMock).toHaveBeenCalledOnce();
    expect(toastWarningMock.mock.calls[0]?.[1]).toMatchObject({
      id: "meeting-disclosure-send-failed",
      description: "Allow Accessibility for Anarlog to post it.",
    });
    expect(unlistenMock).toHaveBeenCalledOnce();
  });

  test("says a notice that may have gone out won't be posted again", async () => {
    watchMeetingNotice("session-1", () => true);
    await settle();
    deliver({ outcome: "unconfirmed" });
    expect(toastWarningMock).toHaveBeenCalledWith(
      "Anarlog may have posted the meeting chat disclosure.",
      expect.objectContaining({
        description: "It wasn't confirmed, so it won't be posted again.",
      }),
    );
  });

  test("stays quiet for a sent or cancelled notice and after the recording stops", async () => {
    watchMeetingNotice("session-1", () => true);
    await settle();
    deliver({ outcome: "sent" });
    watchMeetingNotice("session-1", () => false);
    await settle();
    deliver({ outcome: "notSent" });
    watchMeetingNotice("session-1", () => true);
    await settle();
    stopWatchingMeetingNotice("session-1");
    deliver({ outcome: "notSent" });
    expect(toastWarningMock).not.toHaveBeenCalled();
  });

  test("posts the same disclosure that chat capture ignores", async () => {
    const native = await vi.importActual<typeof import("@anlg/plugin-detect")>(
      "@anlg/plugin-detect",
    );
    expect(MEETING_DISCLOSURE_MESSAGE).toBe(native.MEETING_DISCLOSURE_MESSAGE);
  });
});
