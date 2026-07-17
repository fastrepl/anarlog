import { createFileRoute } from "@tanstack/react-router";

import { fetchAdminUser } from "@/functions/admin";
import { listStarLeads } from "@/functions/github-stars";

export const Route = createFileRoute("/api/admin/stars/leads")({
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

        try {
          const url = new URL(request.url);
          const limit = parseInt(url.searchParams.get("limit") || "50", 10);
          const offset = parseInt(url.searchParams.get("offset") || "0", 10);
          const researchedOnly = url.searchParams.get("researched") === "true";

          const { leads, total } = await listStarLeads({
            limit,
            offset,
            researchedOnly,
          });

          return new Response(JSON.stringify({ leads, total }), {
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
