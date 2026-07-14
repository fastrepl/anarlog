import type { AuthChangeEvent, Session } from "@supabase/supabase-js";
import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";

import {
  claimCloudsyncAccount,
  configureCloudsyncToken,
  logoutCloudsync,
  suspendCloudsync,
} from "@hypr/plugin-db";

import {
  claimCloudsyncAccountForAuth,
  handleCloudsyncAuthChange,
  prepareCloudsyncSignOut,
} from "./cloudsync";

vi.mock("~/env", () => ({
  env: {
    VITE_API_URL: "https://api.test",
  },
}));

const NOW = new Date("2026-07-13T00:00:00Z");

function session(accessToken = "supabase-token") {
  return {
    access_token: accessToken,
    user: { id: "user-id" },
  } as Session;
}

function credentialsResponse(workspaceId = "user-id") {
  return new Response(
    JSON.stringify({
      databaseId: "database-id",
      token: "sqlite-token",
      expiresAt: new Date(NOW.getTime() + 15 * 60 * 1000).toISOString(),
      workspaceId,
    }),
    {
      status: 200,
      headers: { "Content-Type": "application/json" },
    },
  );
}

describe("CloudSync auth lifecycle", () => {
  beforeEach(async () => {
    vi.useFakeTimers();
    vi.setSystemTime(NOW);
    await handleCloudsyncAuthChange("SIGNED_OUT", null);
    vi.clearAllMocks();
    vi.mocked(claimCloudsyncAccount).mockResolvedValue(true);
    vi.mocked(configureCloudsyncToken).mockResolvedValue(true);
    vi.mocked(logoutCloudsync).mockResolvedValue(undefined);
  });

  afterEach(async () => {
    await handleCloudsyncAuthChange("SIGNED_OUT", null);
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
    vi.useRealTimers();
  });

  test("claims the local account without a token exchange", async () => {
    await expect(claimCloudsyncAccountForAuth("user-id")).resolves.toBe(true);

    expect(claimCloudsyncAccount).toHaveBeenCalledWith("user-id");
    expect(configureCloudsyncToken).not.toHaveBeenCalled();
  });

  test("exchanges the Supabase token and refreshes before expiry", async () => {
    const fetchMock = vi.fn(() => Promise.resolve(credentialsResponse()));
    vi.stubGlobal("fetch", fetchMock);

    await handleCloudsyncAuthChange("SIGNED_IN", session());

    expect(fetchMock).toHaveBeenCalledWith(
      new URL("https://api.test/sync/token"),
      expect.objectContaining({
        method: "POST",
        headers: {
          Authorization: "Bearer supabase-token",
        },
      }),
    );
    expect(configureCloudsyncToken).toHaveBeenCalledWith(
      "database-id",
      "sqlite-token",
      "user-id",
    );
    expect(suspendCloudsync).toHaveBeenCalledTimes(1);

    await vi.advanceTimersByTimeAsync(13 * 60 * 1000 - 1);
    expect(fetchMock).toHaveBeenCalledTimes(1);

    await vi.advanceTimersByTimeAsync(1);
    expect(fetchMock).toHaveBeenCalledTimes(2);
  });

  test("suspends sync and ignores an exchange completed after sign-out", async () => {
    let resolveFetch: ((response: Response) => void) | undefined;
    vi.stubGlobal(
      "fetch",
      vi.fn(
        () =>
          new Promise<Response>((resolve) => {
            resolveFetch = resolve;
          }),
      ),
    );

    const activation = handleCloudsyncAuthChange("SIGNED_IN", session());
    await Promise.resolve();
    await Promise.resolve();
    await handleCloudsyncAuthChange("SIGNED_OUT", null);
    resolveFetch?.(credentialsResponse());
    await activation;

    expect(configureCloudsyncToken).not.toHaveBeenCalled();
    expect(suspendCloudsync).toHaveBeenCalledTimes(2);
  });

  test("suspends existing sync when exchange is not configured", async () => {
    const fetchMock = vi.fn(() =>
      Promise.resolve(new Response(null, { status: 404 })),
    );
    vi.stubGlobal("fetch", fetchMock);
    vi.spyOn(console, "warn").mockImplementation(() => {});

    await expect(
      handleCloudsyncAuthChange("INITIAL_SESSION", session()),
    ).resolves.toBe("ok");
    await vi.advanceTimersByTimeAsync(60 * 60 * 1000);

    expect(fetchMock).toHaveBeenCalledTimes(1);
    expect(configureCloudsyncToken).not.toHaveBeenCalled();
    expect(suspendCloudsync).toHaveBeenCalledTimes(1);
  });

  test("keeps sync disabled when the account does not have Pro", async () => {
    const fetchMock = vi.fn(() =>
      Promise.resolve(
        new Response(
          JSON.stringify({
            error: {
              code: "subscription_required",
              message: "Anarlog Pro is required for CloudSync",
            },
          }),
          { status: 403, headers: { "Content-Type": "application/json" } },
        ),
      ),
    );
    vi.stubGlobal("fetch", fetchMock);
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});

    await handleCloudsyncAuthChange("INITIAL_SESSION", session());
    await vi.advanceTimersByTimeAsync(60 * 60 * 1000);

    expect(fetchMock).toHaveBeenCalledTimes(1);
    expect(configureCloudsyncToken).not.toHaveBeenCalled();
    expect(suspendCloudsync).toHaveBeenCalledTimes(1);
    expect(warn).toHaveBeenCalledWith(
      "[cloudsync] Anarlog Pro is required; sync remains disabled",
    );
  });

  test("suspends existing sync when the initial session is empty", async () => {
    const fetchMock = vi.fn();
    vi.stubGlobal("fetch", fetchMock);

    await handleCloudsyncAuthChange("INITIAL_SESSION", null);

    expect(fetchMock).not.toHaveBeenCalled();
    expect(configureCloudsyncToken).not.toHaveBeenCalled();
    expect(suspendCloudsync).toHaveBeenCalledTimes(1);
  });

  test("rejects credentials for a different Supabase user", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(() => Promise.resolve(credentialsResponse("different-user"))),
    );
    vi.spyOn(console, "warn").mockImplementation(() => {});

    await handleCloudsyncAuthChange("SIGNED_IN", session());

    expect(configureCloudsyncToken).not.toHaveBeenCalled();
    expect(suspendCloudsync).toHaveBeenCalledTimes(1);
  });

  test("suspends existing sync when local configuration is rejected", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(() => Promise.resolve(credentialsResponse())),
    );
    vi.mocked(configureCloudsyncToken).mockRejectedValueOnce(
      new Error("workspace mismatch"),
    );
    vi.spyOn(console, "warn").mockImplementation(() => {});

    await expect(
      handleCloudsyncAuthChange("SIGNED_IN", session()),
    ).resolves.toBe("ok");
    await vi.advanceTimersByTimeAsync(60 * 1000);

    expect(configureCloudsyncToken).toHaveBeenCalledTimes(2);
    expect(suspendCloudsync).toHaveBeenCalledTimes(3);
  });

  test("reports the durable account mismatch without retrying", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(() => Promise.resolve(credentialsResponse())),
    );
    vi.mocked(configureCloudsyncToken).mockResolvedValueOnce(false);
    vi.spyOn(console, "warn").mockImplementation(() => {});

    await expect(
      handleCloudsyncAuthChange("SIGNED_IN", session()),
    ).resolves.toBe("account_mismatch");
    await vi.advanceTimersByTimeAsync(60 * 1000);

    expect(configureCloudsyncToken).toHaveBeenCalledTimes(1);
    expect(suspendCloudsync).toHaveBeenCalledTimes(1);
  });

  test("restarts sync after the authenticated user is updated", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(() => Promise.resolve(credentialsResponse())),
    );

    await handleCloudsyncAuthChange("USER_UPDATED", session());

    expect(configureCloudsyncToken).toHaveBeenCalledWith(
      "database-id",
      "sqlite-token",
      "user-id",
    );
    expect(suspendCloudsync).toHaveBeenCalledTimes(1);
  });

  test.each<AuthChangeEvent>(["PASSWORD_RECOVERY", "MFA_CHALLENGE_VERIFIED"])(
    "restarts sync after %s",
    async (event) => {
      vi.stubGlobal(
        "fetch",
        vi.fn(() => Promise.resolve(credentialsResponse())),
      );

      await handleCloudsyncAuthChange(event, session());

      expect(configureCloudsyncToken).toHaveBeenCalledWith(
        "database-id",
        "sqlite-token",
        "user-id",
      );
      expect(suspendCloudsync).toHaveBeenCalledTimes(1);
    },
  );

  test("checks for unsent changes before signing out", async () => {
    const fetchMock = vi.fn(() => Promise.resolve(credentialsResponse()));
    vi.stubGlobal("fetch", fetchMock);
    await handleCloudsyncAuthChange("SIGNED_IN", session());

    await prepareCloudsyncSignOut(session());
    await vi.advanceTimersByTimeAsync(15 * 60 * 1000);

    expect(logoutCloudsync).toHaveBeenCalledWith(false);
    expect(fetchMock).toHaveBeenCalledTimes(1);
  });

  test("resumes token refresh when guarded sign-out is rejected", async () => {
    const fetchMock = vi.fn(() => Promise.resolve(credentialsResponse()));
    vi.stubGlobal("fetch", fetchMock);
    vi.mocked(logoutCloudsync).mockRejectedValueOnce(
      new Error("cloudsync has unsent local changes"),
    );

    await expect(prepareCloudsyncSignOut(session())).rejects.toThrow(
      "unsent local changes",
    );
    await vi.advanceTimersByTimeAsync(1000);

    expect(fetchMock).toHaveBeenCalledTimes(1);
    expect(configureCloudsyncToken).toHaveBeenCalledTimes(1);
  });

  test("retries a transient exchange failure without rejecting auth", async () => {
    const fetchMock = vi
      .fn<() => Promise<Response>>()
      .mockRejectedValueOnce(new Error("offline"))
      .mockImplementation(() => Promise.resolve(credentialsResponse()));
    vi.stubGlobal("fetch", fetchMock);
    vi.spyOn(console, "warn").mockImplementation(() => {});

    await expect(
      handleCloudsyncAuthChange("TOKEN_REFRESHED", session()),
    ).resolves.toBe("ok");
    await vi.advanceTimersByTimeAsync(60 * 1000);

    expect(fetchMock).toHaveBeenCalledTimes(2);
    expect(configureCloudsyncToken).toHaveBeenCalledWith(
      "database-id",
      "sqlite-token",
      "user-id",
    );
    expect(suspendCloudsync).toHaveBeenCalledTimes(2);
  });
});
