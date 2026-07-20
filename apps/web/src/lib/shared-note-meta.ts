import { ANARLOG_SITE_URL, getPublicSharedNoteOgImageUrl } from "./seo.ts";
import {
  getSharedNoteDescription,
  type SharedNoteSnapshot,
} from "./shared-notes.ts";

export const privateShareHeaders = {
  "Cache-Control": "private, no-store",
  "Referrer-Policy": "no-referrer",
  "X-Robots-Tag": "noindex, nofollow, noarchive, nosnippet",
} as const;

export const publicShareHeaders = {
  "Cache-Control": "no-store",
  "Referrer-Policy": "no-referrer",
} as const;

export function getPrivateShareHead() {
  return {
    meta: [
      { title: "Shared note · Anarlog" },
      {
        name: "robots",
        content: "noindex, nofollow, noarchive, nosnippet",
      },
      { name: "referrer", content: "no-referrer" },
      { name: "ai-content", content: "private" },
    ],
  };
}

export function getPublicShareHead(
  publicSlug: string,
  snapshot: SharedNoteSnapshot | null | undefined,
) {
  if (!snapshot) {
    return getPrivateShareHead();
  }

  const title = snapshot.title || "Shared note";
  const description =
    getSharedNoteDescription(snapshot.body) ||
    "A public note shared with Anarlog.";
  const url = `${ANARLOG_SITE_URL}/share/public/${publicSlug}/`;
  const imageUrl = getPublicSharedNoteOgImageUrl(publicSlug);

  return {
    links: [{ rel: "canonical", href: url }],
    meta: [
      { title: `${title} · Anarlog` },
      { name: "description", content: description },
      { name: "robots", content: "index, follow" },
      { name: "referrer", content: "no-referrer" },
      { name: "ai-content", content: "public" },
      { property: "og:type", content: "article" },
      { property: "og:title", content: title },
      { property: "og:description", content: description },
      { property: "og:url", content: url },
      { property: "og:image", content: imageUrl },
      { property: "og:image:type", content: "image/png" },
      { property: "og:image:width", content: "1200" },
      { property: "og:image:height", content: "630" },
      { property: "og:image:alt", content: `Preview of ${title}` },
      { name: "twitter:card", content: "summary_large_image" },
      { name: "twitter:title", content: title },
      { name: "twitter:description", content: description },
      { name: "twitter:image", content: imageUrl },
      { name: "twitter:image:alt", content: `Preview of ${title}` },
    ],
  };
}
