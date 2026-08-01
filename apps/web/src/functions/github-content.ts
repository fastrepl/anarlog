import * as fs from "fs";
import yaml from "js-yaml";
import * as path from "path";

import {
  buildDraftBranchName,
  buildPublishedEditBranchName,
  CONTENT_PATH,
  getCollectionFromPath,
  getDefaultFrontmatter,
  getExistingBranchPrefix,
  getFullPath,
  getGitHubCredentials,
  getLocalContentPath,
  GITHUB_BRANCH,
  GITHUB_REPO,
  isDev,
  sanitizeRelativeFilePath,
  VALID_FOLDERS,
} from "./github-content/shared";

export {
  createContentFile,
  createContentFolder,
  deleteContentFile,
  duplicateContentFile,
  renameContentFile,
  updateContentFile,
} from "./github-content/files";
export {
  generateBranchName,
  REVIEWABLE_CONTENT_FOLDERS,
} from "./github-content/shared";
export { getCollectionFromPath, getGitHubCredentials };
export type {
  GitHubCredentials,
  ReviewableContentFolder,
} from "./github-content/shared";

export async function getBranchSha(
  branchName: string,
): Promise<{ success: boolean; sha?: string; error?: string }> {
  const credentials = await getGitHubCredentials();
  if (!credentials) {
    return { success: false, error: "GitHub token not configured" };
  }
  const { token: githubToken } = credentials;

  try {
    const response = await fetch(
      `https://api.github.com/repos/${GITHUB_REPO}/git/ref/heads/${branchName}`,
      {
        headers: {
          Authorization: `Bearer ${githubToken}`,
          Accept: "application/vnd.github.v3+json",
        },
      },
    );

    if (!response.ok) {
      let message = `GitHub API error: ${response.status}`;
      try {
        const error = await response.json();
        if (typeof error?.message === "string" && error.message.length > 0) {
          message = error.message;
        }
      } catch {}

      return {
        success: false,
        error: `Failed to access branch ref "${branchName}" (${response.status}): ${message}`,
      };
    }

    const data = await response.json();
    return { success: true, sha: data.object.sha };
  } catch (error) {
    return {
      success: false,
      error: `Failed to get branch SHA: ${(error as Error).message}`,
    };
  }
}

export async function deleteBranch(
  branchName: string,
): Promise<{ success: boolean; error?: string }> {
  if (isDev()) {
    return { success: true };
  }

  const credentials = await getGitHubCredentials();
  if (!credentials) {
    return { success: false, error: "GitHub token not configured" };
  }
  const { token: githubToken } = credentials;

  try {
    const response = await fetch(
      `https://api.github.com/repos/${GITHUB_REPO}/git/refs/heads/${branchName}`,
      {
        method: "DELETE",
        headers: {
          Authorization: `Bearer ${githubToken}`,
          Accept: "application/vnd.github.v3+json",
        },
      },
    );

    if (!response.ok && response.status !== 404 && response.status !== 422) {
      const error = await response.json();
      return {
        success: false,
        error: error.message || `GitHub API error: ${response.status}`,
      };
    }

    return { success: true };
  } catch (error) {
    return {
      success: false,
      error: `Failed to delete branch: ${(error as Error).message}`,
    };
  }
}

export async function closePullRequest(
  prNumber: number,
): Promise<{ success: boolean; error?: string }> {
  if (isDev()) {
    return { success: true };
  }

  const credentials = await getGitHubCredentials();
  if (!credentials) {
    return { success: false, error: "GitHub token not configured" };
  }
  const { token: githubToken } = credentials;

  try {
    const response = await fetch(
      `https://api.github.com/repos/${GITHUB_REPO}/pulls/${prNumber}`,
      {
        method: "PATCH",
        headers: {
          Authorization: `Bearer ${githubToken}`,
          Accept: "application/vnd.github.v3+json",
          "Content-Type": "application/json",
        },
        body: JSON.stringify({
          state: "closed",
        }),
      },
    );

    if (!response.ok) {
      const error = await response.json();
      return {
        success: false,
        error: error.message || `GitHub API error: ${response.status}`,
      };
    }

    return { success: true };
  } catch (error) {
    return {
      success: false,
      error: `Failed to close PR: ${(error as Error).message}`,
    };
  }
}

