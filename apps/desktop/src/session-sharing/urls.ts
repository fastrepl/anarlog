import type { DesktopScheme } from "~/shared/utils";

const UUID_PATTERN =
  /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;
const PUBLIC_SLUG_PATTERN = /^s_[0-9a-f]{32}$/;
const CAPABILITY_TOKEN_PATTERN = /^[A-Za-z0-9_-]{43}$/;
const WORKSPACE_SHARE_SLUG_PATTERN = /^[a-z0-9][a-z0-9-]{1,61}[a-z0-9]$/;
const RESERVED_WORKSPACE_SHARE_SLUGS = new Set([
  "admin",
  "api",
  "app",
  "assets",
  "auth",
  "cdn",
  "dev",
  "docs",
  "mail",
  "staging",
  "static",
  "status",
  "support",
  "www",
]);

export type ShareDesktopScheme = DesktopScheme;

export const isWorkspaceShareSlug = (value: string) =>
  WORKSPACE_SHARE_SLUG_PATTERN.test(value) &&
  !RESERVED_WORKSPACE_SHARE_SLUGS.has(value);

export function buildSessionShareLinkUrl({
  appBaseUrl,
  linkId,
  linkToken,
  workspaceShareSlug,
  desktopScheme,
}: {
  appBaseUrl: string;
  linkId: string;
  linkToken: string;
  workspaceShareSlug?: string | null;
  desktopScheme?: ShareDesktopScheme;
}) {
  assertUuid(linkId);
  assertCapabilityToken(linkToken);
  return withToken(
    withDesktopScheme(
      appUrl(appBaseUrl, `/t/${linkId}/`, workspaceShareSlug),
      desktopScheme,
    ),
    linkToken,
  );
}

export function buildSessionInvitationUrl({
  appBaseUrl,
  invitationId,
  inviteToken,
  workspaceShareSlug,
  desktopScheme,
}: {
  appBaseUrl: string;
  invitationId: string;
  inviteToken: string;
  workspaceShareSlug?: string | null;
  desktopScheme?: ShareDesktopScheme;
}) {
  assertUuid(invitationId);
  assertCapabilityToken(inviteToken);
  return withToken(
    withDesktopScheme(
      appUrl(appBaseUrl, `/share/invite/${invitationId}/`, workspaceShareSlug),
      desktopScheme,
    ),
    inviteToken,
  );
}

export function buildPublicSessionShareUrl({
  appBaseUrl,
  publicSlug,
  workspaceShareSlug,
  desktopScheme,
}: {
  appBaseUrl: string;
  publicSlug: string;
  workspaceShareSlug?: string | null;
  desktopScheme?: ShareDesktopScheme;
}) {
  if (!PUBLIC_SLUG_PATTERN.test(publicSlug)) {
    throw invalidUrl();
  }
  return withDesktopScheme(
    appUrl(appBaseUrl, `/share/public/${publicSlug}/`, workspaceShareSlug),
    desktopScheme,
  ).toString();
}

export function buildAccountSessionShareUrl({
  appBaseUrl,
  shareId,
  workspaceShareSlug,
  desktopScheme,
}: {
  appBaseUrl: string;
  shareId: string;
  workspaceShareSlug?: string | null;
  desktopScheme?: ShareDesktopScheme;
}) {
  assertUuid(shareId);
  return withDesktopScheme(
    appUrl(appBaseUrl, `/share/${shareId}/`, workspaceShareSlug),
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

function appUrl(
  appBaseUrl: string,
  path: string,
  workspaceShareSlug?: string | null,
) {
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
    if (
      workspaceShareSlug != null &&
      !isWorkspaceShareSlug(workspaceShareSlug)
    ) {
      throw invalidUrl();
    }
    if (
      workspaceShareSlug &&
      (base.hostname === "anarlog.so" || base.hostname === "www.anarlog.so")
    ) {
      base.hostname = `${workspaceShareSlug}.anarlog.so`;
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
