import * as fs from "fs";
import * as path from "path";

import { getFileContentFromBranch } from "./branch-files";
import { createBranch } from "./branches";
import { createPullRequest, findExistingEditPRForPath } from "./pull-requests";
import {
  buildDraftBranchName,
  buildPublishedEditBranchName,
  CONTENT_PATH,
  getCollectionFromPath,
  getGitHubCredentials,
  getLocalContentPath,
  GITHUB_BRANCH,
  GITHUB_REPO,
  isDev,
} from "./shared";

async function savePublishedContentToBranchInternal(
  filePath: string,
  content: string,
): Promise<{
  success: boolean;
  prNumber?: number;
  prUrl?: string;
  branchName?: string;
  isExistingPR?: boolean;
  error?: string;
}> {
  const branchResult = await ensureContentEditBranch(filePath);
  if (!branchResult.success || !branchResult.branchName) {
    return {
      success: false,
      error: branchResult.error || "Failed to create branch",
    };
  }

  if (isDev()) {
    try {
      const localPath = path.join(getLocalContentPath(), filePath);
      fs.writeFileSync(localPath, content);
      return { success: true };
    } catch (error) {
      return {
        success: false,
        error: `Failed to save locally: ${(error as Error).message}`,
      };
    }
  }

  const credentials = await getGitHubCredentials();
  if (!credentials) {
    return { success: false, error: "GitHub token not configured" };
  }
  const { token: githubToken, author } = credentials;
  const branchName = branchResult.branchName;
  const isExistingPR = branchResult.isExistingPR;

  const fullPath = `${CONTENT_PATH}/${filePath}`;

  try {
    const getResponse = await fetch(
      `https://api.github.com/repos/${GITHUB_REPO}/contents/${fullPath}?ref=${branchName}`,
      {
        headers: {
          Authorization: `Bearer ${githubToken}`,
          Accept: "application/vnd.github.v3+json",
        },
      },
    );

    if (!getResponse.ok) {
      return {
        success: false,
        error: `File not found on branch: ${getResponse.status}`,
      };
    }

    const fileData = await getResponse.json();
    const sha = fileData.sha;
    const contentBase64 = Buffer.from(content).toString("base64");

    const updateResponse = await fetch(
      `https://api.github.com/repos/${GITHUB_REPO}/contents/${fullPath}`,
      {
        method: "PUT",
        headers: {
          Authorization: `Bearer ${githubToken}`,
          "Content-Type": "application/json",
          Accept: "application/vnd.github.v3+json",
        },
        body: JSON.stringify({
          message: `Update ${filePath} via admin`,
          content: contentBase64,
          sha,
          branch: branchName,
          ...(author && { author, committer: author }),
        }),
      },
    );

    if (!updateResponse.ok) {
      const error = await updateResponse.json();
      return {
        success: false,
        error: `Failed to update: ${error.message || updateResponse.status}`,
      };
    }

    return {
      success: true,
      branchName,
      isExistingPR,
      prNumber: branchResult.prNumber,
      prUrl: branchResult.prUrl,
    };
  } catch (error) {
    return {
      success: false,
      error: `Save failed: ${(error as Error).message}`,
    };
  }
}

export async function ensureContentEditBranch(filePath: string): Promise<{
  success: boolean;
  branchName?: string;
  prNumber?: number;
  prUrl?: string;
  isExistingPR?: boolean;
  error?: string;
}> {
  if (isDev()) {
    return {
      success: true,
      branchName: buildPublishedEditBranchName(filePath),
      isExistingPR: false,
    };
  }

  const existingPR = await findExistingEditPRForPath(filePath);
  if (existingPR.found && existingPR.branchName) {
    return {
      success: true,
      branchName: existingPR.branchName,
      prNumber: existingPR.prNumber,
      prUrl: existingPR.prUrl,
      isExistingPR: true,
    };
  }

  const branchName = buildPublishedEditBranchName(filePath);
  const branchResult = await createBranch(branchName, GITHUB_BRANCH);
  if (!branchResult.success) {
    return { success: false, error: branchResult.error };
  }

  return {
    success: true,
    branchName,
    isExistingPR: false,
  };
}

