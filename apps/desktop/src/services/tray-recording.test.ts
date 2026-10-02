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
  test.each([
    ["active", "session-1"],
    ["finalizing", "session-1"],
    ["inactive", ""],
  ] as const)("while %s returns %j", (status, expected) => {
    expect(getTrayRecordingSessionId(status, "session-1")).toBe(expected);
  });
});

describe("getTrayRecordingTitle", () => {
  test.each([
    ["  Customer call  ", "Customer call"],
    [undefined, null],
    ["  ", null],
    ["Untitled", null],
    ["Untitled event", null],
  ])("maps %j to %j", (title, expected) => {
    expect(getTrayRecordingTitle(title)).toBe(expected);
  });
});

describe("resolveLiveSessionTitle", () => {
  test.each([
    [
      "keeps a persisted optimistic title until the live query catches up",
      Date.now(),
      "Untitled",
      "Customer call",
    ],
    [
      "uses a newer store title after the optimistic write is acknowledged",
      Date.now(),
      "Customer follow-up",
      "Customer follow-up",
    ],
    [
      "drops an unacknowledged optimistic title after the live-query window",
      0,
      "Untitled",
      "Untitled",
    ],
  ])("%s", (_name, persistedAt, storeTitle, expected) => {
    expect(
      resolveLiveSessionTitle(
        {
          value: "Customer call",
          persisted: true,
          persistedAt,
          previousTitle: "Untitled",
        },
        storeTitle,
      ),
    ).toBe(expected);
  });
});
