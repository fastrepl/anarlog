import { describe, expect, test } from "vitest";

import {
  AUTO_STOP_ENDED_NOTIFICATION_KEY_PREFIX,
  createAutoStopEndedNotificationKey,
  parseAutoStopEndedNotificationKey,
} from "./auto-stop-notification";

describe("auto-stop notification keys", () => {
  test("creates unique keys that still parse to the session id", () => {
    const firstKey = createAutoStopEndedNotificationKey("session-1");
    const secondKey = createAutoStopEndedNotificationKey("session-1");

    expect(firstKey).not.toBe(secondKey);
    expect(parseAutoStopEndedNotificationKey(firstKey)).toBe("session-1");
    expect(parseAutoStopEndedNotificationKey(secondKey)).toBe("session-1");
  });

  test("parses legacy stable keys", () => {
    expect(
      parseAutoStopEndedNotificationKey(
        `${AUTO_STOP_ENDED_NOTIFICATION_KEY_PREFIX}session-1`,
      ),
    ).toBe("session-1");
  });
});
