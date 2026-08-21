import {
  focusManager,
  QueryClient,
  QueryClientProvider,
} from "@tanstack/react-query";
import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
} from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  getCloudsyncStatus: vi.fn(),
  syncCloudsyncNow: vi.fn(),
  applyCloudsyncPreference: vi.fn(),
  setSettingValue: vi.fn(),
  openNew: vi.fn(),
  signOut: vi.fn(),
  billing: { isPro: true, isReady: true },
  settings: { ready: true, cloudSyncEnabled: true },
  session: { user: { id: "user-1" } },
  credentialBlock: null as string | null,
}));

vi.mock("@anlg/plugin-db", () => ({
  getCloudsyncStatus: mocks.getCloudsyncStatus,
  syncCloudsyncNow: mocks.syncCloudsyncNow,
}));

vi.mock("~/auth", () => ({
  useAuth: () => ({ session: mocks.session, signOut: mocks.signOut }),
}));

vi.mock("~/auth/billing-context", () => ({
  useBillingAccess: () => ({
    isPro: mocks.billing.isPro,
    isReady: mocks.billing.isReady,
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
    activity_paused: false,
    last_sync: null,
    last_sync_at_ms: Date.now() - 60_000,
    has_unsent_changes: false,
    last_error: null,
    last_error_kind: null,
    consecutive_failures: 0,
    deferred_for_capture: false,
    ...overrides,
  };
}

function renderIndicator() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });

  const result = render(
    <QueryClientProvider client={queryClient}>
      <SyncStatusIndicator />
    </QueryClientProvider>,
  );
  return { ...result, queryClient };
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
    mocks.session.user.id = "user-1";
    mocks.credentialBlock = null;
    mocks.getCloudsyncStatus.mockResolvedValue(syncedStatus());
    mocks.applyCloudsyncPreference.mockResolvedValue("ok");
    mocks.setSettingValue.mockResolvedValue(undefined);
    mocks.syncCloudsyncNow.mockResolvedValue({});
  });

  afterEach(() => {
    cleanup();
    focusManager.setFocused(undefined);
    vi.restoreAllMocks();
  });

  it("shows synced state with sync now and pause actions for pro users", async () => {
    renderIndicator();
    await openMenu();

    expect(await screen.findByText("Synced")).toBeTruthy();
    expect(screen.getByText(/Last synced/)).toBeTruthy();
    expect(screen.getByText("Sync now")).toBeTruthy();
    expect(screen.getByText("Pause sync")).toBeTruthy();
    expect(screen.queryByText("Upgrade to Pro")).toBeNull();
    expect(
      screen
        .getByLabelText("Cloud sync status: Synced")
        .querySelector(".text-emerald-500"),
    ).toBeTruthy();
  });

  it("renders below the chat FAB layers", async () => {
    renderIndicator();

    const indicator = await screen.findByTestId("sync-status-indicator");

    expect(indicator.className).toContain("z-10");
    expect(indicator.className).not.toContain("z-40");
  });

  it("does not render for free users", () => {
    mocks.billing.isPro = false;

    renderIndicator();

    expect(screen.queryByTestId("sync-status-indicator")).toBeNull();
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

  it("stays visible when sync is paused and offers resume", async () => {
    mocks.settings.cloudSyncEnabled = false;

    renderIndicator();
    await openMenu();

    expect(await screen.findByText("Sync paused")).toBeTruthy();
    expect(screen.getByText("Resume sync")).toBeTruthy();
    expect(mocks.getCloudsyncStatus).not.toHaveBeenCalled();

    fireEvent.click(screen.getByText("Resume sync"));

    await vi.waitFor(() => {
      expect(mocks.setSettingValue).toHaveBeenCalledWith(
        "cloud_sync_enabled",
        true,
      );
    });
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

  it("distinguishes device approval from first-device setup", async () => {
    mocks.credentialBlock = "approval_pending";
    mocks.getCloudsyncStatus.mockResolvedValue(
      syncedStatus({ configured: false, running: false }),
    );

    renderIndicator();
    await openMenu();

    expect(await screen.findByText("Waiting for device approval")).toBeTruthy();
    expect(
      screen.getByText(
        "Open Anarlog on a device that already has access, then approve this device.",
      ),
    ).toBeTruthy();
    expect(screen.queryByText("Cloud sync setup required")).toBeNull();
  });

  it.each([
    [
      "setup_required",
      "Cloud sync setup required",
      "Create or enter your recovery key in Sync settings to start syncing.",
    ],
    [
      "unavailable",
      "Cloud sync unavailable",
      "Cloud sync could not start on this device. Open Sync settings to try again.",
    ],
    [
      "keychain_access",
      "Sync needs attention",
      "macOS could not access your recovery key. Repair Keychain access, then resume sync.",
    ],
    [
      "reauth_required",
      "Sign in again",
      "Sign out and sign in again to resume cloud sync.",
    ],
    [
      "not_entitled",
      "Anarlog Pro required",
      "Anarlog Pro is required to use cloud sync.",
    ],
    [
      "identity_mismatch",
      "Cloud sync identity mismatch",
      "This device's sync identity does not match your account. Sign in again or check Sync settings.",
    ],
  ])(
    "shows %s as an actionable error instead of a connecting spinner",
    async (block, label, description) => {
      mocks.credentialBlock = block;
      mocks.getCloudsyncStatus.mockResolvedValue(
        syncedStatus({ configured: false, running: false }),
      );

      renderIndicator();
      await openMenu();

      expect(await screen.findByText(label)).toBeTruthy();
      expect(screen.getByText(description)).toBeTruthy();
      expect(screen.queryByText("Connecting...")).toBeNull();
      expect(screen.getByText("Sync settings")).toBeTruthy();
    },
  );

  it("shows a retryable error when local sync status cannot be read", async () => {
    mocks.getCloudsyncStatus
      .mockRejectedValueOnce(new Error("status unavailable"))
      .mockResolvedValue(syncedStatus());

    renderIndicator();
    await openMenu();

    expect(await screen.findByText("Sync status unavailable")).toBeTruthy();
    expect(
      screen.getByText(
        "Anarlog couldn't read cloud sync status. Your notes are still available locally.",
      ),
    ).toBeTruthy();
    fireEvent.click(screen.getByText("Retry"));

    await vi.waitFor(() => {
      expect(mocks.getCloudsyncStatus).toHaveBeenCalledTimes(2);
    });
    expect(mocks.syncCloudsyncNow).not.toHaveBeenCalled();
  });

  it("keeps cached sync status scoped to the signed-in account", async () => {
    const { queryClient, rerender } = renderIndicator();
    await screen.findByLabelText("Cloud sync status: Synced");

    mocks.session.user.id = "user-2";
    rerender(
      <QueryClientProvider client={queryClient}>
        <SyncStatusIndicator />
      </QueryClientProvider>,
    );

    await vi.waitFor(() => {
      expect(mocks.getCloudsyncStatus).toHaveBeenCalledTimes(2);
    });
    expect(
      queryClient
        .getQueryCache()
        .findAll({ queryKey: ["cloudsync-status-indicator"] })
        .map((query) => query.queryKey),
    ).toEqual([
      ["cloudsync-status-indicator", "user-1"],
      ["cloudsync-status-indicator", "user-2"],
    ]);
  });

  it("keeps status current while the app is backgrounded", async () => {
    let pollStatus: (() => void) | undefined;
    vi.spyOn(globalThis, "setInterval").mockImplementation(
      (handler: TimerHandler, delay?: number) => {
        if (delay === 10_000 && typeof handler === "function") {
          pollStatus = () => handler();
        }
        return 1 as unknown as ReturnType<typeof setInterval>;
      },
    );
    focusManager.setFocused(false);
    mocks.getCloudsyncStatus
      .mockResolvedValueOnce(
        syncedStatus({
          configured: false,
          running: false,
          last_sync_at_ms: null,
          has_unsent_changes: null,
        }),
      )
      .mockResolvedValueOnce(
        syncedStatus({
          activity_paused: true,
          deferred_for_capture: true,
          has_unsent_changes: true,
        }),
      )
      .mockResolvedValueOnce(syncedStatus());

    renderIndicator();
    expect(
      await screen.findByLabelText("Cloud sync status: Connecting..."),
    ).toBeTruthy();
    expect(mocks.getCloudsyncStatus).toHaveBeenCalledTimes(1);
    expect(pollStatus).toBeDefined();

    await act(async () => {
      pollStatus?.();
      await Promise.resolve();
    });
    await vi.waitFor(() => {
      expect(screen.queryByTestId("sync-status-indicator")).toBeNull();
    });

    await act(async () => {
      pollStatus?.();
      await Promise.resolve();
    });
    expect(
      await screen.findByLabelText("Cloud sync status: Synced"),
    ).toBeTruthy();
    expect(mocks.getCloudsyncStatus).toHaveBeenCalledTimes(3);
  });

  it("shows a sign-in error without exposing the server message", async () => {
    mocks.getCloudsyncStatus.mockResolvedValue(
      syncedStatus({
        running: false,
        last_error: "token rejected",
        last_error_kind: "auth",
        consecutive_failures: 2,
        has_unsent_changes: null,
        deferred_for_capture: true,
      }),
    );

    renderIndicator();
    await openMenu();

    expect(await screen.findByText("Sign in again")).toBeTruthy();
    expect(
      screen.getByText("Sign out and sign in again to resume cloud sync."),
    ).toBeTruthy();
    expect(screen.queryByText("token rejected")).toBeNull();
  });

  it("hides capture deferral instead of showing a transient sync issue during recording", async () => {
    mocks.getCloudsyncStatus.mockResolvedValue(
      syncedStatus({
        activity_paused: true,
        deferred_for_capture: true,
        has_unsent_changes: true,
        last_error: "connection reset",
        last_error_kind: "transient",
        consecutive_failures: 1,
      }),
    );

    renderIndicator();
    await vi.waitFor(() => {
      expect(screen.queryByTestId("sync-status-indicator")).toBeNull();
    });
  });

  it("uses the last successful sync while native local status is busy", async () => {
    mocks.getCloudsyncStatus.mockResolvedValue(
      syncedStatus({ has_unsent_changes: null }),
    );

    renderIndicator();
    await openMenu();

    expect(await screen.findByText("Synced")).toBeTruthy();
    expect(screen.queryByText("Syncing...")).toBeNull();
  });

  it("does not report synced before the initial sync completes", async () => {
    mocks.getCloudsyncStatus.mockResolvedValue(
      syncedStatus({
        has_unsent_changes: null,
        last_sync_at_ms: null,
      }),
    );

    renderIndicator();
    await openMenu();

    expect(await screen.findByText("Syncing...")).toBeTruthy();
    expect(screen.queryByText("Synced")).toBeNull();
  });

  it("reports known outbound work after a prior successful sync", async () => {
    mocks.getCloudsyncStatus.mockResolvedValue(
      syncedStatus({ has_unsent_changes: true }),
    );

    renderIndicator();
    await openMenu();

    expect(await screen.findByText("Syncing...")).toBeTruthy();
    expect(screen.queryByText("Synced")).toBeNull();
    expect(
      screen
        .getByLabelText("Cloud sync status: Syncing...")
        .querySelector(".animate-spin.text-blue-500"),
    ).toBeTruthy();
  });

  it("hides capture-deferred work", async () => {
    mocks.getCloudsyncStatus.mockResolvedValue(
      syncedStatus({
        activity_paused: true,
        deferred_for_capture: true,
        has_unsent_changes: true,
      }),
    );

    renderIndicator();
    await vi.waitFor(() => {
      expect(screen.queryByTestId("sync-status-indicator")).toBeNull();
    });
  });

  it("shows other paused activity as saved locally without capture-specific copy", async () => {
    mocks.getCloudsyncStatus.mockResolvedValue(
      syncedStatus({
        activity_paused: true,
        has_unsent_changes: true,
      }),
    );

    renderIndicator();
    await openMenu();

    expect(await screen.findByText("Saved locally")).toBeTruthy();
    expect(
      screen.getByText("Cloud sync resumes when the current activity finishes"),
    ).toBeTruthy();
    expect(
      screen.queryByText(
        "Cloud sync resumes after this meeting finishes processing",
      ),
    ).toBeNull();
    expect(
      screen
        .getByLabelText("Cloud sync status: Saved locally")
        .querySelector(".animate-spin"),
    ).toBeNull();

    const syncNowItem = screen.getByRole("menuitem", { name: "Sync now" });
    expect(syncNowItem.getAttribute("data-disabled")).not.toBeNull();
    fireEvent.click(syncNowItem);
    expect(mocks.syncCloudsyncNow).not.toHaveBeenCalled();
  });

  it("hides capture deferral while recovery is paused during recording", async () => {
    mocks.getCloudsyncStatus.mockResolvedValue(
      syncedStatus({
        running: false,
        last_sync_at_ms: null,
        has_unsent_changes: null,
        recovery_pending: true,
        recovery_delayed: true,
        recovery_phase: "need_clean_receive",
        activity_paused: true,
        deferred_for_capture: true,
      }),
    );

    renderIndicator();
    await vi.waitFor(() => {
      expect(screen.queryByTestId("sync-status-indicator")).toBeNull();
    });
  });

  it("explains background recovery without implying local notes are blocked", async () => {
    mocks.getCloudsyncStatus.mockResolvedValue(
      syncedStatus({
        running: false,
        last_sync_at_ms: null,
        has_unsent_changes: null,
        recovery_pending: true,
        recovery_phase: "need_clean_receive",
      }),
    );

    renderIndicator();
    await openMenu();

    expect(await screen.findByText("Restoring cloud sync...")).toBeTruthy();
    expect(
      screen.getByText(
        "Anarlog is repairing cloud sync in the background. Your notes remain available locally.",
      ),
    ).toBeTruthy();
    expect(screen.queryByText("Connecting...")).toBeNull();
    expect(screen.getByText("Sync settings")).toBeTruthy();
  });

  it("shows a non-spinning delayed state after a recovery failure", async () => {
    mocks.getCloudsyncStatus.mockResolvedValue(
      syncedStatus({
        running: false,
        last_sync_at_ms: null,
        recovery_pending: true,
        recovery_delayed: true,
        recovery_phase: "need_clean_receive",
      }),
    );

    renderIndicator();
    await openMenu();

    expect(await screen.findByText("Cloud sync delayed")).toBeTruthy();
    expect(
      screen.getByText(
        "Anarlog will keep retrying in the background. Your notes remain available locally.",
      ),
    ).toBeTruthy();
    expect(
      screen
        .getByLabelText("Cloud sync status: Cloud sync delayed")
        .querySelector(".animate-spin"),
    ).toBeNull();
  });

  it("keeps a startup handshake miss as syncing instead of a sync issue", async () => {
    mocks.getCloudsyncStatus.mockResolvedValue(
      syncedStatus({
        last_error:
          'sqlx error: error returned from database: (code: 1) {"errors": [{"status":"404","code":"not_found","title":"Not Found","detail":"managed database not found"}]}',
        last_error_kind: "fatal",
        consecutive_failures: 1,
        last_sync_at_ms: null,
        has_unsent_changes: null,
      }),
    );

    renderIndicator();
    await openMenu();

    expect(await screen.findByText("Syncing...")).toBeTruthy();
    expect(screen.queryByText("Sync issue")).toBeNull();
    expect(screen.queryByText(/managed database not found/)).toBeNull();
    expect(screen.queryByText(/sqlx error/)).toBeNull();
  });

  it("does not surface a single retryable miss after a successful sync", async () => {
    mocks.getCloudsyncStatus.mockResolvedValue(
      syncedStatus({
        last_error:
          'sqlx error: {"errors":[{"status":"409","code":"already_exists"}]}',
        last_error_kind: "transient",
        consecutive_failures: 1,
      }),
    );

    renderIndicator();
    await openMenu();

    expect(await screen.findByText("Synced")).toBeTruthy();
    expect(screen.queryByText("Sync issue")).toBeNull();
    expect(screen.queryByText(/already_exists/)).toBeNull();
  });

  it("surfaces a stopped sync with a user-facing message instead of the server error", async () => {
    mocks.getCloudsyncStatus.mockResolvedValue(
      syncedStatus({
        running: false,
        last_error:
          'sqlx error: error returned from database: (code: 1) {"errors": [{"status":"404","code":"not_found","detail":"managed database not found"}]}',
        last_error_kind: "fatal",
        consecutive_failures: 1,
        last_sync_at_ms: null,
        has_unsent_changes: null,
      }),
    );

    renderIndicator();
    await openMenu();

    expect(await screen.findByText("Sync issue")).toBeTruthy();
    expect(
      screen.getByText(
        "Cloud sync could not start on this device. Open Sync settings to try again.",
      ),
    ).toBeTruthy();
    expect(screen.queryByText(/sqlx error/)).toBeNull();
  });

  it("makes a persistent transient sync issue retryable without exposing the server error", async () => {
    mocks.getCloudsyncStatus.mockResolvedValue(
      syncedStatus({
        last_error:
          'sqlx error: {"errors":[{"status":"409","code":"already_exists"}]}',
        last_error_kind: "transient",
        consecutive_failures: 3,
      }),
    );

    renderIndicator();
    await openMenu();

    expect(await screen.findByText("Sync issue")).toBeTruthy();
    expect(
      screen.getByText(
        "Anarlog will retry automatically. This does not affect your notes.",
      ),
    ).toBeTruthy();
    expect(screen.queryByText(/already_exists/)).toBeNull();

    fireEvent.click(screen.getByText("Retry"));

    await vi.waitFor(() => {
      expect(mocks.syncCloudsyncNow).toHaveBeenCalledTimes(1);
    });
  });

  it("triggers a manual sync", async () => {
    renderIndicator();
    await openMenu();

    fireEvent.click(await screen.findByText("Sync now"));

    await vi.waitFor(() => {
      expect(mocks.syncCloudsyncNow).toHaveBeenCalledTimes(1);
    });
  });

  it("opens the dedicated sync settings page", async () => {
    renderIndicator();
    await openMenu();

    fireEvent.click(await screen.findByText("Sync settings"));

    expect(mocks.openNew).toHaveBeenCalledWith({
      type: "settings",
      state: { tab: "sync" },
    });
  });
});
