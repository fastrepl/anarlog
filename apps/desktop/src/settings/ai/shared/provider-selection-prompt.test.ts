import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  setSettingValues: vi.fn(),
  toastSuccess: vi.fn(),
}));

vi.mock("@lingui/react/macro", () => ({
  useLingui: () => ({
    t: (strings: TemplateStringsArray, ...values: unknown[]) =>
      strings.reduce(
        (message, part, index) =>
          `${message}${part}${index < values.length ? String(values[index]) : ""}`,
        "",
      ),
  }),
}));

vi.mock("@anlg/ui/components/ui/toast", () => ({
  sonnerToast: { success: mocks.toastSuccess },
}));

vi.mock("~/settings/queries", () => ({
  setSettingValues: mocks.setSettingValues,
}));

import { useProviderSelectionPrompt } from "./provider-selection-prompt";

describe("useProviderSelectionPrompt", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.setSettingValues.mockResolvedValue(undefined);
  });

  it("offers to switch to a provider after its first API key is saved", () => {
    const { result } = renderHook(() =>
      useProviderSelectionPrompt({
        providerType: "llm",
        providerId: "openai",
        providerName: "OpenAI",
        currentProvider: "anthropic",
        providerStateReady: true,
        storedApiKey: "",
      }),
    );

    act(() => result.current("sk-new"));

    expect(mocks.toastSuccess).toHaveBeenCalledWith("API key saved", {
      id: "provider-selection:llm:openai",
      duration: Infinity,
      description: "Set OpenAI as the current provider?",
      action: {
        label: "Set as current",
        onClick: expect.any(Function),
      },
    });

    const toastOptions = mocks.toastSuccess.mock.calls[0][1];
    act(() => toastOptions.action.onClick());

    expect(mocks.setSettingValues).toHaveBeenCalledWith({
      current_llm_provider: "openai",
      current_llm_model: "",
    });
  });

  it("sets a transcription provider and lets model resolution choose its model", () => {
    const { result } = renderHook(() =>
      useProviderSelectionPrompt({
        providerType: "stt",
        providerId: "deepgram",
        providerName: "Deepgram",
        currentProvider: "anarlog",
        providerStateReady: true,
        storedApiKey: "",
      }),
    );

    act(() => result.current("dg-new"));
    const toastOptions = mocks.toastSuccess.mock.calls[0][1];
    act(() => toastOptions.action.onClick());

    expect(mocks.setSettingValues).toHaveBeenCalledWith({
      current_stt_provider: "deepgram",
      current_stt_model: "",
    });
  });

  it("does not prompt for the current provider or an existing API key", () => {
    const currentProvider = renderHook(() =>
      useProviderSelectionPrompt({
        providerType: "llm",
        providerId: "openai",
        providerName: "OpenAI",
        currentProvider: "openai",
        providerStateReady: true,
        storedApiKey: "",
      }),
    );
    const existingKey = renderHook(() =>
      useProviderSelectionPrompt({
        providerType: "stt",
        providerId: "deepgram",
        providerName: "Deepgram",
        currentProvider: "anarlog",
        providerStateReady: true,
        storedApiKey: "dg-existing",
      }),
    );

    act(() => currentProvider.result.current("sk-new"));
    act(() => existingKey.result.current("dg-replaced"));

    expect(mocks.toastSuccess).not.toHaveBeenCalled();
  });

  it("prompts only once while the same new key is being saved", () => {
    const { result } = renderHook(() =>
      useProviderSelectionPrompt({
        providerType: "llm",
        providerId: "openai",
        providerName: "OpenAI",
        currentProvider: "anthropic",
        providerStateReady: true,
        storedApiKey: "",
      }),
    );

    act(() => result.current("s"));
    act(() => result.current("sk-new"));

    expect(mocks.toastSuccess).toHaveBeenCalledTimes(1);
  });
});