export async function createBranch(
  branchName: string,
  baseBranch: string = GITHUB_BRANCH,
): Promise<{ success: boolean; error?: string }> {
  if (isDev()) {
    return { success: true };
  }

  const credentials = await getGitHubCredentials();
  if (!credentials) {
    return { success: false, error: "GitHub token not configured" };
  }
  const { token: githubToken } = credentials;

  try {
    const baseShaResult = await getBranchSha(baseBranch);
    if (!baseShaResult.success || !baseShaResult.sha) {
      return {
        success: false,
        error: `Failed to get base branch SHA: ${baseShaResult.error}`,
      };
    }

    const existingBranch = await getBranchSha(branchName);
    if (existingBranch.success) {
      return { success: true };
    }

    const response = await fetch(
      `https://api.github.com/repos/${GITHUB_REPO}/git/refs`,
      {
        method: "POST",
        headers: {
          Authorization: `Bearer ${githubToken}`,
          Accept: "application/vnd.github.v3+json",
          "Content-Type": "application/json",
        },
        body: JSON.stringify({
          ref: `refs/heads/${branchName}`,
          sha: baseShaResult.sha,
        }),
      },
    );

    if (!response.ok) {
      const error = await response.json();
      return {
        success: false,
        error: error.message || `GitHub API error: ${response.status}`,
      };
    }

    return { success: true };
  } catch (error) {
    return {
      success: false,
      error: `Failed to create branch: ${(error as Error).message}`,
    };
  }
}

export async function createPullRequest(
  head: string,
  base: string,
  title: string,
  body: string,
  options?: { isDraft?: boolean },
): Promise<{
  success: boolean;
  prNumber?: number;
  prUrl?: string;
  isDraft?: boolean;
  error?: string;
}> {
  if (isDev()) {
    return { success: true, prNumber: 0, prUrl: "", isDraft: options?.isDraft };
  }

  const credentials = await getGitHubCredentials();
  if (!credentials) {
    return { success: false, error: "GitHub token not configured" };
  }
  const { token: githubToken } = credentials;

  try {
    const listResponse = await fetch(
      `https://api.github.com/repos/${GITHUB_REPO}/pulls?head=fastrepl:${head}&base=${base}&state=open`,
      {
        headers: {
          Authorization: `Bearer ${githubToken}`,
          Accept: "application/vnd.github.v3+json",
        },
      },
    );

    if (listResponse.ok) {
      const existingPRs = await listResponse.json();
      if (existingPRs.length > 0) {
        return {
          success: true,
          prNumber: existingPRs[0].number,
          prUrl: existingPRs[0].html_url,
          isDraft: existingPRs[0].draft,
        };
      }
    }

    const response = await fetch(
      `https://api.github.com/repos/${GITHUB_REPO}/pulls`,
      {
        method: "POST",
        headers: {
          Authorization: `Bearer ${githubToken}`,
          Accept: "application/vnd.github.v3+json",
          "Content-Type": "application/json",
        },
        body: JSON.stringify({
          title,
          body,
          head,
          base,
          draft: options?.isDraft ?? false,
        }),
      },
    );

    if (!response.ok) {
      const error = await response.json();
      return {
        success: false,
        error: error.message || `GitHub API error: ${response.status}`,
      };
    }

    const data = await response.json();
    return {
      success: true,
      prNumber: data.number,
      prUrl: data.html_url,
      isDraft: data.draft,
    };
  } catch (error) {
    return {
      success: false,
      error: `Failed to create PR: ${(error as Error).message}`,
    };
  }
}

