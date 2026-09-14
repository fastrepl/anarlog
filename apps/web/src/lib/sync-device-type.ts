const mobileDeviceNamePattern =
  /\b(?:android|galaxy|honor|huawei|ios|ipad|iphone|ipod|mobile|moto(?:rola)?|oneplus|oppo|pixel|phone|redmi|tablet|vivo|xiaomi)\b/i;
const mobileModelIdentifierPattern = /^(?:gt|sch|sgh|sm)-[a-z0-9-]+$/i;
const desktopDeviceNamePattern =
  /(?:\b(?:desktop|imac|linux|mac(?: mini| pro| studio)|macbook|pc|ubuntu|windows)\b|\.local$)/i;

export function inferSyncDeviceType(
  deviceName: string | null,
): "desktop" | "mobile" | "unknown" {
  const name = deviceName?.trim();
  if (!name) {
    return "unknown";
  }
  if (
    mobileDeviceNamePattern.test(name) ||
    mobileModelIdentifierPattern.test(name)
  ) {
    return "mobile";
  }
  return desktopDeviceNamePattern.test(name) ? "desktop" : "unknown";
}
