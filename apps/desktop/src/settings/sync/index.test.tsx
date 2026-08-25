import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  getCloudsyncStatus: vi.fn(),
  getE2eeIdentityStatus: vi.fn(),
  getOrCreateE2eeDeviceIdentity: vi.fn(),
  sealE2eeRecoveryKeyForDevice: vi.fn(),
  syncCloudsyncNow: vi.fn(),
  setSettingValue: vi.fn(),
  applyCloudsyncPreference: vi.fn(),
  refreshCloudsyncForSession: vi.fn(),
  requestSyncDevices: vi.fn(),
  registerDeviceEnrollment: vi.fn(),
  sealDeviceEnrollment: vi.fn(),
  removeSyncDevice: vi.fn(),
  getDeviceIdentity: vi.fn(),
  repairKeychainAccess: vi.fn(),
  vaultBase: vi.fn(),
  openUrl: vi.fn(),
  openNew: vi.fn(),
  signOut: vi.fn(),
  trackAnalyticsEvent: vi.fn(),
  billing: { isPro: true, isReady: true },
  credentialBlock: null as string | null,
  platform: "macos",
  syncEnabled: true,
  session: {
    user: { id: "user-1" },
    access_token: "token",
  } as { user: { id: string }; access_token: string } | null,
}));

vi.mock("@anlg/plugin-db", () => ({
  getCloudsyncStatus: mocks.getCloudsyncStatus,
  getE2eeIdentityStatus: mocks.getE2eeIdentityStatus,
  getOrCreateE2eeDeviceIdentity: mocks.getOrCreateE2eeDeviceIdentity,
  sealE2eeRecoveryKeyForDevice: mocks.sealE2eeRecoveryKeyForDevice,
  syncCloudsyncNow: mocks.syncCloudsyncNow,
}));

vi.mock("@anlg/plugin-settings", () => ({
  commands: { vaultBase: mocks.vaultBase },
}));

vi.mock("@anlg/plugin-store2", () => ({
  commands: { repairKeychainAccess: mocks.repairKeychainAccess },
}));

vi.mock("@anlg/plugin-opener2", () => ({
  commands: { openUrl: mocks.openUrl },
}));

vi.mock("@tauri-apps/plugin-os", () => ({
  platform: () => mocks.platform,
}));

vi.mock("~/auth", () => ({
  useAuth: () => ({ session: mocks.session, signOut: mocks.signOut }),
}));

vi.mock("~/auth/billing-context", () => ({
  useBillingAccess: () => mocks.billing,
}));

vi.mock("~/auth/cloudsync", () => ({
  applyCloudsyncPreference: mocks.applyCloudsyncPreference,
  getCloudsyncCredentialBlock: () => mocks.credentialBlock,
  refreshCloudsyncForSession: mocks.refreshCloudsyncForSession,
  subscribeCloudsyncCredentialBlock: () => () => {},
}));

vi.mock("~/auth/cloudsync-credentials", () => ({
  getDeviceIdentity: mocks.getDeviceIdentity,
}));

vi.mock("~/auth/sync-devices", () => ({
  registerDeviceEnrollment: mocks.registerDeviceEnrollment,
  removeSyncDevice: mocks.removeSyncDevice,
  requestSyncDevices: mocks.requestSyncDevices,
  sealDeviceEnrollment: mocks.sealDeviceEnrollment,
}));

vi.mock("~/analytics", () => ({
  trackAnalyticsEvent: mocks.trackAnalyticsEvent,
}));

vi.mock("~/settings/queries", () => ({
  setSettingValue: mocks.setSettingValue,
  useStoredSettingValuesQuery: () => ({
    data: {
      values: { cloud_sync_enabled: mocks.syncEnabled },
      hasValues: new Set(["cloud_sync_enabled"]),
    },
    isLoading: false,
    error: null,
  }),
}));

vi.mock("~/store/zustand/tabs", () => ({
  useTabs: (selector: (state: { openNew: typeof mocks.openNew }) => unknown) =>
    selector({ openNew: mocks.openNew }),
}));

vi.mock("../general/e2ee-setup", () => ({
  E2eeSetupDialog: () => null,
}));

