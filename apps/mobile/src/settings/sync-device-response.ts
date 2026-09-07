export function parseSyncDeviceResponse(body: unknown) {
  if (
    !body ||
    typeof body !== "object" ||
    !Array.isArray((body as { devices?: unknown }).devices)
  )
    throw new Error("Invalid device response");
  const responseBody = body as Record<string, unknown>;
  const maxDevices = responseBody.maxDevices;
  const pendingDevices = responseBody.pendingDevices;
  if (
    typeof maxDevices !== "number" ||
    !Number.isInteger(maxDevices) ||
    maxDevices < 1 ||
    !Array.isArray(pendingDevices)
  )
    throw new Error("Invalid device response");
  const fingerprints = new Set<string>();
  for (const device of pendingDevices) {
    if (
      !device ||
      typeof device !== "object" ||
      typeof device.deviceFingerprint !== "string"
    )
      throw new Error("Invalid device response");
    fingerprints.add(device.deviceFingerprint);
  }
  const devices = (body as { devices: unknown[] }).devices
    .slice(0, 100)
    .map((value) => {
      if (
        !value ||
        typeof value !== "object" ||
        typeof (value as { deviceFingerprint?: unknown }).deviceFingerprint !==
          "string"
      )
        throw new Error("Invalid device response");
      const device = value as Record<string, unknown>;
      fingerprints.add(device.deviceFingerprint as string);
      return {
        deviceFingerprint: device.deviceFingerprint as string,
        deviceName:
          typeof device.deviceName === "string" ? device.deviceName : null,
        deviceKind:
          typeof device.deviceKind === "string" ? device.deviceKind : null,
      };
    });
  return { devices, maxDevices, usedDevices: fingerprints.size };
}
