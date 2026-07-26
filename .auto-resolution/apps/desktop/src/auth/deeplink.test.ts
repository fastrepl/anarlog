import { describe, expect, it, vi } from "vitest";

import { createAuthCallbackHandler } from "./deeplink";

function deferred() {
  let resolve!: () => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<void>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });

  return { promise, reject, resolve };
}

describe("auth callback handling", () => {
  it("coalesces identical callbacks while accepting distinct callbacks", async () => {
    let now = 1_000;
    const first = deferred();
    const second = deferred();
    const third = deferred();
    const setSessionFromTokens = vi
      .fn<(accessToken: string, refreshToken: string) => Promise<void>>()
      .mockReturnValueOnce(first.promise)
      .mockReturnValueOnce(second.promise)
      .mockReturnValueOnce(third.promise);
    const handle = createAuthCallbackHandler({
      setSessionFromTokens,
      now: () => now,
      recentWindowMs: 100,
    });

    expect(handle("access-a", "refresh-a")).toBe(true);
    expect(handle("access-a", "refresh-a")).toBe(false);
    expect(handle("access-b", "refresh-b")).toBe(true);
    expect(setSessionFromTokens.mock.calls).toEqual([
      ["access-a", "refresh-a"],
      ["access-b", "refresh-b"],
    ]);

    first.resolve();
    second.resolve();
    await Promise.all([first.promise, second.promise]);
    expect(handle("access-a", "refresh-a")).toBe(false);

    now += 101;
    expect(handle("access-a", "refresh-a")).toBe(true);
    expect(setSessionFromTokens).toHaveBeenLastCalledWith(
      "access-a",
      "refresh-a",
    );

    third.resolve();
    await third.promise;
  });

  it("allows a failed callback to be retried", async () => {
    const first = deferred();
    const setSessionFromTokens = vi
      .fn<(accessToken: string, refreshToken: string) => Promise<void>>()
      .mockReturnValueOnce(first.promise)
      .mockResolvedValueOnce(undefined);
    const handle = createAuthCallbackHandler({ setSessionFromTokens });

    expect(handle("access", "refresh")).toBe(true);
    first.reject(new Error("temporary failure"));
    await expect(first.promise).rejects.toThrow("temporary failure");

    expect(handle("access", "refresh")).toBe(true);
    expect(setSessionFromTokens).toHaveBeenCalledTimes(2);
  });

  it("keeps a distinct callback guarded when an earlier callback fails", async () => {
    const first = deferred();
    const second = deferred();
    const setSessionFromTokens = vi
      .fn<(accessToken: string, refreshToken: string) => Promise<void>>()
      .mockReturnValueOnce(first.promise)
      .mockReturnValueOnce(second.promise);
    const handle = createAuthCallbackHandler({ setSessionFromTokens });

    expect(handle("access-a", "refresh-a")).toBe(true);
    expect(handle("access-b", "refresh-b")).toBe(true);

    first.reject(new Error("stale callback failed"));
    await expect(first.promise).rejects.toThrow("stale callback failed");

    expect(handle("access-b", "refresh-b")).toBe(false);
    expect(setSessionFromTokens).toHaveBeenCalledTimes(2);

    second.resolve();
    await second.promise;
  });
});
