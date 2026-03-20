import { createFileRoute } from "@tanstack/react-router";

import { fetchAdminUser } from "@/functions/admin";
import {
  savePublishedArticleToBranch,
  updateContentFileOnBranch,
} from "@/functions/github-content";
import { extractBase64Images } from "@/lib/media";

interface ArticleMetadata {
  meta_title?: string;
  display_title?: string;
  meta_description?: string;
  author?: string[];
  date?: string;
  coverImage?: string;
  featured?: boolean;
  category?: string;
}

interface SaveRequest {
  path: string;
  content: string;
  metadata: ArticleMetadata;
  branch?: string;
  isAutoSave?: boolean;
}

function buildFrontmatter(metadata: ArticleMetadata): string {
  // Build frontmatter in specific order:
  // meta_title, display_title, meta_description, author, featured, published, category, date
  const lines: string[] = [];

  if (metadata.meta_title) {
    lines.push(`meta_title: ${JSON.stringify(metadata.meta_title)}`);
  }
  if (metadata.display_title) {
    lines.push(`display_title: ${JSON.stringify(metadata.display_title)}`);
  }
  if (metadata.meta_description) {
    lines.push(
      `meta_description: ${JSON.stringify(metadata.meta_description)}`,
    );
  }
  if (metadata.author && metadata.author.length > 0) {
    lines.push(`author:`);
    for (const name of metadata.author) {
      lines.push(`  - ${JSON.stringify(name)}`);
    }
  }
  if (metadata.coverImage) {
    lines.push(`coverImage: ${JSON.stringify(metadata.coverImage)}`);
  }
  if (metadata.featured !== undefined) {
    lines.push(`featured: ${metadata.featured}`);
  }
  if (metadata.category) {
    lines.push(`category: ${JSON.stringify(metadata.category)}`);
  }
  if (metadata.date) {
    lines.push(`date: ${JSON.stringify(metadata.date)}`);
  }

  return `---\n${lines.join("\n")}\n---\n`;
}

export const Route = createFileRoute("/api/admin/content/save")({
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

        let body: SaveRequest;
        try {
          body = await request.json();
        } catch {
          return new Response(JSON.stringify({ error: "Invalid JSON body" }), {
            status: 400,
            headers: { "Content-Type": "application/json" },
          });
        }

        const { path, content, metadata, branch, isAutoSave } = body;

        if (!path || content === undefined || !metadata) {
          return new Response(
            JSON.stringify({
              error: "Missing required fields: path, content, metadata",
            }),
            { status: 400, headers: { "Content-Type": "application/json" } },
          );
        }

        if (extractBase64Images(content).length > 0) {
          return new Response(
            JSON.stringify({
              error: "Inline base64 images must be uploaded before saving",
            }),
            { status: 400, headers: { "Content-Type": "application/json" } },
          );
        }

        const frontmatter = buildFrontmatter(metadata);
        const fullContent = `${frontmatter}\n${content}`;

        // If there's no branch, the article is on main, so create a PR (handles branch protection)
        // Otherwise, save directly to the draft branch
        const shouldCreatePR = !branch;

        if (shouldCreatePR) {
          const result = await savePublishedArticleToBranch(path, fullContent, {
            meta_title: metadata.meta_title,
            display_title: metadata.display_title,
            author: metadata.author,
          });

          if (!result.success) {
            return new Response(JSON.stringify({ error: result.error }), {
              status: 500,
              headers: { "Content-Type": "application/json" },
            });
          }

          return new Response(
            JSON.stringify({
              success: true,
              prNumber: result.prNumber,
              prUrl: result.prUrl,
              branchName: result.branchName,
              isAutoSave,
            }),
            {
              status: 200,
              headers: { "Content-Type": "application/json" },
            },
          );
        }

        const result = await updateContentFileOnBranch(
          path,
          fullContent,
          branch!,
        );

        if (!result.success) {
          return new Response(JSON.stringify({ error: result.error }), {
            status: 500,
            headers: { "Content-Type": "application/json" },
          });
        }

        return new Response(JSON.stringify({ success: true }), {
          status: 200,
          headers: { "Content-Type": "application/json" },
        });
      },
    },
  },
});
