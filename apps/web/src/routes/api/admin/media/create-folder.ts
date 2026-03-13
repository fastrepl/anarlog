import { createFileRoute } from "@tanstack/react-router";

import { fetchAdminUser } from "@/functions/admin";
import { getSupabaseServerClient } from "@/functions/supabase";
import { createMediaFolder } from "@/functions/supabase-media";

export const Route = createFileRoute("/api/admin/media/create-folder")({
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

        let body: { name: string; parentFolder?: string };
        try {
          body = await request.json();
        } catch {
          return new Response(JSON.stringify({ error: "Invalid JSON body" }), {
            status: 400,
            headers: { "Content-Type": "application/json" },
          });
        }

        const { name, parentFolder } = body;

        if (!name) {
          return new Response(
            JSON.stringify({ error: "Missing required field: name" }),
            {
              status: 400,
              headers: { "Content-Type": "application/json" },
            },
          );
        }

        const supabase = getSupabaseServerClient();
        const result = await createMediaFolder(
          supabase,
          name,
          parentFolder || "",
        );

        if (!result.success) {
          return new Response(JSON.stringify({ error: result.error }), {
            status: 500,
            headers: { "Content-Type": "application/json" },
          });
        }

        return new Response(
          JSON.stringify({
            success: true,
            path: result.path,
          }),
          {
            status: 200,
            headers: { "Content-Type": "application/json" },
          },
        );
      },
    },
  },
});
