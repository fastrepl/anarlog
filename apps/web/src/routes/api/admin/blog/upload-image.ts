import { createFileRoute } from "@tanstack/react-router";

import { fetchAdminUser } from "@/functions/admin";
import { getSupabaseServerClient } from "@/functions/supabase";
import { createSignedMediaUpload } from "@/functions/supabase-media";

export const Route = createFileRoute("/api/admin/blog/upload-image")({
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

        let body: { filename: string; folder?: string };
        try {
          body = await request.json();
        } catch {
          return new Response(JSON.stringify({ error: "Invalid JSON body" }), {
            status: 400,
            headers: { "Content-Type": "application/json" },
          });
        }

        const { filename, folder } = body;

        if (!filename) {
          return new Response(
            JSON.stringify({
              error: "Missing required field: filename",
            }),
            {
              status: 400,
              headers: { "Content-Type": "application/json" },
            },
          );
        }

        const supabase = getSupabaseServerClient();
        const result = await createSignedMediaUpload(supabase, {
          filename,
          folder: folder || "blog",
        });

        if (!result.success) {
          return new Response(JSON.stringify({ error: result.error }), {
            status: 500,
            headers: { "Content-Type": "application/json" },
          });
        }

        return new Response(
          JSON.stringify({
            path: result.path,
            publicUrl: result.publicUrl,
            proxyUrl: result.proxyUrl,
            token: result.token,
            signedUrl: result.signedUrl,
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
