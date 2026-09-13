export type SyncDeviceType = "desktop" | "mobile";

const mobileDeviceNamePattern =
  /\b(?:android|galaxy|honor|huawei|ios|ipad|iphone|ipod|mobile|moto(?:rola)?|oneplus|oppo|pixel|phone|redmi|tablet|vivo|xiaomi)\b/i;

export function inferSyncDeviceType(deviceName: string | null): SyncDeviceType {
  return mobileDeviceNamePattern.test(deviceName ?? "") ? "mobile" : "desktop";
}
