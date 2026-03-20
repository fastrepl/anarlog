import { createFileRoute } from "@tanstack/react-router";

import { fetchAdminUser } from "@/functions/admin";
import { closeIssue, deleteProjectItem } from "@/functions/github-projects";

export const Route = createFileRoute("/api/admin/kanban/delete")({
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
            issueId: string;
            projectId?: string;
            itemId?: string;
          };

          if (!body.issueId) {
            return new Response(
              JSON.stringify({ error: "issueId is required" }),
              { status: 400, headers: { "Content-Type": "application/json" } },
            );
          }

          if (body.projectId && body.itemId) {
            await deleteProjectItem(body.projectId, body.itemId);
          }

          const result = await closeIssue(body.issueId);
          if (result.error) {
            return new Response(JSON.stringify({ error: result.error }), {
              status: 500,
              headers: { "Content-Type": "application/json" },
            });
          }

          return new Response(JSON.stringify({ success: true }), {
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
