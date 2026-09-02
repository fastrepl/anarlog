import { afterEach, describe, expect, test } from "vitest";

import {
  beginScheduledAutoStart,
  finishScheduledAutoStart,
  hasScheduledAutoStartInFlight,
  queueScheduledAutoJoin,
  takeScheduledAutoJoin,
} from "./scheduled-auto-start-state";

const SESSION_IDS = ["session-1", "session-2"];

afterEach(() => {
  for (const sessionId of SESSION_IDS) {
    finishScheduledAutoStart(sessionId);
    takeScheduledAutoJoin(sessionId);
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

  test("hands the queued meeting link to the session that starts listening", () => {
    queueScheduledAutoJoin(SESSION_IDS[0], "https://meet.example/one");
    queueScheduledAutoJoin(SESSION_IDS[1], "https://meet.example/two");

    expect(takeScheduledAutoJoin(SESSION_IDS[0])).toBe(
      "https://meet.example/one",
    );
    expect(takeScheduledAutoJoin(SESSION_IDS[0])).toBeUndefined();
    expect(takeScheduledAutoJoin(SESSION_IDS[1])).toBe(
      "https://meet.example/two",
    );
  });
});
