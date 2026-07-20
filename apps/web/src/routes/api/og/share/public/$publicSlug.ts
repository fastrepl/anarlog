import { createFileRoute } from "@tanstack/react-router";

import { fetchPublicSharedNoteResult } from "@/lib/shared-note-api";
import { renderSharedNoteOgImage } from "@/lib/og-image";
import {
  getSharedNoteDescription,
  publicShareSlugSchema,
} from "@/lib/shared-notes";

export const Route = createFileRoute("/api/og/share/public/$publicSlug")({
  server: {
    handlers: {
      GET: async ({ params }) => {
        const publicSlug = publicShareSlugSchema.safeParse(params.publicSlug);
        if (!publicSlug.success) return notFound();

        const result = await fetchPublicSharedNoteResult(publicSlug.data);
        if (result.status !== "ready") return notFound();

        return renderSharedNoteOgImage({
          title: result.snapshot.title || "Shared note",
          description:
            getSharedNoteDescription(result.snapshot.body) || undefined,
          publishedAt: result.snapshot.publishedAt,
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
