import type { AuthChangeEvent, Session } from "@supabase/supabase-js";
import { hostname } from "@tauri-apps/plugin-os";
import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";

import {
  bindCloudsyncAccount,
  configureCloudsyncToken,
  execute,
  getCloudsyncStatus,
  getE2eeIdentityStatus,
  suspendCloudsync,
} from "@hypr/plugin-db";
import { commands as fsSyncCommands } from "@hypr/plugin-fs-sync";
import { commands as miscCommands } from "@hypr/plugin-misc";
import { sonnerToast } from "@hypr/ui/components/ui/toast";

import {
  applyCloudsyncPreference,
  bindCloudsyncAccountForAuth,
  getCloudsyncCredentialBlock,
  handleCloudsyncAuthChange,
  prepareCloudsyncSignOut,
} from "./cloudsync";
import {
  startCloudsyncInitialSyncProgress,
  stopCloudsyncInitialSyncProgress,
} from "./cloudsync-progress";

import { getStoredSettingValues } from "~/settings/queries";

vi.mock("./cloudsync-progress", () => ({
  startCloudsyncInitialSyncProgress: vi.fn(),
  stopCloudsyncInitialSyncProgress: vi.fn(),
}));

vi.mock("@hypr/plugin-fs-sync", () => ({
  commands: {
    deleteSessionFolder: vi.fn(() =>
      Promise.resolve({ status: "ok", data: null }),
    ),
  },
}));

vi.mock("~/env", () => ({
  env: {
    VITE_API_URL: "https://api.test",
  },
}));

vi.mock("~/settings/queries", () => ({
  getStoredSettingValues: vi.fn(),
}));

vi.mock("@hypr/plugin-misc", () => ({
  commands: {
    getFingerprint: vi.fn(() =>
      Promise.resolve({ status: "error", error: "unavailable" }),
    ),
  },
}));

vi.mock("@tauri-apps/plugin-os", () => ({
  hostname: vi.fn(() => Promise.resolve(null)),
}));

vi.mock("@hypr/ui/components/ui/toast", () => ({
  sonnerToast: { error: vi.fn() },
}));

const NOW = new Date("2026-07-13T00:00:00Z");
const E2EE_KEY_ID = "abcdefghijklmnopqrstuv";

function session(accessToken = "supabase-token") {
  return {
    access_token: accessToken,
    user: { id: "user-id" },
  } as Session;
}

function witness(accessToken = "supabase-token", workspaceId = "user-id") {
  return {
    endpoint: `https://api.test/sync/e2ee/witness/${workspaceId}`,
    accessToken,
  };
}

