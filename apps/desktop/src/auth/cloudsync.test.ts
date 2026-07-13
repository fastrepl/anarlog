import type { Session } from "@supabase/supabase-js";
import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";

import {
  claimCloudsyncAccount,
  configureCloudsyncToken,
  suspendCloudsync,
} from "@hypr/plugin-db";

import {
  canAdmitCloudsyncAccount,
  handleCloudsyncAuthChange,
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
    vi.mocked(claimCloudsyncAccount).mockResolvedValue(undefined);
  });

  afterEach(async () => {
    await handleCloudsyncAuthChange("SIGNED_OUT", null);
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
    vi.useRealTimers();
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

    await handleCloudsyncAuthChange("SIGNED_IN", session());

    expect(suspendCloudsync).toHaveBeenCalledTimes(2);
  });

  test("rejects the account when the local database is bound elsewhere", async () => {
    vi.mocked(claimCloudsyncAccount).mockRejectedValueOnce(
      new Error("this local database is already bound to a different account"),
    );

    await expect(canAdmitCloudsyncAccount("different-user")).resolves.toBe(
      false,
    );

    expect(claimCloudsyncAccount).toHaveBeenCalledWith("different-user");
  });

  test("claims and admits the account without requiring the network", async () => {
    await expect(canAdmitCloudsyncAccount("user-id")).resolves.toBe(true);

    expect(claimCloudsyncAccount).toHaveBeenCalledWith("user-id");
  });

  test("fails closed when the local workspace binding cannot be claimed", async () => {
    vi.mocked(claimCloudsyncAccount).mockRejectedValueOnce(
      new Error("database unavailable"),
    );
    vi.spyOn(console, "warn").mockImplementation(() => {});

    await expect(canAdmitCloudsyncAccount("user-id")).resolves.toBe(false);
  });

  test("reports the durable account mismatch without retrying", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(() => Promise.resolve(credentialsResponse())),
    );
    vi.mocked(configureCloudsyncToken).mockRejectedValueOnce(
      new Error("this local database is already bound to a different account"),
    );
    vi.spyOn(console, "warn").mockImplementation(() => {});

    await expect(
      handleCloudsyncAuthChange("SIGNED_IN", session()),
    ).resolves.toBe("account_mismatch");
    await vi.advanceTimersByTimeAsync(60 * 1000);

    expect(configureCloudsyncToken).toHaveBeenCalledTimes(1);
    expect(suspendCloudsync).toHaveBeenCalledTimes(2);
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
