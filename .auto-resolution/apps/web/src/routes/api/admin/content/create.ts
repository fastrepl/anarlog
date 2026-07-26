import { createFileRoute } from "@tanstack/react-router";

import { fetchAdminUser } from "@/functions/admin";
import {
  createContentFileOnBranch,
  createContentFolder,
} from "@/functions/github-content";

interface CreateRequest {
  folder: string;
  name: string;
  type: "file" | "folder";
  content?: string;
}

export const Route = createFileRoute("/api/admin/content/create")({
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

        let body: CreateRequest;
        try {
          body = await request.json();
        } catch {
          return new Response(JSON.stringify({ error: "Invalid JSON body" }), {
            status: 400,
            headers: { "Content-Type": "application/json" },
          });
        }

        const { folder, name, type, content } = body;

        if (!folder || !name || !type) {
          return new Response(
            JSON.stringify({
              error: "Missing required fields: folder, name, type",
            }),
            { status: 400, headers: { "Content-Type": "application/json" } },
          );
        }

        const result =
          type === "folder"
            ? await createContentFolder(folder, name)
            : await createContentFileOnBranch(folder, name, content);

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
            branch: "branch" in result ? result.branch : undefined,
          }),
          { status: 200, headers: { "Content-Type": "application/json" } },
        );
      },
    },
  },
});
