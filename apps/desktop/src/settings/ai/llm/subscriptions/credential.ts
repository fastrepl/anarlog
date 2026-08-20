export type OAuthCredential = {
  type: "oauth";
  refresh: string;
  access: string;
  expires: number;
  accountId?: string;
};

const REFRESH_SKEW_MS = 2 * 60 * 1000;

export function parseOAuthCredential(value: string): OAuthCredential | null {
  const trimmed = value.trim();
  if (!trimmed.startsWith("{")) {
    return null;
  }

  try {
    const parsed = JSON.parse(trimmed) as Partial<OAuthCredential>;
    if (
      parsed.type !== "oauth" ||
      typeof parsed.refresh !== "string" ||
      parsed.refresh.length === 0 ||
      typeof parsed.access !== "string" ||
      typeof parsed.expires !== "number"
    ) {
      return null;
    }

    return {
      type: "oauth",
      refresh: parsed.refresh,
      access: parsed.access,
      expires: parsed.expires,
      accountId:
        typeof parsed.accountId === "string" ? parsed.accountId : undefined,
    };
  } catch {
    return null;
  }
}

export function serializeOAuthCredential(credential: OAuthCredential): string {
  return JSON.stringify(credential);
}

export function isOAuthCredentialFresh(
  credential: OAuthCredential,
  now = Date.now(),
): boolean {
  return (
    credential.access.length > 0 && credential.expires - REFRESH_SKEW_MS > now
  );
}
