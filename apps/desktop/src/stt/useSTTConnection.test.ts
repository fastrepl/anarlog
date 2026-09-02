import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { renderHook, waitFor } from "@testing-library/react";
import { createElement, type ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const { config, startServerForPathMock } = vi.hoisted(() => ({
  config: {
    current_stt_provider: "anarlog",
    current_stt_model: "cloud",
    local_stt_model_path: "",
  },
  startServerForPathMock: vi.fn(),
}));

vi.mock("@anlg/plugin-local-stt", () => ({
  commands: {
    getServerForModel: vi.fn(),
    isModelDownloaded: vi.fn(),
    startServerForPath: startServerForPathMock,
  },
}));

vi.mock("~/auth", () => ({
  useAuth: () => ({ session: { access_token: "access-token" } }),
}));

vi.mock("~/auth/billing-context", () => ({
  useBillingAccess: () => ({ isPaid: true }),
}));

vi.mock("~/env", () => ({
  env: { VITE_API_URL: "https://api.anarlog.so" },
}));

vi.mock("~/settings/providers", () => ({
  useAiProvider: () => ({
    type: "stt",
    base_url: "   ",
    api_key: "test-key",
  }),
}));

vi.mock("~/shared/config", () => ({
  useConfigValues: () => config,
}));

vi.mock("~/stt/capabilities", () => ({
  isAnarlogCloudSttModel: (provider: string, model: string) =>
    provider === "anarlog" && model === "cloud",
  isLocalFileSttModel: (provider: string, model: string) =>
    provider === "local_file" && model === "local-file",
  isOnDeviceSttModel: () => false,
  isRealtimeLocalModel: () => false,
}));

import { useSTTConnection } from "./useSTTConnection";

describe("useSTTConnection", () => {
  beforeEach(() => {
    config.current_stt_provider = "anarlog";
    config.current_stt_model = "cloud";
    config.local_stt_model_path = "";
    startServerForPathMock.mockReset();
  });

  it("uses the hosted STT URL when the stored Anarlog URL is blank", () => {
    const queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    });
    const wrapper = ({ children }: { children: ReactNode }) =>
      createElement(QueryClientProvider, { client: queryClient }, children);

    const { result } = renderHook(() => useSTTConnection(), { wrapper });

    expect(result.current.conn).toEqual({
      provider: "anarlog",
      model: "cloud",
      baseUrl: "https://api.anarlog.so/stt",
      apiKey: "access-token",
    });
  });

  it("uses the provider endpoint when only a Deepgram API key is stored", () => {
    config.current_stt_provider = "deepgram";
    config.current_stt_model = "nova-3-general";
    const queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    });
    const wrapper = ({ children }: { children: ReactNode }) =>
      createElement(QueryClientProvider, { client: queryClient }, children);

    const { result } = renderHook(() => useSTTConnection(), { wrapper });

    expect(result.current.conn).toEqual({
      provider: "deepgram",
      model: "nova-3-general",
      baseUrl: "https://api.deepgram.com/v1",
      apiKey: "test-key",
    });
  });

  it("starts a selected local model file and exposes its local URL", async () => {
    config.current_stt_provider = "local_file";
    config.current_stt_model = "local-file";
    config.local_stt_model_path = "/models/ggml-small.bin";
    startServerForPathMock.mockResolvedValue({
      status: "ok",
      data: "http://127.0.0.1:4040/v1",
    });
    const queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    });
    const wrapper = ({ children }: { children: ReactNode }) =>
      createElement(QueryClientProvider, { client: queryClient }, children);

    const { result } = renderHook(() => useSTTConnection(), { wrapper });

    await waitFor(() =>
      expect(result.current.conn).toEqual({
        provider: "local_file",
        model: "local-file",
        baseUrl: "http://127.0.0.1:4040/v1",
        apiKey: "",
      }),
    );
    expect(startServerForPathMock).toHaveBeenCalledWith(
      "/models/ggml-small.bin",
    );
  });
});
