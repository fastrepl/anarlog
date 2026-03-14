import { createFileRoute } from "@tanstack/react-router";

import { fetchAdminUser } from "@/functions/admin";
import { deleteContentFile } from "@/functions/github-content";

interface DeleteRequest {
  path: string;
  branch?: string;
}

export const Route = createFileRoute("/api/admin/content/delete")({
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

        let body: DeleteRequest;
        try {
          body = await request.json();
        } catch {
          return new Response(JSON.stringify({ error: "Invalid JSON body" }), {
            status: 400,
            headers: { "Content-Type": "application/json" },
          });
        }

        const { path, branch } = body;

        if (!path) {
          return new Response(
            JSON.stringify({ error: "Missing required field: path" }),
            { status: 400, headers: { "Content-Type": "application/json" } },
          );
        }

        const result = await deleteContentFile(path, branch);

        if (!result.success) {
          return new Response(JSON.stringify({ error: result.error }), {
            status: 500,
            headers: { "Content-Type": "application/json" },
          });
        }

        return new Response(JSON.stringify({ success: true }), {
          status: 200,
          headers: { "Content-Type": "application/json" },
        });
      },
    },
  },
});
