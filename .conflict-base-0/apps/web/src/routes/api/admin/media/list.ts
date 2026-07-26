import { createFileRoute } from "@tanstack/react-router";

import { fetchAdminUser } from "@/functions/admin";
import { listMediaFiles } from "@/functions/supabase-media";

export const Route = createFileRoute("/api/admin/media/list")({
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
        const path = url.searchParams.get("path") || "";

        const result = await listMediaFiles(path);

        if (result.error) {
          return new Response(JSON.stringify({ error: result.error }), {
            status: 500,
            headers: { "Content-Type": "application/json" },
          });
        }

        return new Response(JSON.stringify({ items: result.items }), {
          status: 200,
          headers: { "Content-Type": "application/json" },
        });
      },
    },
  },
});
