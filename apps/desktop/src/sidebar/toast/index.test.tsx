import { act, cleanup, render } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  signIn: vi.fn(),
  dismissToast: vi.fn(),
  openNew: vi.fn(),
  updateSettingsTabState: vi.fn(),
  clearDevtoolsPreview: vi.fn(),
  setToastActionTarget: vi.fn(),
  message: vi.fn(),
  error: vi.fn(),
  warning: vi.fn(),
  loading: vi.fn(),
  dismiss: vi.fn(),
  dismissedToastIds: new Set<string>(),
  sessionMode: "inactive",
  live: {
    status: "inactive" as "inactive" | "active" | "finalizing",
    sessionId: null as string | null,
  },
  currentTab: {
    type: "empty",
  } as {
    type: string;
    id?: string;
    state?: { tab?: string; view?: { type: string } };
  },
  config: {
    current_llm_provider: "local" as string | null,
    current_llm_model: "model" as string | null,
    current_stt_provider: "local" as string | null,
    current_stt_model: "model" as string | null,
  },
  notifications: {
    hasActiveDownload: false,
    downloadingModel: null as string | null,
    activeDownloads: [] as Array<{
      model: string;
      displayName: string;
      progress: number;
    }>,
    localSttStatus: null as null | "loading" | "unreachable",
    isLocalSttModel: false,
  },
  update: {
    status: null as null | "available" | "downloading" | "ready" | "failed",
    version: null as string | null,
    progress: null as number | null,
    errorMessage: null as string | null,
    downloadStarting: false,
    installing: false,
    downloadUpdate: vi.fn(),
    installUpdate: vi.fn(),
  },
}));

vi.mock("@anlg/ui/components/ui/toast", () => ({
  sonnerToast: {
    message: mocks.message,
    error: mocks.error,
    warning: mocks.warning,
    loading: mocks.loading,
    dismiss: mocks.dismiss,
  },
}));

vi.mock("~/auth", () => ({
  useAuth: () => ({ session: null, signIn: mocks.signIn }),
}));

vi.mock("~/auth/cloudsync-progress", () => ({
  useCloudsyncInitialSyncProgress: () => ({ state: "idle" }),
}));

vi.mock("~/contexts/notifications", () => ({
  useNotifications: () => mocks.notifications,
}));

vi.mock("~/main/update-banner", () => ({
  useDesktopUpdateControl: () => mocks.update,
}));

vi.mock("~/shared/config", () => ({
  useConfigValues: () => mocks.config,
}));

vi.mock("~/store/zustand/devtools-toast-preview", () => ({
  useDevtoolsToastPreview: (
    selector: (state: { preview: null; clearPreview: () => void }) => unknown,
  ) =>
    selector({
      preview: null,
      clearPreview: mocks.clearDevtoolsPreview,
    }),
}));

vi.mock("~/store/zustand/tabs", () => ({
  useTabs: (
    selector: (state: {
      currentTab: typeof mocks.currentTab;
      openNew: () => void;
      updateSettingsTabState: () => void;
    }) => unknown,
  ) =>
    selector({
      currentTab: mocks.currentTab,
      openNew: mocks.openNew,
      updateSettingsTabState: mocks.updateSettingsTabState,
    }),
}));

vi.mock("~/store/zustand/toast-action", () => ({
  useToastAction: (
    selector: (state: { setTarget: (target: "stt" | null) => void }) => unknown,
  ) => selector({ setTarget: mocks.setToastActionTarget }),
}));

vi.mock("~/stt/capabilities", () => ({
  isConfiguredSttModel: () => true,
  isAnarlogCloudSttModel: () => false,
}));

vi.mock("~/stt/contexts", () => ({
  useListener: (
    selector: (state: {
      getSessionMode: () => string;
      live: typeof mocks.live;
    }) => unknown,
  ) => selector({ getSessionMode: () => mocks.sessionMode, live: mocks.live }),
}));

vi.mock("./useDismissedToasts", () => ({
  useDismissedToasts: () => ({
    dismissToast: mocks.dismissToast,
    isDismissed: (id: string) => mocks.dismissedToastIds.has(id),
  }),
}));

import { ToastNotifications } from "./index";

const storedValues = new Map<string, string>();
const localStorageMock = {
  getItem: (key: string) => storedValues.get(key) ?? null,
  setItem: (key: string, value: string) => storedValues.set(key, value),
  removeItem: (key: string) => storedValues.delete(key),
  clear: () => storedValues.clear(),
};

