export type GitHubLinkKind =
  | "issue"
  | "pull_request"
  | "issue_comment"
  | "pull_request_review_comment"
  | "discussion"
  | "discussion_comment"
  | "commit"
  | "release"
  | "action_run"
  | "workflow";

export interface GitHubAttrs {
  provider: "github";
  kind: GitHubLinkKind;
  url: string;
  owner: string;
  repo: string;
  number?: number;
  subId?: string;
}

const HOSTS = new Set(["github.com", "www.github.com"]);

function parsePositiveInteger(value: string | undefined): number | null {
  if (!value || !/^\d+$/.test(value)) {
    return null;
  }

  const parsed = Number(value);
  return Number.isSafeInteger(parsed) && parsed > 0 ? parsed : null;
}

function decodePathSegment(value: string | undefined): string {
  if (!value) {
    return "";
  }

  try {
    return decodeURIComponent(value);
  } catch {
    return value;
  }
}

function build(
  url: URL,
  attrs: Omit<GitHubAttrs, "provider" | "url">,
): GitHubAttrs {
  return { provider: "github", url: url.toString(), ...attrs };
}

export function parseGitHubUrl(rawUrl: string): GitHubAttrs | null {
  let url: URL;
  try {
    url = new URL(rawUrl);
  } catch {
    return null;
  }

  if (!HOSTS.has(url.hostname.toLowerCase())) {
    return null;
  }

  const segments = url.pathname.split("/").filter(Boolean);
  const [ownerSegment, repoSegment, first, second, third, ...rest] = segments;
  const owner = decodePathSegment(ownerSegment);
  const repo = decodePathSegment(repoSegment);

  if (!owner || !repo || !first) {
    return null;
  }

  const fragment = url.hash.startsWith("#") ? url.hash.slice(1) : "";

  if (first === "issues") {
    const number = parsePositiveInteger(second);
    if (!number || third || rest.length > 0) {
      return null;
    }

    if (fragment.startsWith("issuecomment-")) {
      return build(url, {
        kind: "issue_comment",
        owner,
        repo,
        number,
        subId: fragment,
      });
    }

    return build(url, { kind: "issue", owner, repo, number });
  }

  if (first === "pull") {
    const number = parsePositiveInteger(second);
    if (!number || third || rest.length > 0) {
      return null;
    }

    if (fragment.startsWith("discussion_r")) {
      return build(url, {
        kind: "pull_request_review_comment",
        owner,
        repo,
        number,
        subId: fragment,
      });
    }

    if (fragment.startsWith("issuecomment-")) {
      return build(url, {
        kind: "issue_comment",
        owner,
        repo,
        number,
        subId: fragment,
      });
    }

    return build(url, { kind: "pull_request", owner, repo, number });
  }

  if (first === "discussions") {
    const number = parsePositiveInteger(second);
    if (!number || third || rest.length > 0) {
      return null;
    }

    if (fragment.startsWith("discussioncomment-")) {
      return build(url, {
        kind: "discussion_comment",
        owner,
        repo,
        number,
        subId: fragment,
      });
    }

    return build(url, { kind: "discussion", owner, repo, number });
  }

  if (first === "commit") {
    const sha = second;
    if (!sha || third || rest.length > 0 || !/^[0-9a-f]{7,40}$/i.test(sha)) {
      return null;
    }

    return build(url, { kind: "commit", owner, repo, subId: sha });
  }

  if (first === "releases" && second === "tag") {
    const tag = decodePathSegment(third);
    if (!tag || rest.length > 0) {
      return null;
    }

    return build(url, { kind: "release", owner, repo, subId: tag });
  }

  if (first === "actions" && second === "runs") {
    const subId = third?.trim();
    if (!subId || rest.length > 0 || !/^\d+$/.test(subId)) {
      return null;
    }

    return build(url, { kind: "action_run", owner, repo, subId });
  }

  if (first === "actions" && second === "workflows") {
    const subId = decodePathSegment(third);
    if (!subId || rest.length > 0) {
      return null;
    }

    return build(url, { kind: "workflow", owner, repo, subId });
  }

  return null;
}

function getKindLabel(attrs: GitHubAttrs): string {
  switch (attrs.kind) {
    case "issue":
      return `Issue #${attrs.number}`;
    case "pull_request":
      return `PR #${attrs.number}`;
    case "issue_comment":
      return `Comment on #${attrs.number}`;
    case "pull_request_review_comment":
      return `Review on #${attrs.number}`;
    case "discussion":
      return `Discussion #${attrs.number}`;
    case "discussion_comment":
      return `Comment on Discussion #${attrs.number}`;
    case "commit":
      return `Commit ${attrs.subId?.slice(0, 7)}`;
    case "release":
      return `Release ${attrs.subId}`;
    case "action_run":
      return `Run ${attrs.subId}`;
    case "workflow":
      return `Workflow ${attrs.subId}`;
  }
}

export function getGitHubDisplayParts(attrs: GitHubAttrs): {
  header: string;
  subline: string;
} {
  return {
    header: `${attrs.owner}/${attrs.repo}`,
    subline: getKindLabel(attrs),
  };
}
