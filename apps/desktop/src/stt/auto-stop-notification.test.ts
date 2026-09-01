import { describe, expect, test } from "vitest";

import {
  AUTO_STOP_ENDED_NOTIFICATION_KEY_PREFIX,
  cancelAutoStopEndedNotification,
  consumeAutoStopEndedNotificationKey,
  createAutoStopEndedNotificationKey,
  parseAutoStopEndedNotificationKey,
} from "./auto-stop-notification";

describe("auto-stop notification keys", () => {
  test("creates a stable key for duplicate prompts in the same session", () => {
    const firstKey = createAutoStopEndedNotificationKey("session-1");
    const secondKey = createAutoStopEndedNotificationKey("session-1");

    expect(firstKey).toMatch(/^auto-stop-ended:session-1:prompt:[0-9a-f-]+$/);
    expect(secondKey).toBe(firstKey);
    expect(parseAutoStopEndedNotificationKey(firstKey)).toBe("session-1");
    cancelAutoStopEndedNotification("session-1");
  });

  test("invalidates a pending prompt when the meeting resumes", () => {
    const key = createAutoStopEndedNotificationKey("session-1");

    expect(cancelAutoStopEndedNotification("session-1")).toBe(true);
    expect(consumeAutoStopEndedNotificationKey(key)).toBeNull();
    expect(createAutoStopEndedNotificationKey("session-1")).not.toBe(key);
    cancelAutoStopEndedNotification("session-1");
  });

  test("parses legacy unique keys", () => {
    expect(
      parseAutoStopEndedNotificationKey(
        `${AUTO_STOP_ENDED_NOTIFICATION_KEY_PREFIX}session-1:prompt:nonce`,
      ),
    ).toBe("session-1");
  });
});