export async function findOpenPullRequestByBranch(branchName: string): Promise<{
  found: boolean;
  prNumber?: number;
  prUrl?: string;
}> {
  if (isDev()) {
    return { found: false };
  }

  const credentials = await getGitHubCredentials();
  if (!credentials) {
    return { found: false };
  }
  const { token: githubToken } = credentials;

  try {
    const params = new URLSearchParams({
      head: `fastrepl:${branchName}`,
      base: GITHUB_BRANCH,
      state: "open",
    });
    const response = await fetch(
      `https://api.github.com/repos/${GITHUB_REPO}/pulls?${params}`,
      {
        headers: {
          Authorization: `Bearer ${githubToken}`,
          Accept: "application/vnd.github.v3+json",
        },
      },
    );

    if (!response.ok) {
      return { found: false };
    }

    const prs = await response.json();
    const pr = Array.isArray(prs) ? prs[0] : undefined;

    if (!pr) {
      return { found: false };
    }

    return {
      found: true,
      prNumber: pr.number,
      prUrl: pr.html_url,
    };
  } catch {
    return { found: false };
  }
}

export async function createContentFileOnBranch(
  folder: string,
  filename: string,
  content: string = "",
  branchName?: string,
): Promise<{
  success: boolean;
  path?: string;
  branch?: string;
  error?: string;
}> {
  if (!VALID_FOLDERS.includes(folder)) {
    return {
      success: false,
      error: `Invalid folder. Must be one of: ${VALID_FOLDERS.join(", ")}`,
    };
  }

  let safeFilename = sanitizeRelativeFilePath(filename);
  if (!safeFilename.endsWith(".mdx")) {
    safeFilename = `${safeFilename}.mdx`;
  }

  const targetFilePath = `${folder}/${safeFilename}`;
  const targetBranch = branchName || buildDraftBranchName(targetFilePath);
  const defaultContent = content || getDefaultFrontmatter(folder);

  if (isDev()) {
    try {
      const localPath = path.join(getLocalContentPath(), folder, safeFilename);
      if (fs.existsSync(localPath)) {
        return {
          success: false,
          error: `File already exists: ${safeFilename}`,
        };
      }
      const dir = path.dirname(localPath);
      if (!fs.existsSync(dir)) {
        fs.mkdirSync(dir, { recursive: true });
      }
      fs.writeFileSync(localPath, defaultContent);
      return {
        success: true,
        path: targetFilePath,
        branch: targetBranch,
      };
    } catch (error) {
      return {
        success: false,
        error: `Failed to create file locally: ${(error as Error).message}`,
      };
    }
  }

  const credentials = await getGitHubCredentials();
  if (!credentials) {
    return { success: false, error: "GitHub token not configured" };
  }
  const { token: githubToken, author } = credentials;

  const branchResult = await createBranch(targetBranch);
  if (!branchResult.success) {
    return { success: false, error: branchResult.error };
  }

  const filePath = getFullPath(folder, safeFilename);

  try {
    const checkResponse = await fetch(
      `https://api.github.com/repos/${GITHUB_REPO}/contents/${filePath}?ref=${targetBranch}`,
      {
        headers: {
          Authorization: `Bearer ${githubToken}`,
          Accept: "application/vnd.github.v3+json",
        },
      },
    );

    if (checkResponse.status === 200) {
      return { success: false, error: `File already exists: ${safeFilename}` };
    }

    const contentBase64 = Buffer.from(defaultContent).toString("base64");

    const createResponse = await fetch(
      `https://api.github.com/repos/${GITHUB_REPO}/contents/${filePath}`,
      {
        method: "PUT",
        headers: {
          Authorization: `Bearer ${githubToken}`,
          Accept: "application/vnd.github.v3+json",
          "Content-Type": "application/json",
        },
        body: JSON.stringify({
          message: `Create ${folder}/${safeFilename} via admin`,
          content: contentBase64,
          branch: targetBranch,
          ...(author && { author, committer: author }),
        }),
      },
    );

    if (!createResponse.ok) {
      const error = await createResponse.json();
      return {
        success: false,
        error: error.message || `GitHub API error: ${createResponse.status}`,
      };
    }

    return {
      success: true,
      path: targetFilePath,
      branch: targetBranch,
    };
  } catch (error) {
    return {
      success: false,
      error: `Failed to create file: ${(error as Error).message}`,
    };
  }
}

