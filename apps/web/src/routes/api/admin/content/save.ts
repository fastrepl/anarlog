import { createFileRoute } from "@tanstack/react-router";

import { fetchAdminUser } from "@/functions/admin";
import {
  savePublishedArticleToBranch,
  updateContentFileOnBranch,
} from "@/functions/github-content";
import { getSupabaseServerClient } from "@/functions/supabase";
import { uploadMediaFile } from "@/functions/supabase-media";

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

interface Base64Image {
  fullMatch: string;
  mimeType: string;
  base64Data: string;
}

export function extractBase64Images(markdown: string): Base64Image[] {
  const regex = /!\[[^\]]*\]\((data:image\/([^;]+);base64,([^)]+))\)/g;
  const images: Base64Image[] = [];
  let match;

  while ((match = regex.exec(markdown)) !== null) {
    images.push({
      fullMatch: match[0],
      mimeType: match[2],
      base64Data: match[3],
    });
  }

  return images;
}

export function getExtensionFromMimeType(mimeType: string): string {
  const extensionMap: Record<string, string> = {
    jpeg: "jpg",
    jpg: "jpg",
    png: "png",
    gif: "gif",
    webp: "webp",
    svg: "svg",
    "svg+xml": "svg",
    avif: "avif",
  };
  return extensionMap[mimeType] || "png";
}

function extractSlugFromPath(path: string): string {
  const filename = path.split("/").pop() || "";
  return filename.replace(/\.mdx$/, "");
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

        let processedContent = content;

        const base64Images = extractBase64Images(content);
        if (base64Images.length > 0) {
          const supabase = getSupabaseServerClient();
          const slug = extractSlugFromPath(path);
          const folder = `articles/${slug}`;

          for (let i = 0; i < base64Images.length; i++) {
            const image = base64Images[i];
            const extension = getExtensionFromMimeType(image.mimeType);
            const filename = `image-${i + 1}.${extension}`;

            const uploadResult = await uploadMediaFile(
              supabase,
              filename,
              image.base64Data,
              folder,
            );

            if (uploadResult.success && uploadResult.publicUrl) {
              processedContent = processedContent.replace(
                image.fullMatch,
                `![](${uploadResult.publicUrl})`,
              );
            }
          }
        }

        const frontmatter = buildFrontmatter(metadata);
        const fullContent = `${frontmatter}\n${processedContent}`;

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
