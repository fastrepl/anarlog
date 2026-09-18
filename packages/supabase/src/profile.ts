import type { User } from "@supabase/supabase-js";

type ProviderProfile = Pick<User, "email" | "identities" | "user_metadata">;

const metadataValue = (
  metadata: Record<string, unknown> | undefined,
  keys: readonly string[],
): string | null => {
  for (const key of keys) {
    const value = metadata?.[key];
    if (typeof value === "string" && value.trim()) {
      return value.trim();
    }
  }
  return null;
};

const profileMetadata = (
  user: ProviderProfile,
): Array<Record<string, unknown> | undefined> => [
  user.user_metadata,
  ...(user.identities?.map((identity) => identity.identity_data) ?? []),
];

export function getCustomProfileImageUrl(
  user: ProviderProfile | null | undefined,
): string | null | undefined {
  const avatar = user?.user_metadata?.profile_avatar;
  if (!avatar || typeof avatar !== "object" || !("url" in avatar))
    return undefined;
  if (typeof avatar.url !== "string") return null;
  try {
    const url = new URL(avatar.url);
    return url.protocol === "https:" && !url.username && !url.password
      ? avatar.url
      : null;
  } catch {
    return null;
  }
}

export function getProviderProfileImageUrl(
  user: ProviderProfile | null | undefined,
): string | null {
  if (!user) return null;
  const custom = getCustomProfileImageUrl(user);
  if (custom !== undefined) return custom;

  for (const metadata of profileMetadata(user)) {
    const value = metadataValue(metadata, ["avatar_url", "picture"]);
    if (!value) continue;

    try {
      const url = new URL(value);
      if (url.protocol === "https:" && !url.username && !url.password) {
        return value;
      }
    } catch {
      continue;
    }
  }

  return null;
}

export function getProviderProfileName(
  user: ProviderProfile | null | undefined,
): string | null {
  if (!user) return null;

  for (const metadata of profileMetadata(user)) {
    const value = metadataValue(metadata, ["full_name", "name"]);
    if (value) return value;
  }

  return user.email?.trim() || null;
}
