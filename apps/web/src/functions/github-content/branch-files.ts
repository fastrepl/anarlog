import * as fs from "fs";
import yaml from "js-yaml";
import * as path from "path";

import { createBranch } from "./branches";
import {
  buildDraftBranchName,
  CONTENT_PATH,
  getDefaultFrontmatter,
  getFullPath,
  getGitHubCredentials,
  getLocalContentPath,
  GITHUB_REPO,
  isDev,
  sanitizeRelativeFilePath,
  VALID_FOLDERS,
} from "./shared";

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
