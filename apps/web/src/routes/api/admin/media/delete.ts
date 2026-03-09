import { createFileRoute } from "@tanstack/react-router";

import { fetchAdminUser } from "@/functions/admin";
import { getSupabaseServerClient } from "@/functions/supabase";
import { deleteMediaFiles } from "@/functions/supabase-media";

export const Route = createFileRoute("/api/admin/media/delete")({
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

        let body: { paths: string[] };
        try {
          body = await request.json();
        } catch {
          return new Response(JSON.stringify({ error: "Invalid JSON body" }), {
            status: 400,
            headers: { "Content-Type": "application/json" },
          });
        }

        const { paths } = body;

        if (!paths || !Array.isArray(paths) || paths.length === 0) {
          return new Response(
            JSON.stringify({ error: "Missing required field: paths (array)" }),
            {
              status: 400,
              headers: { "Content-Type": "application/json" },
            },
          );
        }

        const supabase = getSupabaseServerClient();
        const result = await deleteMediaFiles(supabase, paths);

        return new Response(
          JSON.stringify({
            success: result.success,
            deleted: result.deleted,
            errors: result.errors,
          }),
          {
            status: result.success ? 200 : 207,
            headers: { "Content-Type": "application/json" },
          },
        );
      },
    },
  },
});
