import {
  getExistingBranchPrefix,
  getGitHubCredentials,
  GITHUB_BRANCH,
  GITHUB_REPO,
  isDev,
} from "./shared";

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
