import { afterEach, describe, expect, test } from "vitest";

import {
  beginScheduledAutoStart,
  finishScheduledAutoStart,
  hasScheduledAutoStartInFlight,
  isScheduledAutoStartInFlight,
} from "./scheduled-auto-start-state";

const SESSION_IDS = ["session-1", "session-2"];

afterEach(() => {
  for (const sessionId of SESSION_IDS) {
    finishScheduledAutoStart(sessionId);
  }
});

describe("scheduled auto-start state", () => {
  test("stays in flight until every start finishes", () => {
    beginScheduledAutoStart(SESSION_IDS[0]);
    beginScheduledAutoStart(SESSION_IDS[1]);

    finishScheduledAutoStart(SESSION_IDS[0]);

    expect(hasScheduledAutoStartInFlight()).toBe(true);

    finishScheduledAutoStart(SESSION_IDS[1]);

    expect(hasScheduledAutoStartInFlight()).toBe(false);
  });

  test("tracks in-flight starts per session", () => {
    beginScheduledAutoStart(SESSION_IDS[0]);

    expect(isScheduledAutoStartInFlight(SESSION_IDS[0])).toBe(true);
    expect(isScheduledAutoStartInFlight(SESSION_IDS[1])).toBe(false);

    finishScheduledAutoStart(SESSION_IDS[0]);

    expect(isScheduledAutoStartInFlight(SESSION_IDS[0])).toBe(false);
  });
});