export async function updateContentFileOnBranch(
  filePath: string,
  content: string,
  branchName: string,
): Promise<{ success: boolean; error?: string }> {
  if (isDev()) {
    try {
      const localPath = path.join(getLocalContentPath(), filePath);
      if (!fs.existsSync(localPath)) {
        return { success: false, error: `File not found: ${filePath}` };
      }
      fs.writeFileSync(localPath, content);
      return { success: true };
    } catch (error) {
      return {
        success: false,
        error: `Failed to update file locally: ${(error as Error).message}`,
      };
    }
  }

  const credentials = await getGitHubCredentials();
  if (!credentials) {
    return { success: false, error: "GitHub token not configured" };
  }
  const { token: githubToken, author } = credentials;

  const fullPath = filePath.startsWith("apps/web/content")
    ? filePath
    : `${CONTENT_PATH}/${filePath}`;

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
        error: `File not found on branch ${branchName}: ${getResponse.status}`,
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

    return { success: true };
  } catch (error) {
    return {
      success: false,
      error: `Update failed: ${(error as Error).message}`,
    };
  }
}

export async function findExistingEditPRForPath(filePath: string): Promise<{
  found: boolean;
  branchName?: string;
  prNumber?: number;
  prUrl?: string;
}> {
  if (isDev()) {
    return { found: false };
  }

  const credentials = await getGitHubCredentials();
  if (!credentials) {
    return { found: false };
  }
  const { token: githubToken } = credentials;

  try {
    const response = await fetch(
      `https://api.github.com/repos/${GITHUB_REPO}/pulls?state=open&base=${GITHUB_BRANCH}`,
      {
        headers: {
          Authorization: `Bearer ${githubToken}`,
          Accept: "application/vnd.github.v3+json",
        },
      },
    );

    if (!response.ok) {
      return { found: false };
    }

    const prs = await response.json();
    const editPrefix = getExistingBranchPrefix(filePath);

    for (const pr of prs) {
      const headRef = pr.head?.ref || "";
      if (headRef.startsWith(editPrefix)) {
        return {
          found: true,
          branchName: headRef,
          prNumber: pr.number,
          prUrl: pr.html_url,
        };
      }
    }

    return { found: false };
  } catch {
    return { found: false };
  }
}

export async function findExistingEditPR(slug: string): Promise<{
  found: boolean;
  branchName?: string;
  prNumber?: number;
  prUrl?: string;
}> {
  return findExistingEditPRForPath(`articles/${slug}.mdx`);
}

export async function getExistingEditPRForContent(filePath: string): Promise<{
  success: boolean;
  hasPendingPR: boolean;
  prNumber?: number;
  prUrl?: string;
  branchName?: string;
  error?: string;
}> {
  if (isDev()) {
    return { success: true, hasPendingPR: false };
  }

  const existingPR = await findExistingEditPRForPath(filePath);
  if (existingPR.found) {
    return {
      success: true,
      hasPendingPR: true,
      prNumber: existingPR.prNumber,
      prUrl: existingPR.prUrl,
      branchName: existingPR.branchName,
    };
  }

  return { success: true, hasPendingPR: false };
}

export async function getExistingEditPRForArticle(filePath: string): Promise<{
  success: boolean;
  hasPendingPR: boolean;
  prNumber?: number;
  prUrl?: string;
  branchName?: string;
  error?: string;
}> {
  return getExistingEditPRForContent(filePath);
}

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

