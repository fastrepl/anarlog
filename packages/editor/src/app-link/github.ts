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
