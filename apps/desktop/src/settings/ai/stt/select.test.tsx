import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, renderHook } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, expect, test, vi } from "vitest";

const { useProviderAvailabilityMock } = vi.hoisted(() => ({
  useProviderAvailabilityMock: vi.fn(),
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
