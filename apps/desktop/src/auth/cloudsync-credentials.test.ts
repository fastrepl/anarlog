import { hostname } from "@tauri-apps/plugin-os";
import { afterEach, expect, test, vi } from "vitest";

import { commands as miscCommands } from "@anlg/plugin-misc";

import { getDeviceIdentity, sanitizeDeviceName } from "./cloudsync-credentials";

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

vi.mock("~/settings/queries", () => ({
  getStoredSettingValues: vi.fn(),
}));

afterEach(() => {
  vi.mocked(hostname).mockReset();
  vi.mocked(hostname).mockResolvedValue(null);
  vi.mocked(miscCommands.getFingerprint).mockReset();
  vi.mocked(miscCommands.getFingerprint).mockResolvedValue({
    status: "error",
    error: "unavailable",
  });
});

test("keeps printable hostnames and drops empty or non-ascii names", () => {
  expect(sanitizeDeviceName("Johns-M4-Max.local")).toBe("Johns-M4-Max.local");
  expect(sanitizeDeviceName(" John's MacBook ")).toBe("John's MacBook");
  expect(sanitizeDeviceName("  ")).toBeNull();
  expect(sanitizeDeviceName(null)).toBeNull();
  expect(sanitizeDeviceName("존의 MacBook")).toBe("MacBook");
  expect(sanitizeDeviceName("존의맥북")).toBeNull();
  expect(sanitizeDeviceName("a".repeat(200))).toBe("a".repeat(128));
});

test("treats hostname lookup failures as an unnamed device", async () => {
  vi.mocked(hostname).mockRejectedValue(new Error("not allowed"));

  await expect(getDeviceIdentity()).resolves.toEqual({
    fingerprint: null,
    name: null,
  });
});

test("reads the machine hostname as the sync device name", async () => {
  vi.mocked(hostname).mockResolvedValue("Johns-M4-Max.local");
  vi.mocked(miscCommands.getFingerprint).mockResolvedValue({
    status: "ok",
    data: "fingerprint-1234",
  });

  await expect(getDeviceIdentity()).resolves.toEqual({
    fingerprint: "fingerprint-1234",
    name: "Johns-M4-Max.local",
  });
});
