import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, renderHook, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, beforeEach, expect, test, vi } from "vitest";

const { providerFetchMock, useProviderAvailabilityMock } = vi.hoisted(() => ({
  providerFetchMock: vi.fn(),
  useProviderAvailabilityMock: vi.fn(),
}));

vi.mock("~/ai/provider-fetch", () => ({
  providerFetch: providerFetchMock,
}));

vi.mock("~/auth/billing-context", () => ({
  useBillingAccess: () => ({ isPaid: true }),
}));

vi.mock("~/settings/ai/shared", async (importOriginal) => ({
  ...(await importOriginal<typeof import("~/settings/ai/shared")>()),
  useProviderAvailability: useProviderAvailabilityMock,
}));

vi.mock("~/settings/providers", () => ({
  useAiProvidersState: () => ({
    isReady: true,
    providers: { "stt:deepgram": { api_key: "saved-key" } },
  }),
}));

vi.mock("~/shared/config", async (importOriginal) => ({
  ...(await importOriginal<typeof import("~/shared/config")>()),
  useConfigValues: () => ({ local_stt_model_path: "" }),
}));

import { useConfiguredMapping } from "./select";

beforeEach(() => {
  providerFetchMock.mockReset();
  providerFetchMock.mockResolvedValue(new Response(null, { status: 404 }));
});

afterEach(cleanup);

test.each([true, false])(
  "waits for saved transcription credentials before enabling selection repair (%s)",
  (verified) => {
    const client = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    });
    client.setQueryData(["device-info"], { totalMemoryBytes: 16e9 });
    client.setQueryData(["list-supported-models"], []);
    useProviderAvailabilityMock.mockReturnValue({ deepgram: undefined });

    const { result, rerender } = renderHook(useConfiguredMapping, {
      wrapper: ({ children }: { children: ReactNode }) => (
        <QueryClientProvider client={client}>{children}</QueryClientProvider>
      ),
    });

    expect(result.current.providers.anarlog.configured).toBe(true);
    expect(result.current.providers.deepgram.configured).toBe(false);
    expect(result.current.isReady).toBe(false);

    useProviderAvailabilityMock.mockReturnValue({ deepgram: verified });
    rerender();

    expect(result.current.isReady).toBe(true);
    expect(result.current.providers.deepgram.configured).toBe(verified);
  },
);

test("merges dynamically discovered STT models without blocking readiness", async () => {
  providerFetchMock.mockResolvedValue(
    new Response(
      JSON.stringify({
        stt: [{ canonical_name: "nova-3-live-discovery" }],
      }),
      { status: 200 },
    ),
  );
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  client.setQueryData(["device-info"], { totalMemoryBytes: 16e9 });
  client.setQueryData(["list-supported-models"], []);
  useProviderAvailabilityMock.mockReturnValue({ deepgram: true });

  const { result } = renderHook(useConfiguredMapping, {
    wrapper: ({ children }: { children: ReactNode }) => (
      <QueryClientProvider client={client}>{children}</QueryClientProvider>
    ),
  });

  expect(result.current.isReady).toBe(true);
  await waitFor(() => {
    expect(
      result.current.providers.deepgram.models.map((m) => m.id),
    ).toContain("nova-3-live-discovery");
  });
});
