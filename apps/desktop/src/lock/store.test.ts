import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  authenticateDevice: vi.fn(),
  isDeviceAuthAvailable: vi.fn(),
}));

vi.mock("./auth", () => ({
  authenticateDevice: mocks.authenticateDevice,
  isDeviceAuthAvailable: mocks.isDeviceAuthAvailable,
}));

import { isLockedFlag } from "./flag";
import { useAppLock } from "./store";

describe("isLockedFlag", () => {
  it("treats sqlite integers and booleans as locked", () => {
    expect(isLockedFlag(1)).toBe(true);
    expect(isLockedFlag(true)).toBe(true);
    expect(isLockedFlag(0)).toBe(false);
    expect(isLockedFlag(false)).toBe(false);
    expect(isLockedFlag(null)).toBe(false);
  });
});

describe("app lock store", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.authenticateDevice.mockResolvedValue(true);
    useAppLock.setState({
      available: null,
      authenticating: false,
      appUnlocked: false,
      revealedNoteIds: {},
    });
  });

  it("reveals a note after authentication", async () => {
    await expect(
      useAppLock.getState().revealNote("session-1", "open"),
    ).resolves.toBe(true);
    expect(useAppLock.getState().isNoteRevealed("session-1")).toBe(true);
    expect(useAppLock.getState().appUnlocked).toBe(false);
  });

  it("unlocks the app after authentication", async () => {
    await expect(useAppLock.getState().unlockApp("open")).resolves.toBe(true);
    expect(useAppLock.getState().appUnlocked).toBe(true);
  });

  it("clears revealed notes when the app locks", () => {
    useAppLock.setState({ revealedNoteIds: { "session-1": true } });
    useAppLock.getState().lockApp();
    expect(useAppLock.getState().isNoteRevealed("session-1")).toBe(false);
    expect(useAppLock.getState().appUnlocked).toBe(false);
  });

  it("ignores an in-flight unlock after the app locks", async () => {
    let resolveAuth!: (value: boolean) => void;
    mocks.authenticateDevice.mockReturnValue(
      new Promise<boolean>((resolve) => {
        resolveAuth = resolve;
      }),
    );

    const pending = useAppLock.getState().unlockApp("open");
    await waitForAuthenticating();
    useAppLock.getState().lockApp();
    resolveAuth(true);

    await expect(pending).resolves.toBe(false);
    expect(useAppLock.getState().appUnlocked).toBe(false);
  });

  it("unlocks after a later authentication that starts after lock", async () => {
    let resolveAuth!: (value: boolean) => void;
    mocks.authenticateDevice.mockReturnValue(
      new Promise<boolean>((resolve) => {
        resolveAuth = resolve;
      }),
    );

    const stale = useAppLock.getState().unlockApp("open");
    await waitForAuthenticating();
    useAppLock.getState().lockApp();
    resolveAuth(true);
    await expect(stale).resolves.toBe(false);

    mocks.authenticateDevice.mockResolvedValue(true);
    await expect(useAppLock.getState().unlockApp("open")).resolves.toBe(true);
    expect(useAppLock.getState().appUnlocked).toBe(true);
  });
});

function waitForAuthenticating() {
  return vi.waitFor(() => {
    expect(useAppLock.getState().authenticating).toBe(true);
  });
}
