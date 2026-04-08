import { createFileRoute } from "@tanstack/react-router";

import { fetchAdminUser } from "@/functions/admin";
import { auditArticleContent } from "@/functions/content-audit";
import { getCollectionFromPath } from "@/functions/github-content";

interface AuditRequest {
  path: string;
  content: string;
  metadata?: Record<string, unknown>;
}

export const Route = createFileRoute("/api/admin/content/audit")({
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

        let body: AuditRequest;
        try {
          body = await request.json();
        } catch {
          return new Response(JSON.stringify({ error: "Invalid JSON body" }), {
            status: 400,
            headers: { "Content-Type": "application/json" },
          });
        }

        const { path, content, metadata = {} } = body;

        if (!path || !content) {
          return new Response(
            JSON.stringify({ error: "Missing required fields: path, content" }),
            {
              status: 400,
              headers: { "Content-Type": "application/json" },
            },
          );
        }

        if (getCollectionFromPath(path) !== "articles") {
          return new Response(
            JSON.stringify({ error: "Audit is only available for articles" }),
            {
              status: 400,
              headers: { "Content-Type": "application/json" },
            },
          );
        }

        const openrouterApiKey = process.env.OPENROUTER_API_KEY || "";
        if (!openrouterApiKey) {
          return new Response(
            JSON.stringify({
              error:
                "OpenRouter API key is required. Set OPENROUTER_API_KEY to enable article audit.",
            }),
            {
              status: 400,
              headers: { "Content-Type": "application/json" },
            },
          );
        }

        const result = await auditArticleContent({
          path,
          content,
          metadata,
          openrouterApiKey,
        });

        return new Response(JSON.stringify(result), {
          status: result.success ? 200 : 400,
          headers: { "Content-Type": "application/json" },
        });
      },
    },
  },
});
