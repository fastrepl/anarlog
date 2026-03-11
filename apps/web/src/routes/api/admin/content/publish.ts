import { createFileRoute } from "@tanstack/react-router";
import yaml from "js-yaml";

import { fetchAdminUser } from "@/functions/admin";
import {
  publishArticle,
  updateContentFileOnBranch,
} from "@/functions/github-content";
import { getSupabaseServerClient } from "@/functions/supabase";
import { uploadMediaFile } from "@/functions/supabase-media";

interface ArticleMetadata {
  meta_title?: string;
  display_title?: string;
  meta_description?: string;
  author?: string;
  date?: string;
  coverImage?: string;
  featured?: boolean;
  category?: string;
}

interface PublishRequest {
  path: string;
  content?: string;
  branch: string;
  metadata: ArticleMetadata;
  action?: "publish" | "unpublish";
}

export const Route = createFileRoute("/api/admin/content/publish")({
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

        let body: PublishRequest;
        try {
          body = await request.json();
        } catch {
          return new Response(JSON.stringify({ error: "Invalid JSON body" }), {
            status: 400,
            headers: { "Content-Type": "application/json" },
          });
        }

        const { path, content, branch, metadata, action = "publish" } = body;

        if (!path || !branch) {
          return new Response(
            JSON.stringify({
              error: "Missing required fields: path, branch",
            }),
            { status: 400, headers: { "Content-Type": "application/json" } },
          );
        }

        if (content !== undefined && metadata) {
          let processedContent = content;

          const base64ImageRegex =
            /!\[[^\]]*\]\((data:image\/([^;]+);base64,([^)]+))\)/g;
          const base64Images: Array<{
            fullMatch: string;
            mimeType: string;
            base64Data: string;
          }> = [];
          let match;
          while ((match = base64ImageRegex.exec(content)) !== null) {
            base64Images.push({
              fullMatch: match[0],
              mimeType: match[2],
              base64Data: match[3],
            });
          }

          if (base64Images.length > 0) {
            const supabase = getSupabaseServerClient();
            const slug =
              path
                .split("/")
                .pop()
                ?.replace(/\.mdx$/, "") || "";
            const folder = `articles/${slug}`;
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

            for (let i = 0; i < base64Images.length; i++) {
              const image = base64Images[i];
              const extension = extensionMap[image.mimeType] || "png";
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

          const frontmatterObj: Record<string, unknown> = {};
          if (metadata.meta_title)
            frontmatterObj.meta_title = metadata.meta_title;
          if (metadata.display_title)
            frontmatterObj.display_title = metadata.display_title;
          if (metadata.meta_description)
            frontmatterObj.meta_description = metadata.meta_description;
          if (metadata.author) frontmatterObj.author = metadata.author;
          if (metadata.coverImage)
            frontmatterObj.coverImage = metadata.coverImage;
          if (metadata.featured !== undefined)
            frontmatterObj.featured = metadata.featured;
          if (metadata.date) frontmatterObj.date = metadata.date;
          if (metadata.category) frontmatterObj.category = metadata.category;

          const frontmatter = `---\n${yaml.dump(frontmatterObj, { quotingType: '"', forceQuotes: true, lineWidth: -1 })}---`;
          const fullContent = `${frontmatter}\n\n${processedContent}`;

          const saveResult = await updateContentFileOnBranch(
            path,
            fullContent,
            branch,
          );

          if (!saveResult.success) {
            return new Response(
              JSON.stringify({
                error: `Failed to save content before publishing: ${saveResult.error}`,
              }),
              { status: 500, headers: { "Content-Type": "application/json" } },
            );
          }
        }

        const result = await publishArticle(
          path,
          branch,
          metadata || {},
          action,
        );

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
          }),
          { status: 200, headers: { "Content-Type": "application/json" } },
        );
      },
    },
  },
});
