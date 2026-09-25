import { afterEach, expect, test, vi } from "vitest";

import { sealE2eeRecoveryKeyForDevice } from "@anlg/plugin-db";

import { startAutomaticDeviceEnrollment } from "./automatic-device-enrollment";
import { requestSyncDevices, sealDeviceEnrollment } from "./sync-devices";
vi.mock("./sync-devices", () => ({
  requestSyncDevices: vi.fn(),
  sealDeviceEnrollment: vi.fn(),
}));
afterEach(() => {
  vi.useRealTimers();
  vi.clearAllMocks();
});
const device = {
  requestId: "request",
  publicKey: "public",
  status: "pending" as const,
  expiresAt: "2099-01-01",
  createdAt: "",
  deviceFingerprint: "new",
  deviceName: null,
};
test("automatically seals pending same-account requests and stops on teardown", async () => {
  vi.useFakeTimers();
  vi.mocked(requestSyncDevices).mockResolvedValue({
    devices: [],
    maxDevices: 3,
    pendingDevices: [
      device,
      { ...device, status: "sealed" },
      { ...device, expiresAt: "2000-01-01" },
    ],
  });
  const stop = startAutomaticDeviceEnrollment("account-a", "token-a");
  await vi.advanceTimersByTimeAsync(5000);
  expect(requestSyncDevices).toHaveBeenCalledWith(
    "token-a",
    expect.any(AbortSignal),
  );
  expect(sealE2eeRecoveryKeyForDevice).toHaveBeenCalledExactlyOnceWith(
    "account-a",
    "request",
    "public",
  );
  expect(sealDeviceEnrollment).toHaveBeenCalledWith(
    expect.objectContaining({ accessToken: "token-a", requestId: "request" }),
  );
  stop();
  await vi.advanceTimersByTimeAsync(15000);
  expect(requestSyncDevices).toHaveBeenCalledTimes(1);
});
test("does not publish a key after sign-out during native sealing", async () => {
  vi.useFakeTimers();
  vi.mocked(requestSyncDevices).mockResolvedValue({
    devices: [],
    maxDevices: 3,
    pendingDevices: [device],
  });
  let resolve!: (
    value: Awaited<ReturnType<typeof sealE2eeRecoveryKeyForDevice>>,
  ) => void;
  vi.mocked(sealE2eeRecoveryKeyForDevice).mockImplementationOnce(
    () =>
      new Promise((r) => {
        resolve = r;
      }),
  );
  const stop = startAutomaticDeviceEnrollment("account-a", "token-a");
  await vi.advanceTimersByTimeAsync(5000);
  stop();
  resolve({ ephemeralPublicKey: "", nonce: "", ciphertext: "" });
  await vi.advanceTimersByTimeAsync(10000);
  expect(sealDeviceEnrollment).not.toHaveBeenCalled();
});
test("retries after a timed-out device listing", async () => {
  vi.useFakeTimers();
  vi.mocked(requestSyncDevices)
    .mockImplementationOnce(
      (_token, signal) =>
        new Promise((_resolve, reject) =>
          signal!.addEventListener("abort", () => reject(new Error("timeout"))),
        ),
    )
    .mockResolvedValue({ devices: [], maxDevices: 3, pendingDevices: [] });
  const stop = startAutomaticDeviceEnrollment("account", "token");
  await vi.advanceTimersByTimeAsync(35000);
  expect(requestSyncDevices).toHaveBeenCalledTimes(2);
  stop();
});
