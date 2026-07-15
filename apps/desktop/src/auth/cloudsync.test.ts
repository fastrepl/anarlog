import type { AuthChangeEvent, Session } from "@supabase/supabase-js";
import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";

import {
  bindCloudsyncAccount,
  configureCloudsyncToken,
  getCloudsyncStatus,
  suspendCloudsync,
} from "@hypr/plugin-db";

import {
  bindCloudsyncAccountForAuth,
  handleCloudsyncAuthChange,
  prepareCloudsyncSignOut,
} from "./cloudsync";
import {
  startCloudsyncInitialSyncProgress,
  stopCloudsyncInitialSyncProgress,
} from "./cloudsync-progress";

vi.mock("./cloudsync-progress", () => ({
  startCloudsyncInitialSyncProgress: vi.fn(),
  stopCloudsyncInitialSyncProgress: vi.fn(),
}));

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
    vi.mocked(bindCloudsyncAccount).mockResolvedValue(true);
    vi.mocked(configureCloudsyncToken).mockResolvedValue("configured");
    vi.mocked(getCloudsyncStatus).mockResolvedValue({
      cloudsync_enabled: true,
      extension_loaded: true,
      configured: false,
      running: false,
      network_initialized: false,
      last_sync: null,
      last_sync_at_ms: null,
      has_unsent_changes: null,
      last_error: null,
      last_error_kind: null,
      consecutive_failures: 0,
    });
    vi.mocked(suspendCloudsync).mockResolvedValue(undefined);
  });

  afterEach(async () => {
    await handleCloudsyncAuthChange("SIGNED_OUT", null);
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
    vi.useRealTimers();
  });

  test("binds the local account without a token exchange", async () => {
    await expect(bindCloudsyncAccountForAuth("user-id")).resolves.toBe(true);

    expect(bindCloudsyncAccount).toHaveBeenCalledWith("user-id");
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
    expect(startCloudsyncInitialSyncProgress).toHaveBeenCalledWith("user-id");
    expect(suspendCloudsync).toHaveBeenCalledTimes(1);
    expect(stopCloudsyncInitialSyncProgress).toHaveBeenCalledTimes(1);

    await vi.advanceTimersByTimeAsync(13 * 60 * 1000 - 1);
    expect(fetchMock).toHaveBeenCalledTimes(1);

    await vi.advanceTimersByTimeAsync(1);
    expect(fetchMock).toHaveBeenCalledTimes(2);
    expect(suspendCloudsync).toHaveBeenCalledTimes(1);
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

  test.each([404, 501])(
    "suspends active sync when renewal returns %s",
    async (status) => {
      const fetchMock = vi
        .fn<() => Promise<Response>>()
        .mockResolvedValueOnce(credentialsResponse())
        .mockResolvedValueOnce(new Response(null, { status }));
      vi.stubGlobal("fetch", fetchMock);
      vi.spyOn(console, "warn").mockImplementation(() => {});

      await handleCloudsyncAuthChange("INITIAL_SESSION", session());
      await vi.advanceTimersByTimeAsync(13 * 60 * 1000);

      expect(fetchMock).toHaveBeenCalledTimes(2);
      expect(configureCloudsyncToken).toHaveBeenCalledTimes(1);
      expect(suspendCloudsync).toHaveBeenCalledTimes(2);
    },
  );

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

  test("does not retry credential exchange in local-only mode", async () => {
    const fetchMock = vi.fn();
    vi.stubGlobal("fetch", fetchMock);
    vi.mocked(getCloudsyncStatus).mockResolvedValueOnce({
      cloudsync_enabled: false,
      extension_loaded: false,
      configured: false,
      running: false,
      network_initialized: false,
      last_sync: null,
      last_sync_at_ms: null,
      has_unsent_changes: null,
      last_error: null,
      last_error_kind: null,
      consecutive_failures: 0,
    });
    vi.spyOn(console, "warn").mockImplementation(() => {});

    await handleCloudsyncAuthChange("INITIAL_SESSION", session());
    await vi.advanceTimersByTimeAsync(60 * 60 * 1000);

    expect(getCloudsyncStatus).toHaveBeenCalledTimes(1);
    expect(fetchMock).not.toHaveBeenCalled();
    expect(configureCloudsyncToken).not.toHaveBeenCalled();
  });

  test("does not configure local sync when the session is rejected", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(() => Promise.resolve(new Response(null, { status: 401 }))),
    );
    vi.spyOn(console, "warn").mockImplementation(() => {});

    await handleCloudsyncAuthChange("INITIAL_SESSION", session());

    expect(configureCloudsyncToken).not.toHaveBeenCalled();
    expect(suspendCloudsync).toHaveBeenCalledTimes(1);
  });

  test("suspends active sync when the account loses Pro", async () => {
    const fetchMock = vi
      .fn<() => Promise<Response>>()
      .mockResolvedValueOnce(credentialsResponse())
      .mockResolvedValueOnce(new Response(null, { status: 403 }));
    vi.stubGlobal("fetch", fetchMock);
    vi.spyOn(console, "warn").mockImplementation(() => {});

    await handleCloudsyncAuthChange("INITIAL_SESSION", session());
    await vi.advanceTimersByTimeAsync(13 * 60 * 1000);
    await vi.advanceTimersByTimeAsync(60 * 1000);

    expect(fetchMock).toHaveBeenCalledTimes(2);
    expect(configureCloudsyncToken).toHaveBeenCalledTimes(1);
    expect(suspendCloudsync).toHaveBeenCalledTimes(2);
  });

  test("retries suspension without exchanging again after Pro rejection", async () => {
    const fetchMock = vi
      .fn<() => Promise<Response>>()
      .mockResolvedValueOnce(credentialsResponse())
      .mockResolvedValueOnce(new Response(null, { status: 403 }));
    vi.stubGlobal("fetch", fetchMock);
    vi.spyOn(console, "warn").mockImplementation(() => {});
    vi.mocked(suspendCloudsync)
      .mockResolvedValueOnce(undefined)
      .mockRejectedValueOnce(new Error("cloudsync suspension failed"))
      .mockResolvedValueOnce(undefined);

    await handleCloudsyncAuthChange("INITIAL_SESSION", session());
    await vi.advanceTimersByTimeAsync(13 * 60 * 1000);

    expect(fetchMock).toHaveBeenCalledTimes(2);
    expect(suspendCloudsync).toHaveBeenCalledTimes(2);

    await vi.advanceTimersByTimeAsync(60 * 1000);
    await vi.advanceTimersByTimeAsync(60 * 60 * 1000);

    expect(fetchMock).toHaveBeenCalledTimes(2);
    expect(suspendCloudsync).toHaveBeenCalledTimes(3);
  });

  test("suspends active sync when the session is rejected", async () => {
    const fetchMock = vi
      .fn<() => Promise<Response>>()
      .mockResolvedValueOnce(credentialsResponse())
      .mockResolvedValueOnce(new Response(null, { status: 401 }));
    vi.stubGlobal("fetch", fetchMock);
    vi.spyOn(console, "warn").mockImplementation(() => {});

    await handleCloudsyncAuthChange("INITIAL_SESSION", session());
    await vi.advanceTimersByTimeAsync(13 * 60 * 1000);

    expect(fetchMock).toHaveBeenCalledTimes(2);
    expect(suspendCloudsync).toHaveBeenCalledTimes(2);
  });

  test("suspends active sync when renewed credentials change workspace", async () => {
    const fetchMock = vi
      .fn<() => Promise<Response>>()
      .mockResolvedValueOnce(credentialsResponse())
      .mockResolvedValueOnce(credentialsResponse("different-user"));
    vi.stubGlobal("fetch", fetchMock);
    vi.spyOn(console, "warn").mockImplementation(() => {});

    await handleCloudsyncAuthChange("INITIAL_SESSION", session());
    await vi.advanceTimersByTimeAsync(13 * 60 * 1000);

    expect(fetchMock).toHaveBeenCalledTimes(2);
    expect(configureCloudsyncToken).toHaveBeenCalledTimes(1);
    expect(suspendCloudsync).toHaveBeenCalledTimes(2);
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

  test("suspends and retries a transient configuration failure", async () => {
    const fetchMock = vi.fn(() => Promise.resolve(credentialsResponse()));
    vi.stubGlobal("fetch", fetchMock);
    vi.mocked(configureCloudsyncToken).mockRejectedValueOnce(
      new Error("workspace mismatch"),
    );
    vi.spyOn(console, "warn").mockImplementation(() => {});

    await expect(
      handleCloudsyncAuthChange("SIGNED_IN", session()),
    ).resolves.toBe("ok");
    await vi.advanceTimersByTimeAsync(60 * 1000);

    expect(fetchMock).toHaveBeenCalledTimes(2);
    expect(configureCloudsyncToken).toHaveBeenCalledTimes(2);
    expect(suspendCloudsync).toHaveBeenCalledTimes(2);
  });

  test("reports a permanent configuration rejection without re-exchanging", async () => {
    const fetchMock = vi.fn(() => Promise.resolve(credentialsResponse()));
    vi.stubGlobal("fetch", fetchMock);
    vi.mocked(configureCloudsyncToken).mockResolvedValueOnce(
      "account_mismatch",
    );
    vi.spyOn(console, "warn").mockImplementation(() => {});

    await expect(
      handleCloudsyncAuthChange("SIGNED_IN", session()),
    ).resolves.toBe("account_mismatch");
    await vi.advanceTimersByTimeAsync(60 * 1000);

    expect(fetchMock).toHaveBeenCalledTimes(1);
    expect(configureCloudsyncToken).toHaveBeenCalledTimes(1);
    expect(suspendCloudsync).toHaveBeenCalledTimes(1);
  });

  test("rejects auth when a scheduled renewal finds an account mismatch", async () => {
    const fetchMock = vi.fn(() => Promise.resolve(credentialsResponse()));
    const rejectAccountMismatch = vi.fn(() => Promise.resolve());
    vi.stubGlobal("fetch", fetchMock);
    vi.mocked(configureCloudsyncToken)
      .mockResolvedValueOnce("configured")
      .mockResolvedValueOnce("account_mismatch");
    vi.spyOn(console, "warn").mockImplementation(() => {});

    await handleCloudsyncAuthChange(
      "INITIAL_SESSION",
      session(),
      rejectAccountMismatch,
    );
    expect(rejectAccountMismatch).not.toHaveBeenCalled();

    await vi.advanceTimersByTimeAsync(13 * 60 * 1000);
    await vi.advanceTimersByTimeAsync(60 * 60 * 1000);

    expect(fetchMock).toHaveBeenCalledTimes(2);
    expect(configureCloudsyncToken).toHaveBeenCalledTimes(2);
    expect(rejectAccountMismatch).toHaveBeenCalledTimes(1);
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

  test("suspends sync without deleting local rows before signing out", async () => {
    const fetchMock = vi.fn(() => Promise.resolve(credentialsResponse()));
    vi.stubGlobal("fetch", fetchMock);
    await handleCloudsyncAuthChange("SIGNED_IN", session());
    vi.mocked(suspendCloudsync).mockClear();

    await prepareCloudsyncSignOut(session());
    await vi.advanceTimersByTimeAsync(15 * 60 * 1000);

    expect(suspendCloudsync).toHaveBeenCalledTimes(1);
    expect(fetchMock).toHaveBeenCalledTimes(1);
  });

  test("resumes token refresh when sign-out suspension fails", async () => {
    const fetchMock = vi.fn(() => Promise.resolve(credentialsResponse()));
    vi.stubGlobal("fetch", fetchMock);
    vi.mocked(suspendCloudsync).mockRejectedValueOnce(
      new Error("cloudsync suspension failed"),
    );

    await expect(prepareCloudsyncSignOut(session())).rejects.toThrow(
      "suspension failed",
    );
    await vi.advanceTimersByTimeAsync(1000);

    expect(fetchMock).toHaveBeenCalledTimes(1);
    expect(configureCloudsyncToken).toHaveBeenCalledTimes(1);
  });

  test("fails closed when sign-out suspension fails without a session", async () => {
    const fetchMock = vi.fn(() => Promise.resolve(credentialsResponse()));
    vi.stubGlobal("fetch", fetchMock);
    vi.mocked(suspendCloudsync).mockRejectedValueOnce(
      new Error("cloudsync suspension failed"),
    );

    await expect(prepareCloudsyncSignOut(null)).rejects.toThrow(
      "suspension failed",
    );
    await vi.advanceTimersByTimeAsync(1000);

    expect(fetchMock).not.toHaveBeenCalled();
    expect(configureCloudsyncToken).not.toHaveBeenCalled();
  });

  test("bounds a stalled exchange and retries", async () => {
    const fetchMock = vi.fn(
      (_input: RequestInfo | URL, init?: RequestInit) =>
        new Promise<Response>((_resolve, reject) => {
          init?.signal?.addEventListener("abort", () => {
            reject(new Error("aborted"));
          });
        }),
    );
    vi.stubGlobal("fetch", fetchMock);
    vi.spyOn(console, "warn").mockImplementation(() => {});

    const activation = handleCloudsyncAuthChange("TOKEN_REFRESHED", session());
    await vi.advanceTimersByTimeAsync(10 * 1000);

    await expect(activation).resolves.toBe("ok");
    expect(fetchMock).toHaveBeenCalledTimes(1);

    await vi.advanceTimersByTimeAsync(60 * 1000);
    expect(fetchMock).toHaveBeenCalledTimes(2);
  });

  test("bounds a stalled exchange response body and retries", async () => {
    const fetchMock = vi.fn((_input: RequestInfo | URL, init?: RequestInit) =>
      Promise.resolve({
        ok: true,
        json: () =>
          new Promise((_resolve, reject) => {
            init?.signal?.addEventListener("abort", () => {
              reject(new Error("aborted"));
            });
          }),
      } as Response),
    );
    vi.stubGlobal("fetch", fetchMock);
    vi.spyOn(console, "warn").mockImplementation(() => {});

    const activation = handleCloudsyncAuthChange("TOKEN_REFRESHED", session());
    await vi.advanceTimersByTimeAsync(10 * 1000);

    await expect(activation).resolves.toBe("ok");
    expect(fetchMock).toHaveBeenCalledTimes(1);

    await vi.advanceTimersByTimeAsync(60 * 1000);
    expect(fetchMock).toHaveBeenCalledTimes(2);
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
    expect(suspendCloudsync).toHaveBeenCalledTimes(1);
  });
});
