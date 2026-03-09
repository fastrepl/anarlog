import { createFileRoute } from "@tanstack/react-router";

import { fetchAdminUser } from "@/functions/admin";
import {
  getFileContentFromBranch,
  listBlogBranches,
  parseMDX,
} from "@/functions/github-content";

interface DraftArticle {
  name: string;
  path: string;
  slug: string;
  branch: string;
  meta_title?: string;
  author?: string;
  date?: string;
  ready_for_review?: boolean;
}

export const Route = createFileRoute("/api/admin/content/list-drafts")({
  server: {
    handlers: {
      GET: async () => {
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
          const branchesResult = await listBlogBranches();
          if (!branchesResult.success) {
            return new Response(
              JSON.stringify({
                error: branchesResult.error || "Failed to list blog branches",
              }),
              {
                status: 500,
                headers: { "Content-Type": "application/json" },
              },
            );
          }

          const branches = branchesResult.branches || [];

          const drafts: DraftArticle[] = [];

          for (const branch of branches) {
            const slugMatch = branch.match(/^blog\/(.+)$/);
            if (!slugMatch) continue;

            const slug = slugMatch[1];
            const filename = `${slug}.mdx`;
            const filePath = `articles/${filename}`;

            const fileResult = await getFileContentFromBranch(filePath, branch);

            if (fileResult.success && fileResult.content) {
              const { frontmatter } = parseMDX(fileResult.content);

              drafts.push({
                name: filename,
                path: filePath,
                slug,
                branch,
                meta_title: frontmatter.meta_title as string | undefined,
                author: frontmatter.author as string | undefined,
                date: frontmatter.date as string | undefined,
                ready_for_review: frontmatter.ready_for_review as
                  | boolean
                  | undefined,
              });
            }
          }

          return new Response(JSON.stringify({ drafts }), {
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
