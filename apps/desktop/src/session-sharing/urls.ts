import type { DesktopScheme } from "~/shared/utils";

const UUID_PATTERN =
  /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;
const PUBLIC_SLUG_PATTERN = /^s_[0-9a-f]{32}$/;
const CAPABILITY_TOKEN_PATTERN = /^[A-Za-z0-9_-]{43}$/;
const LINK_PREVIEW_TOKEN_PATTERN = /^[0-9a-f]{64}$/;

export type ShareDesktopScheme = DesktopScheme;

export function buildSessionShareLinkUrl({
  appBaseUrl,
  shareId,
  linkToken,
  previewToken,
  desktopScheme,
}: {
  appBaseUrl: string;
  shareId: string;
  linkToken: string;
  previewToken: string;
  desktopScheme?: ShareDesktopScheme;
}) {
  assertUuid(shareId);
  assertCapabilityToken(linkToken);
  if (!LINK_PREVIEW_TOKEN_PATTERN.test(previewToken)) {
    throw invalidUrl();
  }
  const url = withDesktopScheme(
    appUrl(appBaseUrl, `/share/link/${shareId}/`),
    desktopScheme,
  );
  url.searchParams.set("preview", previewToken);
  return withToken(url, linkToken);
}

export async function createSessionShareLinkPreviewToken(linkToken: string) {
  assertCapabilityToken(linkToken);
  const digest = new Uint8Array(
    await crypto.subtle.digest("SHA-256", new TextEncoder().encode(linkToken)),
  );
  return Array.from(digest, (byte) => byte.toString(16).padStart(2, "0")).join(
    "",
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
  if (!desktopScheme || desktopScheme === "anarlog") {
    return url;
  }
  if (desktopScheme !== "anarlog-staging" && desktopScheme !== "anarlog-dev") {
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
