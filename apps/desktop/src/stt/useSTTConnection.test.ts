import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { renderHook, waitFor } from "@testing-library/react";
import { createElement, type ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const {
  authState,
  billingState,
  config,
  getServerForModelMock,
  isModelDownloadedMock,
  readiness,
  startServerForPathMock,
} = vi.hoisted(() => ({
  authState: {
    session: { access_token: "access-token" } as
      | { access_token: string }
      | null
      | undefined,
  },
  billingState: {
    isPaid: true,
    isReady: true,
  },
  config: {
    current_stt_provider: "anarlog",
    current_stt_model: "cloud",
    local_stt_model_path: "",
  },
  getServerForModelMock: vi.fn(),
  isModelDownloadedMock: vi.fn(),
  readiness: {
    provider: true,
    settings: true,
  },
  startServerForPathMock: vi.fn(),
}));

vi.mock("@anlg/plugin-local-stt", () => ({
  commands: {
    getServerForModel: getServerForModelMock,
    isModelDownloaded: isModelDownloadedMock,
    startServerForPath: startServerForPathMock,
  },
}));

vi.mock("~/auth", () => ({
  useAuth: () => ({ session: authState.session }),
}));

vi.mock("~/auth/billing-context", () => ({
  useBillingAccess: () => billingState,
}));

vi.mock("~/env", () => ({
  env: { VITE_API_URL: "https://api.anarlog.so" },
}));

vi.mock("~/settings/providers", () => ({
  useAiProvidersState: () => ({
    isReady: readiness.provider,
    providers: {
      "stt:anarlog": {
        type: "stt",
        base_url: "   ",
        api_key: "test-key",
      },
      "stt:deepgram": {
        type: "stt",
        base_url: "   ",
        api_key: "test-key",
      },
    },
  }),
}));

vi.mock("~/settings/queries", () => ({
  useSettingsReady: () => readiness.settings,
}));

vi.mock("~/shared/config", () => ({
  useConfigValues: () => config,
}));

vi.mock("~/stt/capabilities", () => ({
  isAnarlogCloudSttModel: (provider: string, model: string) =>
    provider === "anarlog" && model === "cloud",
  isLocalFileSttModel: (provider: string, model: string) =>
    provider === "local_file" && model === "local-file",
  isOnDeviceSttModel: (provider: string, model: string) =>
    provider === "soniqo" && model === "soniqo-parakeet-streaming",
  isRealtimeLocalModel: (model: string) =>
    model === "soniqo-parakeet-streaming",
}));

import { useSTTConnection } from "./useSTTConnection";

describe("useSTTConnection", () => {
  beforeEach(() => {
    config.current_stt_provider = "anarlog";
    config.current_stt_model = "cloud";
    config.local_stt_model_path = "";
    authState.session = { access_token: "access-token" };
    billingState.isPaid = true;
    billingState.isReady = true;
    readiness.provider = true;
    readiness.settings = true;
    getServerForModelMock.mockReset();
    isModelDownloadedMock.mockReset();
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
    expect(result.current.isReady).toBe(true);
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

  it("waits for stored settings and secure provider configuration", () => {
    config.current_stt_provider = "deepgram";
    config.current_stt_model = "nova-3-general";
    readiness.provider = false;
    const queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    });
    const wrapper = ({ children }: { children: ReactNode }) =>
      createElement(QueryClientProvider, { client: queryClient }, children);

    const providerPending = renderHook(() => useSTTConnection(), { wrapper });

    expect(providerPending.result.current.isReady).toBe(false);

    providerPending.unmount();
    readiness.provider = true;
    readiness.settings = false;
    const settingsPending = renderHook(() => useSTTConnection(), { wrapper });

    expect(settingsPending.result.current.isReady).toBe(false);
  });

  it("waits for cloud authentication and billing access", () => {
    billingState.isReady = false;
    const queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    });
    const wrapper = ({ children }: { children: ReactNode }) =>
      createElement(QueryClientProvider, { client: queryClient }, children);

    const billingPending = renderHook(() => useSTTConnection(), { wrapper });

    expect(billingPending.result.current.isReady).toBe(false);

    billingPending.unmount();
    billingState.isReady = true;
    authState.session = undefined;
    const authPending = renderHook(() => useSTTConnection(), { wrapper });

    expect(authPending.result.current.conn).toBeNull();
    expect(authPending.result.current.isReady).toBe(false);
  });

  it("waits for an on-device model server to become ready", async () => {
    config.current_stt_provider = "soniqo";
    config.current_stt_model = "soniqo-parakeet-streaming";
    isModelDownloadedMock.mockResolvedValue({ status: "ok", data: true });
    getServerForModelMock.mockResolvedValue({
      status: "ok",
      data: { status: "loading" },
    });
    const queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    });
    const wrapper = ({ children }: { children: ReactNode }) =>
      createElement(QueryClientProvider, { client: queryClient }, children);

    const { result } = renderHook(() => useSTTConnection(), { wrapper });

    await waitFor(() => expect(result.current.local.isPending).toBe(false));
    expect(result.current.conn).toBeNull();
    expect(result.current.local.data?.status).toBe("loading");
    expect(result.current.isReady).toBe(false);
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
