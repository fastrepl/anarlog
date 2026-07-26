import { describe, expect, it, vi } from "vitest";

import { runWebSearch } from "./web-search";

describe("web search chat tool", () => {
  it("returns a tool error when auth headers are unavailable", async () => {
    const fetch = vi.fn();

    const result = await runWebSearch(
      { query: "how can char.com help?" },
      {
        getAuthHeaders: () => null,
        fetch: fetch as unknown as typeof globalThis.fetch,
      },
    );

    expect(fetch).not.toHaveBeenCalled();
    expect(result).toEqual({
      status: "error",
      message: "Sign in to use web search.",
      query: "how can char.com help?",
      results: [],
    });
  });

  it("posts search requests to the hosted research endpoint", async () => {
    const fetch = vi.fn(
      async (_input: RequestInfo | URL, _init?: RequestInit) =>
        new Response(
          JSON.stringify({
            query: "how can char.com help?",
            results: [
              {
                title: "Char",
                url: "https://char.com",
                snippet: "Char helps teams turn meetings into useful notes.",
                publishedDate: null,
                author: null,
              },
            ],
          }),
          { status: 200, headers: { "Content-Type": "application/json" } },
        ),
    );

    const result = await runWebSearch(
      {
        query: "how can char.com help?",
        includeDomains: ["char.com"],
        limit: 3,
      },
      {
        getAuthHeaders: () => ({
          Authorization: "Bearer token",
          "x-request-id": "request-1",
        }),
        fetch: fetch as unknown as typeof globalThis.fetch,
      },
    );

    expect(fetch).toHaveBeenCalledOnce();
    const [url, init] = fetch.mock.calls[0]!;
    expect(url).toBe("http://localhost:3001/research/search");
    expect(init?.method).toBe("POST");
    expect((init?.headers as Headers).get("Authorization")).toBe(
      "Bearer token",
    );
    expect((init?.headers as Headers).get("Content-Type")).toBe(
      "application/json",
    );
    expect(JSON.parse(init?.body as string)).toEqual({
      query: "how can char.com help?",
      numResults: 3,
      includeDomains: ["char.com"],
    });
    expect(result).toEqual({
      status: "ok",
      query: "how can char.com help?",
      results: [
        {
          title: "Char",
          url: "https://char.com",
          snippet: "Char helps teams turn meetings into useful notes.",
          publishedDate: null,
          author: null,
        },
      ],
    });
  });
});
