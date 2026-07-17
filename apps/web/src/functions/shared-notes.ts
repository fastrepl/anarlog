import { createServerFn } from "@tanstack/react-start";
import { setResponseHeader } from "@tanstack/react-start/server";
import { z } from "zod";

import { env } from "@/env";
import { getSupabaseServerClient } from "@/functions/supabase";
import {
  fetchPublicSharedNoteResult,
  type SharedNoteReadResult,
} from "@/lib/shared-note-api";
import {
  type AuthenticatedSharedNote,
  parseAuthenticatedSharedNote,
  parseSharedNoteAttachmentDownload,
  publicShareSlugSchema,
  shareIdSchema,
} from "@/lib/shared-notes";

export type AuthenticatedSharedNoteReadResult =
  | { status: "ready"; note: AuthenticatedSharedNote }
  | { status: "unavailable" }
  | { status: "error" };

const attachmentDownloadInputSchema = z
  .object({
    shareId: shareIdSchema,
    attachmentId: shareIdSchema,
  })
  .strict();

export const readAuthenticatedSharedNote = createServerFn({ method: "GET" })
  .inputValidator(shareIdSchema)
  .handler(
    async ({ data: shareId }): Promise<AuthenticatedSharedNoteReadResult> => {
      setPrivateShareResponseHeaders();

      const supabase = getSupabaseServerClient();
      const { data, error } = await supabase.rpc(
        "read_my_session_share_snapshot_with_attachments",
        { p_share_id: shareId },
      );
      if (error || !Array.isArray(data)) {
        return { status: "error" };
      }
      if (data.length === 0) {
        return { status: "unavailable" };
      }
      if (data.length !== 1) {
        return { status: "error" };
      }

      try {
        return { status: "ready", note: parseAuthenticatedSharedNote(data[0]) };
      } catch {
        return { status: "error" };
      }
    },
  );

export const readPublicSharedNote = createServerFn({ method: "GET" })
  .inputValidator(publicShareSlugSchema)
  .handler(async ({ data: publicSlug }): Promise<SharedNoteReadResult> => {
    setResponseHeader("Cache-Control", "no-store");
    setResponseHeader("Referrer-Policy", "no-referrer");
    return fetchPublicSharedNoteResult(publicSlug);
  });

export const createAuthenticatedSharedAttachmentDownload = createServerFn({
  method: "POST",
})
  .inputValidator(attachmentDownloadInputSchema)
  .handler(async ({ data }) => {
    setPrivateShareResponseHeaders();
    const supabase = getSupabaseServerClient();
    const { data: sessionData } = await supabase.auth.getSession();
    const accessToken = sessionData.session?.access_token;
    if (!accessToken) return null;

    try {
      const response = await fetch(
        new URL(
          `/shared-notes/access/${encodeURIComponent(data.shareId)}/attachments/${encodeURIComponent(data.attachmentId)}/download`,
          apiBaseUrl(),
        ),
        {
          method: "POST",
          cache: "no-store",
          headers: {
            Accept: "application/json",
            Authorization: `Bearer ${accessToken}`,
          },
        },
      );
      if (!response.ok) return null;
      const text = await response.text();
      if (new TextEncoder().encode(text).byteLength > 32 * 1024) return null;
      return parseSharedNoteAttachmentDownload(JSON.parse(text) as unknown);
    } catch {
      return null;
    }
  });

function setPrivateShareResponseHeaders() {
  setResponseHeader("Cache-Control", "private, no-store");
  setResponseHeader("Referrer-Policy", "no-referrer");
  setResponseHeader("X-Robots-Tag", "noindex, nofollow, noarchive, nosnippet");
}

function apiBaseUrl() {
  return env.VITE_API_URL.endsWith("/")
    ? env.VITE_API_URL
    : `${env.VITE_API_URL}/`;
}
