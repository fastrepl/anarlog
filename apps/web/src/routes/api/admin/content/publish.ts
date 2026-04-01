import { createFileRoute } from "@tanstack/react-router";
import yaml from "js-yaml";

import { fetchAdminUser } from "@/functions/admin";
import {
  getCollectionFromPath,
  getFileContentFromBranch,
  parseMDX,
  publishContentPR,
  publishArticle,
  updateContentFileOnBranch,
} from "@/functions/github-content";
import { extractBase64Images } from "@/lib/media";

interface PublishRequest {
  path: string;
  content?: string;
  branch: string;
  metadata: Record<string, unknown>;
  action?: "publish" | "unpublish";
}

async function getExistingFrontmatter(
  path: string,
  branch: string,
): Promise<Record<string, unknown>> {
  const result = await getFileContentFromBranch(path, branch);
  if (!result.success || !result.content) {
    return {};
  }

  return parseMDX(result.content).frontmatter;
}

async function buildFullContent(
  path: string,
  content: string,
  metadata: Record<string, unknown>,
  branch: string,
): Promise<{ fullContent: string; collection: string }> {
  const collection = getCollectionFromPath(path);
  if (!collection) {
    throw new Error(`Unsupported content collection for path: ${path}`);
  }

  if (collection === "articles") {
    const frontmatterObj: Record<string, unknown> = {};
    if (metadata.meta_title) frontmatterObj.meta_title = metadata.meta_title;
    if (metadata.display_title)
      frontmatterObj.display_title = metadata.display_title;
    if (metadata.meta_description)
      frontmatterObj.meta_description = metadata.meta_description;
    if (metadata.author) frontmatterObj.author = metadata.author;
    if (metadata.coverImage) frontmatterObj.coverImage = metadata.coverImage;
    if (metadata.featured !== undefined)
      frontmatterObj.featured = metadata.featured;
    if (metadata.date) frontmatterObj.date = metadata.date;
    if (metadata.category) frontmatterObj.category = metadata.category;

    const frontmatter = `---\n${yaml.dump(frontmatterObj, {
      quotingType: '"',
      forceQuotes: true,
      lineWidth: -1,
    })}---`;

    return { fullContent: `${frontmatter}\n\n${content}`, collection };
  }

  const nextFrontmatter = {
    ...(await getExistingFrontmatter(path, branch)),
  };

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
  })}---`;

  return { fullContent: `${frontmatter}\n\n${content}`, collection };
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

        const collection = getCollectionFromPath(path);
        if (!collection) {
          return new Response(
            JSON.stringify({ error: `Unsupported content path: ${path}` }),
            { status: 400, headers: { "Content-Type": "application/json" } },
          );
        }

        if (content !== undefined && metadata) {
          if (extractBase64Images(content).length > 0) {
            return new Response(
              JSON.stringify({
                error:
                  "Inline base64 images must be uploaded before publishing",
              }),
              { status: 400, headers: { "Content-Type": "application/json" } },
            );
          }

          let fullContent: string;
          try {
            fullContent = (
              await buildFullContent(path, content, metadata, branch)
            ).fullContent;
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

        const result =
          collection === "articles"
            ? await publishArticle(path, branch, metadata || {}, action)
            : await publishContentPR(path, branch, {
                title: metadata.title as string | undefined,
                description: metadata.description as string | undefined,
                summary: metadata.summary as string | undefined,
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
          }),
          { status: 200, headers: { "Content-Type": "application/json" } },
        );
      },
    },
  },
});