describe("ToastNotifications", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.stubGlobal("localStorage", localStorageMock);
    mocks.signIn.mockClear();
    mocks.dismissToast.mockClear();
    mocks.message.mockClear();
    mocks.error.mockClear();
    mocks.warning.mockClear();
    mocks.loading.mockClear();
    mocks.dismiss.mockClear();
    mocks.dismissedToastIds.clear();
    localStorage.clear();
    mocks.live = { status: "inactive", sessionId: null };
    mocks.openNew.mockClear();
    mocks.updateSettingsTabState.mockClear();
    mocks.currentTab = { type: "empty" };
    mocks.config.current_llm_provider = "local";
    mocks.config.current_llm_model = "model";
    mocks.config.current_stt_provider = "local";
    mocks.config.current_stt_model = "model";
    mocks.notifications.hasActiveDownload = false;
    mocks.notifications.downloadingModel = null;
    mocks.notifications.activeDownloads = [];
    mocks.notifications.localSttStatus = null;
    mocks.notifications.isLocalSttModel = false;
    mocks.update.status = null;
    mocks.update.version = null;
    mocks.update.progress = null;
    mocks.update.errorMessage = null;
    mocks.update.downloadStarting = false;
    mocks.update.installing = false;
    mocks.update.downloadUpdate.mockClear();
    mocks.update.installUpdate.mockClear();
  });

  afterEach(() => {
    cleanup();
    vi.unstubAllGlobals();
    vi.useRealTimers();
  });

  it("routes the sign-in suggestion through Sonner", () => {
    render(<ToastNotifications />);

    act(() => vi.advanceTimersByTime(500));

    expect(mocks.message).toHaveBeenCalledWith(
      "Sign in to get the most out of Anarlog",
      expect.objectContaining({
        id: "sign-in-benefits",
        duration: Infinity,
        closeButton: true,
        action: expect.objectContaining({ label: "Sign in" }),
      }),
    );

    const options = mocks.message.mock.calls[0][1];
    options.action.onClick();
    expect(mocks.signIn).toHaveBeenCalledOnce();

    options.onDismiss();
    expect(mocks.dismissToast).not.toHaveBeenCalled();
  });

  it("persists explicit Sonner dismissals", () => {
    render(<ToastNotifications />);

    act(() => vi.advanceTimersByTime(500));

    const options = mocks.message.mock.calls[0][1];
    options.onDismiss();
    expect(mocks.dismissToast).toHaveBeenCalledWith("auth-promotion");
  });

  it("uses a Sonner loading toast for model downloads", () => {
    mocks.notifications.hasActiveDownload = true;
    mocks.notifications.downloadingModel = "Parakeet v3";
    mocks.notifications.activeDownloads = [
      { model: "am-parakeet-v3", displayName: "Parakeet v3", progress: 42 },
    ];

    render(<ToastNotifications />);

    act(() => vi.advanceTimersByTime(500));

    expect(mocks.loading).toHaveBeenCalledWith(
      "Downloading Parakeet v3",
      expect.objectContaining({
        id: "downloading-model",
        duration: Infinity,
        closeButton: false,
      }),
    );
  });

  it("uses the latest registry action while a toast remains visible", () => {
    mocks.dismissedToastIds.add("auth-promotion");
    mocks.config.current_llm_provider = null;
    mocks.config.current_llm_model = null;

    const view = render(<ToastNotifications />);

    act(() => vi.advanceTimersByTime(500));

    const options = mocks.message.mock.calls[0][1];

    mocks.currentTab = { type: "settings", state: { tab: "general" } };
    view.rerender(<ToastNotifications />);

    options.action.onClick();

    expect(mocks.updateSettingsTabState).toHaveBeenCalledWith(
      mocks.currentTab,
      { tab: "intelligence" },
    );
    expect(mocks.openNew).not.toHaveBeenCalled();
  });

  it("snoozes dismissed available updates for one day", () => {
    mocks.update.status = "available";
    mocks.update.version = "1.0.34";

    const view = render(<ToastNotifications />);
    act(() => vi.advanceTimersByTime(500));

    const firstOptions = mocks.message.mock.calls[0][1];
    expect(mocks.message).toHaveBeenCalledWith(
      "Anarlog 1.0.34 is available",
      expect.objectContaining({
        id: "desktop-update:1.0.34:available",
        closeButton: true,
      }),
    );

    act(() => firstOptions.onDismiss());
    expect(mocks.dismissToast).not.toHaveBeenCalled();

    mocks.message.mockClear();
    view.rerender(<ToastNotifications />);
    expect(mocks.message).not.toHaveBeenCalledWith(
      "Anarlog 1.0.34 is available",
      expect.anything(),
    );
  });

  it("keeps an available update snoozed across relaunches", () => {
    mocks.update.status = "available";
    mocks.update.version = "1.0.34";

    const firstLaunch = render(<ToastNotifications />);
    act(() => vi.advanceTimersByTime(500));

    const firstOptions = mocks.message.mock.calls[0][1];
    act(() => firstOptions.onDismiss());

    firstLaunch.unmount();
    mocks.message.mockClear();
    render(<ToastNotifications />);
    act(() => vi.advanceTimersByTime(500));

    expect(mocks.message).not.toHaveBeenCalledWith(
      "Anarlog 1.0.34 is available",
      expect.anything(),
    );
  });

  it("resurfaces an available update after its one-day snooze expires", () => {
    vi.setSystemTime(new Date("2026-08-09T00:00:00Z"));
    mocks.update.status = "available";
    mocks.update.version = "1.0.34";

    const firstLaunch = render(<ToastNotifications />);
    act(() => vi.advanceTimersByTime(500));
    act(() => mocks.message.mock.calls[0][1].onDismiss());
    firstLaunch.unmount();

    vi.setSystemTime(new Date("2026-08-10T00:00:00.001Z"));
    mocks.message.mockClear();
    render(<ToastNotifications />);
    act(() => vi.advanceTimersByTime(500));

    expect(mocks.message).toHaveBeenCalledWith(
      "Anarlog 1.0.34 is available",
      expect.objectContaining({ id: "desktop-update:1.0.34:available" }),
    );
  });

  it("resurfaces a dismissed ready update after relaunch", () => {
    mocks.update.status = "ready";
    mocks.update.version = "1.0.34";

    const firstLaunch = render(<ToastNotifications />);
    act(() => vi.advanceTimersByTime(500));
    act(() => mocks.message.mock.calls[0][1].onDismiss());
    firstLaunch.unmount();

    mocks.message.mockClear();
    render(<ToastNotifications />);
    act(() => vi.advanceTimersByTime(500));

    expect(mocks.message).toHaveBeenCalledWith(
      "Anarlog 1.0.34 is ready to install",
      expect.objectContaining({ id: "desktop-update:1.0.34:ready" }),
    );
  });

  it("resurfaces a dismissed failed update after another download attempt", () => {
    mocks.update.status = "failed";
    mocks.update.version = "1.0.34";

    const view = render(<ToastNotifications />);
    act(() => vi.advanceTimersByTime(500));
    act(() => mocks.error.mock.calls[0][1].onDismiss());

    mocks.update.status = "downloading";
    view.rerender(<ToastNotifications />);
    mocks.update.status = "failed";
    mocks.error.mockClear();
    view.rerender(<ToastNotifications />);

    expect(mocks.error).toHaveBeenCalledWith(
      "The update download failed",
      expect.objectContaining({ id: "desktop-update:1.0.34:failed" }),
    );
  });

  it("hides the update notice while a meeting is recording and resurfaces it after", () => {
    mocks.update.status = "available";
    mocks.update.version = "1.0.34";

    const view = render(<ToastNotifications />);
    act(() => vi.advanceTimersByTime(500));

    expect(mocks.message).toHaveBeenCalledWith(
      "Anarlog 1.0.34 is available",
      expect.objectContaining({ id: "desktop-update:1.0.34:available" }),
    );

    mocks.live = { status: "active", sessionId: "meeting-1" };
    view.rerender(<ToastNotifications />);
    expect(mocks.dismiss).toHaveBeenCalledWith(
      "desktop-update:1.0.34:available",
    );

    mocks.message.mockClear();
    mocks.live = { status: "inactive", sessionId: null };
    view.rerender(<ToastNotifications />);

    expect(mocks.message).toHaveBeenCalledWith(
      "Anarlog 1.0.34 is available",
      expect.objectContaining({ id: "desktop-update:1.0.34:available" }),
    );
  });

  it("keeps a dismissed available update snoozed after a meeting ends", () => {
    mocks.update.status = "available";
    mocks.update.version = "1.0.34";

    const view = render(<ToastNotifications />);
    act(() => vi.advanceTimersByTime(500));

    const firstOptions = mocks.message.mock.calls[0][1];
    act(() => firstOptions.onDismiss());

    mocks.message.mockClear();
    mocks.live = { status: "active", sessionId: "meeting-1" };
    view.rerender(<ToastNotifications />);
    expect(mocks.message).not.toHaveBeenCalledWith(
      "Anarlog 1.0.34 is available",
      expect.anything(),
    );

    mocks.live = { status: "inactive", sessionId: null };
    view.rerender(<ToastNotifications />);

    expect(mocks.message).not.toHaveBeenCalledWith(
      "Anarlog 1.0.34 is available",
      expect.anything(),
    );
  });
});
