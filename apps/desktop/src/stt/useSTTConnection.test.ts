import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { renderHook } from "@testing-library/react";
import { createElement, type ReactNode } from "react";
import { describe, expect, it, vi } from "vitest";

vi.mock("@anlg/plugin-local-stt", () => ({
  commands: {
    getServerForModel: vi.fn(),
    isModelDownloaded: vi.fn(),
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
    api_key: "",
  }),
}));

vi.mock("~/shared/config", () => ({
  useConfigValues: () => ({
    current_stt_provider: "anarlog",
    current_stt_model: "cloud",
  }),
}));

vi.mock("~/stt/capabilities", () => ({
  isAnarlogCloudSttModel: () => true,
  isOnDeviceSttModel: () => false,
}));

import { useSTTConnection } from "./useSTTConnection";

describe("useSTTConnection", () => {
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
});
