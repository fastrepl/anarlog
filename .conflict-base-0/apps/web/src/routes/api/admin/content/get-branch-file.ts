import { createFileRoute } from "@tanstack/react-router";

import { fetchAdminUser } from "@/functions/admin";
import { getFileContentFromBranch, parseMDX } from "@/functions/github-content";

export const Route = createFileRoute("/api/admin/content/get-branch-file")({
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
          const path = url.searchParams.get("path");
          const branch = url.searchParams.get("branch");

          if (!path || !branch) {
            return new Response(
              JSON.stringify({
                error: "Missing required parameters: path, branch",
              }),
              { status: 400, headers: { "Content-Type": "application/json" } },
            );
          }

          const result = await getFileContentFromBranch(path, branch);

          if (!result.success || !result.content) {
            return new Response(JSON.stringify({ error: result.error }), {
              status: 404,
              headers: { "Content-Type": "application/json" },
            });
          }

          const { frontmatter, content } = parseMDX(result.content);

          return new Response(
            JSON.stringify({
              success: true,
              content,
              frontmatter,
              sha: result.sha,
            }),
            { status: 200, headers: { "Content-Type": "application/json" } },
          );
        } catch (err) {
          return new Response(
            JSON.stringify({
              error: (err as Error).message,
            }),
            { status: 500, headers: { "Content-Type": "application/json" } },
          );
        }
      },
    },
  },
});
