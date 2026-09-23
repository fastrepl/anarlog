import { z } from "zod";

import type { DownloadPlatform } from "@/lib/download";

export const desktopDownloadPageUrl = "https://anarlog.so/download/";

export const downloadLinkSources = ["homepage", "download_page"] as const;
export type DownloadLinkSource = (typeof downloadLinkSources)[number];

export const downloadLinkRequestSchema = z.object({
  email: z.string().trim().toLowerCase().email("Enter a valid email address."),
  source: z.enum(downloadLinkSources),
});

export type DownloadLinkRequest = z.infer<typeof downloadLinkRequestSchema>;

export function isMobileDownloadPlatform(platform: DownloadPlatform) {
  return platform === "ios" || platform === "android";
}

export function getDownloadLinkIdempotencyKey(email: string, now: Date) {
  const day = now.toISOString().slice(0, 10);
  return `desktop-download-link:${email}:${day}`;
}
