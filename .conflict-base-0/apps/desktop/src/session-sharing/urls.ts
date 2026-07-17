const UUID_PATTERN =
  /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;
const PUBLIC_SLUG_PATTERN = /^s_[0-9a-f]{32}$/;
const CAPABILITY_TOKEN_PATTERN = /^[A-Za-z0-9_-]{43}$/;

export function buildSessionShareLinkUrl({
  appBaseUrl,
  shareId,
  linkToken,
}: {
  appBaseUrl: string;
  shareId: string;
  linkToken: string;
}) {
  assertUuid(shareId);
  assertCapabilityToken(linkToken);
  return withToken(appUrl(appBaseUrl, `/share/link/${shareId}`), linkToken);
}

export function buildSessionInvitationUrl({
  appBaseUrl,
  invitationId,
  inviteToken,
}: {
  appBaseUrl: string;
  invitationId: string;
  inviteToken: string;
}) {
  assertUuid(invitationId);
  assertCapabilityToken(inviteToken);
  return withToken(
    appUrl(appBaseUrl, `/share/invite/${invitationId}`),
    inviteToken,
  );
}

export function buildPublicSessionShareUrl({
  appBaseUrl,
  publicSlug,
}: {
  appBaseUrl: string;
  publicSlug: string;
}) {
  if (!PUBLIC_SLUG_PATTERN.test(publicSlug)) {
    throw invalidUrl();
  }
  return appUrl(appBaseUrl, `/share/public/${publicSlug}`).toString();
}

export function buildAccountSessionShareUrl({
  appBaseUrl,
  shareId,
}: {
  appBaseUrl: string;
  shareId: string;
}) {
  assertUuid(shareId);
  return appUrl(appBaseUrl, `/share/${shareId}`).toString();
}

function withToken(url: URL, token: string) {
  url.hash = new URLSearchParams({ token }).toString();
  return url.toString();
}

function appUrl(appBaseUrl: string, path: string) {
  try {
    const base = new URL(appBaseUrl);
    if (
      !["http:", "https:"].includes(base.protocol) ||
      base.username !== "" ||
      base.password !== "" ||
      base.search !== "" ||
      base.hash !== ""
    ) {
      throw invalidUrl();
    }
    return new URL(path, base.origin);
  } catch {
    throw invalidUrl();
  }
}

function assertUuid(value: string) {
  if (!UUID_PATTERN.test(value)) {
    throw invalidUrl();
  }
}

function assertCapabilityToken(value: string) {
  if (!CAPABILITY_TOKEN_PATTERN.test(value)) {
    throw invalidUrl();
  }
}

function invalidUrl() {
  return new Error("Share URL is unavailable");
}
