import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { renderHook, waitFor } from "@testing-library/react";
import { createElement, type ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  execute: vi.fn(),
  useLiveQuery: vi.fn(),
  getSecret: vi.fn(async () => ({
    status: "ok",
    data: null as string | null,
  })),
  setSecret: vi.fn(async () => ({ status: "ok", data: null })),
  deleteSecret: vi.fn(async () => ({ status: "ok", data: null })),
  repairKeychainAccess: vi.fn<
    () => Promise<
      { status: "ok"; data: null } | { status: "error"; error: string }
    >
  >(async () => ({ status: "ok", data: null })),
  executeTransaction: vi.fn(
    (_statements: Array<{ sql: string; params: unknown[] }>) =>
      Promise.resolve([1]),
  ),
}));

vi.mock("@hypr/plugin-store2", () => ({
  commands: {
    getSecret: mocks.getSecret,
    setSecret: mocks.setSecret,
    deleteSecret: mocks.deleteSecret,
    repairKeychainAccess: mocks.repairKeychainAccess,
  },
}));

vi.mock("~/db", () => ({
  executeTransaction: mocks.executeTransaction,
  liveQueryClient: { execute: mocks.execute },
  useLiveQuery: mocks.useLiveQuery,
}));

vi.mock("~/db/write-queue", () => ({
  enqueueDatabaseWrite: (_key: string, operation: () => Promise<unknown>) =>
    operation(),
}));

import {
  getStoredAiProvider,
  isKeychainAccessError,
  loadSecureAiProviderApiKeys,
  migratePlaintextAiProviderApiKeys,
  parseAiProviders,
  repairKeychainAccess,
  setAiProvider,
  useAiProvidersState,
} from "./providers";

