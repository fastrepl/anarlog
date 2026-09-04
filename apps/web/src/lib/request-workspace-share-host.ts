import { getWorkspaceShareSlug } from "./workspace-share-host.ts";

const firstHeaderValue = (value: string | null) =>
  value
    ?.split(",")
    .map((part) => part.trim())
    .find(Boolean);

export const getWorkspaceShareSlugFromHeaders = (headers: Headers) => {
  const host =
    firstHeaderValue(headers.get("x-forwarded-host")) ??
    firstHeaderValue(headers.get("host"));
  if (!host) return null;

  try {
    return getWorkspaceShareSlug(
      new URL(`https://${host}`).hostname.toLowerCase(),
    );
  } catch {
    return null;
  }
};
