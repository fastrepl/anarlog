import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  execute: vi.fn(),
  executeTransaction: vi.fn(
    (_statements: Array<{ sql: string; params: unknown[] }>) =>
      Promise.resolve([1]),
  ),
}));

vi.mock("~/db", () => ({
  executeTransaction: mocks.executeTransaction,
  liveQueryClient: { execute: mocks.execute },
  useLiveQuery: vi.fn(() => ({ data: undefined })),
}));

vi.mock("~/db/write-queue", () => ({
  enqueueDatabaseWrite: (_key: string, operation: () => Promise<unknown>) =>
    operation(),
}));

import { parseAiProviders, setAiProvider } from "./providers";

describe("SQLite AI providers", () => {
  beforeEach(() => {
    vi.clearAllMocks();
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

    const statement = mocks.executeTransaction.mock.calls[0][0][0];
    expect(statement.sql).toContain("INSERT INTO app_settings");
    expect(statement.params[0]).toBe("ai_provider:stt:deepgram");
    expect(JSON.parse(String(statement.params[1]))).toEqual({
      type: "stt",
      base_url: "https://legacy.example",
      api_key: "new-key",
    });
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
      api_key: "new-key",
    });
  });
});