describe("SQLite AI providers", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.executeTransaction.mockResolvedValue([1]);
    mocks.getSecret.mockResolvedValue({ status: "ok", data: null });
    mocks.setSecret.mockResolvedValue({ status: "ok", data: null });
    mocks.deleteSecret.mockResolvedValue({ status: "ok", data: null });
    mocks.repairKeychainAccess.mockResolvedValue({
      status: "ok",
      data: null,
    });
    mocks.useLiveQuery.mockReturnValue({ data: [], isLoading: false });
  });

  it("waits for secure provider keys before reporting provider state as ready", async () => {
    let resolveSecret!: (value: { status: "ok"; data: string | null }) => void;
    mocks.useLiveQuery.mockReturnValue({
      data: [
        {
          id: "ai_provider:stt:deepgram",
          value_json: JSON.stringify({
            type: "stt",
            base_url: "https://api.deepgram.com/v1",
            api_key: "",
          }),
        },
      ],
      isLoading: false,
    });
    mocks.getSecret.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          resolveSecret = resolve;
        }),
    );
    const queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    });
    const wrapper = ({ children }: { children: ReactNode }) =>
      createElement(QueryClientProvider, { client: queryClient }, children);

    const { result } = renderHook(() => useAiProvidersState("stt"), {
      wrapper,
    });

    expect(result.current.isReady).toBe(false);

    await waitFor(() => expect(mocks.getSecret).toHaveBeenCalledTimes(1));
    resolveSecret({ status: "ok", data: "deepgram-key" });

    await waitFor(() => expect(result.current.isReady).toBe(true));
    expect(result.current.providers["stt:deepgram"]?.api_key).toBe(
      "deepgram-key",
    );
  });

  it("uses direct provider rows over imported legacy configuration", () => {
    const providers = parseAiProviders(
      [
        {
          id: "legacy_settings_document",
          value_json: JSON.stringify({
            ai: {
              llm: {
                openai: {
                  base_url: "https://legacy.example",
                  api_key: "legacy-key",
                },
              },
            },
          }),
        },
        {
          id: "ai_provider:llm:openai",
          value_json: JSON.stringify({
            type: "llm",
            base_url: "https://direct.example",
            api_key: "direct-key",
          }),
        },
      ],
      "llm",
    );

    expect(providers["llm:openai"]).toEqual({
      type: "llm",
      base_url: "https://direct.example",
      api_key: "direct-key",
    });
  });

  it("promotes legacy provider fields on the first partial write", async () => {
    mocks.execute.mockResolvedValueOnce([
      {
        id: "legacy_settings_document",
        value_json: JSON.stringify({
          ai: {
            stt: {
              deepgram: {
                base_url: "https://legacy.example",
                api_key: "legacy-key",
              },
            },
          },
        }),
      },
    ]);

    await setAiProvider("stt", "deepgram", { api_key: "new-key" });

    const statement = mocks.executeTransaction.mock.calls[0][0].find(
      (candidate) => candidate.sql.includes("INSERT INTO app_settings"),
    )!;
    expect(statement.sql).toContain("INSERT INTO app_settings");
    expect(statement.params[0]).toBe("ai_provider:stt:deepgram");
    expect(JSON.parse(String(statement.params[1]))).toEqual({
      type: "stt",
      base_url: "https://legacy.example",
      api_key: "",
    });
    expect(mocks.setSecret).toHaveBeenCalledWith(
      "ai-provider-api-keys",
      "stt:deepgram",
      "new-key",
    );
  });

  it("retries partial writes without dropping a concurrent field", async () => {
    mocks.execute
      .mockResolvedValueOnce([
        {
          id: "ai_provider:llm:openai",
          value_json: JSON.stringify({
            type: "llm",
            base_url: "https://old.example",
            api_key: "old-key",
          }),
        },
      ])
      .mockResolvedValueOnce([
        {
          id: "ai_provider:llm:openai",
          value_json: JSON.stringify({
            type: "llm",
            base_url: "https://concurrent.example",
            api_key: "old-key",
          }),
        },
      ]);
    mocks.executeTransaction
      .mockResolvedValueOnce([0])
      .mockResolvedValueOnce([1]);

    await setAiProvider("llm", "openai", { api_key: "new-key" });

    const statement = mocks.executeTransaction.mock.calls[1][0][0];
    expect(JSON.parse(String(statement.params[0]))).toEqual({
      type: "llm",
      base_url: "https://concurrent.example",
      api_key: "",
    });
  });

  it("restores secure storage when the SQLite write never commits", async () => {
    mocks.execute.mockResolvedValue([
      {
        id: "ai_provider:llm:openai",
        value_json: JSON.stringify({
          type: "llm",
          base_url: "https://old.example",
          api_key: "",
        }),
      },
    ]);
    mocks.executeTransaction.mockResolvedValue([0]);

    await expect(
      setAiProvider("llm", "openai", { api_key: "new-key" }),
    ).rejects.toThrow("changed too frequently");

    expect(mocks.setSecret).toHaveBeenCalledWith(
      "ai-provider-api-keys",
      "llm:openai",
      "new-key",
    );
    expect(mocks.deleteSecret).toHaveBeenCalledWith(
      "ai-provider-api-keys",
      "llm:openai",
    );
  });

  it("loads secure API keys by provider ID", async () => {
    mocks.getSecret.mockResolvedValueOnce({
      status: "ok",
      data: "deepgram-key",
    });

    const apiKeys = await loadSecureAiProviderApiKeys(["stt:deepgram"], "stt");

    expect(apiKeys).toEqual({ "stt:deepgram": "deepgram-key" });
    expect(mocks.getSecret).toHaveBeenCalledWith(
      "ai-provider-api-keys",
      "stt:deepgram",
    );
    expect(mocks.setSecret).not.toHaveBeenCalled();
    expect(mocks.executeTransaction).not.toHaveBeenCalled();
  });

  it("loads one stored provider with its secure API key", async () => {
    mocks.execute.mockResolvedValueOnce([
      {
        id: "ai_provider:llm:anthropic",
        value_json: JSON.stringify({
          type: "llm",
          base_url: "https://api.anthropic.com/v1",
          api_key: "",
        }),
      },
    ]);
    mocks.getSecret.mockResolvedValueOnce({
      status: "ok",
      data: "anthropic-key",
    });

    await expect(getStoredAiProvider("llm", "anthropic")).resolves.toEqual({
      type: "llm",
      base_url: "https://api.anthropic.com/v1",
      api_key: "anthropic-key",
    });
  });

  it("repairs macOS Keychain access through the secure store", async () => {
    await expect(repairKeychainAccess()).resolves.toBeUndefined();
    expect(mocks.repairKeychainAccess).toHaveBeenCalledOnce();
  });

  it("surfaces Keychain repair failures", async () => {
    mocks.repairKeychainAccess.mockResolvedValueOnce({
      status: "error",
      error: "unlock cancelled",
    });

    await expect(repairKeychainAccess()).rejects.toThrow("unlock cancelled");
  });

  it("recognizes only the recoverable macOS Keychain error", () => {
    expect(
      isKeychainAccessError(
        new Error(
          "macOS couldn't access your login Keychain. Use repair below.",
        ),
      ),
    ).toBe(true);
    expect(
      isKeychainAccessError(new Error("Platform failure: missing entitlement")),
    ).toBe(false);
  });

  it("keeps the plaintext key when secure storage rejects migration", async () => {
    mocks.execute.mockResolvedValueOnce([
      {
        id: "ai_provider:stt:deepgram",
        value_json: JSON.stringify({
          type: "stt",
          base_url: "https://api.deepgram.com/v1",
          api_key: "deepgram-key",
        }),
      },
    ]);
    mocks.setSecret.mockRejectedValueOnce(new Error("keychain unavailable"));

    await expect(migratePlaintextAiProviderApiKeys()).rejects.toThrow(
      "keychain unavailable",
    );

    expect(mocks.executeTransaction).not.toHaveBeenCalled();
  });

  it("securely deletes migrated values and truncates the SQLite WAL", async () => {
    mocks.execute.mockResolvedValueOnce([
      {
        id: "ai_provider:stt:deepgram",
        value_json: JSON.stringify({
          type: "stt",
          base_url: "https://api.deepgram.com/v1",
          api_key: "deepgram-key",
        }),
      },
    ]);

    await migratePlaintextAiProviderApiKeys();

    const statements = mocks.executeTransaction.mock.calls[0][0];
    expect(statements[0].sql).toContain("PRAGMA secure_delete = ON");
    expect(mocks.execute).toHaveBeenLastCalledWith(
      "PRAGMA wal_checkpoint(TRUNCATE)",
    );
  });

  it("redacts reintroduced legacy keys without replacing an existing secret", async () => {
    mocks.getSecret.mockResolvedValue({
      status: "ok",
      data: "current-secure-key",
    });
    mocks.execute.mockResolvedValueOnce([
      {
        id: "legacy_settings_document",
        value_json: JSON.stringify({
          ai: {
            stt: {
              deepgram: {
                base_url: "https://legacy.example",
                api_key: "stale-legacy-key",
              },
            },
          },
        }),
      },
      {
        id: "ai_provider:stt:deepgram",
        value_json: JSON.stringify({
          type: "stt",
          base_url: "https://direct.example",
          api_key: "",
        }),
      },
    ]);

    await migratePlaintextAiProviderApiKeys();

    expect(mocks.setSecret).not.toHaveBeenCalled();
    const legacyUpdate = mocks.executeTransaction.mock.calls[0][0].find(
      (candidate) =>
        candidate.sql.includes("UPDATE app_settings") &&
        candidate.params.includes("legacy_settings_document"),
    )!;
    expect(String(legacyUpdate.params[0])).not.toContain("stale-legacy-key");
  });

  it("redacts stale direct keys without replacing an existing secret", async () => {
    mocks.getSecret.mockResolvedValue({
      status: "ok",
      data: "current-secure-key",
    });
    mocks.execute.mockResolvedValueOnce([
      {
        id: "ai_provider:stt:deepgram",
        value_json: JSON.stringify({
          type: "stt",
          base_url: "https://direct.example",
          api_key: "stale-direct-key",
        }),
      },
    ]);

    await migratePlaintextAiProviderApiKeys();

    expect(mocks.setSecret).not.toHaveBeenCalled();
    const directUpdate = mocks.executeTransaction.mock.calls[0][0].find(
      (candidate) => candidate.params.includes("ai_provider:stt:deepgram"),
    )!;
    expect(String(directUpdate.params[0])).not.toContain("stale-direct-key");
  });

  it("refreshes legacy settings before migrating the next provider type", async () => {
    const legacySettings = (llmKey: string, sttKey: string) => ({
      id: "legacy_settings_document",
      value_json: JSON.stringify({
        ai: {
          llm: {
            openai: {
              base_url: "https://api.openai.com/v1",
              api_key: llmKey,
            },
          },
          stt: {
            deepgram: {
              base_url: "https://api.deepgram.com/v1",
              api_key: sttKey,
            },
          },
        },
      }),
    });
    mocks.execute
      .mockResolvedValueOnce([legacySettings("llm-key", "stt-key")])
      .mockResolvedValueOnce([legacySettings("", "stt-key")]);

    await migratePlaintextAiProviderApiKeys();

    const sttStatements = mocks.executeTransaction.mock.calls[1][0];
    const legacyUpdate = sttStatements.find((candidate) =>
      candidate.params.includes("legacy_settings_document"),
    )!;
    expect(String(legacyUpdate.params[0])).not.toContain("llm-key");
    expect(String(legacyUpdate.params[0])).not.toContain("stt-key");
  });
});
