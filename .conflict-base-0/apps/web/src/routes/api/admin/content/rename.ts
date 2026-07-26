import { createFileRoute } from "@tanstack/react-router";
import yaml from "js-yaml";

import { fetchAdminUser } from "@/functions/admin";
import {
  ensureContentEditBranch,
  getCollectionFromPath,
  getFileContentFromBranch,
  parseMDX,
  renameContentFile,
  updateContentFileOnBranch,
} from "@/functions/github-content";

interface RenameRequest {
  fromPath: string;
  toPath: string;
  branch?: string;
}

const DOCS_SECTION_TITLES: Record<string, string> = {
  about: "About",
  "getting-started": "Getting Started",
  guides: "Guides",
  calendar: "Calendar",
  cli: "CLI",
  developers: "Developers",
  pro: "Pro",
  faq: "FAQ",
};

const HANDBOOK_SECTION_TITLES: Record<string, string> = {
  about: "About",
  "how-we-work": "How We Work",
  teams: "Teams",
  "who-we-want": "Who We Want",
  "go-to-market": "Go To Market",
  onboarding: "Onboarding",
};

function getSectionTitle(
  collection: "docs" | "handbook",
  sectionFolder: string,
): string {
  if (collection === "docs") {
    return (
      DOCS_SECTION_TITLES[sectionFolder] ||
      sectionFolder
        .split("-")
        .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
        .join(" ")
    );
  }

  return HANDBOOK_SECTION_TITLES[sectionFolder] || sectionFolder;
}

async function syncStructuredSectionFrontmatter(
  path: string,
  branch: string,
  collection: "docs" | "handbook",
): Promise<{ success: boolean; error?: string }> {
  const [, sectionFolder] = path.split("/");

  if (!sectionFolder) {
    return { success: true };
  }

  const result = await getFileContentFromBranch(path, branch);
  if (!result.success || !result.content) {
    return {
      success: false,
      error: result.error || "Failed to load renamed file from branch",
    };
  }

  const { frontmatter, content } = parseMDX(result.content);
  const nextFrontmatter = {
    ...frontmatter,
    section: getSectionTitle(collection, sectionFolder),
  };
  const fullContent = `---\n${yaml.dump(nextFrontmatter, {
    quotingType: '"',
    forceQuotes: true,
    lineWidth: -1,
  })}---\n\n${content}`;

  return updateContentFileOnBranch(path, fullContent, branch);
}

export const Route = createFileRoute("/api/admin/content/rename")({
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

        let body: RenameRequest;
        try {
          body = await request.json();
        } catch {
          return new Response(JSON.stringify({ error: "Invalid JSON body" }), {
            status: 400,
            headers: { "Content-Type": "application/json" },
          });
        }

        const { fromPath, toPath, branch } = body;

        if (!fromPath || !toPath) {
          return new Response(
            JSON.stringify({
              error: "Missing required fields: fromPath, toPath",
            }),
            { status: 400, headers: { "Content-Type": "application/json" } },
          );
        }

        const collection = getCollectionFromPath(fromPath);
        let targetBranch = branch;
        let pendingPR: Awaited<
          ReturnType<typeof ensureContentEditBranch>
        > | null = null;

        if (!targetBranch && collection && collection !== "articles") {
          pendingPR = await ensureContentEditBranch(fromPath);
          if (!pendingPR.success || !pendingPR.branchName) {
            return new Response(JSON.stringify({ error: pendingPR.error }), {
              status: 500,
              headers: { "Content-Type": "application/json" },
            });
          }
          targetBranch = pendingPR.branchName;
        }

        const result = await renameContentFile(fromPath, toPath, targetBranch);

        if (!result.success) {
          return new Response(JSON.stringify({ error: result.error }), {
            status: 500,
            headers: { "Content-Type": "application/json" },
          });
        }

        if (
          targetBranch &&
          result.newPath &&
          (collection === "docs" || collection === "handbook")
        ) {
          const syncResult = await syncStructuredSectionFrontmatter(
            result.newPath,
            targetBranch,
            collection,
          );

          if (!syncResult.success) {
            return new Response(JSON.stringify({ error: syncResult.error }), {
              status: 500,
              headers: { "Content-Type": "application/json" },
            });
          }
        }

        return new Response(
          JSON.stringify({
            success: true,
            newPath: result.newPath,
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
