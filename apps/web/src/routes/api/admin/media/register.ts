import { createFileRoute } from "@tanstack/react-router";

import { fetchAdminUser } from "@/functions/admin";
import { registerStorageMediaAsset } from "@/functions/media-catalog";
import { getSupabaseServerClient } from "@/functions/supabase";
import { invalidateMediaListCache } from "@/functions/supabase-media";
import { getMediaFolderFromPath } from "@/lib/media-library";

export const Route = createFileRoute("/api/admin/media/register")({
  server: {
    handlers: {
      POST: async ({ request }) => {
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

        let body: {
          path?: string;
          publicUrl?: string;
          mimeType?: string | null;
          size?: number;
        };
        try {
          body = await request.json();
        } catch {
          return new Response(JSON.stringify({ error: "Invalid JSON body" }), {
            status: 400,
            headers: { "Content-Type": "application/json" },
          });
        }

        if (!body.path || !body.publicUrl) {
          return new Response(
            JSON.stringify({
              error: "Missing required fields: path, publicUrl",
            }),
            {
              status: 400,
              headers: { "Content-Type": "application/json" },
            },
          );
        }

        const supabase = getSupabaseServerClient();
        await registerStorageMediaAsset(supabase, {
          path: body.path,
          publicUrl: body.publicUrl,
          mimeType: body.mimeType || null,
          size: body.size || 0,
        });
        invalidateMediaListCache([getMediaFolderFromPath(body.path)]);

        return new Response(JSON.stringify({ success: true }), {
          status: 200,
          headers: { "Content-Type": "application/json" },
        });
      },
    },
  },
});
