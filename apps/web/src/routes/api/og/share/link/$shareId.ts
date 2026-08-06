import { createFileRoute } from "@tanstack/react-router";

import { renderSharedNoteOgImage } from "@/lib/og-image";
import { fetchLinkSharedNotePreviewResult } from "@/lib/shared-note-api";
import { linkSharePreviewTokenSchema, shareIdSchema } from "@/lib/shared-notes";

export const Route = createFileRoute("/api/og/share/link/$shareId")({
  server: {
    handlers: {
      GET: async ({ params, request }) => {
        const shareId = shareIdSchema.safeParse(params.shareId);
        const previewToken = linkSharePreviewTokenSchema.safeParse(
          new URL(request.url).searchParams.get("preview"),
        );
        if (!shareId.success || !previewToken.success) return notFound();

        const result = await fetchLinkSharedNotePreviewResult(
          shareId.data,
          previewToken.data,
        );
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