function credentialsResponse(workspaceId = "user-id", encryptionVersion = 2) {
  return new Response(
    JSON.stringify({
      encryptionVersion,
      encryptionKeyId: E2EE_KEY_ID,
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

function projectedCredentialsPayload() {
  return {
    encryptionVersion: 2,
    encryptionKeyId: E2EE_KEY_ID,
    databaseId: "database-id",
    token: "sqlite-token",
    expiresAt: new Date(NOW.getTime() + 15 * 60 * 1000).toISOString(),
    workspaceId: "user-id",
    accountUserId: "user-id",
    personalWorkspaceId: "user-id",
    workspaces: [
      {
        id: "user-id",
        ownerUserId: "user-id",
        kind: "personal",
        name: "Personal",
        membershipId: "membership-personal",
        role: "owner",
        membershipCreatedAt: "2026-07-01T01:00:00Z",
        membershipUpdatedAt: "2026-07-16T01:00:00Z",
        createdAt: "2026-07-01T00:00:00Z",
        updatedAt: "2026-07-16T00:00:00Z",
      },
      {
        id: "workspace-shared",
        ownerUserId: "other-user",
        kind: "shared",
        name: "Shared",
        membershipId: "membership-shared",
        role: "member",
        membershipCreatedAt: "2026-07-02T01:00:00Z",
        membershipUpdatedAt: "2026-07-15T01:00:00Z",
        createdAt: "2026-07-02T00:00:00Z",
        updatedAt: "2026-07-15T00:00:00Z",
      },
    ],
  };
}

type ProjectedCredentialsPayload = ReturnType<
  typeof projectedCredentialsPayload
>;

function projectedCredentialsResponse(
  mutate?: (payload: ProjectedCredentialsPayload) => void,
) {
  const payload = projectedCredentialsPayload();
  mutate?.(payload);
  return new Response(JSON.stringify(payload), {
    status: 200,
    headers: { "Content-Type": "application/json" },
  });
}

describe("CloudSync auth lifecycle", () => {
  beforeEach(async () => {
    vi.useFakeTimers();
    vi.setSystemTime(NOW);
    await handleCloudsyncAuthChange("SIGNED_OUT", null);
    vi.clearAllMocks();
    vi.mocked(bindCloudsyncAccount).mockResolvedValue(true);
    vi.mocked(configureCloudsyncToken).mockResolvedValue("configured");
    vi.mocked(execute).mockResolvedValue([]);
    vi.mocked(getE2eeIdentityStatus).mockResolvedValue({
      configured: true,
      keyId: E2EE_KEY_ID,
    });
    vi.mocked(fsSyncCommands.deleteSessionFolder).mockResolvedValue({
      status: "ok",
      data: null,
    });
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
    vi.mocked(getStoredSettingValues).mockResolvedValue({
      values: {},
      hasValues: new Set(),
    });
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

  test("keeps cloud sync suspended when the local preference is off", async () => {
    const fetchMock = vi.fn();
    vi.stubGlobal("fetch", fetchMock);
    vi.mocked(getStoredSettingValues).mockResolvedValue({
      values: { cloud_sync_enabled: false },
      hasValues: new Set(["cloud_sync_enabled"]),
    });

    await applyCloudsyncPreference(session());
    await vi.advanceTimersByTimeAsync(60 * 60 * 1000);

    expect(fetchMock).not.toHaveBeenCalled();
    expect(configureCloudsyncToken).not.toHaveBeenCalled();
    expect(suspendCloudsync).toHaveBeenCalledTimes(1);
  });

  test("does not request cloud credentials before E2EE recovery setup", async () => {
    const fetchMock = vi.fn();
    vi.stubGlobal("fetch", fetchMock);
    vi.mocked(getE2eeIdentityStatus).mockResolvedValue({
      configured: false,
      keyId: null,
    });
    vi.spyOn(console, "warn").mockImplementation(() => {});

    await handleCloudsyncAuthChange("SIGNED_IN", session());
    await vi.advanceTimersByTimeAsync(60 * 60 * 1000);

    expect(fetchMock).not.toHaveBeenCalled();
    expect(configureCloudsyncToken).not.toHaveBeenCalled();
    expect(suspendCloudsync).toHaveBeenCalledTimes(1);
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
          "X-Anarlog-E2EE-Key-Id": E2EE_KEY_ID,
        },
      }),
    );
    expect(configureCloudsyncToken).toHaveBeenCalledWith(
      "database-id",
      "sqlite-token",
      "user-id",
      witness(),
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

  test("passes server workspace metadata to the native projection", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(() => Promise.resolve(projectedCredentialsResponse())),
    );

    await handleCloudsyncAuthChange("SIGNED_IN", session());

    expect(configureCloudsyncToken).toHaveBeenCalledWith(
      "database-id",
      "sqlite-token",
      "user-id",
      witness(),
      {
        accountUserId: "user-id",
        personalWorkspaceId: "user-id",
        workspaces: [
          expect.objectContaining({
            id: "user-id",
            membershipId: "membership-personal",
            role: "owner",
            membershipCreatedAt: "2026-07-01T01:00:00Z",
            membershipUpdatedAt: "2026-07-16T01:00:00Z",
          }),
          expect.objectContaining({
            id: "workspace-shared",
            membershipId: "membership-shared",
            role: "member",
            membershipCreatedAt: "2026-07-02T01:00:00Z",
            membershipUpdatedAt: "2026-07-15T01:00:00Z",
          }),
        ],
      },
    );
  });

  test("deletes queued folders only after native revocation succeeds", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(() => Promise.resolve(projectedCredentialsResponse())),
    );
    vi.mocked(execute)
      .mockResolvedValueOnce([
        { sessionId: "session-shared", workspaceId: "workspace-shared" },
      ])
      .mockResolvedValueOnce([]);

    await handleCloudsyncAuthChange("SIGNED_IN", session());

    expect(fsSyncCommands.deleteSessionFolder).toHaveBeenCalledWith(
      "session-shared",
    );
    expect(execute).toHaveBeenNthCalledWith(
      2,
      expect.stringContaining("DELETE FROM cloudsync_session_evictions"),
      [
        "session-shared",
        "workspace-shared",
        "workspace-shared",
        "session-shared",
      ],
    );
  });

  test("keeps failed folder evictions queued for retry", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(() => Promise.resolve(projectedCredentialsResponse())),
    );
    vi.mocked(execute)
      .mockResolvedValueOnce([
        { sessionId: "session-shared", workspaceId: "workspace-shared" },
      ])
      .mockResolvedValueOnce([]);
    vi.mocked(fsSyncCommands.deleteSessionFolder).mockResolvedValueOnce({
      status: "error",
      error: "folder busy",
    });

    await handleCloudsyncAuthChange("SIGNED_IN", session());

    expect(execute).toHaveBeenNthCalledWith(
      2,
      expect.stringContaining("UPDATE cloudsync_session_evictions"),
      ["folder busy", "session-shared", "workspace-shared"],
    );
    expect(execute).toHaveBeenCalledTimes(2);

    await vi.advanceTimersByTimeAsync(30 * 1000);
    expect(execute).toHaveBeenCalledTimes(3);
  });

  test("rejects partial workspace metadata instead of treating it as legacy", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(() =>
        Promise.resolve(
          new Response(
            JSON.stringify({
              encryptionVersion: 2,
              encryptionKeyId: E2EE_KEY_ID,
              databaseId: "database-id",
              token: "sqlite-token",
              expiresAt: new Date(NOW.getTime() + 15 * 60 * 1000).toISOString(),
              workspaceId: "user-id",
              accountUserId: "user-id",
            }),
            { status: 200 },
          ),
        ),
      ),
    );
    vi.spyOn(console, "warn").mockImplementation(() => {});

    await handleCloudsyncAuthChange("SIGNED_IN", session());

    expect(configureCloudsyncToken).not.toHaveBeenCalled();
  });

  test("rejects credentials from the pre-witness encryption protocol", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(() => Promise.resolve(credentialsResponse("user-id", 1))),
    );
    vi.spyOn(console, "warn").mockImplementation(() => {});

    await handleCloudsyncAuthChange("SIGNED_IN", session());

    expect(configureCloudsyncToken).not.toHaveBeenCalled();
  });

  test.each([
    [
      "an unknown role",
      (payload: ProjectedCredentialsPayload) => {
        payload.workspaces[1]!.role = "viewer";
      },
    ],
    [
      "an unknown workspace kind",
      (payload: ProjectedCredentialsPayload) => {
        payload.workspaces[1]!.kind = "team";
      },
    ],
    [
      "multiple personal workspaces",
      (payload: ProjectedCredentialsPayload) => {
        payload.workspaces[1]!.kind = "personal";
      },
    ],
    [
      "zero personal workspaces",
      (payload: ProjectedCredentialsPayload) => {
        payload.workspaces[0]!.kind = "shared";
      },
    ],
    [
      "duplicate workspace IDs",
      (payload: ProjectedCredentialsPayload) => {
        payload.workspaces[1]!.id = payload.workspaces[0]!.id;
      },
    ],
    [
      "duplicate membership IDs",
      (payload: ProjectedCredentialsPayload) => {
        payload.workspaces[1]!.membershipId =
          payload.workspaces[0]!.membershipId;
      },
    ],
    [
      "an invalid membership timestamp",
      (payload: ProjectedCredentialsPayload) => {
        payload.workspaces[1]!.membershipCreatedAt = "not-a-timestamp";
      },
    ],
  ])("rejects projected credentials with %s", async (_label, mutate) => {
    vi.stubGlobal(
      "fetch",
      vi.fn(() => Promise.resolve(projectedCredentialsResponse(mutate))),
    );
    vi.spyOn(console, "warn").mockImplementation(() => {});

    await handleCloudsyncAuthChange("SIGNED_IN", session());

    expect(configureCloudsyncToken).not.toHaveBeenCalled();
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
    for (let index = 0; index < 20 && !resolveFetch; index += 1) {
      await Promise.resolve();
    }
    expect(resolveFetch).toBeDefined();
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
      witness(),
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
        witness(),
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
    expect(fetchMock).toHaveBeenCalledTimes(1);
    expect(configureCloudsyncToken).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(15 * 1000);

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
    expect(fetchMock).toHaveBeenCalledTimes(1);
    expect(configureCloudsyncToken).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(15 * 1000);

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
      witness(),
    );
    expect(suspendCloudsync).toHaveBeenCalledTimes(1);
  });

  // Keep this test last: a successful fingerprint lookup is cached at module
  // level and would add device headers to every later exchange in this file.
  test("sends the device identity and surfaces the device limit rejection", async () => {
    vi.mocked(miscCommands.getFingerprint).mockResolvedValue({
      status: "ok",
      data: "device-fingerprint-1",
    });
    vi.mocked(hostname).mockResolvedValue("Johns-M4-Max");
    const fetchMock = vi.fn(() =>
      Promise.resolve(
        new Response(
          JSON.stringify({
            error: {
              code: "sync_device_limit_reached",
              message: "Cloud sync is limited to 5 devices per account",
            },
          }),
          { status: 403, headers: { "Content-Type": "application/json" } },
        ),
      ),
    );
    vi.stubGlobal("fetch", fetchMock);
    vi.spyOn(console, "warn").mockImplementation(() => {});

    await expect(
      handleCloudsyncAuthChange("TOKEN_REFRESHED", session()),
    ).resolves.toBe("ok");

    expect(fetchMock).toHaveBeenCalledWith(
      new URL("https://api.test/sync/token"),
      expect.objectContaining({
        headers: expect.objectContaining({
          "x-device-fingerprint": "device-fingerprint-1",
          "x-anarlog-device-name": "Johns-M4-Max",
        }),
      }),
    );
    expect(configureCloudsyncToken).not.toHaveBeenCalled();
    expect(suspendCloudsync).toHaveBeenCalledTimes(1);
    expect(sonnerToast.error).toHaveBeenCalledWith(
      expect.stringContaining("limited to 5 devices"),
      expect.objectContaining({ id: "cloudsync-device-limit" }),
    );
    expect(getCloudsyncCredentialBlock()).toBe("device_limit");

    await vi.advanceTimersByTimeAsync(10 * 60 * 1000);
    expect(fetchMock).toHaveBeenCalledTimes(1);

    vi.stubGlobal(
      "fetch",
      vi.fn(() => Promise.resolve(credentialsResponse())),
    );
    await expect(
      handleCloudsyncAuthChange("TOKEN_REFRESHED", session()),
    ).resolves.toBe("ok");
    expect(getCloudsyncCredentialBlock()).toBeNull();
  });
});
