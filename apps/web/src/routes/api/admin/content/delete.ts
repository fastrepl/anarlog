import { createFileRoute } from "@tanstack/react-router";
import { allArticles, allDocs, allHandbooks } from "content-collections";

import { fetchAdminUser } from "@/functions/admin";
import {
  closePullRequest,
  deleteBranch,
  deleteContentFile,
  ensureContentEditBranch,
  findExistingEditPRForPath,
  findOpenPullRequestByBranch,
  getFileContentFromBranch,
  getCollectionFromPath,
} from "@/functions/github-content";
import { deleteCatalogMediaAssets } from "@/functions/media-catalog";
import { getSupabaseServerClient } from "@/functions/supabase";
import {
  deleteMediaFiles,
  invalidateMediaListCache,
} from "@/functions/supabase-media";
import { extractManagedMediaPaths } from "@/lib/media";

interface DeleteRequest {
  path: string;
  branch?: string;
}

function getPublishedReferencedMediaPaths(excludePath?: string) {
  const referencedPaths = new Set<string>();

  for (const article of allArticles) {
    const path = `articles/${article._meta.fileName}`;
    if (path === excludePath) {
      continue;
    }

    for (const mediaPath of extractManagedMediaPaths(
      [article.content, article.coverImage].filter(Boolean).join("\n"),
    )) {
      referencedPaths.add(mediaPath);
    }
  }

  for (const doc of allDocs) {
    const path = `docs/${doc._meta.filePath}`;
    if (path === excludePath) {
      continue;
    }

    for (const mediaPath of extractManagedMediaPaths(doc.content)) {
      referencedPaths.add(mediaPath);
    }
  }

  for (const handbook of allHandbooks) {
    const path = `handbook/${handbook._meta.filePath}`;
    if (path === excludePath) {
      continue;
    }

    for (const mediaPath of extractManagedMediaPaths(handbook.content)) {
      referencedPaths.add(mediaPath);
    }
  }

  return referencedPaths;
}

async function getContentOwnedMediaPaths(params: {
  path: string;
  ref: string;
  excludePublishedPath?: string;
}) {
  const fileResult = await getFileContentFromBranch(params.path, params.ref);
  if (!fileResult.success || !fileResult.content) {
    return [];
  }

  const publishedReferences = getPublishedReferencedMediaPaths(
    params.excludePublishedPath,
  );

  return extractManagedMediaPaths(fileResult.content).filter(
    (mediaPath) => !publishedReferences.has(mediaPath),
  );
}

async function cleanupDeletedContentMedia(paths: string[]) {
  if (paths.length === 0) {
    return { deleted: [] as string[], errors: [] as string[] };
  }

  const supabase = getSupabaseServerClient();
  const result = await deleteMediaFiles(supabase, paths);
  await deleteCatalogMediaAssets(supabase, result.deleted);
  invalidateMediaListCache(paths);

  return {
    deleted: result.deleted,
    errors: result.errors,
  };
}

export const Route = createFileRoute("/api/admin/content/delete")({
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

        let body: DeleteRequest;
        try {
          body = await request.json();
        } catch {
          return new Response(JSON.stringify({ error: "Invalid JSON body" }), {
            status: 400,
            headers: { "Content-Type": "application/json" },
          });
        }

        const { path, branch } = body;

        if (!path) {
          return new Response(
            JSON.stringify({ error: "Missing required field: path" }),
            { status: 400, headers: { "Content-Type": "application/json" } },
          );
        }

        const collection = getCollectionFromPath(path);

        if (!isDev && collection) {
          if (branch) {
            const mediaPaths = await getContentOwnedMediaPaths({
              path,
              ref: branch,
            });
            const pendingPR = await findOpenPullRequestByBranch(branch);

            if (pendingPR.found && pendingPR.prNumber) {
              const closeResult = await closePullRequest(pendingPR.prNumber);
              if (!closeResult.success) {
                return new Response(
                  JSON.stringify({ error: closeResult.error }),
                  {
                    status: 500,
                    headers: { "Content-Type": "application/json" },
                  },
                );
              }
            }

            const deleteBranchResult = await deleteBranch(branch);
            if (!deleteBranchResult.success) {
              return new Response(
                JSON.stringify({ error: deleteBranchResult.error }),
                {
                  status: 500,
                  headers: { "Content-Type": "application/json" },
                },
              );
            }

            const mediaCleanup = await cleanupDeletedContentMedia(mediaPaths);

            return new Response(
              JSON.stringify({
                success: true,
                mode: "discard-branch",
                branch,
                prNumber: pendingPR.prNumber,
                prUrl: pendingPR.prUrl,
                mediaDeleted: mediaCleanup.deleted,
                mediaCleanupErrors: mediaCleanup.errors,
              }),
              {
                status: 200,
                headers: { "Content-Type": "application/json" },
              },
            );
          }

          const pendingPR = await findExistingEditPRForPath(path);
          if (pendingPR.found && pendingPR.branchName) {
            const mediaPaths = await getContentOwnedMediaPaths({
              path,
              ref: pendingPR.branchName,
            });
            if (pendingPR.prNumber) {
              const closeResult = await closePullRequest(pendingPR.prNumber);
              if (!closeResult.success) {
                return new Response(
                  JSON.stringify({ error: closeResult.error }),
                  {
                    status: 500,
                    headers: { "Content-Type": "application/json" },
                  },
                );
              }
            }

            const deleteBranchResult = await deleteBranch(pendingPR.branchName);
            if (!deleteBranchResult.success) {
              return new Response(
                JSON.stringify({ error: deleteBranchResult.error }),
                {
                  status: 500,
                  headers: { "Content-Type": "application/json" },
                },
              );
            }

            const mediaCleanup = await cleanupDeletedContentMedia(mediaPaths);

            return new Response(
              JSON.stringify({
                success: true,
                mode: "discard-branch",
                branch: pendingPR.branchName,
                prNumber: pendingPR.prNumber,
                prUrl: pendingPR.prUrl,
                mediaDeleted: mediaCleanup.deleted,
                mediaCleanupErrors: mediaCleanup.errors,
              }),
              {
                status: 200,
                headers: { "Content-Type": "application/json" },
              },
            );
          }
        }

        const mediaPaths = await getContentOwnedMediaPaths({
          path,
          ref: "main",
          excludePublishedPath: path,
        });

        let targetBranch = branch;
        let pendingPR: Awaited<
          ReturnType<typeof ensureContentEditBranch>
        > | null = null;

        if (!targetBranch && collection && collection !== "articles") {
          pendingPR = await ensureContentEditBranch(path);
          if (!pendingPR.success || !pendingPR.branchName) {
            return new Response(JSON.stringify({ error: pendingPR.error }), {
              status: 500,
              headers: { "Content-Type": "application/json" },
            });
          }
          targetBranch = pendingPR.branchName;
        }

        const result = await deleteContentFile(path, targetBranch);

        if (!result.success) {
          return new Response(JSON.stringify({ error: result.error }), {
            status: 500,
            headers: { "Content-Type": "application/json" },
          });
        }

        const mediaCleanup = await cleanupDeletedContentMedia(mediaPaths);

        return new Response(
          JSON.stringify({
            success: true,
            mode: "delete-file",
            branch: targetBranch,
            prNumber: pendingPR?.prNumber,
            prUrl: pendingPR?.prUrl,
            mediaDeleted: mediaCleanup.deleted,
            mediaCleanupErrors: mediaCleanup.errors,
          }),
          {
            status: 200,
            headers: { "Content-Type": "application/json" },
          },
        );
      },
    },
  },
});
