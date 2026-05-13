import { beforeEach, describe, expect, it, vi } from "vitest";

const isTauriMock = vi.hoisted(() => vi.fn());

vi.mock("@tauri-apps/api/core", () => ({
  isTauri: isTauriMock,
}));

import {
  getOnboardingNeeded,
  isShellEntryPath,
  normalizeAppPath,
  resolveAppEntryPath,
  resolveShellEntryPath,
} from "./-resolve-entry-path";

import { commands } from "~/types/tauri.gen";

describe("app entry path resolution", () => {
  beforeEach(() => {
    isTauriMock.mockReturnValue(true);
    vi.mocked(commands.getOnboardingNeeded).mockResolvedValue({
      status: "ok",
      data: false,
    });
  });

  it("uses classic main in non-tauri environments", async () => {
    isTauriMock.mockReturnValue(false);

    await expect(getOnboardingNeeded()).resolves.toBe(false);
    await expect(resolveShellEntryPath()).resolves.toBe("/app/main");
    await expect(resolveAppEntryPath()).resolves.toBe("/app/main");
  });

  it("routes to onboarding before either shell", async () => {
    vi.mocked(commands.getOnboardingNeeded).mockResolvedValue({
      status: "ok",
      data: true,
    });

    await expect(resolveAppEntryPath()).resolves.toBe("/app/onboarding");
  });

  it("normalizes and identifies shell entry paths", () => {
    expect(normalizeAppPath("/app/main/")).toBe("/app/main");
    expect(isShellEntryPath("/app")).toBe(true);
    expect(isShellEntryPath("/app/main/")).toBe(true);
    expect(isShellEntryPath("/app/unknown")).toBe(false);
    expect(isShellEntryPath("/app/onboarding")).toBe(false);
  });
});
