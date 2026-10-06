export const pendingAuthStorageKey = "anarlog:auth:pending-handoff";
let consuming = false;

export async function consumeAuthCallback({
  url,
  scheme,
  storage,
  now = Date.now(),
}: {
  url: string;
  scheme: string;
  storage: {
    getItem: (key: string) => Promise<string | null>;
    removeItem: (key: string) => Promise<void>;
  };
  now?: number;
}) {
  if (consuming) return null;
  consuming = true;
  try {
    const parsed = new URL(url);
    if (
      parsed.protocol !== `${scheme}:` ||
      parsed.hostname !== "auth" ||
      parsed.pathname !== "/callback"
    )
      return null;
    const accessToken = parsed.searchParams.get("access_token");
    const refreshToken = parsed.searchParams.get("refresh_token");
    const state = parsed.searchParams.get("state");
    if (!accessToken || !refreshToken || !state) return null;
    const raw = await storage.getItem(pendingAuthStorageKey);
    if (!raw) return null;
    const pending = JSON.parse(raw);
    if (
      pending.state !== state ||
      typeof pending.createdAt !== "number" ||
      now < pending.createdAt ||
      now - pending.createdAt > 15 * 60 * 1_000
    )
      return null;
    await storage.removeItem(pendingAuthStorageKey);
    return { accessToken, refreshToken };
  } catch {
    return null;
  } finally {
    consuming = false;
  }
}
