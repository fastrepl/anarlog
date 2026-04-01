import { createFileRoute } from "@tanstack/react-router";

import { fetchAdminUser } from "@/functions/admin";
import { listMediaFiles } from "@/functions/supabase-media";
import { getMediaFolderFromPath } from "@/lib/media-library";

function getAttachmentDisposition(filename: string) {
  const encodedFilename = encodeURIComponent(filename);
  const safeFilename = filename.replace(/["\\]/g, "_");

  return `attachment; filename="${safeFilename}"; filename*=UTF-8''${encodedFilename}`;
}

export const Route = createFileRoute("/api/admin/media/download")({
  server: {
    handlers: {
      GET: async ({ request }) => {
        const isDev = process.env.NODE_ENV === "development";
        if (!isDev) {
          const user = await fetchAdminUser();
          if (!user?.isAdmin) {
            return new Response(JSON.stringify({ error: "Unauthorized" }), {
              status: 401,
              headers: { "Content-Type": "application/json" },
            });
          }
        }

        const url = new URL(request.url);
        const path = url.searchParams.get("path")?.trim();

        if (!path) {
          return new Response(JSON.stringify({ error: "Missing path" }), {
            status: 400,
            headers: { "Content-Type": "application/json" },
          });
        }

        const folder = getMediaFolderFromPath(path);
        const result = await listMediaFiles(folder);

        if (result.error) {
          return new Response(JSON.stringify({ error: result.error }), {
            status: 500,
            headers: { "Content-Type": "application/json" },
          });
        }

        const item = result.items.find(
          (mediaItem) => mediaItem.type === "file" && mediaItem.path === path,
        );

        if (!item?.publicUrl) {
          return new Response(JSON.stringify({ error: "File not found" }), {
            status: 404,
            headers: { "Content-Type": "application/json" },
          });
        }

        const upstream = await fetch(item.publicUrl);

        if (!upstream.ok) {
          return new Response(
            JSON.stringify({ error: "Failed to fetch media asset" }),
            {
              status: 502,
              headers: { "Content-Type": "application/json" },
            },
          );
        }

        return new Response(upstream.body, {
          status: 200,
          headers: {
            "Cache-Control": "private, no-store",
            "Content-Disposition": getAttachmentDisposition(item.name),
            "Content-Type":
              upstream.headers.get("content-type") ||
              item.mimeType ||
              "application/octet-stream",
          },
        });
      },
    },
  },
});
