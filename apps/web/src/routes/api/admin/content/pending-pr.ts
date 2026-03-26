import { createFileRoute } from "@tanstack/react-router";

import { fetchAdminUser } from "@/functions/admin";
import { getExistingEditPRForContent } from "@/functions/github-content";

export const Route = createFileRoute("/api/admin/content/pending-pr")({
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
        const path = url.searchParams.get("path");

        if (!path) {
          return new Response(
            JSON.stringify({ error: "Missing path parameter" }),
            { status: 400, headers: { "Content-Type": "application/json" } },
          );
        }

        const result = await getExistingEditPRForContent(path);

        if (!result.success) {
          return new Response(JSON.stringify({ error: result.error }), {
            status: 500,
            headers: { "Content-Type": "application/json" },
          });
        }

        return new Response(
          JSON.stringify({
            hasPendingPR: result.hasPendingPR,
            prNumber: result.prNumber,
            prUrl: result.prUrl,
            branchName: result.branchName,
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
