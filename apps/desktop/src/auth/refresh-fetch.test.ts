import { createClient } from "@supabase/supabase-js";
import { expect, test, vi } from "vitest";

import { createRefreshFetch } from "./refresh-fetch";

test("concurrent windows do not duplicate refreshes or receive another account's refresh result", async () => {
  let busy = false;
  let release!: (response: Response) => void;
  const options = {
    supabaseUrl: "https://project.supabase.co",
    fetch: async (input: RequestInfo | URL) => {
      if (!input.toString().includes("grant_type=refresh_token")) {
        return Response.json({ operation: "sign-in" });
      }
      return new Promise<Response>((resolve) => {
        release = resolve;
      });
    },
    beginRefresh: async () => {
      const leaseId = busy ? null : 1;
      busy = true;
      return { leaseId, retryAfterMs: 1000 };
    },
    finishRefresh: async () => {
      busy = false;
    },
  };
  const firstWindow = createRefreshFetch(options);
  const secondWindow = createRefreshFetch(options);
  const url =
    "https://project.supabase.co/auth/v1/token?grant_type=refresh_token";
  const request = {
    method: "POST",
    body: JSON.stringify({ refresh_token: "first-account" }),
  };
  const first = firstWindow(url, request);
  const joined = firstWindow(url, request);
  const otherAccount = await firstWindow(url, {
    ...request,
    body: JSON.stringify({ refresh_token: "other-account" }),
  });
  expect(otherAccount.status).toBe(429);
  expect((await secondWindow(url, request)).status).toBe(429);
  const signIn = await secondWindow(
    "https://project.supabase.co/auth/v1/token?grant_type=password",
    { method: "POST" },
  );
  expect(await signIn.json()).toEqual({ operation: "sign-in" });
  release(Response.json({ access_token: "first-account-result" }));
  expect(await (await first).json()).toEqual({
    access_token: "first-account-result",
  });
  expect(await (await joined).json()).toEqual({
    access_token: "first-account-result",
  });
});

test.each(["rate-limit", "offline"])(
  "%s session reads retain credentials without repeatedly hitting auth and recover after cooldown",
  async (failure) => {
    let now = Date.now();
    const clock = vi.spyOn(Date, "now").mockImplementation(() => now);
    let retryAt = 0;
    let upstreamAttempts = 0;
    const storage = new Map<string, string>();
    const storageKey = "test-auth";
    const session = {
      access_token: "expired-token",
      refresh_token: "refresh-token",
      token_type: "bearer",
      expires_at: Math.floor(Date.now() / 1000) - 60,
      user: { id: "test-user" },
    };
    storage.set(storageKey, JSON.stringify(session));
    const fetch = createRefreshFetch({
      supabaseUrl: "https://project.supabase.co/",
      fetch: async () => {
        upstreamAttempts += 1;
        if (upstreamAttempts === 1) {
          if (failure === "offline") throw new TypeError("Network unavailable");
          return Response.json(
            { message: "Too many requests" },
            { status: 429, headers: { "Retry-After": "90" } },
          );
        }
        return Response.json({
          ...session,
          access_token: "recovered-token",
          refresh_token: "rotated-token",
          expires_in: 3600,
          expires_at: Math.floor(now / 1000) + 3600,
        });
      },
      beginRefresh: async () => ({
        leaseId: retryAt > Date.now() ? null : 1,
        retryAfterMs: Math.max(0, retryAt - Date.now()),
      }),
      finishRefresh: async (_, status, retryAfterMs) => {
        retryAt = Date.now() + (status === 200 ? 0 : (retryAfterMs ?? 30_000));
        if (status === 200)
          throw new Error("Gate release failed after rotation");
      },
    });
    const client = createClient("https://project.supabase.co", "test-key", {
      global: { fetch },
      auth: {
        autoRefreshToken: false,
        detectSessionInUrl: false,
        storageKey,
        storage: {
          getItem: (key) => storage.get(key) ?? null,
          setItem: (key, value) => {
            storage.set(key, value);
          },
          removeItem: () => {},
        },
      },
    });
    try {
      await client.auth.getSession();
      for (let read = 0; read < 5; read += 1) {
        expect((await client.auth.getSession()).error?.status).toBe(429);
      }
      expect(storage.get(storageKey)).toBe(JSON.stringify(session));
      expect(upstreamAttempts).toBe(1);
      now += 90_000;
      const recovered = await client.auth.getSession();
      expect(recovered.error).toBeNull();
      expect(recovered.data.session?.access_token).toBe("recovered-token");
      expect(JSON.parse(storage.get(storageKey)!).refresh_token).toBe(
        "rotated-token",
      );
    } finally {
      client.auth.stopAutoRefresh();
      clock.mockRestore();
    }
  },
);
