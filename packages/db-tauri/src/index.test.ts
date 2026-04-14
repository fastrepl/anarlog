import { beforeEach, describe, expect, it, vi } from "vitest";

const { executeMock, subscribeMock } = vi.hoisted(() => ({
  executeMock: vi.fn(),
  subscribeMock: vi.fn(),
}));

vi.mock("@hypr/plugin-db", () => ({
  execute: executeMock,
  subscribe: subscribeMock,
}));

describe("@hypr/db-tauri", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("delegates execute to the db plugin", async () => {
    const { tauriLiveQueryClient } = await import("./index");
    executeMock.mockResolvedValue([{ id: 1 }]);

    await expect(
      tauriLiveQueryClient.execute("SELECT id FROM test", [1]),
    ).resolves.toEqual([{ id: 1 }]);

    expect(executeMock).toHaveBeenCalledWith("SELECT id FROM test", [1]);
  });

  it("delegates subscribe to the db plugin", async () => {
    const { tauriLiveQueryClient } = await import("./index");
    const unsubscribe = vi.fn();
    subscribeMock.mockResolvedValue(unsubscribe);

    const nextUnsubscribe = await tauriLiveQueryClient.subscribe(
      "SELECT id FROM test",
      [1],
      {
        onData: vi.fn(),
      },
    );

    expect(subscribeMock).toHaveBeenCalledWith(
      "SELECT id FROM test",
      [1],
      expect.objectContaining({
        onData: expect.any(Function),
      }),
    );

    nextUnsubscribe();
    expect(unsubscribe).toHaveBeenCalledTimes(1);
  });
});
