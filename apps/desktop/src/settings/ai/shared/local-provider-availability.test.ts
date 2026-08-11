import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  fetch: vi.fn(),
}));

vi.mock("@tauri-apps/plugin-http", () => ({
  fetch: mocks.fetch,
}));

import {
  checkLMStudioAvailability,
  checkOllamaAvailability,
  checkUnslothAvailability,
} from "./local-provider-availability";

describe("local provider availability", () => {
  beforeEach(() => {
    mocks.fetch.mockReset();
  });

  it("checks the Ollama version endpoint", async () => {
    mocks.fetch.mockResolvedValueOnce(new Response(null, { status: 200 }));

    await expect(
      checkOllamaAvailability("http://127.0.0.1:11434/v1"),
    ).resolves.toBe(true);
    expect(mocks.fetch).toHaveBeenCalledWith(
      "http://127.0.0.1:11434/api/version",
      {
        method: "GET",
        headers: { Origin: "http://127.0.0.1:11434" },
      },
    );
  });

  it("reports Ollama as unavailable when its server is not running", async () => {
    mocks.fetch.mockRejectedValueOnce(new Error("connection refused"));

    await expect(
      checkOllamaAvailability("http://127.0.0.1:11434/v1"),
    ).resolves.toBe(false);
  });

  it("checks LM Studio with its optional API key", async () => {
    mocks.fetch.mockResolvedValueOnce(new Response(null, { status: 200 }));

    await expect(
      checkLMStudioAvailability("http://127.0.0.1:1234/v1", "secret"),
    ).resolves.toBe(true);
    expect(mocks.fetch).toHaveBeenCalledWith(
      "http://127.0.0.1:1234/api/v1/models",
      {
        method: "GET",
        headers: { Authorization: "Bearer secret" },
      },
    );
  });

  it("falls back to the OpenAI-compatible LM Studio endpoint", async () => {
    mocks.fetch
      .mockResolvedValueOnce(new Response(null, { status: 404 }))
      .mockResolvedValueOnce(new Response(null, { status: 200 }));

    await expect(
      checkLMStudioAvailability("http://127.0.0.1:1234/v1", ""),
    ).resolves.toBe(true);
    expect(mocks.fetch).toHaveBeenNthCalledWith(
      2,
      "http://127.0.0.1:1234/v1/models",
      { method: "GET", headers: {} },
    );
  });

  it("checks Unsloth with its bearer key", async () => {
    mocks.fetch.mockResolvedValueOnce(new Response(null, { status: 200 }));

    await expect(
      checkUnslothAvailability("http://127.0.0.1:8888/v1", "sk-unsloth-test"),
    ).resolves.toBe(true);
    expect(mocks.fetch).toHaveBeenCalledWith(
      "http://127.0.0.1:8888/v1/models",
      {
        method: "GET",
        headers: { Authorization: "Bearer sk-unsloth-test" },
      },
    );
  });

  it("reports Unsloth as unavailable when the key is rejected", async () => {
    mocks.fetch.mockResolvedValueOnce(new Response(null, { status: 401 }));

    await expect(
      checkUnslothAvailability("http://127.0.0.1:8888/v1", "sk-unsloth-bad"),
    ).resolves.toBe(false);
  });

  it("requires a successful local provider response", async () => {
    mocks.fetch.mockResolvedValue(new Response(null, { status: 503 }));

    await expect(
      checkLMStudioAvailability("http://127.0.0.1:1234/v1", ""),
    ).resolves.toBe(false);
  });
});
