import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  settingsReady: true,
  lockAppEnabled: true,
  authenticateDevice: vi.fn(),
  isDeviceAuthAvailable: vi.fn(),
  getCurrentWebviewWindowLabel: vi.fn(() => "main"),
  visibilityListen: vi.fn(),
}));

vi.mock("@lingui/react/macro", () => ({
  Trans: ({ children }: { children?: unknown }) => children,
  useLingui: () => ({
    t: (strings: TemplateStringsArray) => strings[0],
  }),
}));

vi.mock("@tauri-apps/plugin-os", () => ({
  platform: () => "macos",
}));

vi.mock("@anlg/plugin-windows", () => ({
  events: {
    visibilityEvent: {
      listen: mocks.visibilityListen,
    },
  },
  getCurrentWebviewWindowLabel: mocks.getCurrentWebviewWindowLabel,
}));

vi.mock("~/settings/queries", () => ({
  useSettingsReady: () => mocks.settingsReady,
}));

vi.mock("~/shared/config", () => ({
  useConfigValue: (key: string) =>
    key === "lock_app" ? mocks.lockAppEnabled : undefined,
}));

vi.mock("./auth", () => ({
  DEVICE_AUTH_REASON: {
    openApp: "open",
    lockNote: "lock this note",
    unlockNote: "unlock this note",
    changeLockSettings: "change lock settings",
  },
  authenticateDevice: mocks.authenticateDevice,
  isDeviceAuthAvailable: mocks.isDeviceAuthAvailable,
}));

import { AppLockGate } from "./gate";
import { useAppLock } from "./store";

type VisibilityPayload = {
  window: { type: string };
  visible: boolean;
};

function getVisibilityHandler() {
  const calls = mocks.visibilityListen.mock.calls;
  const listener = calls[calls.length - 1]?.[0] as
    | ((event: { payload: VisibilityPayload }) => void)
    | undefined;
  if (!listener) {
    throw new Error("visibility listener was not registered");
  }
  return listener;
}

async function renderLockedGate() {
  render(
    <AppLockGate>
      <div>app content</div>
    </AppLockGate>,
  );

  await waitFor(() => {
    expect(mocks.authenticateDevice).toHaveBeenCalledWith("open");
    expect(useAppLock.getState().appUnlocked).toBe(true);
    expect(screen.getByText("app content")).toBeTruthy();
  });
}

describe("AppLockGate", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.settingsReady = true;
    mocks.lockAppEnabled = true;
    mocks.getCurrentWebviewWindowLabel.mockReturnValue("main");
    mocks.isDeviceAuthAvailable.mockResolvedValue(true);
    mocks.authenticateDevice.mockResolvedValue(true);
    mocks.visibilityListen.mockResolvedValue(vi.fn());
    useAppLock.setState({
      available: true,
      authenticating: false,
      appUnlocked: false,
      revealedNoteIds: {},
    });
  });

  afterEach(cleanup);

  it("does not lock or prompt when the window stays open", async () => {
    await renderLockedGate();

    expect(mocks.authenticateDevice).toHaveBeenCalledTimes(1);
    expect(screen.queryByText("Anarlog is Locked")).toBeNull();
  });

  it("locks on main window close without prompting", async () => {
    await renderLockedGate();
    const emit = getVisibilityHandler();

    await act(async () => {
      emit({ payload: { window: { type: "main" }, visible: false } });
    });

    expect(useAppLock.getState().appUnlocked).toBe(false);
    expect(mocks.authenticateDevice).toHaveBeenCalledTimes(1);
    expect(screen.getByText("Anarlog is Locked")).toBeTruthy();
  });

  it("prompts only after the closed window is opened again", async () => {
    await renderLockedGate();
    const emit = getVisibilityHandler();

    await act(async () => {
      emit({ payload: { window: { type: "main" }, visible: false } });
    });

    expect(mocks.authenticateDevice).toHaveBeenCalledTimes(1);
    expect(screen.getByText("Anarlog is Locked")).toBeTruthy();

    await act(async () => {
      emit({ payload: { window: { type: "main" }, visible: true } });
    });

    await waitFor(() => {
      expect(mocks.authenticateDevice).toHaveBeenCalledTimes(2);
      expect(useAppLock.getState().appUnlocked).toBe(true);
    });
    expect(screen.queryByText("Anarlog is Locked")).toBeNull();
  });

  it("reprompts on reopen if close interrupted an in-flight prompt", async () => {
    let resolveAuth!: (value: boolean) => void;
    mocks.authenticateDevice.mockReturnValue(
      new Promise<boolean>((resolve) => {
        resolveAuth = resolve;
      }),
    );

    render(
      <AppLockGate>
        <div>app content</div>
      </AppLockGate>,
    );

    await waitFor(() => {
      expect(mocks.authenticateDevice).toHaveBeenCalledTimes(1);
      expect(useAppLock.getState().authenticating).toBe(true);
    });

    const emit = getVisibilityHandler();
    await act(async () => {
      emit({ payload: { window: { type: "main" }, visible: false } });
    });
    await act(async () => {
      emit({ payload: { window: { type: "main" }, visible: true } });
    });

    expect(mocks.authenticateDevice).toHaveBeenCalledTimes(1);
    expect(useAppLock.getState().appUnlocked).toBe(false);
    expect(screen.getByText("Anarlog is Locked")).toBeTruthy();

    mocks.authenticateDevice.mockResolvedValue(true);
    await act(async () => {
      resolveAuth(true);
    });

    await waitFor(() => {
      expect(mocks.authenticateDevice).toHaveBeenCalledTimes(2);
      expect(useAppLock.getState().appUnlocked).toBe(true);
    });
    expect(screen.queryByText("Anarlog is Locked")).toBeNull();
  });

  it("does not unlock from a prompt that finishes after close", async () => {
    let resolveAuth!: (value: boolean) => void;
    mocks.authenticateDevice.mockReturnValue(
      new Promise<boolean>((resolve) => {
        resolveAuth = resolve;
      }),
    );

    render(
      <AppLockGate>
        <div>app content</div>
      </AppLockGate>,
    );

    await waitFor(() => {
      expect(mocks.authenticateDevice).toHaveBeenCalledTimes(1);
      expect(useAppLock.getState().authenticating).toBe(true);
    });

    const emit = getVisibilityHandler();
    await act(async () => {
      emit({ payload: { window: { type: "main" }, visible: false } });
    });

    await act(async () => {
      resolveAuth(true);
    });

    await waitFor(() => {
      expect(useAppLock.getState().authenticating).toBe(false);
    });
    expect(useAppLock.getState().appUnlocked).toBe(false);
    expect(screen.getByText("Anarlog is Locked")).toBeTruthy();

    mocks.authenticateDevice.mockResolvedValue(true);
    await act(async () => {
      emit({ payload: { window: { type: "main" }, visible: true } });
    });

    await waitFor(() => {
      expect(mocks.authenticateDevice).toHaveBeenCalledTimes(2);
      expect(useAppLock.getState().appUnlocked).toBe(true);
    });
    expect(screen.queryByText("Anarlog is Locked")).toBeNull();
  });

  it("ignores visibility changes for other windows", async () => {
    await renderLockedGate();
    const emit = getVisibilityHandler();

    await act(async () => {
      emit({ payload: { window: { type: "composer" }, visible: false } });
    });

    expect(useAppLock.getState().appUnlocked).toBe(true);
    expect(mocks.authenticateDevice).toHaveBeenCalledTimes(1);
    expect(screen.queryByText("Anarlog is Locked")).toBeNull();
  });
});
