import { createFileRoute } from "@tanstack/react-router";

import { fetchAdminUser } from "@/functions/admin";
import {
  closePullRequest,
  deleteBranch,
  deleteContentFile,
  ensureContentEditBranch,
  findExistingEditPRForPath,
  findOpenPullRequestByBranch,
  getCollectionFromPath,
} from "@/functions/github-content";

interface DeleteRequest {
  path: string;
  branch?: string;
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

            return new Response(
              JSON.stringify({
                success: true,
                mode: "discard-branch",
                branch,
                prNumber: pendingPR.prNumber,
                prUrl: pendingPR.prUrl,
              }),
              {
                status: 200,
                headers: { "Content-Type": "application/json" },
              },
            );
          }

          const pendingPR = await findExistingEditPRForPath(path);
          if (pendingPR.found && pendingPR.branchName) {
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

            return new Response(
              JSON.stringify({
                success: true,
                mode: "discard-branch",
                branch: pendingPR.branchName,
                prNumber: pendingPR.prNumber,
                prUrl: pendingPR.prUrl,
              }),
              {
                status: 200,
                headers: { "Content-Type": "application/json" },
              },
            );
          }
        }

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

        return new Response(
          JSON.stringify({
            success: true,
            mode: "delete-file",
            branch: targetBranch,
            prNumber: pendingPR?.prNumber,
            prUrl: pendingPR?.prUrl,
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
