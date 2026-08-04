import { describe, expect, test } from "vitest";

import {
  createTrayRecordingTitlePublisher,
  getTrayRecordingSessionId,
  getTrayRecordingTitle,
} from "./tray-recording";

import { resolveLiveSessionTitle } from "~/store/zustand/live-title";

describe("createTrayRecordingTitlePublisher", () => {
  test("publishes title changes in order", async () => {
    let resolveFirst: (() => void) | undefined;
    const published: Array<string | null> = [];
    const publish = createTrayRecordingTitlePublisher(
      (title) =>
        new Promise<void>((resolve) => {
          published.push(title);
          if (title === "First") {
            resolveFirst = resolve;
          } else {
            resolve();
          }
        }),
    );

    const first = publish("First");
    const second = publish("Second");

    await Promise.resolve();
    expect(published).toEqual(["First"]);

    resolveFirst?.();
    await Promise.all([first, second]);

    expect(published).toEqual(["First", "Second"]);
  });
});

describe("getTrayRecordingSessionId", () => {
  test.each(["active", "finalizing"] as const)(
    "keeps the recording session while %s",
    (status) => {
      expect(getTrayRecordingSessionId(status, "session-1")).toBe("session-1");
    },
  );

  test("clears the recording session when inactive", () => {
    expect(getTrayRecordingSessionId("inactive", "session-1")).toBe("");
  });
});

describe("getTrayRecordingTitle", () => {
  test("returns a real session title", () => {
    expect(getTrayRecordingTitle("  Customer call  ")).toBe("Customer call");
  });

  test.each([undefined, null, "", "  ", "Untitled", "Untitled event"])(
    "hides an ad-hoc placeholder title: %s",
    (title) => {
      expect(getTrayRecordingTitle(title)).toBeNull();
    },
  );
});

describe("resolveLiveSessionTitle", () => {
  test("keeps a persisted optimistic title until the live query catches up", () => {
    expect(
      resolveLiveSessionTitle(
        {
          value: "Customer call",
          persisted: true,
          persistedAt: Date.now(),
          previousTitle: "Untitled",
        },
        "Untitled",
      ),
    ).toBe("Customer call");
  });

  test("uses a newer store title after the optimistic write is acknowledged", () => {
    expect(
      resolveLiveSessionTitle(
        {
          value: "Customer call",
          persisted: true,
          persistedAt: Date.now(),
          previousTitle: "Untitled",
        },
        "Customer follow-up",
      ),
    ).toBe("Customer follow-up");
  });

  test("drops an unacknowledged optimistic title after the live-query window", () => {
    expect(
      resolveLiveSessionTitle(
        {
          value: "Customer call",
          persisted: true,
          persistedAt: 0,
          previousTitle: "Untitled",
        },
        "Untitled",
      ),
    ).toBe("Untitled");
  });
});
