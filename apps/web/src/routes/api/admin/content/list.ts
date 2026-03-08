import { createFileRoute } from "@tanstack/react-router";

import { fetchAdminUser } from "@/functions/admin";
import { getGitHubCredentials } from "@/functions/github-content";

export const Route = createFileRoute("/api/admin/content/list")({
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
          const path = url.searchParams.get("path") || "";

          const owner = "fastrepl";
          const repo = "char";
          const branch = "main";
          const contentPath = `apps/web/content/${path}`;

          const credentials = await getGitHubCredentials();
          if (!credentials) {
            return new Response(
              JSON.stringify({
                error: "GitHub token not configured",
              }),
              { status: 500, headers: { "Content-Type": "application/json" } },
            );
          }
          const { token } = credentials;

          const response = await fetch(
            `https://api.github.com/repos/${owner}/${repo}/contents/${contentPath}?ref=${branch}`,
            {
              headers: {
                Authorization: `Bearer ${token}`,
                Accept: "application/vnd.github.v3+json",
              },
            },
          );

          if (!response.ok) {
            if (response.status === 404) {
              return new Response(JSON.stringify({ items: [] }), {
                status: 200,
                headers: { "Content-Type": "application/json" },
              });
            }
            return new Response(
              JSON.stringify({
                error: `GitHub API error: ${response.statusText}`,
              }),
              {
                status: response.status,
                headers: { "Content-Type": "application/json" },
              },
            );
          }

          const data = await response.json();

          const items = Array.isArray(data)
            ? data
                .filter(
                  (item: { name: string; type: string }) =>
                    item.name.endsWith(".mdx") || item.type === "dir",
                )
                .map(
                  (item: {
                    name: string;
                    path: string;
                    type: string;
                    sha: string;
                    html_url: string;
                  }) => ({
                    name: item.name,
                    path: item.path.replace("apps/web/content/", ""),
                    type: item.type === "dir" ? "dir" : "file",
                    sha: item.sha,
                    url: item.html_url,
                  }),
                )
            : [];

          return new Response(JSON.stringify({ items }), {
            status: 200,
            headers: { "Content-Type": "application/json" },
          });
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
