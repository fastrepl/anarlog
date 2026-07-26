import { createFileRoute } from "@tanstack/react-router";

import { fetchAdminUser } from "@/functions/admin";
import { addIssueToProject, createIssue } from "@/functions/github-projects";

export const Route = createFileRoute("/api/admin/kanban/create")({
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
            title: string;
            body?: string;
            labels?: string[];
            projectId?: string;
          };

          if (!body.title) {
            return new Response(
              JSON.stringify({ error: "Title is required" }),
              { status: 400, headers: { "Content-Type": "application/json" } },
            );
          }

          const issueResult = await createIssue(
            body.title,
            body.body || "",
            body.labels,
          );

          if (issueResult.error) {
            return new Response(JSON.stringify({ error: issueResult.error }), {
              status: 500,
              headers: { "Content-Type": "application/json" },
            });
          }

          if (body.projectId && issueResult.issue) {
            const addResult = await addIssueToProject(
              body.projectId,
              issueResult.issue.id,
            );
            if (addResult.error) {
              return new Response(
                JSON.stringify({
                  issue: issueResult.issue,
                  warning: `Issue created but failed to add to project: ${addResult.error}`,
                }),
                {
                  status: 200,
                  headers: { "Content-Type": "application/json" },
                },
              );
            }
          }

          return new Response(JSON.stringify({ issue: issueResult.issue }), {
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
