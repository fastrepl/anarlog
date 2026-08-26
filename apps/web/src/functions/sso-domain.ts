const SSO_DOMAIN_PATTERN =
  /^[a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?(\.[a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?)+$/;

const SSO_AUTH_METHODS = new Set(["sso", "sso/saml", "saml"]);

export const SSO_REQUIRED_MESSAGE = "This organization requires SSO.";

export function sessionUsesSso(session: {
  access_token?: string;
  user?: { app_metadata?: { provider?: unknown } };
}): boolean {
  const provider = session.user?.app_metadata?.provider;
  if (typeof provider === "string" && provider.startsWith("sso:")) {
    return true;
  }

  return readJwtAmr(session.access_token).some((method) =>
    SSO_AUTH_METHODS.has(method),
  );
}

function readJwtAmr(accessToken: string | undefined): string[] {
  if (!accessToken) {
    return [];
  }

  const payload = accessToken.split(".")[1];
  if (!payload) {
    return [];
  }

  try {
    const padded =
      payload.replace(/-/g, "+").replace(/_/g, "/") +
      "=".repeat((4 - (payload.length % 4)) % 4);
    const claims = JSON.parse(atob(padded)) as { amr?: unknown };
    if (!Array.isArray(claims.amr)) {
      return [];
    }
    return claims.amr.flatMap((entry) => {
      if (
        entry &&
        typeof entry === "object" &&
        "method" in entry &&
        typeof entry.method === "string"
      ) {
        return [entry.method];
      }
      return [];
    });
  } catch {
    return [];
  }
}

export function normalizeSsoDomain(input: string): string | null {
  const trimmed = input.trim().toLowerCase();
  if (!trimmed) {
    return null;
  }

  const host = trimmed
    .replace(/^https?:\/\//, "")
    .split("/")[0]
    .replace(/:\d+$/, "");
  const domain = host.includes("@")
    ? host.slice(host.lastIndexOf("@") + 1)
    : host;

  if (domain.length > 253 || !SSO_DOMAIN_PATTERN.test(domain)) {
    return null;
  }

  return domain;
}

export const SSO_UNAVAILABLE_MESSAGE =
  "No SSO provider is configured for this domain.";

export function mapSsoAuthError(message: string): string {
  const lower = message.toLowerCase();
  if (lower.includes("organization requires sso")) {
    return SSO_REQUIRED_MESSAGE;
  }

  if (
    lower.includes("saml 2.0 is disabled") ||
    lower.includes("saml is disabled") ||
    (lower.includes("sso") &&
      (lower.includes("not found") ||
        lower.includes("no sso") ||
        lower.includes("provider") ||
        lower.includes("does not exist") ||
        lower.includes("disabled")))
  ) {
    return SSO_UNAVAILABLE_MESSAGE;
  }

  return message;
}
