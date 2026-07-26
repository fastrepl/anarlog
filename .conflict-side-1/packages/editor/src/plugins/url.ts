import tldList from "tlds";

const VALID_TLDS = new Set(tldList.map((t: string) => t.toLowerCase()));
const HTTP_SCHEME_REGEX = /^https?:\/\//i;
const DOMAIN_TEXT_REGEX =
  /^(?:www\.)?(?:[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?\.)+[a-z]{2,63}(?::\d{1,5})?(?:[/?#].*)?$/i;

export function normalizeUrlHref(text: string): string {
  const trimmed = text.trim();
  return HTTP_SCHEME_REGEX.test(trimmed) ? trimmed : `https://${trimmed}`;
}

export function looksLikeUrlText(text: string): boolean {
  const trimmed = text.trim();
  return HTTP_SCHEME_REGEX.test(trimmed) || DOMAIN_TEXT_REGEX.test(trimmed);
}

export function isValidUrl(text: string): boolean {
  if (!looksLikeUrlText(text)) {
    return false;
  }

  try {
    const parsed = new URL(normalizeUrlHref(text));
    if (parsed.protocol !== "http:" && parsed.protocol !== "https:") {
      return false;
    }

    const parts = parsed.hostname.split(".");
    if (parts.length < 2) return false;
    return VALID_TLDS.has(parts[parts.length - 1].toLowerCase());
  } catch {
    return false;
  }
}

export function isLinkTextForHref(text: string, href: unknown): boolean {
  return (
    typeof href === "string" &&
    (text === href || normalizeUrlHref(text) === href)
  );
}
