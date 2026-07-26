import { beforeEach, describe, expect, it, vi } from "vitest";

const { executeMock, executeProxyMock, executeTransactionMock, subscribeMock } =
  vi.hoisted(() => ({
    executeMock: vi.fn(),
    executeProxyMock: vi.fn(),
    executeTransactionMock: vi.fn(),
    subscribeMock: vi.fn(),
  }));

vi.mock("@hypr/plugin-db", () => ({
  execute: executeMock,
  executeProxy: executeProxyMock,
  executeTransaction: executeTransactionMock,
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
    const unsubscribe = vi.fn().mockResolvedValue(undefined);
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

    await nextUnsubscribe();
    expect(unsubscribe).toHaveBeenCalledTimes(1);
  });

  it("delegates executeProxy to the db plugin", async () => {
    const { tauriLiveQueryClient } = await import("./index");
    executeProxyMock.mockResolvedValue({ rows: [[1]] });

    await expect(
      tauriLiveQueryClient.executeProxy("SELECT 1", [], "all"),
    ).resolves.toEqual({ rows: [[1]] });

    expect(executeProxyMock).toHaveBeenCalledWith("SELECT 1", [], "all");
  });

  it("delegates transactions to the db plugin", async () => {
    const { tauriTransactionClient } = await import("./index");
    const statements = [
      {
        sql: "UPDATE test SET value = ?",
        params: [1],
        expectedRowsAffected: 1,
      },
    ];
    executeTransactionMock.mockResolvedValue([1]);

    await expect(
      tauriTransactionClient.executeTransaction(statements),
    ).resolves.toEqual([1]);
    expect(executeTransactionMock).toHaveBeenCalledWith(statements);
  });
});
