import type { AuthChangeEvent, Session } from "@supabase/supabase-js";
import { hostname } from "@tauri-apps/plugin-os";
import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";

import {
  bindCloudsyncAccount,
  configureCloudsyncToken,
  configureE2eeReplica,
  execute,
  getCloudsyncStatus,
  getE2eeIdentityStatus,
  suspendCloudsync,
  suspendCloudsyncAfterAuthLoss,
  suspendCloudsyncForSignOut,
} from "@anlg/plugin-db";
import { commands as fsSyncCommands } from "@anlg/plugin-fs-sync";
import { commands as miscCommands } from "@anlg/plugin-misc";
import { sonnerToast } from "@anlg/ui/components/ui/toast";

import {
  applyCloudsyncPreference,
  bindCloudsyncAccountForAuth,
  getCloudsyncCredentialBlock,
  handleCloudsyncAuthChange,
  prepareCloudsyncSignOut,
  refreshCloudsyncForSession,
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

vi.mock("@anlg/plugin-fs-sync", () => ({
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

vi.mock("@anlg/plugin-misc", () => ({
  commands: {
    getFingerprint: vi.fn(() =>
      Promise.resolve({ status: "error", error: "unavailable" }),
    ),
  },
}));

vi.mock("@tauri-apps/plugin-os", () => ({
  hostname: vi.fn(() => Promise.resolve(null)),
}));

vi.mock("@anlg/ui/components/ui/toast", () => ({
  sonnerToast: { error: vi.fn(), dismiss: vi.fn() },
}));

const NOW = new Date("2026-07-13T00:00:00Z");
const E2EE_KEY_ID = "abcdefghijklmnopqrstuv";

function deferred<T = void>() {
  let resolve!: (value: T | PromiseLike<T>) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, resolve, reject };
}

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

function credentialsResponse(
  workspaceId = "user-id",
  encryptionVersion = 2,
  encryptionKeyId = E2EE_KEY_ID,
  token = "sqlite-token",
) {
  return new Response(
    JSON.stringify({
      encryptionVersion,
      encryptionKeyId,
      databaseId: "database-id",
      token,
      expiresAt: new Date(NOW.getTime() + 15 * 60 * 1000).toISOString(),
      workspaceId,
    }),
    {
      status: 200,
      headers: { "Content-Type": "application/json" },
    },
  );
}

function replicaCredentialsResponse() {
  return new Response(
    JSON.stringify({
      transport: "replica",
      encryptionVersion: 2,
      encryptionKeyId: E2EE_KEY_ID,
      expiresAt: new Date(NOW.getTime() + 15 * 60 * 1000).toISOString(),
      workspaceId: "user-id",
      accountUserId: "user-id",
    }),
    {
      status: 200,
      headers: { "Content-Type": "application/json" },
    },
  );
}

function cloudsyncStatus(activityPaused = false) {
  return {
    cloudsync_enabled: true,
    extension_loaded: true,
    configured: false,
    running: false,
    network_initialized: false,
    activity_paused: activityPaused,
    last_sync: null,
    last_sync_at_ms: null,
    has_unsent_changes: null,
    last_error: null,
    last_error_kind: null,
    consecutive_failures: 0,
    deferred_for_capture: activityPaused,
  };
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
    vi.mocked(configureE2eeReplica).mockResolvedValue("configured");
    vi.mocked(execute).mockResolvedValue([]);
    vi.mocked(getE2eeIdentityStatus).mockResolvedValue({
      configured: true,
      keyId: E2EE_KEY_ID,
    });
    vi.mocked(fsSyncCommands.deleteSessionFolder).mockResolvedValue({
      status: "ok",
      data: null,
    });
    vi.mocked(getCloudsyncStatus).mockResolvedValue(cloudsyncStatus());
    vi.mocked(suspendCloudsync).mockResolvedValue(undefined);
    vi.mocked(suspendCloudsyncAfterAuthLoss).mockResolvedValue(undefined);
    vi.mocked(suspendCloudsyncForSignOut).mockResolvedValue(undefined);
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
    expect(getCloudsyncCredentialBlock()).toBeNull();
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
    expect(getCloudsyncCredentialBlock()).toBe("setup_required");
  });

  test("surfaces macOS Keychain access failures separately", async () => {
    const fetchMock = vi.fn();
    vi.stubGlobal("fetch", fetchMock);
    vi.mocked(getE2eeIdentityStatus).mockRejectedValue(
      "macOS couldn't access your login Keychain. Use “Repair Keychain Access” below, then try again.",
    );
    vi.spyOn(console, "warn").mockImplementation(() => {});

    await handleCloudsyncAuthChange("SIGNED_IN", session());
    await vi.advanceTimersByTimeAsync(60 * 60 * 1000);

    expect(fetchMock).not.toHaveBeenCalled();
    expect(configureCloudsyncToken).not.toHaveBeenCalled();
    expect(suspendCloudsync).toHaveBeenCalledTimes(1);
    expect(getE2eeIdentityStatus).toHaveBeenCalledTimes(1);
    expect(getCloudsyncCredentialBlock()).toBe("keychain_access");
  });

  test("polls only native status while CloudSync activity is paused", async () => {
    const fetchMock = vi.fn(() => Promise.resolve(credentialsResponse()));
    vi.stubGlobal("fetch", fetchMock);
    vi.mocked(getCloudsyncStatus)
      .mockResolvedValueOnce(cloudsyncStatus(true))
      .mockResolvedValueOnce(cloudsyncStatus(true))
      .mockResolvedValueOnce(cloudsyncStatus());

    await handleCloudsyncAuthChange("SIGNED_IN", session());

    expect(getCloudsyncStatus).toHaveBeenCalledTimes(1);
    expect(getE2eeIdentityStatus).not.toHaveBeenCalled();
    expect(fetchMock).not.toHaveBeenCalled();
    expect(configureCloudsyncToken).not.toHaveBeenCalled();
    expect(suspendCloudsync).not.toHaveBeenCalled();

    await vi.advanceTimersByTimeAsync(5 * 1000);

    expect(getCloudsyncStatus).toHaveBeenCalledTimes(2);
    expect(getE2eeIdentityStatus).not.toHaveBeenCalled();
    expect(fetchMock).not.toHaveBeenCalled();

    await vi.advanceTimersByTimeAsync(5 * 1000);

    expect(getCloudsyncStatus).toHaveBeenCalledTimes(4);
    expect(getE2eeIdentityStatus).toHaveBeenCalledTimes(1);
    expect(fetchMock).toHaveBeenCalledTimes(1);
    expect(configureCloudsyncToken).toHaveBeenCalledTimes(1);
    expect(suspendCloudsync).toHaveBeenCalledTimes(1);
  });

  test("sign-out cancels a pending activity status retry", async () => {
    const fetchMock = vi.fn(() => Promise.resolve(credentialsResponse()));
    vi.stubGlobal("fetch", fetchMock);
    vi.mocked(getCloudsyncStatus).mockResolvedValue(cloudsyncStatus(true));

    await handleCloudsyncAuthChange("SIGNED_IN", session());
    await handleCloudsyncAuthChange("SIGNED_OUT", null);
    await vi.advanceTimersByTimeAsync(60 * 1000);

    expect(getCloudsyncStatus).toHaveBeenCalledTimes(1);
    expect(getE2eeIdentityStatus).not.toHaveBeenCalled();
    expect(fetchMock).not.toHaveBeenCalled();
    expect(configureCloudsyncToken).not.toHaveBeenCalled();
    expect(suspendCloudsync).not.toHaveBeenCalled();
    expect(suspendCloudsyncAfterAuthLoss).toHaveBeenCalledTimes(1);
    expect(suspendCloudsyncForSignOut).not.toHaveBeenCalled();
  });

  test("retries native activity deferral with fresh credentials", async () => {
    const fetchMock = vi
      .fn<() => Promise<Response>>()
      .mockResolvedValueOnce(
        credentialsResponse("user-id", 2, E2EE_KEY_ID, "stale-token"),
      )
      .mockResolvedValueOnce(
        credentialsResponse("user-id", 2, E2EE_KEY_ID, "fresh-token"),
      );
    vi.stubGlobal("fetch", fetchMock);
    vi.mocked(getCloudsyncStatus)
      .mockResolvedValueOnce(cloudsyncStatus())
      .mockResolvedValueOnce(cloudsyncStatus(true))
      .mockResolvedValueOnce(cloudsyncStatus());
    vi.mocked(configureCloudsyncToken)
      .mockRejectedValueOnce("cloudsync_activity_deferred")
      .mockResolvedValueOnce("configured");

    await refreshCloudsyncForSession(session());

    expect(configureCloudsyncToken).toHaveBeenCalledTimes(1);
    expect(configureCloudsyncToken).toHaveBeenLastCalledWith(
      "database-id",
      "stale-token",
      "user-id",
      witness(),
    );
    expect(suspendCloudsync).not.toHaveBeenCalled();
    expect(getCloudsyncCredentialBlock()).toBeNull();

    await vi.advanceTimersByTimeAsync(5 * 1000);

    expect(getCloudsyncStatus).toHaveBeenCalledTimes(2);
    expect(fetchMock).toHaveBeenCalledTimes(1);
    expect(getE2eeIdentityStatus).toHaveBeenCalledTimes(1);

    await vi.advanceTimersByTimeAsync(5 * 1000);

    expect(getCloudsyncStatus).toHaveBeenCalledTimes(4);
    expect(fetchMock).toHaveBeenCalledTimes(2);
    expect(getE2eeIdentityStatus).toHaveBeenCalledTimes(2);
    expect(configureCloudsyncToken).toHaveBeenCalledTimes(2);
    expect(configureCloudsyncToken).toHaveBeenLastCalledWith(
      "database-id",
      "fresh-token",
      "user-id",
      witness(),
    );
    expect(suspendCloudsync).not.toHaveBeenCalled();
    expect(startCloudsyncInitialSyncProgress).toHaveBeenCalledTimes(1);
  });

  test("preserves sign-in suspension across native activity deferral", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(() => Promise.resolve(credentialsResponse())),
    );
    vi.mocked(getCloudsyncStatus).mockResolvedValue(cloudsyncStatus());
    vi.mocked(configureCloudsyncToken)
      .mockRejectedValueOnce("cloudsync_activity_deferred")
      .mockResolvedValueOnce("configured");

    await handleCloudsyncAuthChange("SIGNED_IN", session());

    expect(suspendCloudsync).toHaveBeenCalledTimes(1);
    expect(configureCloudsyncToken).toHaveBeenCalledTimes(1);

    await vi.advanceTimersByTimeAsync(5 * 1000);

    expect(suspendCloudsync).toHaveBeenCalledTimes(2);
    expect(configureCloudsyncToken).toHaveBeenCalledTimes(2);
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

  test("falls back to the portable encrypted replica transport", async () => {
    const fetchMock = vi
      .fn()
      .mockResolvedValueOnce(new Response(null, { status: 404 }))
      .mockResolvedValueOnce(replicaCredentialsResponse());
    vi.stubGlobal("fetch", fetchMock);

    await handleCloudsyncAuthChange("SIGNED_IN", session());

    expect(fetchMock).toHaveBeenNthCalledWith(
      1,
      new URL("https://api.test/sync/token"),
      expect.objectContaining({ method: "POST" }),
    );
    expect(fetchMock).toHaveBeenNthCalledWith(
      2,
      new URL("https://api.test/sync/replica/credentials"),
      expect.objectContaining({ method: "POST" }),
    );
    expect(configureE2eeReplica).toHaveBeenCalledWith("user-id", witness());
    expect(configureCloudsyncToken).not.toHaveBeenCalled();
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
    await vi.advanceTimersByTimeAsync(0);
    expect(resolveFetch).toBeDefined();
    await handleCloudsyncAuthChange("SIGNED_OUT", null);
    resolveFetch?.(credentialsResponse());
    await activation;

    expect(configureCloudsyncToken).not.toHaveBeenCalled();
    expect(suspendCloudsync).toHaveBeenCalledTimes(1);
    expect(suspendCloudsyncAfterAuthLoss).toHaveBeenCalledTimes(1);
    expect(suspendCloudsyncForSignOut).not.toHaveBeenCalled();
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

    expect(fetchMock).toHaveBeenCalledTimes(2);
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
      if (status === 404) {
        fetchMock.mockResolvedValueOnce(new Response(null, { status: 404 }));
      }
      vi.stubGlobal("fetch", fetchMock);
      vi.spyOn(console, "warn").mockImplementation(() => {});

      await handleCloudsyncAuthChange("INITIAL_SESSION", session());
      await vi.advanceTimersByTimeAsync(13 * 60 * 1000);

      expect(fetchMock).toHaveBeenCalledTimes(status === 404 ? 3 : 2);
      expect(configureCloudsyncToken).toHaveBeenCalledTimes(1);
      expect(suspendCloudsync).toHaveBeenCalledTimes(2);
      expect(getCloudsyncCredentialBlock()).toBe("unavailable");
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
    expect(getCloudsyncCredentialBlock()).toBe("not_entitled");
  });

  test("uses the portable transport directly in local-only mode", async () => {
    const fetchMock = vi.fn(() =>
      Promise.resolve(replicaCredentialsResponse()),
    );
    vi.stubGlobal("fetch", fetchMock);
    vi.mocked(getCloudsyncStatus).mockResolvedValueOnce({
      cloudsync_enabled: false,
      extension_loaded: false,
      configured: false,
      running: false,
      network_initialized: false,
      activity_paused: false,
      last_sync: null,
      last_sync_at_ms: null,
      has_unsent_changes: null,
      last_error: null,
      last_error_kind: null,
      consecutive_failures: 0,
      deferred_for_capture: false,
    });
    vi.spyOn(console, "warn").mockImplementation(() => {});

    await handleCloudsyncAuthChange("INITIAL_SESSION", session());
    expect(getCloudsyncStatus).toHaveBeenCalledTimes(1);
    expect(fetchMock).toHaveBeenCalledTimes(1);
    expect(fetchMock).toHaveBeenCalledWith(
      new URL("https://api.test/sync/replica/credentials"),
      expect.objectContaining({ method: "POST" }),
    );
    expect(configureE2eeReplica).toHaveBeenCalledWith("user-id", witness());
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
    expect(getCloudsyncCredentialBlock()).toBe("reauth_required");
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

  test("bounds a stalled rejection suspension without exchanging again", async () => {
    const stalledCleanup = deferred();
    const fetchMock = vi.fn(() =>
      Promise.resolve(new Response(null, { status: 401 })),
    );
    vi.stubGlobal("fetch", fetchMock);
    vi.mocked(suspendCloudsync)
      .mockReturnValueOnce(stalledCleanup.promise)
      .mockResolvedValueOnce(undefined);
    vi.spyOn(console, "warn").mockImplementation(() => {});

    const activation = refreshCloudsyncForSession(session());
    await vi.advanceTimersByTimeAsync(2_000);

    await expect(activation).resolves.toBe("ok");
    expect(fetchMock).toHaveBeenCalledTimes(1);
    expect(configureCloudsyncToken).not.toHaveBeenCalled();
    expect(suspendCloudsync).toHaveBeenCalledTimes(1);

    await vi.advanceTimersByTimeAsync(60 * 1000);

    expect(fetchMock).toHaveBeenCalledTimes(1);
    expect(configureCloudsyncToken).not.toHaveBeenCalled();
    expect(suspendCloudsync).toHaveBeenCalledTimes(2);

    stalledCleanup.resolve();
    await stalledCleanup.promise;
    await vi.advanceTimersByTimeAsync(0);
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
    expect(suspendCloudsync).not.toHaveBeenCalled();
    expect(suspendCloudsyncAfterAuthLoss).toHaveBeenCalledTimes(1);
    expect(suspendCloudsyncForSignOut).not.toHaveBeenCalled();
    expect(getCloudsyncCredentialBlock()).toBeNull();
  });

  test("surfaces a recovery identity mismatch without configuring sync", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(() =>
        Promise.resolve(
          credentialsResponse("user-id", 2, "zyxwvutsrqponmlkjihgfe"),
        ),
      ),
    );
    vi.spyOn(console, "warn").mockImplementation(() => {});

    await handleCloudsyncAuthChange("SIGNED_IN", session());

    expect(configureCloudsyncToken).not.toHaveBeenCalled();
    expect(suspendCloudsync).toHaveBeenCalledTimes(2);
    expect(getCloudsyncCredentialBlock()).toBe("identity_mismatch");
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
    expect(getCloudsyncCredentialBlock()).toBe("identity_mismatch");
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
    vi.mocked(suspendCloudsyncForSignOut).mockClear();

    await prepareCloudsyncSignOut(session());
    await vi.advanceTimersByTimeAsync(15 * 60 * 1000);

    expect(suspendCloudsync).not.toHaveBeenCalled();
    expect(suspendCloudsyncForSignOut).toHaveBeenCalledTimes(1);
    expect(fetchMock).toHaveBeenCalledTimes(1);
  });

  test("uses the auth-loss suspension for null-session teardown", async () => {
    await handleCloudsyncAuthChange("SIGNED_OUT", null);

    expect(suspendCloudsync).not.toHaveBeenCalled();
    expect(suspendCloudsyncAfterAuthLoss).toHaveBeenCalledTimes(1);
    expect(suspendCloudsyncForSignOut).not.toHaveBeenCalled();
  });

  test("continues null-session teardown when auth-loss suspension fails", async () => {
    vi.mocked(suspendCloudsyncAfterAuthLoss).mockRejectedValueOnce(
      new Error("cloudsync suspension failed"),
    );
    vi.spyOn(console, "warn").mockImplementation(() => {});

    await expect(handleCloudsyncAuthChange("SIGNED_OUT", null)).resolves.toBe(
      "ok",
    );

    expect(console.warn).toHaveBeenCalledWith(
      "[cloudsync] local sync suspension failed",
    );
    expect(suspendCloudsync).toHaveBeenCalledTimes(1);
    expect(suspendCloudsyncAfterAuthLoss).toHaveBeenCalledTimes(1);
    expect(suspendCloudsyncForSignOut).not.toHaveBeenCalled();
  });

  test("does not block sign-out on a stalled cloudsync suspension", async () => {
    let finishSuspension!: () => void;
    const suspension = new Promise<void>((resolve) => {
      finishSuspension = resolve;
    });
    vi.mocked(suspendCloudsyncForSignOut).mockReturnValueOnce(suspension);
    vi.spyOn(console, "warn").mockImplementation(() => {});

    const preparation = prepareCloudsyncSignOut(session());
    await vi.advanceTimersByTimeAsync(1999);

    let completed = false;
    void preparation.then(() => {
      completed = true;
    });
    expect(completed).toBe(false);

    await vi.advanceTimersByTimeAsync(1);
    await preparation;
    expect(console.warn).toHaveBeenCalledWith(
      "[cloudsync] sign-out suspension is finishing in background",
    );

    finishSuspension();
    await suspension;
  });

  test("does not resume token refresh when a timed-out suspension later fails", async () => {
    let failSuspension!: (error: Error) => void;
    const suspension = new Promise<void>((_resolve, reject) => {
      failSuspension = reject;
    });
    const fetchMock = vi.fn(() => Promise.resolve(credentialsResponse()));
    vi.stubGlobal("fetch", fetchMock);
    vi.mocked(suspendCloudsyncForSignOut).mockReturnValueOnce(suspension);
    vi.spyOn(console, "warn").mockImplementation(() => {});

    const preparation = prepareCloudsyncSignOut(session());
    await vi.advanceTimersByTimeAsync(2000);
    await preparation;

    failSuspension(new Error("cloudsync suspension failed"));
    await vi.advanceTimersByTimeAsync(1000);

    expect(fetchMock).not.toHaveBeenCalled();
    expect(configureCloudsyncToken).not.toHaveBeenCalled();
  });

  test("clears a failed cleanup retry after the late teardown rejects", async () => {
    const suspension = deferred();
    const fetchMock = vi.fn(() => Promise.resolve(credentialsResponse()));
    vi.stubGlobal("fetch", fetchMock);
    vi.mocked(suspendCloudsyncAfterAuthLoss).mockReturnValueOnce(
      suspension.promise,
    );
    vi.mocked(suspendCloudsync)
      .mockRejectedValueOnce(new Error("forced cleanup failed"))
      .mockResolvedValueOnce(undefined);
    vi.spyOn(console, "warn").mockImplementation(() => {});

    const teardown = handleCloudsyncAuthChange("SIGNED_OUT", null);
    await vi.advanceTimersByTimeAsync(2_000);
    await expect(teardown).resolves.toBe("ok");
    await vi.advanceTimersByTimeAsync(0);

    expect(suspendCloudsync).toHaveBeenCalledTimes(1);

    suspension.reject(new Error("cloudsync suspension failed"));
    await suspension.promise.catch(() => {});
    await vi.advanceTimersByTimeAsync(0);

    expect(suspendCloudsync).toHaveBeenCalledTimes(2);
    expect(fetchMock).not.toHaveBeenCalled();
    expect(configureCloudsyncToken).not.toHaveBeenCalled();

    await vi.advanceTimersByTimeAsync(60_000);

    expect(suspendCloudsync).toHaveBeenCalledTimes(2);
    expect(fetchMock).not.toHaveBeenCalled();
    expect(configureCloudsyncToken).not.toHaveBeenCalled();
  });

  test("bounds serialized auth teardown and resumes re-login after cleanup", async () => {
    let finishSignOutSuspension!: () => void;
    let finishAuthLossSuspension!: () => void;
    const signOutSuspension = new Promise<void>((resolve) => {
      finishSignOutSuspension = resolve;
    });
    const authLossSuspension = signOutSuspension.then(
      () =>
        new Promise<void>((resolve) => {
          finishAuthLossSuspension = resolve;
        }),
    );
    const fetchMock = vi.fn(() => Promise.resolve(credentialsResponse()));
    vi.stubGlobal("fetch", fetchMock);
    vi.mocked(suspendCloudsyncForSignOut).mockReturnValueOnce(
      signOutSuspension,
    );
    vi.mocked(suspendCloudsyncAfterAuthLoss).mockReturnValueOnce(
      authLossSuspension,
    );
    vi.spyOn(console, "warn").mockImplementation(() => {});

    const preparation = prepareCloudsyncSignOut(session());
    await vi.advanceTimersByTimeAsync(2_000);
    await preparation;

    const teardown = handleCloudsyncAuthChange("SIGNED_OUT", null);
    await vi.advanceTimersByTimeAsync(2_000);
    await expect(teardown).resolves.toBe("ok");

    await expect(
      handleCloudsyncAuthChange("SIGNED_IN", session()),
    ).resolves.toBe("ok");
    expect(fetchMock).not.toHaveBeenCalled();
    expect(configureCloudsyncToken).not.toHaveBeenCalled();

    finishSignOutSuspension();
    await signOutSuspension;
    await Promise.resolve();
    expect(finishAuthLossSuspension).toBeTypeOf("function");
    expect(fetchMock).not.toHaveBeenCalled();

    finishAuthLossSuspension();
    await authLossSuspension;
    await vi.advanceTimersByTimeAsync(999);
    expect(fetchMock).not.toHaveBeenCalled();

    await vi.advanceTimersByTimeAsync(1);
    expect(fetchMock).toHaveBeenCalledTimes(1);
    expect(configureCloudsyncToken).toHaveBeenCalledTimes(1);
  });

  test("ignores a stale teardown-wait timeout after auth is superseded", async () => {
    let finishSuspension!: () => void;
    const suspension = new Promise<void>((resolve) => {
      finishSuspension = resolve;
    });
    const fetchMock = vi.fn(() => Promise.resolve(credentialsResponse()));
    vi.stubGlobal("fetch", fetchMock);
    vi.mocked(suspendCloudsyncAfterAuthLoss).mockReturnValueOnce(suspension);
    vi.spyOn(console, "warn").mockImplementation(() => {});

    const teardown = handleCloudsyncAuthChange("SIGNED_OUT", null);
    await vi.advanceTimersByTimeAsync(2_000);
    await expect(teardown).resolves.toBe("ok");

    await handleCloudsyncAuthChange("SIGNED_IN", session("first-session"));
    await vi.advanceTimersByTimeAsync(1_000);
    await handleCloudsyncAuthChange("SIGNED_IN", session("second-session"));
    await vi.advanceTimersByTimeAsync(1_000);

    await handleCloudsyncAuthChange("SIGNED_IN", session("latest-session"));

    expect(fetchMock).not.toHaveBeenCalled();
    expect(configureCloudsyncToken).not.toHaveBeenCalled();

    finishSuspension();
    await suspension;
    await vi.advanceTimersByTimeAsync(1_000);

    expect(fetchMock).toHaveBeenCalledTimes(1);
    expect(fetchMock).toHaveBeenCalledWith(
      new URL("/sync/token", "https://api.test"),
      expect.objectContaining({
        headers: expect.objectContaining({
          Authorization: "Bearer latest-session",
        }),
      }),
    );
    expect(configureCloudsyncToken).toHaveBeenCalledTimes(1);
  });

  test("retries suspension when a waited teardown rejects", async () => {
    const suspension = deferred();
    const firstRetry = deferred();
    const secondRetry = deferred();
    const fetchMock = vi.fn(() => Promise.resolve(credentialsResponse()));
    vi.stubGlobal("fetch", fetchMock);
    vi.mocked(suspendCloudsyncAfterAuthLoss).mockReturnValueOnce(
      suspension.promise,
    );
    vi.mocked(suspendCloudsync)
      .mockReturnValueOnce(firstRetry.promise)
      .mockReturnValueOnce(secondRetry.promise);
    vi.spyOn(console, "warn").mockImplementation(() => {});

    const teardown = handleCloudsyncAuthChange("SIGNED_OUT", null);
    await vi.advanceTimersByTimeAsync(2_000);
    await expect(teardown).resolves.toBe("ok");

    await handleCloudsyncAuthChange("SIGNED_IN", session());
    suspension.reject(new Error("cloudsync suspension failed"));
    await suspension.promise.catch(() => {});

    await vi.advanceTimersByTimeAsync(999);
    expect(suspendCloudsync).toHaveBeenCalledTimes(1);
    expect(fetchMock).not.toHaveBeenCalled();

    await vi.advanceTimersByTimeAsync(1);
    expect(suspendCloudsync).toHaveBeenCalledTimes(2);
    expect(fetchMock).not.toHaveBeenCalled();

    firstRetry.resolve();
    await firstRetry.promise;
    await vi.advanceTimersByTimeAsync(0);
    expect(fetchMock).not.toHaveBeenCalled();

    secondRetry.resolve();
    await secondRetry.promise;
    await vi.advanceTimersByTimeAsync(0);

    expect(fetchMock).toHaveBeenCalledTimes(1);
    expect(configureCloudsyncToken).toHaveBeenCalledTimes(1);
  });

  test("services failed teardown cleanup before reading native status", async () => {
    let failSuspension!: (error: Error) => void;
    const suspension = new Promise<void>((_resolve, reject) => {
      failSuspension = reject;
    });
    const fetchMock = vi.fn(() => Promise.resolve(credentialsResponse()));
    vi.stubGlobal("fetch", fetchMock);
    vi.mocked(suspendCloudsyncAfterAuthLoss).mockReturnValueOnce(suspension);
    vi.mocked(getCloudsyncStatus).mockRejectedValueOnce(
      new Error("database locked"),
    );
    vi.spyOn(console, "warn").mockImplementation(() => {});

    const teardown = handleCloudsyncAuthChange("SIGNED_OUT", null);
    await vi.advanceTimersByTimeAsync(2_000);
    await expect(teardown).resolves.toBe("ok");

    await handleCloudsyncAuthChange("SIGNED_IN", session());
    failSuspension(new Error("cloudsync suspension failed"));
    await suspension.catch(() => {});
    await vi.advanceTimersByTimeAsync(1_000);

    expect(suspendCloudsync).toHaveBeenCalledTimes(2);
    expect(getCloudsyncStatus).toHaveBeenCalledTimes(1);
    expect(fetchMock).not.toHaveBeenCalled();
    expect(configureCloudsyncToken).not.toHaveBeenCalled();
  });

  test("keeps cleanup required after a timeout and network retry", async () => {
    const suspension = deferred();
    const fetchMock = vi
      .fn<() => Promise<Response>>()
      .mockRejectedValueOnce(new Error("offline"))
      .mockResolvedValueOnce(credentialsResponse());
    vi.stubGlobal("fetch", fetchMock);
    vi.mocked(suspendCloudsyncAfterAuthLoss).mockReturnValueOnce(
      suspension.promise,
    );
    vi.spyOn(console, "warn").mockImplementation(() => {});

    const teardown = handleCloudsyncAuthChange("SIGNED_OUT", null);
    await vi.advanceTimersByTimeAsync(2_000);
    await expect(teardown).resolves.toBe("ok");

    await handleCloudsyncAuthChange("SIGNED_IN", session());
    await vi.advanceTimersByTimeAsync(2_000);
    await vi.advanceTimersByTimeAsync(1_000);

    expect(fetchMock).toHaveBeenCalledTimes(1);
    expect(suspendCloudsync).toHaveBeenCalledTimes(1);

    suspension.reject(new Error("cloudsync suspension failed"));
    await suspension.promise.catch(() => {});
    await vi.advanceTimersByTimeAsync(999);

    expect(suspendCloudsync).toHaveBeenCalledTimes(1);
    expect(fetchMock).toHaveBeenCalledTimes(1);

    await vi.advanceTimersByTimeAsync(1);
    expect(suspendCloudsync).toHaveBeenCalledTimes(2);
    expect(fetchMock).toHaveBeenCalledTimes(2);
    expect(configureCloudsyncToken).toHaveBeenCalledTimes(1);
  });

  test("does not suspend again when a timed-out teardown finishes before reactivation", async () => {
    let finishSuspension!: () => void;
    const suspension = new Promise<void>((resolve) => {
      finishSuspension = resolve;
    });
    const fetchMock = vi.fn(() => Promise.resolve(credentialsResponse()));
    vi.stubGlobal("fetch", fetchMock);
    vi.mocked(suspendCloudsyncAfterAuthLoss).mockReturnValueOnce(suspension);
    vi.spyOn(console, "warn").mockImplementation(() => {});

    const teardown = handleCloudsyncAuthChange("SIGNED_OUT", null);
    await vi.advanceTimersByTimeAsync(2_000);
    await expect(teardown).resolves.toBe("ok");

    await handleCloudsyncAuthChange("SIGNED_IN", session());
    await vi.advanceTimersByTimeAsync(2_000);
    finishSuspension();
    await suspension;
    await vi.advanceTimersByTimeAsync(0);

    await vi.advanceTimersByTimeAsync(999);
    expect(fetchMock).not.toHaveBeenCalled();
    expect(suspendCloudsync).toHaveBeenCalledTimes(1);

    await vi.advanceTimersByTimeAsync(1);
    expect(fetchMock).toHaveBeenCalledTimes(1);
    expect(configureCloudsyncToken).toHaveBeenCalledTimes(1);
    expect(suspendCloudsync).toHaveBeenCalledTimes(1);
  });

  test("bypasses a blocked plugin queue only after cleanup succeeds", async () => {
    const status = deferred<ReturnType<typeof cloudsyncStatus>>();
    vi.mocked(getCloudsyncStatus).mockReturnValueOnce(status.promise);

    const activation = refreshCloudsyncForSession(session());
    await vi.advanceTimersByTimeAsync(0);
    expect(getCloudsyncStatus).toHaveBeenCalledTimes(1);

    await prepareCloudsyncSignOut(session());
    const binding = bindCloudsyncAccountForAuth("user-id");
    await vi.advanceTimersByTimeAsync(0);

    await expect(binding).resolves.toBe(true);
    expect(suspendCloudsync).toHaveBeenCalledTimes(1);
    expect(bindCloudsyncAccount).toHaveBeenCalledTimes(1);
    expect(
      vi.mocked(suspendCloudsync).mock.invocationCallOrder[0],
    ).toBeLessThan(
      vi.mocked(bindCloudsyncAccount).mock.invocationCallOrder[0]!,
    );

    status.resolve(cloudsyncStatus());
    await status.promise;
    await activation;
  });

  test("services required cleanup before reading sync settings", async () => {
    const cleanup = deferred();
    const fetchMock = vi.fn(() => Promise.resolve(credentialsResponse()));
    vi.stubGlobal("fetch", fetchMock);

    await prepareCloudsyncSignOut(session());
    vi.mocked(suspendCloudsync).mockReturnValueOnce(cleanup.promise);

    const activation = refreshCloudsyncForSession(session());
    await vi.advanceTimersByTimeAsync(0);

    expect(suspendCloudsync).toHaveBeenCalledTimes(1);
    expect(getStoredSettingValues).not.toHaveBeenCalled();
    expect(fetchMock).not.toHaveBeenCalled();

    cleanup.resolve();
    await cleanup.promise;
    await activation;

    expect(getStoredSettingValues).toHaveBeenCalledTimes(1);
    expect(fetchMock).toHaveBeenCalledTimes(1);
    expect(configureCloudsyncToken).toHaveBeenCalledTimes(1);
  });

  test("bounds a stalled settings read and retries without piling reads", async () => {
    const settings =
      deferred<Awaited<ReturnType<typeof getStoredSettingValues>>>();
    const fetchMock = vi.fn(() => Promise.resolve(credentialsResponse()));
    vi.stubGlobal("fetch", fetchMock);
    vi.mocked(getStoredSettingValues).mockReturnValueOnce(settings.promise);
    vi.spyOn(console, "warn").mockImplementation(() => {});

    const activation = refreshCloudsyncForSession(session());
    await vi.advanceTimersByTimeAsync(25 * 1000);

    await expect(activation).resolves.toBe("ok");
    expect(getStoredSettingValues).toHaveBeenCalledTimes(1);
    expect(suspendCloudsync).toHaveBeenCalledTimes(1);
    expect(fetchMock).not.toHaveBeenCalled();
    expect(configureCloudsyncToken).not.toHaveBeenCalled();

    await vi.advanceTimersByTimeAsync(60 * 1000);

    expect(getStoredSettingValues).toHaveBeenCalledTimes(1);
    expect(fetchMock).not.toHaveBeenCalled();
    expect(configureCloudsyncToken).not.toHaveBeenCalled();

    settings.resolve({ values: {}, hasValues: new Set() });
    await settings.promise;
    await vi.advanceTimersByTimeAsync(0);

    expect(fetchMock).toHaveBeenCalledTimes(1);
    expect(configureCloudsyncToken).toHaveBeenCalledTimes(1);
  });

  test("serializes an ordinary account bind behind the plugin queue", async () => {
    const status = deferred<ReturnType<typeof cloudsyncStatus>>();
    vi.mocked(getCloudsyncStatus).mockReturnValueOnce(status.promise);

    const activation = refreshCloudsyncForSession(session());
    await vi.advanceTimersByTimeAsync(0);
    expect(getCloudsyncStatus).toHaveBeenCalledTimes(1);

    const binding = bindCloudsyncAccountForAuth("user-id");
    await vi.advanceTimersByTimeAsync(0);
    expect(bindCloudsyncAccount).not.toHaveBeenCalled();

    status.resolve(cloudsyncStatus());
    await status.promise;
    await activation;
    await expect(binding).resolves.toBe(true);

    expect(bindCloudsyncAccount).toHaveBeenCalledTimes(1);
  });

  test("does not run a queued account bind after it is superseded", async () => {
    const status = deferred<ReturnType<typeof cloudsyncStatus>>();
    vi.mocked(getCloudsyncStatus).mockReturnValueOnce(status.promise);

    const activation = refreshCloudsyncForSession(session());
    await vi.advanceTimersByTimeAsync(0);
    expect(getCloudsyncStatus).toHaveBeenCalledTimes(1);

    const binding = bindCloudsyncAccountForAuth("user-id");
    await vi.advanceTimersByTimeAsync(0);
    expect(bindCloudsyncAccount).not.toHaveBeenCalled();

    await handleCloudsyncAuthChange("SIGNED_OUT", null);
    status.resolve(cloudsyncStatus());
    await status.promise;
    await activation;
    await expect(binding).resolves.toBe(true);

    expect(bindCloudsyncAccount).not.toHaveBeenCalled();
  });

  test("suspends a configuration superseded while it is in flight", async () => {
    const configuration = deferred<"configured">();
    const fetchMock = vi.fn(() => Promise.resolve(credentialsResponse()));
    vi.stubGlobal("fetch", fetchMock);
    vi.mocked(configureCloudsyncToken).mockReturnValueOnce(
      configuration.promise,
    );

    const activation = handleCloudsyncAuthChange("SIGNED_IN", session());
    await vi.advanceTimersByTimeAsync(0);
    expect(configureCloudsyncToken).toHaveBeenCalledTimes(1);
    expect(startCloudsyncInitialSyncProgress).not.toHaveBeenCalled();

    await prepareCloudsyncSignOut(session());
    vi.mocked(suspendCloudsync).mockClear();

    configuration.resolve("configured");
    await configuration.promise;
    await vi.advanceTimersByTimeAsync(0);
    await activation;

    expect(suspendCloudsync).toHaveBeenCalledTimes(1);
    expect(startCloudsyncInitialSyncProgress).not.toHaveBeenCalled();
    expect(configureCloudsyncToken).toHaveBeenCalledTimes(1);
  });

  test("services a stale credential cleanup failure for the current signed-out generation", async () => {
    const staleCleanup = deferred();
    const currentCleanup = deferred();
    const fetchMock = vi.fn(() =>
      Promise.resolve(new Response(null, { status: 401 })),
    );
    vi.stubGlobal("fetch", fetchMock);
    vi.mocked(suspendCloudsync)
      .mockReturnValueOnce(staleCleanup.promise)
      .mockReturnValueOnce(currentCleanup.promise);
    vi.spyOn(console, "warn").mockImplementation(() => {});

    const rejectedActivation = refreshCloudsyncForSession(session());
    await vi.advanceTimersByTimeAsync(0);
    expect(fetchMock).toHaveBeenCalledTimes(1);
    expect(suspendCloudsync).toHaveBeenCalledTimes(1);
    expect(configureCloudsyncToken).not.toHaveBeenCalled();

    fetchMock.mockClear();
    await handleCloudsyncAuthChange("SIGNED_OUT", null);
    expect(suspendCloudsyncAfterAuthLoss).toHaveBeenCalledTimes(1);

    staleCleanup.reject(new Error("stale cleanup failed"));
    await staleCleanup.promise.catch(() => {});
    await rejectedActivation;
    await vi.advanceTimersByTimeAsync(0);

    expect(suspendCloudsync).toHaveBeenCalledTimes(2);
    expect(fetchMock).not.toHaveBeenCalled();
    expect(configureCloudsyncToken).not.toHaveBeenCalled();

    currentCleanup.resolve();
    await currentCleanup.promise;
    await vi.advanceTimersByTimeAsync(0);

    expect(suspendCloudsync).toHaveBeenCalledTimes(2);
    expect(fetchMock).not.toHaveBeenCalled();
    expect(configureCloudsyncToken).not.toHaveBeenCalled();
  });

  test("reactivates after a stale cleanup succeeds against a newer session", async () => {
    const staleCleanup = deferred();
    const fetchMock = vi
      .fn<() => Promise<Response>>()
      .mockResolvedValueOnce(new Response(null, { status: 401 }))
      .mockResolvedValueOnce(credentialsResponse())
      .mockResolvedValueOnce(credentialsResponse());
    vi.stubGlobal("fetch", fetchMock);
    vi.mocked(suspendCloudsync)
      .mockReturnValueOnce(staleCleanup.promise)
      .mockResolvedValueOnce(undefined);
    vi.spyOn(console, "warn").mockImplementation(() => {});

    const staleActivation = refreshCloudsyncForSession(session("stale"));
    await vi.advanceTimersByTimeAsync(0);
    expect(suspendCloudsync).toHaveBeenCalledTimes(1);

    await handleCloudsyncAuthChange("SIGNED_IN", session("current"));
    expect(fetchMock).toHaveBeenCalledTimes(2);
    expect(configureCloudsyncToken).toHaveBeenCalledTimes(1);

    staleCleanup.resolve();
    await staleCleanup.promise;
    await staleActivation;
    await vi.advanceTimersByTimeAsync(1_000);

    expect(fetchMock).toHaveBeenCalledTimes(3);
    expect(configureCloudsyncToken).toHaveBeenCalledTimes(2);
  });

  test("bounds a stalled identity read and retries without piling keychain reads", async () => {
    const identity =
      deferred<Awaited<ReturnType<typeof getE2eeIdentityStatus>>>();
    const fetchMock = vi.fn(() => Promise.resolve(credentialsResponse()));
    vi.stubGlobal("fetch", fetchMock);
    vi.mocked(getE2eeIdentityStatus).mockReturnValueOnce(identity.promise);
    vi.spyOn(console, "warn").mockImplementation(() => {});

    const activation = refreshCloudsyncForSession(session());
    await vi.advanceTimersByTimeAsync(25 * 1000);

    await expect(activation).resolves.toBe("ok");
    expect(getE2eeIdentityStatus).toHaveBeenCalledTimes(1);
    expect(suspendCloudsync).toHaveBeenCalledTimes(1);
    expect(fetchMock).not.toHaveBeenCalled();
    expect(configureCloudsyncToken).not.toHaveBeenCalled();

    await vi.advanceTimersByTimeAsync(60 * 1000);

    expect(getE2eeIdentityStatus).toHaveBeenCalledTimes(1);
    expect(fetchMock).not.toHaveBeenCalled();
    expect(configureCloudsyncToken).not.toHaveBeenCalled();

    identity.resolve({ configured: true, keyId: E2EE_KEY_ID });
    await identity.promise;
    await vi.advanceTimersByTimeAsync(0);

    expect(fetchMock).toHaveBeenCalledTimes(1);
    expect(configureCloudsyncToken).toHaveBeenCalledTimes(1);
  });

  test("does not let a stalled identity read poison a re-login bind", async () => {
    const identity =
      deferred<Awaited<ReturnType<typeof getE2eeIdentityStatus>>>();
    const fetchMock = vi.fn(() => Promise.resolve(credentialsResponse()));
    vi.stubGlobal("fetch", fetchMock);
    vi.mocked(getE2eeIdentityStatus).mockReturnValueOnce(identity.promise);
    vi.spyOn(console, "warn").mockImplementation(() => {});

    const activation = refreshCloudsyncForSession(session());
    await vi.advanceTimersByTimeAsync(0);
    expect(getE2eeIdentityStatus).toHaveBeenCalledTimes(1);

    await handleCloudsyncAuthChange("SIGNED_OUT", null);
    await expect(bindCloudsyncAccountForAuth("user-id")).resolves.toBe(true);
    expect(bindCloudsyncAccount).toHaveBeenCalledTimes(1);

    await vi.advanceTimersByTimeAsync(25 * 1000);
    await activation;

    identity.resolve({ configured: true, keyId: E2EE_KEY_ID });
    await identity.promise;
    await vi.advanceTimersByTimeAsync(0);

    expect(fetchMock).not.toHaveBeenCalled();
    expect(configureCloudsyncToken).not.toHaveBeenCalled();
  });

  test("reactivates without waiting forever for a stalled teardown", async () => {
    let finishSuspension!: () => void;
    const suspension = new Promise<void>((resolve) => {
      finishSuspension = resolve;
    });
    const fetchMock = vi.fn(() => Promise.resolve(credentialsResponse()));
    vi.stubGlobal("fetch", fetchMock);
    vi.mocked(suspendCloudsyncAfterAuthLoss).mockReturnValueOnce(suspension);
    vi.spyOn(console, "warn").mockImplementation(() => {});

    const teardown = handleCloudsyncAuthChange("SIGNED_OUT", null);
    await vi.advanceTimersByTimeAsync(2_000);
    await expect(teardown).resolves.toBe("ok");

    await expect(
      handleCloudsyncAuthChange("SIGNED_IN", session()),
    ).resolves.toBe("ok");
    await vi.advanceTimersByTimeAsync(1_999);
    expect(fetchMock).not.toHaveBeenCalled();
    expect(configureCloudsyncToken).not.toHaveBeenCalled();

    await vi.advanceTimersByTimeAsync(1);
    expect(fetchMock).not.toHaveBeenCalled();
    expect(configureCloudsyncToken).not.toHaveBeenCalled();

    await vi.advanceTimersByTimeAsync(1_000);
    expect(fetchMock).toHaveBeenCalledTimes(1);
    expect(configureCloudsyncToken).toHaveBeenCalledTimes(1);
    expect(suspendCloudsync).toHaveBeenCalledTimes(1);

    finishSuspension();
    await suspension;
    await vi.advanceTimersByTimeAsync(1_000);

    expect(fetchMock).toHaveBeenCalledTimes(2);
    expect(configureCloudsyncToken).toHaveBeenCalledTimes(2);
  });

  test("resumes token refresh when sign-out suspension fails", async () => {
    const fetchMock = vi.fn(() => Promise.resolve(credentialsResponse()));
    vi.stubGlobal("fetch", fetchMock);
    vi.mocked(suspendCloudsyncForSignOut).mockRejectedValueOnce(
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
    vi.mocked(suspendCloudsyncForSignOut).mockRejectedValueOnce(
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

  test("bounds stalled device identity lookup and retries", async () => {
    vi.mocked(miscCommands.getFingerprint)
      .mockReturnValueOnce(new Promise(() => {}))
      .mockReturnValueOnce(new Promise(() => {}));
    const fetchMock = vi.fn();
    vi.stubGlobal("fetch", fetchMock);
    vi.spyOn(console, "warn").mockImplementation(() => {});

    const activation = handleCloudsyncAuthChange("TOKEN_REFRESHED", session());
    await vi.advanceTimersByTimeAsync(25 * 1000);

    await expect(activation).resolves.toBe("ok");
    expect(miscCommands.getFingerprint).toHaveBeenCalledTimes(1);
    expect(fetchMock).not.toHaveBeenCalled();
    expect(configureCloudsyncToken).not.toHaveBeenCalled();

    await vi.advanceTimersByTimeAsync(60 * 1000);

    expect(miscCommands.getFingerprint).toHaveBeenCalledTimes(2);
    expect(fetchMock).not.toHaveBeenCalled();
    expect(configureCloudsyncToken).not.toHaveBeenCalled();
  });

  test("fails closed when a 403 response body stalls", async () => {
    const fetchMock = vi.fn((_input: RequestInfo | URL, init?: RequestInit) =>
      Promise.resolve({
        ok: false,
        status: 403,
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

    const activation = refreshCloudsyncForSession(session());
    await vi.advanceTimersByTimeAsync(10 * 1000);
    expect(fetchMock).toHaveBeenCalledTimes(1);
    expect(suspendCloudsync).not.toHaveBeenCalled();
    expect(configureCloudsyncToken).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(15 * 1000);

    await expect(activation).resolves.toBe("ok");
    expect(fetchMock).toHaveBeenCalledTimes(1);
    expect(suspendCloudsync).toHaveBeenCalledTimes(1);
    expect(configureCloudsyncToken).not.toHaveBeenCalled();
    expect(getCloudsyncCredentialBlock()).toBe("not_entitled");
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
