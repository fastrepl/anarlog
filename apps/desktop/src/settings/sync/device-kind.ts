export function resolveDeviceKind(
  kind: unknown,
  name: string | null,
): "desktop" | "mobile" | "watch" {
  if (kind === "desktop" || kind === "mobile" || kind === "watch") {
    return kind;
  }
  if (
    name &&
    /\b(?:mac|macbook|imac|windows|desktop|laptop|pc|linux)\b/i.test(name)
  ) {
    return "desktop";
  }
  if (
    name &&
    (/\b(?:android|galaxy|honor|huawei|ios|ipad|iphone|ipod|mobile|moto(?:rola)?|oneplus|oppo|pixel|phone|redmi|tablet|vivo|xiaomi)\b/i.test(
      name,
    ) ||
      /^(?:gt|sch|sgh|sm)-[a-z0-9-]+$/i.test(name.trim()))
  ) {
    return "mobile";
  }
  return "desktop";
}
