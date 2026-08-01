import {
  getGitHubCredentials,
  GITHUB_BRANCH,
  GITHUB_REPO,
  isDev,
} from "./shared";

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
