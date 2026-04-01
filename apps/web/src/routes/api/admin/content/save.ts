import { createFileRoute } from "@tanstack/react-router";
import yaml from "js-yaml";

import { fetchAdminUser } from "@/functions/admin";
import {
  getCollectionFromPath,
  getFileContentFromBranch,
  parseMDX,
  savePublishedArticleToBranch,
  savePublishedContentToBranch,
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
  metadata: Record<string, unknown>;
  branch?: string;
  isAutoSave?: boolean;
}

function buildArticleFrontmatter(metadata: ArticleMetadata): string {
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

async function getExistingFrontmatter(
  path: string,
  branch?: string,
): Promise<Record<string, unknown>> {
  const result = await getFileContentFromBranch(path, branch || "main");
  if (!result.success || !result.content) {
    return {};
  }

  return parseMDX(result.content).frontmatter;
}

async function buildFullContent(
  path: string,
  content: string,
  metadata: Record<string, unknown>,
  branch?: string,
): Promise<{ fullContent: string; collection: string }> {
  const collection = getCollectionFromPath(path);
  if (!collection) {
    throw new Error(`Unsupported content collection for path: ${path}`);
  }

  if (collection === "articles") {
    const frontmatter = buildArticleFrontmatter(metadata as ArticleMetadata);
    return { fullContent: `${frontmatter}\n${content}`, collection };
  }

  const existingFrontmatter = await getExistingFrontmatter(path, branch);
  const nextFrontmatter = { ...existingFrontmatter };

  if (collection === "docs") {
    nextFrontmatter.title = (metadata.title as string | undefined) || "";
    nextFrontmatter.section = (metadata.section as string | undefined) || "";

    const description =
      (metadata.description as string | undefined) ||
      (metadata.summary as string | undefined) ||
      "";

    if (description) {
      nextFrontmatter.description = description;
    } else {
      delete nextFrontmatter.description;
    }
  }

  if (collection === "handbook") {
    nextFrontmatter.title = (metadata.title as string | undefined) || "";
    nextFrontmatter.section = (metadata.section as string | undefined) || "";

    const summary =
      (metadata.summary as string | undefined) ||
      (metadata.description as string | undefined) ||
      "";

    if (summary) {
      nextFrontmatter.summary = summary;
    } else {
      delete nextFrontmatter.summary;
    }
  }

  const frontmatter = `---\n${yaml.dump(nextFrontmatter, {
    quotingType: '"',
    forceQuotes: true,
    lineWidth: -1,
  })}---\n`;

  return { fullContent: `${frontmatter}\n${content}`, collection };
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

        let fullContent: string;
        let collection: string;

        try {
          const built = await buildFullContent(path, content, metadata, branch);
          fullContent = built.fullContent;
          collection = built.collection;
        } catch (error) {
          return new Response(
            JSON.stringify({
              error:
                error instanceof Error
                  ? error.message
                  : "Failed to build content",
            }),
            { status: 400, headers: { "Content-Type": "application/json" } },
          );
        }

        const shouldCreatePR = !branch;

        if (shouldCreatePR) {
          const result =
            collection === "articles"
              ? await savePublishedArticleToBranch(path, fullContent, {
                  meta_title: metadata.meta_title as string | undefined,
                  display_title: metadata.display_title as string | undefined,
                  author: metadata.author as string[] | undefined,
                })
              : await savePublishedContentToBranch(path, fullContent);

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
