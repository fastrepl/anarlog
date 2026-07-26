import { createFileRoute } from "@tanstack/react-router";

import { fetchAdminUser } from "@/functions/admin";
import {
  getProjectItems,
  getProjectStatusField,
} from "@/functions/github-projects";

export const Route = createFileRoute("/api/admin/kanban/items")({
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
          const projectNumber = parseInt(
            url.searchParams.get("projectNumber") || "1",
            10,
          );
          const projectId = url.searchParams.get("projectId") || "";

          const [itemsResult, statusResult] = await Promise.all([
            getProjectItems(projectNumber),
            projectId
              ? getProjectStatusField(projectId)
              : Promise.resolve({ fieldId: "", options: [] }),
          ]);

          if (itemsResult.error) {
            return new Response(JSON.stringify({ error: itemsResult.error }), {
              status: 500,
              headers: { "Content-Type": "application/json" },
            });
          }

          return new Response(
            JSON.stringify({
              items: itemsResult.items,
              statusField: {
                fieldId: statusResult.fieldId,
                options: statusResult.options,
              },
            }),
            { status: 200, headers: { "Content-Type": "application/json" } },
          );
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
