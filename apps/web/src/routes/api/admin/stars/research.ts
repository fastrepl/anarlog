import { createFileRoute } from "@tanstack/react-router";

import { fetchAdminUser } from "@/functions/admin";
import { researchLead } from "@/functions/github-stars";

export const Route = createFileRoute("/api/admin/stars/research")({
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
          const body = (await request.json()) as {
            username: string;
            apiKey?: string;
          };
          const { username, apiKey } = body;

          if (!username) {
            return new Response(
              JSON.stringify({ error: "username is required" }),
              {
                status: 400,
                headers: { "Content-Type": "application/json" },
              },
            );
          }

          const openrouterKey = apiKey || process.env.OPENROUTER_API_KEY || "";
          if (!openrouterKey) {
            return new Response(
              JSON.stringify({
                error:
                  "OpenRouter API key is required. Set OPENROUTER_API_KEY env var or pass apiKey in body.",
              }),
              {
                status: 400,
                headers: { "Content-Type": "application/json" },
              },
            );
          }

          const result = await researchLead(username, openrouterKey);

          return new Response(JSON.stringify(result), {
            status: result.success ? 200 : 400,
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
