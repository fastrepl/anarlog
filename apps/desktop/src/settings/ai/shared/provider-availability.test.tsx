import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, beforeEach, expect, test, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  fetch: vi.fn(),
  key: 0,
  baseUrl: "",
  provider: "openai",
}));

vi.mock("@tauri-apps/plugin-http", () => ({ fetch: mocks.fetch }));
vi.mock("~/settings/providers", () => ({
  useAiProviders: (type: string) => ({
    [`${type}:${mocks.provider}`]: {
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
  mocks.provider = "openai";
});
afterEach(cleanup);

test("makes an authenticated local provider available despite its origin restriction", async () => {
  mocks.baseUrl = "http://127.0.0.1:8000/v1";
  mocks.fetch.mockImplementation(async (_input, init) => {
    const headers = new Headers(init?.headers);
    if (headers.get("Origin") !== "")
      return new Response(null, { status: 403 });
    return headers.get("Authorization") === `Bearer saved-key-${mocks.key}`
      ? Response.json({ data: [{ id: "mtplx" }] })
      : new Response(null, { status: 401 });
  });

  const { result, client, unmount } = setup("llm");
  await waitFor(() => expect(result.current.openai).toBe(true));
  unmount();
  client.clear();
});

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

test("keeps a saved Custom STT endpoint available without model-list verification", async () => {
  mocks.provider = "custom";
  mocks.baseUrl = "http://127.0.0.1:8000/v1";
  mocks.fetch.mockResolvedValue(new Response(null, { status: 404 }));
  const { result, client, unmount } = setup("stt");
  await waitFor(() => expect(result.current.custom).toBe(true));
  expect(mocks.fetch).not.toHaveBeenCalled();
  unmount();
  client.clear();
});

test("still probes Custom LLM credentials and rejects an unsupported model-list endpoint", async () => {
  mocks.provider = "custom";
  mocks.baseUrl = "http://127.0.0.1:8000/v1";
  mocks.fetch.mockResolvedValue(new Response(null, { status: 401 }));
  const { result, client, unmount } = setup("llm");
  await waitFor(() => expect(result.current.custom).toBe(false));
  expect(mocks.fetch).toHaveBeenCalled();
  unmount();
  client.clear();
});

function setup(type: "stt" | "llm") {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  const hook = renderHook(
    () =>
      useProviderAvailability(type, [
        {
          id: mocks.provider,
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
