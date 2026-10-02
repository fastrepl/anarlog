import { beforeEach, describe, expect, it, vi } from "vitest";

const { executeMock, executeProxyMock, executeTransactionMock, subscribeMock } =
  vi.hoisted(() => ({
    executeMock: vi.fn(),
    executeProxyMock: vi.fn(),
    executeTransactionMock: vi.fn(),
    subscribeMock: vi.fn(),
  }));

vi.mock("@anlg/plugin-db", () => ({
  execute: executeMock,
  executeProxy: executeProxyMock,
  executeTransaction: executeTransactionMock,
  subscribe: subscribeMock,
}));

describe("@anlg/db-tauri", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("exposes the plugin commands through its clients", async () => {
    const { tauriLiveQueryClient, tauriTransactionClient } =
      await import("./index");

    expect(tauriLiveQueryClient).toEqual({
      execute: executeMock,
      executeProxy: executeProxyMock,
      subscribe: subscribeMock,
    });
    expect(tauriTransactionClient).toEqual({
      executeTransaction: executeTransactionMock,
    });
  });
});
