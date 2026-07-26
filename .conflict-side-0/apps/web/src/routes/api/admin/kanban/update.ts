import { createFileRoute } from "@tanstack/react-router";

import { fetchAdminUser } from "@/functions/admin";
import { updateIssue, updateItemStatus } from "@/functions/github-projects";

export const Route = createFileRoute("/api/admin/kanban/update")({
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
            issueId?: string;
            title?: string;
            body?: string;
            projectId?: string;
            itemId?: string;
            fieldId?: string;
            optionId?: string;
          };

          if (body.issueId && (body.title || body.body)) {
            const result = await updateIssue(
              body.issueId,
              body.title,
              body.body,
            );
            if (result.error) {
              return new Response(JSON.stringify({ error: result.error }), {
                status: 500,
                headers: { "Content-Type": "application/json" },
              });
            }
          }

          if (body.projectId && body.itemId && body.fieldId && body.optionId) {
            const result = await updateItemStatus(
              body.projectId,
              body.itemId,
              body.fieldId,
              body.optionId,
            );
            if (result.error) {
              return new Response(JSON.stringify({ error: result.error }), {
                status: 500,
                headers: { "Content-Type": "application/json" },
              });
            }
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
