import { env } from "@/lib/env";

export async function requestSyncDeviceList(
  token: string,
  signal?: AbortSignal,
): Promise<
  Array<{
    deviceFingerprint: string;
    deviceName: string | null;
    deviceKind: string | null;
  }>
> {
  const response = await fetch(new URL("/sync/devices", env.apiUrl), {
    headers: { Authorization: `Bearer ${token}` },
    signal,
  });
  if (!response.ok) throw new Error("Could not load your devices.");
  const body: unknown = await response.json();
  if (
    !body ||
    typeof body !== "object" ||
    !Array.isArray((body as { devices?: unknown }).devices)
  )
    throw new Error("Invalid device response");
  const devices = (body as { devices: unknown[] }).devices;
  return devices.slice(0, 100).map((value) => {
    if (
      !value ||
      typeof value !== "object" ||
      typeof (value as { deviceFingerprint?: unknown }).deviceFingerprint !==
        "string"
    )
      throw new Error("Invalid device response");
    const device = value as Record<string, unknown>;
    return {
      deviceFingerprint: device.deviceFingerprint as string,
      deviceName:
        typeof device.deviceName === "string" ? device.deviceName : null,
      deviceKind:
        typeof device.deviceKind === "string" ? device.deviceKind : null,
    };
  });
}