vi.mock("@lingui/react/macro", () => ({
  Trans: ({ children }: { children?: ReactNode }) => <>{children}</>,
  useLingui: () => ({
    t: (strings: TemplateStringsArray, ...values: unknown[]) =>
      strings.reduce(
        (message, part, index) =>
          `${message}${part}${index < values.length ? String(values[index]) : ""}`,
        "",
      ),
  }),
}));

import { SettingsSync } from ".";

function syncedStatus() {
  return {
    cloudsync_enabled: true,
    extension_loaded: true,
    configured: true,
    running: true,
    network_initialized: true,
    activity_paused: false,
    deferred_for_capture: false,
    last_sync: null,
    last_sync_at_ms: Date.now() - 60_000,
    has_unsent_changes: false,
    last_error: null,
    last_error_kind: null,
    consecutive_failures: 0,
    activity_log: [],
  };
}

function renderSettings() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });

  return {
    ...render(
      <QueryClientProvider client={queryClient}>
        <SettingsSync />
      </QueryClientProvider>,
    ),
    queryClient,
  };
}

describe("SettingsSync", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.billing.isPro = true;
    mocks.billing.isReady = true;
    mocks.credentialBlock = null;
    mocks.platform = "macos";
    mocks.syncEnabled = true;
    mocks.session = {
      user: { id: "user-1" },
      access_token: "token",
    };
    mocks.getE2eeIdentityStatus.mockResolvedValue({ configured: true });
    mocks.getOrCreateE2eeDeviceIdentity.mockResolvedValue({
      publicKey: "A".repeat(43),
    });
    mocks.sealE2eeRecoveryKeyForDevice.mockResolvedValue({
      ephemeralPublicKey: "E".repeat(43),
      nonce: "N".repeat(32),
      ciphertext: "C".repeat(100),
    });
    mocks.requestSyncDevices.mockResolvedValue({
      devices: [],
      pendingDevices: [],
      maxDevices: 5,
    });
    mocks.getDeviceIdentity.mockResolvedValue({
      fingerprint: "current-device",
      name: "Current Mac",
    });
    mocks.registerDeviceEnrollment.mockResolvedValue({
      requestId: "request-id",
      expiresAt: "2026-08-21T00:00:00Z",
      status: "pending",
      package: null,
    });
    mocks.sealDeviceEnrollment.mockResolvedValue(undefined);
    mocks.removeSyncDevice.mockResolvedValue(undefined);
    mocks.refreshCloudsyncForSession.mockResolvedValue("ok");
    mocks.getCloudsyncStatus.mockResolvedValue(syncedStatus());
    mocks.vaultBase.mockResolvedValue({
      status: "ok",
      data: "/Users/test/Library/Application Support/anarlog",
    });
    mocks.syncCloudsyncNow.mockResolvedValue({});
    mocks.setSettingValue.mockResolvedValue(undefined);
    mocks.applyCloudsyncPreference.mockResolvedValue("ok");
    mocks.repairKeychainAccess.mockResolvedValue({
      status: "ok",
      data: null,
    });
  });

  afterEach(cleanup);

  it("shows sync status and encryption state", async () => {
    renderSettings();

    expect(await screen.findByText("Synced")).toBeTruthy();
    expect(screen.getByRole("switch", { name: "Cloud sync" })).toBeTruthy();
    expect(screen.getByText("End-to-end encryption")).toBeTruthy();
    expect(
      screen.getByText(/Keep synced notes readable only on your devices/),
    ).toBeTruthy();
    expect(screen.queryByText(/conflicted copies/)).toBeNull();
  });

  it("retries when the device list cannot load", async () => {
    mocks.requestSyncDevices
      .mockRejectedValueOnce(new Error("offline"))
      .mockResolvedValue({
        devices: [],
        pendingDevices: [],
        maxDevices: 5,
      });
    renderSettings();

    expect(
      await screen.findByText("Could not load your devices."),
    ).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Retry" }));

    await vi.waitFor(() =>
      expect(mocks.requestSyncDevices).toHaveBeenCalledTimes(2),
    );
    expect(await screen.findByText("No devices registered yet.")).toBeTruthy();
  });

  it("approves a pending device without sharing the recovery key", async () => {
    mocks.requestSyncDevices.mockResolvedValue({
      devices: [],
      pendingDevices: [
        {
          requestId: "11111111-1111-4111-8111-111111111111",
          deviceFingerprint: "new-device",
          deviceName: "New Mac",
          publicKey: "A".repeat(43),
          createdAt: "2026-08-20T00:00:00Z",
          expiresAt: "2026-08-21T00:00:00Z",
          status: "pending",
        },
      ],
      maxDevices: 5,
    });
    renderSettings();

    fireEvent.click(await screen.findByRole("button", { name: "Approve" }));

    await vi.waitFor(() =>
      expect(mocks.sealE2eeRecoveryKeyForDevice).toHaveBeenCalledWith(
        "user-1",
        "11111111-1111-4111-8111-111111111111",
        "A".repeat(43),
      ),
    );
    expect(mocks.sealDeviceEnrollment).toHaveBeenCalledWith({
      accessToken: "token",
      requestId: "11111111-1111-4111-8111-111111111111",
      packageValue: {
        ephemeralPublicKey: "E".repeat(43),
        nonce: "N".repeat(32),
        ciphertext: "C".repeat(100),
      },
    });
  });

  it("shows this-device as a chip and disconnects other devices", async () => {
    mocks.requestSyncDevices.mockResolvedValue({
      devices: [
        {
          deviceFingerprint: "current-device",
          deviceName: "Johns-M4-Max.local",
          createdAt: "2026-08-20T00:00:00Z",
          lastSeenAt: "2026-08-20T00:00:00Z",
        },
        {
          deviceFingerprint: "other-device",
          deviceName: null,
          createdAt: "2026-08-18T00:00:00Z",
          lastSeenAt: "2026-08-18T00:00:00Z",
        },
      ],
      pendingDevices: [],
      maxDevices: 5,
    });
    renderSettings();

    expect(await screen.findByText("Johns-M4-Max.local")).toBeTruthy();
    const thisDevice = screen.getByText("This device");
    expect(thisDevice.className).toContain("rounded-full");
    expect(screen.queryByText(/· This device/)).toBeNull();
    expect(screen.queryByRole("button", { name: "Remove device" })).toBeNull();

    const disconnect = screen.getByRole("button", { name: "Disconnect" });
    expect(disconnect.className).toContain("text-destructive");
    expect(disconnect.className).toContain("hover:!bg-destructive/10");
    expect(disconnect.className).toContain("hover:!text-destructive");
    expect(
      document.querySelectorAll("[data-device-kind='desktop']"),
    ).toHaveLength(2);
    fireEvent.click(disconnect);

    await vi.waitFor(() =>
      expect(mocks.removeSyncDevice).toHaveBeenCalledWith(
        "token",
        "other-device",
      ),
    );
  });

  it("shows mobile and watch icons when those device kinds are present", async () => {
    mocks.requestSyncDevices.mockResolvedValue({
      devices: [
        {
          deviceFingerprint: "current-device",
          deviceName: "Johns-M4-Max.local",
          deviceKind: "desktop",
          createdAt: "2026-08-20T00:00:00Z",
          lastSeenAt: "2026-08-20T00:00:00Z",
        },
        {
          deviceFingerprint: "phone-device",
          deviceName: "iPhone",
          deviceKind: "mobile",
          createdAt: "2026-08-19T00:00:00Z",
          lastSeenAt: "2026-08-19T00:00:00Z",
        },
        {
          deviceFingerprint: "watch-device",
          deviceName: "Apple Watch",
          deviceKind: "watch",
          createdAt: "2026-08-18T00:00:00Z",
          lastSeenAt: "2026-08-18T00:00:00Z",
        },
      ],
      pendingDevices: [],
      maxDevices: 5,
    });
    renderSettings();

    expect(await screen.findByText("iPhone")).toBeTruthy();
    expect(screen.getByText("Apple Watch")).toBeTruthy();
    expect(
      document.querySelectorAll("[data-device-kind='desktop']"),
    ).toHaveLength(1);
    expect(
      document.querySelectorAll("[data-device-kind='mobile']"),
    ).toHaveLength(1);
    expect(
      document.querySelectorAll("[data-device-kind='watch']"),
    ).toHaveLength(1);
  });

  it("replaces an active device when the device limit is reached", async () => {
    mocks.credentialBlock = "device_limit";
    mocks.getE2eeIdentityStatus.mockResolvedValue({ configured: false });
    mocks.requestSyncDevices.mockResolvedValue({
      devices: [
        {
          deviceFingerprint: "old-device",
          deviceName: "Old Mac",
          createdAt: "2026-08-01T00:00:00Z",
          lastSeenAt: "2026-08-19T00:00:00Z",
        },
      ],
      pendingDevices: [],
      maxDevices: 5,
    });
    renderSettings();

    expect(await screen.findByText("Device limit reached")).toBeTruthy();
    fireEvent.click(await screen.findByRole("button", { name: "Replace" }));

    await vi.waitFor(() =>
      expect(mocks.registerDeviceEnrollment).toHaveBeenCalledWith({
        accessToken: "token",
        publicKey: "A".repeat(43),
        fingerprint: "current-device",
        deviceName: "Current Mac",
        replaceFingerprint: "old-device",
      }),
    );
    expect(mocks.refreshCloudsyncForSession).toHaveBeenCalledWith(
      mocks.session,
    );
  });

  it("keeps recovery-key import available while approval is pending", async () => {
    mocks.credentialBlock = "approval_pending";
    mocks.getE2eeIdentityStatus.mockResolvedValue({ configured: false });
    renderSettings();

    expect(await screen.findByText("Waiting for device approval")).toBeTruthy();
    expect(
      screen.getByRole("button", { name: "Use recovery key instead" }),
    ).toBeTruthy();
  });

  it("keeps first-device recovery setup actionable", async () => {
    mocks.credentialBlock = "setup_required";
    mocks.getE2eeIdentityStatus.mockResolvedValue({ configured: false });
    renderSettings();

    const syncSwitch = await screen.findByRole("switch", {
      name: "Cloud sync",
    });
    expect(syncSwitch.getAttribute("data-state")).toBe("unchecked");
    await vi.waitFor(() =>
      expect(syncSwitch.hasAttribute("disabled")).toBe(false),
    );
    fireEvent.click(syncSwitch);

    await vi.waitFor(() =>
      expect(mocks.setSettingValue).toHaveBeenCalledWith(
        "cloud_sync_enabled",
        true,
      ),
    );
  });

  it("shows recent sync activity on demand", async () => {
    mocks.getCloudsyncStatus.mockResolvedValue({
      ...syncedStatus(),
      activity_log: [
        {
          timestamp_ms: Date.now(),
          trigger: "manual",
          status: "completed",
          sent_bytes: 2048,
          received_bytes: 1024,
          error: null,
        },
        {
          timestamp_ms: Date.now() - 1_000,
          trigger: "background",
          status: "failed",
          sent_bytes: 0,
          received_bytes: 0,
          error:
            "sqlx error: error returned from database: (code: 1) Connection timed out after 5002 milliseconds",
        },
      ],
    });
    renderSettings();

    expect(await screen.findByText("Synced")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "View sync log" }));

    expect(screen.getByText("Manual sync")).toBeTruthy();
    expect(screen.getByText("Sent 2.0 KB · Received 1.0 KB")).toBeTruthy();
    expect(screen.getByText("Background sync")).toBeTruthy();
    expect(
      screen.getByText(
        "Anarlog couldn't complete this sync. Your notes are safe on this device.",
      ),
    ).toBeTruthy();
    expect(screen.queryByText(/sqlx error/)).toBeNull();
    expect(screen.getByRole("button", { name: "Hide sync log" })).toBeTruthy();
  });

  it("warns when the storage location is inside a cloud-synced folder", async () => {
    mocks.vaultBase.mockResolvedValue({
      status: "ok",
      data: "/Users/test/Library/Mobile Documents/iCloud~md~obsidian/Documents/Vault",
    });
    renderSettings();

    expect(await screen.findByText(/storage location is inside/)).toBeTruthy();
    expect(screen.getAllByText(/iCloud Drive/).length).toBeGreaterThan(0);

    fireEvent.click(screen.getByRole("button", { name: "Learn more" }));
    expect(mocks.openUrl).toHaveBeenCalledWith(
      "https://docs.anarlog.so/sync",
      null,
    );
  });

  it("pauses cloud sync from its settings page", async () => {
    renderSettings();

    const syncSwitch = await screen.findByRole("switch", {
      name: "Cloud sync",
    });
    await vi.waitFor(() =>
      expect(syncSwitch.hasAttribute("disabled")).toBe(false),
    );
    fireEvent.click(syncSwitch);

    await vi.waitFor(() => {
      expect(mocks.setSettingValue).toHaveBeenCalledWith(
        "cloud_sync_enabled",
        false,
      );
    });
    expect(mocks.applyCloudsyncPreference).toHaveBeenCalledWith(mocks.session);
  });

  it("repairs macOS Keychain access and retries cloud sync", async () => {
    mocks.credentialBlock = "keychain_access";
    mocks.getE2eeIdentityStatus
      .mockRejectedValueOnce(
        "macOS couldn't access your login Keychain. Use “Repair Keychain Access” below, then try again.",
      )
      .mockResolvedValue({ configured: true });
    renderSettings();

    const repair = await screen.findByRole("button", {
      name: "Repair Keychain Access",
    });
    expect(screen.getByText(/could not access your recovery key/)).toBeTruthy();
    fireEvent.click(repair);

    await vi.waitFor(() =>
      expect(mocks.repairKeychainAccess).toHaveBeenCalledOnce(),
    );
    await vi.waitFor(() =>
      expect(mocks.getE2eeIdentityStatus).toHaveBeenCalledTimes(2),
    );
    await vi.waitFor(() =>
      expect(mocks.setSettingValue).toHaveBeenCalledWith(
        "cloud_sync_enabled",
        true,
      ),
    );
  });

  it("does not offer Keychain repair for generic sync failures", async () => {
    mocks.credentialBlock = "unavailable";
    mocks.getE2eeIdentityStatus.mockRejectedValue(
      "E2EE recovery key read timed out",
    );

    renderSettings();

    expect(await screen.findByText("Sync needs attention")).toBeTruthy();
    expect(
      screen.queryByRole("button", { name: "Repair Keychain Access" }),
    ).toBeNull();
  });

  it("shows native cloud sync preflight errors", async () => {
    mocks.syncEnabled = false;
    mocks.getE2eeIdentityStatus
      .mockResolvedValueOnce({ configured: true })
      .mockRejectedValueOnce("E2EE recovery key read timed out");
    renderSettings();

    const syncSwitch = await screen.findByRole("switch", {
      name: "Cloud sync",
    });
    await vi.waitFor(() =>
      expect(syncSwitch.hasAttribute("disabled")).toBe(false),
    );
    fireEvent.click(syncSwitch);

    expect(
      await screen.findByText("E2EE recovery key read timed out"),
    ).toBeTruthy();
  });

  it("records an account mismatch as a failed preference change", async () => {
    mocks.applyCloudsyncPreference.mockResolvedValue("account_mismatch");
    renderSettings();

    const syncSwitch = await screen.findByRole("switch", {
      name: "Cloud sync",
    });
    await vi.waitFor(() =>
      expect(syncSwitch.hasAttribute("disabled")).toBe(false),
    );
    fireEvent.click(syncSwitch);

    await vi.waitFor(() => expect(mocks.signOut).toHaveBeenCalledOnce());
    expect(mocks.trackAnalyticsEvent).not.toHaveBeenCalledWith(
      "cloud_sync_disabled",
      expect.anything(),
    );
    expect(mocks.trackAnalyticsEvent).toHaveBeenCalledWith(
      "cloud_sync_failed",
      {
        trigger: "disable",
        failure_stage: "preference",
      },
    );
    expect(screen.queryByText("Cloud sync account mismatch.")).toBeNull();
  });

  it("runs an on-demand sync", async () => {
    mocks.getCloudsyncStatus
      .mockResolvedValueOnce({ ...syncedStatus(), last_sync_at_ms: 1 })
      .mockResolvedValue({ ...syncedStatus(), last_sync_at_ms: 2 });
    renderSettings();

    const syncNow = await screen.findByRole("button", { name: "Sync now" });
    await vi.waitFor(() =>
      expect(syncNow.hasAttribute("disabled")).toBe(false),
    );
    fireEvent.click(syncNow);

    await vi.waitFor(() => {
      expect(mocks.syncCloudsyncNow).toHaveBeenCalledTimes(1);
    });
    await vi.waitFor(() => {
      expect(mocks.getCloudsyncStatus).toHaveBeenCalledTimes(2);
    });
    expect(
      mocks.trackAnalyticsEvent.mock.calls.filter(
        ([event]) => event === "cloud_sync_completed",
      ),
    ).toEqual([["cloud_sync_completed", { trigger: "manual" }]]);
  });

  it("does not count a status poll during manual sync as background", async () => {
    let finishSync: (() => void) | undefined;
    mocks.getCloudsyncStatus
      .mockResolvedValueOnce({ ...syncedStatus(), last_sync_at_ms: 1 })
      .mockResolvedValue({ ...syncedStatus(), last_sync_at_ms: 2 });
    mocks.syncCloudsyncNow.mockReturnValueOnce(
      new Promise<void>((resolve) => {
        finishSync = resolve;
      }),
    );
    const { queryClient } = renderSettings();

    const syncNow = await screen.findByRole("button", { name: "Sync now" });
    await vi.waitFor(() =>
      expect(syncNow.hasAttribute("disabled")).toBe(false),
    );
    fireEvent.click(syncNow);
    await vi.waitFor(() =>
      expect(mocks.syncCloudsyncNow).toHaveBeenCalledOnce(),
    );

    await queryClient.refetchQueries({
      queryKey: ["cloudsync-status-settings", "user-1"],
    });
    expect(
      mocks.trackAnalyticsEvent.mock.calls.filter(
        ([event]) => event === "cloud_sync_completed",
      ),
    ).toEqual([]);

    finishSync?.();
    await vi.waitFor(() =>
      expect(
        mocks.trackAnalyticsEvent.mock.calls.filter(
          ([event]) => event === "cloud_sync_completed",
        ),
      ).toEqual([["cloud_sync_completed", { trigger: "manual" }]]),
    );
  });

  it("does not count a failed on-demand sync again as background", async () => {
    mocks.getCloudsyncStatus
      .mockResolvedValueOnce({
        ...syncedStatus(),
        last_sync_at_ms: 1,
        consecutive_failures: 0,
      })
      .mockResolvedValue({
        ...syncedStatus(),
        last_sync_at_ms: 1,
        consecutive_failures: 1,
        last_error_kind: "network",
      });
    mocks.syncCloudsyncNow.mockRejectedValueOnce(new Error("offline"));
    renderSettings();

    const syncNow = await screen.findByRole("button", { name: "Sync now" });
    await vi.waitFor(() =>
      expect(syncNow.hasAttribute("disabled")).toBe(false),
    );
    fireEvent.click(syncNow);

    await vi.waitFor(() => {
      expect(mocks.getCloudsyncStatus).toHaveBeenCalledTimes(2);
    });
    expect(
      mocks.trackAnalyticsEvent.mock.calls.filter(
        ([event]) => event === "cloud_sync_failed",
      ),
    ).toEqual([
      ["cloud_sync_failed", { trigger: "manual", failure_stage: "sync" }],
    ]);
  });

  it("routes free users to account plans", () => {
    mocks.billing.isPro = false;
    renderSettings();

    fireEvent.click(screen.getByRole("button", { name: "View plans" }));

    expect(mocks.openNew).toHaveBeenCalledWith({
      type: "settings",
      state: { tab: "account" },
    });
    expect(mocks.getCloudsyncStatus).not.toHaveBeenCalled();
  });
});
