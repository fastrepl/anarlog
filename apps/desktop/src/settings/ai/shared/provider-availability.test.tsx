import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, beforeEach, expect, test, vi } from "vitest";

const mocks = vi.hoisted(() => ({ fetch: vi.fn(), key: 0, baseUrl: "" }));

vi.mock("@tauri-apps/plugin-http", () => ({ fetch: mocks.fetch }));
vi.mock("~/auth/billing-context", () => ({
  useBillingAccess: () => ({ isPaid: true }),
}));
vi.mock("~/settings/providers", () => ({
  useAiProviders: (type: string) => ({
    [`${type}:openai`]: {
      api_key: `saved-key-${mocks.key}`,
      base_url: mocks.baseUrl,
    },
  }),
}));

import { useProviderAvailability } from "./index";

beforeEach(() => {
  mocks.fetch.mockReset();
  mocks.key++;
  mocks.baseUrl = "";
});
afterEach(cleanup);

test.each([
  "http://192.168.1.10/v1",
  "https://provider.test/v1?key=invalid",
  "not-a-url",
])(
  "invalid saved endpoint %s settles availability without a request",
  async (baseUrl) => {
    mocks.baseUrl = baseUrl;
    const { result, client, unmount } = setup("stt");
    await waitFor(() => expect(result.current.openai).toBe(false));
    expect(mocks.fetch).not.toHaveBeenCalled();
    unmount();
    client.clear();
  },
);

function setup(type: "stt" | "llm") {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  const hook = renderHook(
    () =>
      useProviderAvailability(type, [
        {
          id: "openai",
          displayName: "OpenAI",
          icon: null,
          baseUrl: "https://api.openai.com/v1",
          requirements: [{ kind: "requires_config", fields: ["api_key"] }],
        },
      ]),
    {
      wrapper: ({ children }: { children: ReactNode }) => (
        <QueryClientProvider client={client}>{children}</QueryClientProvider>
      ),
    },
  );
  return { ...hook, client };
}

test.each(["stt", "llm"] as const)(
  "keeps %s availability unknown after a network failure and recovers on retry",
  async (type) => {
    mocks.fetch.mockRejectedValue(new Error("offline"));
    const { result, client, unmount } = setup(type);
    await waitFor(() =>
      expect(client.getQueryCache().getAll()[0]?.state.status).toBe("error"),
    );
    expect(result.current.openai).toBeUndefined();

    mocks.fetch.mockResolvedValue(Response.json({ data: [] }));
    await act(() => client.refetchQueries());
    await waitFor(() => expect(result.current.openai).toBe(true));
    unmount();
    client.clear();
  },
);

test.each([401, 403, 429, 500])(
  "distinguishes rejected credentials from inconclusive HTTP %i responses",
  async (status) => {
    mocks.fetch.mockResolvedValue(new Response(null, { status }));
    const { result, client, unmount } = setup("stt");
    await waitFor(() => expect(client.isFetching()).toBe(0));
    if (status === 401 || status === 403) {
      await waitFor(() => expect(result.current.openai).toBe(false));
    } else {
      expect(result.current.openai).toBeUndefined();
    }
    unmount();
    client.clear();
  },
);
