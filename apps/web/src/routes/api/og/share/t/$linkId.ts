import { createFileRoute } from "@tanstack/react-router";

import { renderSharedNoteOgImage } from "@/lib/og-image";
import { fetchShortLinkSharedNotePreviewResult } from "@/lib/shared-note-api";
import { shareLinkIdSchema } from "@/lib/shared-notes";

export const Route = createFileRoute("/api/og/share/t/$linkId")({
  server: {
    handlers: {
      GET: async ({ params }) => {
        const linkId = shareLinkIdSchema.safeParse(params.linkId);
        if (!linkId.success) return notFound();

        const result = await fetchShortLinkSharedNotePreviewResult(linkId.data);
        if (result.status === "unavailable") return notFound();
        if (result.status === "error") return serviceUnavailable();

        return renderSharedNoteOgImage({
          title: result.preview.title || "Shared note",
          summary: result.preview.summary,
          participants: result.preview.participants,
          meetingAt: result.preview.meetingAt,
        });
      },
    },
  },
});

function notFound() {
  return new Response("Not found", {
    status: 404,
    headers: { "Cache-Control": "no-store" },
  });
}

function serviceUnavailable() {
  return new Response("Unable to load shared note", {
    status: 503,
    headers: { "Cache-Control": "no-store" },
  });
}
