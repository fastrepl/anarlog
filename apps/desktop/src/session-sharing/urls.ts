const UUID_PATTERN =
  /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;
const PUBLIC_SLUG_PATTERN = /^s_[0-9a-f]{32}$/;
const CAPABILITY_TOKEN_PATTERN = /^[A-Za-z0-9_-]{43}$/;

export type ShareDesktopScheme = "hyprnote" | "hyprnote-staging";

export function buildSessionShareLinkUrl({
  appBaseUrl,
  shareId,
  linkToken,
  desktopScheme,
}: {
  appBaseUrl: string;
  shareId: string;
  linkToken: string;
  desktopScheme?: ShareDesktopScheme;
}) {
  assertUuid(shareId);
  assertCapabilityToken(linkToken);
  return withToken(
    withDesktopScheme(
      appUrl(appBaseUrl, `/share/link/${shareId}/`),
      desktopScheme,
    ),
    linkToken,
  );
}

export function buildSessionInvitationUrl({
  appBaseUrl,
  invitationId,
  inviteToken,
  desktopScheme,
}: {
  appBaseUrl: string;
  invitationId: string;
  inviteToken: string;
  desktopScheme?: ShareDesktopScheme;
}) {
  assertUuid(invitationId);
  assertCapabilityToken(inviteToken);
  return withToken(
    withDesktopScheme(
      appUrl(appBaseUrl, `/share/invite/${invitationId}/`),
      desktopScheme,
    ),
    inviteToken,
  );
}

export function buildPublicSessionShareUrl({
  appBaseUrl,
  publicSlug,
  desktopScheme,
}: {
  appBaseUrl: string;
  publicSlug: string;
  desktopScheme?: ShareDesktopScheme;
}) {
  if (!PUBLIC_SLUG_PATTERN.test(publicSlug)) {
    throw invalidUrl();
  }
  return withDesktopScheme(
    appUrl(appBaseUrl, `/share/public/${publicSlug}/`),
    desktopScheme,
  ).toString();
}

export function buildAccountSessionShareUrl({
  appBaseUrl,
  shareId,
  desktopScheme,
}: {
  appBaseUrl: string;
  shareId: string;
  desktopScheme?: ShareDesktopScheme;
}) {
  assertUuid(shareId);
  return withDesktopScheme(
    appUrl(appBaseUrl, `/share/${shareId}/`),
    desktopScheme,
  ).toString();
}

function withToken(url: URL, token: string) {
  url.hash = new URLSearchParams({ token }).toString();
  return url.toString();
}

function withDesktopScheme(
  url: URL,
  desktopScheme: ShareDesktopScheme | undefined,
) {
  if (!desktopScheme || desktopScheme === "hyprnote") {
    return url;
  }
  if (desktopScheme !== "hyprnote-staging") {
    throw invalidUrl();
  }
  url.searchParams.set("scheme", desktopScheme);
  return url;
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
