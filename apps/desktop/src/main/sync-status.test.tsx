import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  getCloudsyncStatus: vi.fn(),
  getE2eeIdentityStatus: vi.fn(),
  syncCloudsyncNow: vi.fn(),
  applyCloudsyncPreference: vi.fn(),
  setSettingValue: vi.fn(),
  upgradeToPro: vi.fn(),
  openNew: vi.fn(),
  signOut: vi.fn(),
  billing: { isPro: true, isReady: true },
  settings: { ready: true, cloudSyncEnabled: true },
  session: { user: { id: "user-1" } },
  credentialBlock: null as string | null,
}));

vi.mock("@hypr/plugin-db", () => ({
  getCloudsyncStatus: mocks.getCloudsyncStatus,
  getE2eeIdentityStatus: mocks.getE2eeIdentityStatus,
  syncCloudsyncNow: mocks.syncCloudsyncNow,
}));

vi.mock("~/auth", () => ({
  useAuth: () => ({ session: mocks.session, signOut: mocks.signOut }),
}));

vi.mock("~/auth/billing-context", () => ({
  useBillingAccess: () => ({
    isPro: mocks.billing.isPro,
    isReady: mocks.billing.isReady,
    upgradeToPro: mocks.upgradeToPro,
  }),
}));

vi.mock("~/auth/cloudsync", () => ({
  applyCloudsyncPreference: mocks.applyCloudsyncPreference,
  getCloudsyncCredentialBlock: () => mocks.credentialBlock,
  subscribeCloudsyncCredentialBlock: () => () => {},
}));

vi.mock("~/settings/queries", () => ({
  setSettingValue: mocks.setSettingValue,
  useSettingsReady: () => mocks.settings.ready,
  useStoredSettingValues: () => ({
    values: { cloud_sync_enabled: mocks.settings.cloudSyncEnabled },
    hasValues: new Set(["cloud_sync_enabled"]),
  }),
}));

vi.mock("~/shared/config", () => ({
  resolveConfigValue: (
    key: string,
    stored: { values: Record<string, unknown> },
  ) => stored.values[key],
}));

vi.mock("~/store/zustand/tabs", () => ({
  useTabs: (selector: (state: { openNew: typeof mocks.openNew }) => unknown) =>
    selector({ openNew: mocks.openNew }),
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

import { SyncStatusIndicator } from "./sync-status";

function syncedStatus(overrides: Record<string, unknown> = {}) {
  return {
    cloudsync_enabled: true,
    extension_loaded: true,
    configured: true,
    running: true,
    network_initialized: true,
    last_sync: null,
    last_sync_at_ms: Date.now() - 60_000,
    has_unsent_changes: false,
    last_error: null,
    last_error_kind: null,
    consecutive_failures: 0,
    ...overrides,
  };
}

function renderIndicator() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });

  return render(
    <QueryClientProvider client={queryClient}>
      <SyncStatusIndicator />
    </QueryClientProvider>,
  );
}

async function openMenu() {
  const trigger = await screen.findByTestId("sync-status-indicator");
  fireEvent.pointerDown(trigger);
  fireEvent.click(trigger);
}

describe("SyncStatusIndicator", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.billing.isPro = true;
    mocks.billing.isReady = true;
    mocks.settings.ready = true;
    mocks.settings.cloudSyncEnabled = true;
    mocks.credentialBlock = null;
    mocks.getCloudsyncStatus.mockResolvedValue(syncedStatus());
    mocks.getE2eeIdentityStatus.mockResolvedValue({
      configured: true,
      keyId: "key",
    });
    mocks.applyCloudsyncPreference.mockResolvedValue("ok");
    mocks.setSettingValue.mockResolvedValue(undefined);
    mocks.syncCloudsyncNow.mockResolvedValue({});
  });

  afterEach(() => {
    cleanup();
  });

  it("shows synced state with sync now and pause actions for pro users", async () => {
    renderIndicator();
    await openMenu();

    expect(await screen.findByText("Synced")).toBeTruthy();
    expect(screen.getByText(/Last synced/)).toBeTruthy();
    expect(screen.getByText("Sync now")).toBeTruthy();
    expect(screen.getByText("Pause sync")).toBeTruthy();
    expect(screen.queryByText("Upgrade to Pro")).toBeNull();
  });

  it("shows an upsell instead of sync controls for free users", async () => {
    mocks.billing.isPro = false;

    renderIndicator();
    await openMenu();

    expect(await screen.findByText("Available with Anarlog Pro")).toBeTruthy();
    expect(screen.getByText("Upgrade to Pro")).toBeTruthy();
    expect(screen.queryByText("Sync now")).toBeNull();
    expect(screen.queryByText("Pause sync")).toBeNull();
    expect(mocks.getCloudsyncStatus).not.toHaveBeenCalled();
  });

  it("pauses sync through the stored preference", async () => {
    renderIndicator();
    await openMenu();

    fireEvent.click(await screen.findByText("Pause sync"));

    await vi.waitFor(() => {
      expect(mocks.setSettingValue).toHaveBeenCalledWith(
        "cloud_sync_enabled",
        false,
      );
    });
    expect(mocks.applyCloudsyncPreference).toHaveBeenCalledWith(mocks.session);
  });

  it("offers resume when sync is paused", async () => {
    mocks.settings.cloudSyncEnabled = false;

    renderIndicator();
    await openMenu();

    expect(await screen.findByText("Sync paused")).toBeTruthy();
    fireEvent.click(screen.getByText("Resume sync"));

    await vi.waitFor(() => {
      expect(mocks.setSettingValue).toHaveBeenCalledWith(
        "cloud_sync_enabled",
        true,
      );
    });
  });

  it("routes resume to sync settings when E2EE is not set up", async () => {
    mocks.settings.cloudSyncEnabled = false;
    mocks.getE2eeIdentityStatus.mockResolvedValue({
      configured: false,
      keyId: null,
    });

    renderIndicator();
    await openMenu();
    fireEvent.click(await screen.findByText("Resume sync"));

    await vi.waitFor(() => {
      expect(mocks.openNew).toHaveBeenCalledWith({
        type: "settings",
        state: { tab: "app" },
      });
    });
    expect(mocks.setSettingValue).not.toHaveBeenCalled();
  });

  it("shows a blocked state when the device limit was hit instead of connecting forever", async () => {
    mocks.credentialBlock = "device_limit";
    mocks.getCloudsyncStatus.mockResolvedValue(
      syncedStatus({ configured: false, running: false }),
    );

    renderIndicator();
    await openMenu();

    expect(await screen.findByText("Device limit reached")).toBeTruthy();
    expect(
      screen.getByText(
        "This account already syncs on 5 devices. Remove another device to sync here.",
      ),
    ).toBeTruthy();
    expect(screen.queryByText("Connecting...")).toBeNull();
  });

  it("shows a sync issue with the last error", async () => {
    mocks.getCloudsyncStatus.mockResolvedValue(
      syncedStatus({
        last_error: "token rejected",
        last_error_kind: "auth",
        consecutive_failures: 2,
      }),
    );

    renderIndicator();
    await openMenu();

    expect(await screen.findByText("Sync issue")).toBeTruthy();
    expect(screen.getByText("token rejected")).toBeTruthy();
  });

  it("triggers a manual sync", async () => {
    renderIndicator();
    await openMenu();

    fireEvent.click(await screen.findByText("Sync now"));

    await vi.waitFor(() => {
      expect(mocks.syncCloudsyncNow).toHaveBeenCalledTimes(1);
    });
  });
});