export async function listBlogBranches(): Promise<{
  success: boolean;
  branches?: string[];
  error?: string;
}> {
  if (isDev()) {
    return { success: true, branches: [] };
  }

  const credentials = await getGitHubCredentials();
  if (!credentials) {
    return { success: false, error: "GitHub token not configured" };
  }
  const { token: githubToken } = credentials;

  try {
    const response = await fetch(
      `https://api.github.com/repos/${GITHUB_REPO}/git/matching-refs/heads/blog/`,
      {
        headers: {
          Authorization: `Bearer ${githubToken}`,
          Accept: "application/vnd.github.v3+json",
        },
      },
    );

    if (!response.ok) {
      if (response.status === 404) {
        return { success: true, branches: [] };
      }
      return {
        success: false,
        error: `Failed to list branches: ${response.status}`,
      };
    }

    const data = await response.json();
    const branches = Array.isArray(data)
      ? data.map((ref: { ref: string }) => ref.ref.replace("refs/heads/", ""))
      : [];

    return { success: true, branches };
  } catch (error) {
    return {
      success: false,
      error: `Failed to list branches: ${(error as Error).message}`,
    };
  }
}

export async function getFileContentFromBranch(
  filePath: string,
  branchName: string,
): Promise<{
  success: boolean;
  content?: string;
  sha?: string;
  error?: string;
}> {
  if (isDev()) {
    try {
      const localPath = path.join(getLocalContentPath(), filePath);
      if (!fs.existsSync(localPath)) {
        return { success: false, error: `File not found: ${filePath}` };
      }
      const content = fs.readFileSync(localPath, "utf-8");
      return { success: true, content, sha: "local" };
    } catch (error) {
      return {
        success: false,
        error: `Failed to read file: ${(error as Error).message}`,
      };
    }
  }

  const credentials = await getGitHubCredentials();
  if (!credentials) {
    return { success: false, error: "GitHub token not configured" };
  }
  const { token: githubToken } = credentials;

  const fullPath = filePath.startsWith("apps/web/content")
    ? filePath
    : `${CONTENT_PATH}/${filePath}`;

  try {
    const response = await fetch(
      `https://api.github.com/repos/${GITHUB_REPO}/contents/${fullPath}?ref=${branchName}`,
      {
        headers: {
          Authorization: `Bearer ${githubToken}`,
          Accept: "application/vnd.github.v3+json",
        },
      },
    );

    if (!response.ok) {
      return { success: false, error: `File not found: ${response.status}` };
    }

    const data = await response.json();

    let content: string;
    if (!data.content && data.sha) {
      const blobResponse = await fetch(
        `https://api.github.com/repos/${GITHUB_REPO}/git/blobs/${data.sha}`,
        {
          headers: {
            Authorization: `Bearer ${githubToken}`,
            Accept: "application/vnd.github.v3+json",
          },
        },
      );

      if (!blobResponse.ok) {
        return {
          success: false,
          error: `Failed to fetch blob: ${blobResponse.status}`,
        };
      }

      const blobData = await blobResponse.json();
      content = Buffer.from(blobData.content, "base64").toString("utf-8");
    } else if (data.content) {
      content = Buffer.from(data.content, "base64").toString("utf-8");
    } else {
      return {
        success: false,
        error: "File content not available in response",
      };
    }

    return { success: true, content, sha: data.sha };
  } catch (error) {
    return {
      success: false,
      error: `Failed to fetch file: ${(error as Error).message}`,
    };
  }
}

export function parseMDX(rawContent: string): {
  frontmatter: Record<string, unknown>;
  content: string;
} {
  const frontmatterRegex = /^---\n([\s\S]*?)\n---\n([\s\S]*)$/;
  const match = rawContent.match(frontmatterRegex);

  if (!match) {
    return { frontmatter: {}, content: rawContent };
  }

  const [, frontmatterYaml, content] = match;

  try {
    const parsed = yaml.load(frontmatterYaml);
    const frontmatter =
      parsed && typeof parsed === "object"
        ? (parsed as Record<string, unknown>)
        : {};
    return { frontmatter, content: content.trim() };
  } catch {
    return { frontmatter: {}, content: rawContent };
  }
}
