import { describe, expect, test } from "vitest";

import {
  AUTO_STOP_ENDED_NOTIFICATION_KEY_PREFIX,
  createAutoStopEndedNotificationKey,
  parseAutoStopEndedNotificationKey,
} from "./auto-stop-notification";

describe("auto-stop notification keys", () => {
  test("creates a stable key for duplicate prompts in the same session", () => {
    const firstKey = createAutoStopEndedNotificationKey("session-1");
    const secondKey = createAutoStopEndedNotificationKey("session-1");

    expect(firstKey).toBe("auto-stop-ended:session-1");
    expect(secondKey).toBe(firstKey);
    expect(parseAutoStopEndedNotificationKey(firstKey)).toBe("session-1");
  });

  test("parses legacy unique keys", () => {
    expect(
      parseAutoStopEndedNotificationKey(
        `${AUTO_STOP_ENDED_NOTIFICATION_KEY_PREFIX}session-1:prompt:nonce`,
      ),
    ).toBe("session-1");
  });
});
