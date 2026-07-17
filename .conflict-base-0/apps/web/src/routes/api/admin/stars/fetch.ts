import { createFileRoute } from "@tanstack/react-router";

import { fetchAdminUser } from "@/functions/admin";
import {
  fetchGitHubActivity,
  fetchGitHubStargazers,
} from "@/functions/github-stars";

export const Route = createFileRoute("/api/admin/stars/fetch")({
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

        try {
          const body = await request.json().catch(() => ({}));
          const source = (body as { source?: string }).source || "stargazers";

          let result;
          if (source === "activity") {
            result = await fetchGitHubActivity();
          } else {
            result = await fetchGitHubStargazers();
          }

          return new Response(JSON.stringify(result), {
            status: 200,
            headers: { "Content-Type": "application/json" },
          });
        } catch (err) {
          return new Response(
            JSON.stringify({ error: (err as Error).message }),
            { status: 500, headers: { "Content-Type": "application/json" } },
          );
        }
      },
    },
  },
});
