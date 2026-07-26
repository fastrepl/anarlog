import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  getStoredAiProvider: vi.fn(),
  getStoredSettingValues: vi.fn(),
  setSettingValues: vi.fn(async () => undefined),
}));

vi.mock("~/settings/providers", () => ({
  getStoredAiProvider: mocks.getStoredAiProvider,
}));

vi.mock("~/settings/queries", () => ({
  getStoredSettingValues: mocks.getStoredSettingValues,
  setSettingValues: mocks.setSettingValues,
}));

import { configurePaidSettings } from "./configure-paid-settings";

describe("configurePaidSettings", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.getStoredAiProvider.mockResolvedValue(undefined);
  });

  it("selects hosted AI when no language model is configured", async () => {
    mocks.getStoredSettingValues.mockResolvedValue({
      values: {},
      hasValues: new Set(),
    });

    await configurePaidSettings();

    expect(mocks.setSettingValues).toHaveBeenCalledWith({
      current_stt_provider: "hyprnote",
      current_stt_model: "cloud",
      current_llm_provider: "hyprnote",
      current_llm_model: "Auto",
    });
  });

  it("repairs a selected provider whose required API key is missing", async () => {
    mocks.getStoredSettingValues.mockResolvedValue({
      values: {
        current_stt_provider: "hyprnote",
        current_stt_model: "cloud",
        current_llm_provider: "anthropic",
        current_llm_model: "claude-opus-4-5-20251101",
      },
      hasValues: new Set(),
    });

    await configurePaidSettings();

    expect(mocks.setSettingValues).toHaveBeenCalledWith({
      current_llm_provider: "hyprnote",
      current_llm_model: "Auto",
    });
  });

  it("repairs hosted defaults when secure provider lookup fails", async () => {
    mocks.getStoredSettingValues.mockResolvedValue({
      values: {
        current_llm_provider: "anthropic",
        current_llm_model: "claude-opus-4-5-20251101",
      },
      hasValues: new Set(),
    });
    mocks.getStoredAiProvider.mockRejectedValue(
      new Error("secure store unavailable"),
    );

    await configurePaidSettings();

    expect(mocks.setSettingValues).toHaveBeenCalledWith({
      current_stt_provider: "hyprnote",
      current_stt_model: "cloud",
      current_llm_provider: "hyprnote",
      current_llm_model: "Auto",
    });
  });

  it("preserves a configured bring-your-own provider", async () => {
    mocks.getStoredSettingValues.mockResolvedValue({
      values: {
        current_stt_provider: "hyprnote",
        current_stt_model: "cloud",
        current_llm_provider: "anthropic",
        current_llm_model: "claude-opus-4-5-20251101",
      },
      hasValues: new Set(),
    });
    mocks.getStoredAiProvider.mockResolvedValue({
      type: "llm",
      base_url: "https://api.anthropic.com/v1",
      api_key: "anthropic-key",
    });

    await configurePaidSettings();

    expect(mocks.setSettingValues).toHaveBeenCalledWith({});
  });

  it("preserves local providers that do not require credentials", async () => {
    mocks.getStoredSettingValues.mockResolvedValue({
      values: {
        current_stt_provider: "hyprnote",
        current_stt_model: "cloud",
        current_llm_provider: "ollama",
        current_llm_model: "llama3.2",
      },
      hasValues: new Set(),
    });

    await configurePaidSettings();

    expect(mocks.setSettingValues).toHaveBeenCalledWith({});
    expect(mocks.getStoredAiProvider).not.toHaveBeenCalled();
  });
});
