import { createFileRoute } from "@tanstack/react-router";

import { fetchAdminUser } from "@/functions/admin";
import {
  duplicateContentFile,
  ensureContentEditBranch,
  getCollectionFromPath,
} from "@/functions/github-content";

interface DuplicateRequest {
  sourcePath: string;
  newFilename?: string;
  branch?: string;
}

export const Route = createFileRoute("/api/admin/content/duplicate")({
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

        let body: DuplicateRequest;
        try {
          body = await request.json();
        } catch {
          return new Response(JSON.stringify({ error: "Invalid JSON body" }), {
            status: 400,
            headers: { "Content-Type": "application/json" },
          });
        }

        const { sourcePath, newFilename, branch } = body;

        if (!sourcePath) {
          return new Response(
            JSON.stringify({ error: "Missing required field: sourcePath" }),
            { status: 400, headers: { "Content-Type": "application/json" } },
          );
        }

        const collection = getCollectionFromPath(sourcePath);
        let targetBranch = branch;
        let pendingPR: Awaited<
          ReturnType<typeof ensureContentEditBranch>
        > | null = null;

        if (!targetBranch && collection && collection !== "articles") {
          pendingPR = await ensureContentEditBranch(sourcePath);
          if (!pendingPR.success || !pendingPR.branchName) {
            return new Response(JSON.stringify({ error: pendingPR.error }), {
              status: 500,
              headers: { "Content-Type": "application/json" },
            });
          }
          targetBranch = pendingPR.branchName;
        }

        const result = await duplicateContentFile(
          sourcePath,
          newFilename,
          targetBranch,
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
            path: result.path,
            branch: targetBranch,
            prNumber: pendingPR?.prNumber,
            prUrl: pendingPR?.prUrl,
          }),
          { status: 200, headers: { "Content-Type": "application/json" } },
        );
      },
    },
  },
});
