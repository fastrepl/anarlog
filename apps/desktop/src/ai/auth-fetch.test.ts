import { describe, expect, it, vi } from "vitest";

import { createAuthFetch } from "./auth-fetch";

function jsonResponse(status: number, body: string) {
  return new Response(body, { status });
}

function authorizationHeader(
  fetchMock: ReturnType<typeof vi.fn>,
  callIndex: number,
) {
  const init = fetchMock.mock.calls[callIndex]?.[1] as RequestInit | undefined;
  return new Headers(init?.headers).get("Authorization");
}

describe("createAuthFetch", () => {
  it("attaches the current access token before calling the base fetch", async () => {
    const baseFetch = vi.fn(async () => jsonResponse(200, "ok"));
    const fetchWithAuth = createAuthFetch(baseFetch, () => "fresh-token");

    const response = await fetchWithAuth("https://api.anarlog.so/llm", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: "{}",
    });

    expect(response.status).toBe(200);
    expect(baseFetch).toHaveBeenCalledOnce();
    expect(authorizationHeader(baseFetch, 0)).toBe("Bearer fresh-token");
  });

  it("refreshes once and retries when the hosted API rejects an expired token", async () => {
    const baseFetch = vi
      .fn()
      .mockResolvedValueOnce(jsonResponse(401, "invalid_token"))
      .mockResolvedValueOnce(jsonResponse(200, "ok"));
    const refreshAccessToken = vi.fn(async () => "rotated-token");
    const fetchWithAuth = createAuthFetch(
      baseFetch,
      () => "stale-token",
      refreshAccessToken,
    );

    const response = await fetchWithAuth("https://api.anarlog.so/llm", {
      method: "POST",
      body: "{}",
    });

    expect(response.status).toBe(200);
    expect(await response.text()).toBe("ok");
    expect(refreshAccessToken).toHaveBeenCalledOnce();
    expect(baseFetch).toHaveBeenCalledTimes(2);
    expect(authorizationHeader(baseFetch, 0)).toBe("Bearer stale-token");
    expect(authorizationHeader(baseFetch, 1)).toBe("Bearer rotated-token");
  });

  it("does not retry when refresh cannot produce a different token", async () => {
    const unauthorized = jsonResponse(401, "invalid_token");
    const baseFetch = vi.fn(async () => unauthorized);
    const refreshAccessToken = vi.fn(async () => "stale-token");
    const fetchWithAuth = createAuthFetch(
      baseFetch,
      () => "stale-token",
      refreshAccessToken,
    );

    const response = await fetchWithAuth("https://api.anarlog.so/llm");

    expect(response).toBe(unauthorized);
    expect(refreshAccessToken).toHaveBeenCalledOnce();
    expect(baseFetch).toHaveBeenCalledOnce();
  });
});
