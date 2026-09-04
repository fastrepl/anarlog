import { beforeEach, describe, expect, test, vi } from "vitest";

import {
  AUTO_STOP_NETWORK_HOLD_MS,
  AUTO_STOP_RECENT_OFFLINE_MS,
  isRecentNetworkDrop,
  resolveNetworkHoldUntilMs,
  showMeetingEndedPrompt,
} from "./auto-stop";
import { cancelAutoStopEndedNotification } from "./auto-stop-notification";

const { getNotificationIconForAppMock, showNotificationMock } = vi.hoisted(
  () => ({
    getNotificationIconForAppMock: vi.fn(),
    showNotificationMock: vi.fn(),
  }),
);

vi.mock("@anlg/plugin-notification", () => ({
  commands: {
    showNotification: showNotificationMock,
  },
}));

vi.mock("./meeting-apps", async (importOriginal) => ({
  ...(await importOriginal()),
  getNotificationIconForApp: getNotificationIconForAppMock,
}));

beforeEach(() => {
  cancelAutoStopEndedNotification("session-1");
  getNotificationIconForAppMock.mockReset();
  showNotificationMock.mockReset();
});

describe("resolveNetworkHoldUntilMs", () => {
  test("keeps a future calendar deadline", () => {
    expect(
      resolveNetworkHoldUntilMs({
        calendarDeadlineMs: 2_000,
        nowMs: 1_000,
      }),
    ).toBe(2_000);
  });

  test("falls back to the default hold when no calendar deadline exists", () => {
    expect(
      resolveNetworkHoldUntilMs({
        calendarDeadlineMs: null,
        nowMs: 1_000,
      }),
    ).toBe(1_000 + AUTO_STOP_NETWORK_HOLD_MS);
  });

  test("falls back to the default hold when the calendar deadline has passed", () => {
    expect(
      resolveNetworkHoldUntilMs({
        calendarDeadlineMs: 500,
        nowMs: 1_000,
      }),
    ).toBe(1_000 + AUTO_STOP_NETWORK_HOLD_MS);
  });
});

describe("isRecentNetworkDrop", () => {
  test("is true inside the recent-reconnect window", () => {
    expect(
      isRecentNetworkDrop(1_000, 1_000 + AUTO_STOP_RECENT_OFFLINE_MS),
    ).toBe(true);
  });

  test("is false after the recent-reconnect window", () => {
    expect(
      isRecentNetworkDrop(1_000, 1_000 + AUTO_STOP_RECENT_OFFLINE_MS + 1),
    ).toBe(false);
  });

  test("is false when no reconnect was recorded", () => {
    expect(isRecentNetworkDrop(null, 1_000)).toBe(false);
  });
});

describe("showMeetingEndedPrompt", () => {
  test("does not show a prompt when recording notifications are disabled", async () => {
    await showMeetingEndedPrompt({
      sessionId: "session-1",
      stoppedTriggerAppIds: ["com.google.Chrome"],
      stoppedApps: [{ id: "com.google.Chrome", name: "Google Chrome" }],
      notificationEnabled: false,
    });

    expect(getNotificationIconForAppMock).not.toHaveBeenCalled();
    expect(showNotificationMock).not.toHaveBeenCalled();
  });

  test("does not show a prompt cancelled while its icon loads", async () => {
    let resolveIcon: (value: null) => void = () => {};
    getNotificationIconForAppMock.mockReturnValue(
      new Promise<null>((resolve) => {
        resolveIcon = resolve;
      }),
    );

    const prompt = showMeetingEndedPrompt({
      sessionId: "session-1",
      stoppedTriggerAppIds: ["com.google.Chrome"],
      stoppedApps: [{ id: "com.google.Chrome", name: "Google Chrome" }],
    });

    expect(cancelAutoStopEndedNotification("session-1")).toBe(true);
    resolveIcon(null);
    await prompt;

    expect(showNotificationMock).not.toHaveBeenCalled();
  });
});
