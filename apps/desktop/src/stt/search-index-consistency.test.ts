import { beforeEach, describe, expect, it, vi } from "vitest";

const executeMock = vi.hoisted(() => vi.fn());

vi.mock("~/db", () => ({
  liveQueryClient: { execute: executeMock },
}));

import { waitForSessionSearchIndex } from "./search-index-consistency";

describe("waitForSessionSearchIndex", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("waits for the generation observed after transcript finalization", async () => {
    executeMock
      .mockResolvedValueOnce([{ generation: 4, acknowledged_generation: 3 }])
      .mockResolvedValueOnce([{ generation: 5, acknowledged_generation: 4 }]);

    await expect(
      waitForSessionSearchIndex("session-1", { pollIntervalMs: 0 }),
    ).resolves.toBeUndefined();
    expect(executeMock).toHaveBeenCalledTimes(2);
  });

  it("retries transient reads without advancing the acknowledgement", async () => {
    executeMock
      .mockRejectedValueOnce(new Error("database busy"))
      .mockResolvedValueOnce([{ generation: 2, acknowledged_generation: 2 }]);

    await expect(
      waitForSessionSearchIndex("session-1", { pollIntervalMs: 0 }),
    ).resolves.toBeUndefined();
  });

  it("returns when the session has no queued search generation", async () => {
    executeMock.mockResolvedValueOnce([]);

    await expect(
      waitForSessionSearchIndex("session-1", { pollIntervalMs: 0 }),
    ).resolves.toBeUndefined();
  });
});
