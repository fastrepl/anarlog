import { createFileRoute } from "@tanstack/react-router";

import { fetchAdminUser } from "@/functions/admin";
import { getGitHubCredentials } from "@/functions/github-content";

interface CommitInfo {
  sha: string;
  message: string;
  author: string;
  date: string;
  url: string;
}

interface HistoryResponse {
  commits: CommitInfo[];
}

export const Route = createFileRoute("/api/admin/content/history")({
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

        const url = new URL(request.url);
        const filePath = url.searchParams.get("path");

        if (!filePath) {
          return new Response(
            JSON.stringify({ error: "Missing required parameter: path" }),
            { status: 400, headers: { "Content-Type": "application/json" } },
          );
        }

        const credentials = await getGitHubCredentials();
        if (!credentials) {
          return new Response(
            JSON.stringify({ error: "GitHub token not configured" }),
            { status: 500, headers: { "Content-Type": "application/json" } },
          );
        }
        const { token } = credentials;

        const owner = "fastrepl";
        const repo = "char";
        const branch = "main";
        const fullPath = filePath.startsWith("apps/web/content")
          ? filePath
          : `apps/web/content/${filePath}`;

        try {
          const response = await fetch(
            `https://api.github.com/repos/${owner}/${repo}/commits?path=${encodeURIComponent(fullPath)}&sha=${branch}&per_page=20`,
            {
              headers: {
                Authorization: `Bearer ${token}`,
                Accept: "application/vnd.github.v3+json",
              },
            },
          );

          if (!response.ok) {
            const error = await response.json();
            return new Response(
              JSON.stringify({
                error: error.message || `GitHub API error: ${response.status}`,
              }),
              { status: 500, headers: { "Content-Type": "application/json" } },
            );
          }

          const data = await response.json();

          const commits: CommitInfo[] = data.map(
            (commit: {
              sha: string;
              commit: {
                message: string;
                author: { name: string; date: string };
              };
              html_url: string;
            }) => ({
              sha: commit.sha.substring(0, 7),
              message: commit.commit.message.split("\n")[0],
              author: commit.commit.author.name,
              date: commit.commit.author.date,
              url: commit.html_url,
            }),
          );

          const result: HistoryResponse = { commits };

          return new Response(JSON.stringify(result), {
            status: 200,
            headers: { "Content-Type": "application/json" },
          });
        } catch (error) {
          return new Response(
            JSON.stringify({ error: (error as Error).message }),
            { status: 500, headers: { "Content-Type": "application/json" } },
          );
        }
      },
    },
  },
});