export async function savePublishedArticleToBranch(
  filePath: string,
  content: string,
  _metadata: {
    meta_title?: string;
    display_title?: string;
    author?: string | string[];
  },
): Promise<{
  success: boolean;
  prNumber?: number;
  prUrl?: string;
  branchName?: string;
  isExistingPR?: boolean;
  error?: string;
}> {
  return savePublishedContentToBranchInternal(filePath, content);
}

export async function savePublishedContentToBranch(
  filePath: string,
  content: string,
): Promise<{
  success: boolean;
  prNumber?: number;
  prUrl?: string;
  branchName?: string;
  isExistingPR?: boolean;
  error?: string;
}> {
  return savePublishedContentToBranchInternal(filePath, content);
}

export async function publishArticle(
  filePath: string,
  branchName: string,
  metadata: {
    meta_title?: string;
    author?: string | string[];
    date?: string;
    category?: string;
  },
  action: "publish" | "unpublish" = "publish",
): Promise<{
  success: boolean;
  prNumber?: number;
  prUrl?: string;
  error?: string;
}> {
  const actionLabel = await getArticlePullRequestActionLabel(
    filePath,
    branchName,
    action,
  );
  const title = `${actionLabel}: ${metadata.meta_title || filePath}`;
  const statusText =
    action === "publish" ? "Ready for Publication" : "To Be Unpublished";
  const body = `## Article ${statusText}

**Title:** ${metadata.meta_title || "Untitled"}
**Author:** ${Array.isArray(metadata.author) ? metadata.author.join(", ") : metadata.author || "Unknown"}
**Date:** ${metadata.date || "Not set"}
**Category:** ${metadata.category || "Uncategorized"}

**Branch:** ${branchName}
**File:** apps/web/content/${filePath}

---
Auto-generated PR from admin panel.`;

  const prResult = await createPullRequest(
    branchName,
    GITHUB_BRANCH,
    title,
    body,
  );

  if (prResult.success && prResult.prNumber) {
    const credentials = await getGitHubCredentials();
    if (credentials?.token) {
      try {
        await fetch(
          `https://api.github.com/repos/${GITHUB_REPO}/pulls/${prResult.prNumber}/requested_reviewers`,
          {
            method: "POST",
            headers: {
              Authorization: `Bearer ${credentials.token}`,
              Accept: "application/vnd.github.v3+json",
              "Content-Type": "application/json",
            },
            body: JSON.stringify({
              reviewers: ["computelesscomputer"],
            }),
          },
        );
      } catch {}
    }
  }

  return prResult;
}

async function getArticlePullRequestActionLabel(
  filePath: string,
  branchName: string,
  action: "publish" | "unpublish",
): Promise<"Publish" | "Edit" | "Unpublish"> {
  if (action === "unpublish") {
    return "Unpublish";
  }

  if (branchName === buildDraftBranchName(filePath)) {
    return "Publish";
  }

  if (isDev()) {
    return "Edit";
  }

  const existingArticle = await getFileContentFromBranch(
    filePath,
    GITHUB_BRANCH,
  );
  return existingArticle.success ? "Edit" : "Publish";
}

export async function publishContentPR(
  filePath: string,
  branchName: string,
  metadata: { title?: string; description?: string; summary?: string },
): Promise<{
  success: boolean;
  prNumber?: number;
  prUrl?: string;
  error?: string;
}> {
  const collection = getCollectionFromPath(filePath);
  const contentLabel =
    collection === "docs"
      ? "Documentation"
      : collection === "handbook"
        ? "Company Handbook"
        : "Content";
  const title = `${contentLabel}: ${metadata.title || filePath}`;
  const description = metadata.description || metadata.summary || "Not set";
  const body = `## ${contentLabel} Update

**Title:** ${metadata.title || "Untitled"}
**Description:** ${description}
**Branch:** ${branchName}
**File:** apps/web/content/${filePath}

---
Auto-generated PR from admin panel.`;

  return createPullRequest(branchName, GITHUB_BRANCH, title, body);
}
