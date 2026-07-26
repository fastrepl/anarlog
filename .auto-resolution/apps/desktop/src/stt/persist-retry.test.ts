import { describe, expect, test, vi } from "vitest";

import { isDatabaseLockError, persistTranscriptWrite } from "./persist-retry";

describe("persistTranscriptWrite", () => {
  test("recognizes SQLite busy and locked errors", () => {
    expect(isDatabaseLockError(new Error("database is locked"))).toBe(true);
    expect(
      isDatabaseLockError(
        "error returned from database: (code: 5) database is locked",
      ),
    ).toBe(true);
    expect(isDatabaseLockError("database table is locked")).toBe(true);
    expect(isDatabaseLockError(new Error("constraint failed"))).toBe(false);
  });

  test("retries transient database locks", async () => {
    const write = vi
      .fn<() => Promise<void>>()
      .mockRejectedValueOnce(new Error("database is locked"))
      .mockResolvedValue(undefined);

    await persistTranscriptWrite(write, [0]);

    expect(write).toHaveBeenCalledTimes(2);
  });

  test("does not retry non-locking failures", async () => {
    const write = vi
      .fn<() => Promise<void>>()
      .mockRejectedValue(new Error("constraint failed"));

    await expect(persistTranscriptWrite(write, [0])).rejects.toThrow(
      "constraint failed",
    );
    expect(write).toHaveBeenCalledTimes(1);
  });
});
