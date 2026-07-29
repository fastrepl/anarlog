import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  getCloudsyncStatus: vi.fn(),
  getE2eeIdentityStatus: vi.fn(),
  syncCloudsyncNow: vi.fn(),
  setSettingValue: vi.fn(),
  applyCloudsyncPreference: vi.fn(),
  vaultBase: vi.fn(),
  openUrl: vi.fn(),
  openNew: vi.fn(),
  signOut: vi.fn(),
  trackAnalyticsEvent: vi.fn(),
  billing: { isPro: true, isReady: true },
  syncEnabled: true,
  session: {
    user: { id: "user-1" },
    access_token: "token",
  } as { user: { id: string }; access_token: string } | null,
}));

vi.mock("@anlg/plugin-db", () => ({
  getCloudsyncStatus: mocks.getCloudsyncStatus,
  getE2eeIdentityStatus: mocks.getE2eeIdentityStatus,
  syncCloudsyncNow: mocks.syncCloudsyncNow,
}));

vi.mock("@anlg/plugin-settings", () => ({
  commands: { vaultBase: mocks.vaultBase },
}));

vi.mock("@anlg/plugin-opener2", () => ({
  commands: { openUrl: mocks.openUrl },
}));

vi.mock("~/auth", () => ({
  useAuth: () => ({ session: mocks.session, signOut: mocks.signOut }),
}));

vi.mock("~/auth/billing-context", () => ({
  useBillingAccess: () => mocks.billing,
}));

vi.mock("~/auth/cloudsync", () => ({
  applyCloudsyncPreference: mocks.applyCloudsyncPreference,
  getCloudsyncCredentialBlock: () => null,
  subscribeCloudsyncCredentialBlock: () => () => {},
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
    mocks.syncEnabled = true;
    mocks.session = {
      user: { id: "user-1" },
      access_token: "token",
    };
    mocks.getE2eeIdentityStatus.mockResolvedValue({ configured: true });
    mocks.getCloudsyncStatus.mockResolvedValue(syncedStatus());
    mocks.vaultBase.mockResolvedValue({
      status: "ok",
      data: "/Users/test/Library/Application Support/anarlog",
    });
    mocks.syncCloudsyncNow.mockResolvedValue({});
    mocks.setSettingValue.mockResolvedValue(undefined);
    mocks.applyCloudsyncPreference.mockResolvedValue("ok");
  });

  afterEach(cleanup);

  it("shows sync status and encryption state", async () => {
    renderSettings();

    expect(await screen.findByText("Synced")).toBeTruthy();
    expect(screen.getByRole("switch", { name: "Cloud sync" })).toBeTruthy();
    expect(screen.getByText("End-to-end encryption")).toBeTruthy();
    expect(screen.getByText(/Anarlog cannot read them/)).toBeTruthy();
    expect(screen.queryByText(/conflicted copies/)).toBeNull();
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
