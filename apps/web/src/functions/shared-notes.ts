import { createServerFn } from "@tanstack/react-start";
import { setResponseHeader } from "@tanstack/react-start/server";

import { getSupabaseServerClient } from "@/functions/supabase";
import { fetchPublicSharedNote } from "@/lib/shared-note-api";
import {
  parseAuthenticatedSharedNote,
  publicShareSlugSchema,
  shareIdSchema,
} from "@/lib/shared-notes";

export const readAuthenticatedSharedNote = createServerFn({ method: "GET" })
  .inputValidator(shareIdSchema)
  .handler(async ({ data: shareId }) => {
    setPrivateShareResponseHeaders();

    const supabase = getSupabaseServerClient();
    const { data, error } = await supabase.rpc(
      "read_my_session_share_snapshot",
      { p_share_id: shareId },
    );
    if (error || !Array.isArray(data) || data.length !== 1) {
      return null;
    }

    try {
      return parseAuthenticatedSharedNote(data[0]);
    } catch {
      return null;
    }
  });

export const readPublicSharedNote = createServerFn({ method: "GET" })
  .inputValidator(publicShareSlugSchema)
  .handler(async ({ data: publicSlug }) => {
    setResponseHeader("Cache-Control", "no-store");
    setResponseHeader("Referrer-Policy", "no-referrer");
    return fetchPublicSharedNote(publicSlug);
  });

function setPrivateShareResponseHeaders() {
  setResponseHeader("Cache-Control", "private, no-store");
  setResponseHeader("Referrer-Policy", "no-referrer");
  setResponseHeader("X-Robots-Tag", "noindex, nofollow, noarchive, nosnippet");
}
