import { sealE2eeRecoveryKeyForDevice } from "@anlg/plugin-db";

import { requestSyncDevices, sealDeviceEnrollment } from "./sync-devices";

export function startAutomaticDeviceEnrollment(
  accountUserId: string,
  accessToken: string,
) {
  let stopped = false;
  let controller = new AbortController();
  let timer: ReturnType<typeof setTimeout> | undefined;
  const poll = async () => {
    controller = new AbortController();
    const timeout = setTimeout(() => controller.abort(), 25_000);
    try {
      const { pendingDevices } = await requestSyncDevices(
        accessToken,
        controller.signal,
      );
      for (const device of pendingDevices) {
        if (controller.signal.aborted) return;
        if (
          device.status !== "pending" ||
          !(Date.parse(device.expiresAt) > Date.now())
        )
          continue;
        const packageValue = await sealE2eeRecoveryKeyForDevice(
          accountUserId,
          device.requestId,
          device.publicKey,
        );
        if (controller.signal.aborted) return;
        await sealDeviceEnrollment({
          accessToken,
          requestId: device.requestId,
          packageValue,
          signal: controller.signal,
        });
      }
    } catch {
      // Enrollment retries independently of note sync; never log key material.
    } finally {
      clearTimeout(timeout);
      if (!stopped) timer = setTimeout(() => void poll(), 5_000);
    }
  };
  timer = setTimeout(() => void poll(), 5_000);
  return () => {
    stopped = true;
    controller.abort();
    clearTimeout(timer);
  };
}
