import { useQuery } from "@tanstack/react-query";
import {
  CircleDotIcon,
  ExternalLinkIcon,
  GitMergeIcon,
  GitPullRequestIcon,
  MessageSquareIcon,
  XCircleIcon,
} from "lucide-react";
import { defaultRehypePlugins, Streamdown } from "streamdown";

import { commands as openerCommands } from "@hypr/plugin-opener2";
import { commands as todoCommands } from "@hypr/plugin-todo";
import { cn } from "@hypr/utils";

import { streamdownComponents } from "~/session/components/streamdown";
import { type TaskResource } from "~/store/zustand/tabs";

const rehypePlugins = [defaultRehypePlugins.raw, defaultRehypePlugins.sanitize];

export function ResourceView({ resource }: { resource: TaskResource }) {
  const {
    data: issue,
    isLoading,
    error,
  } = useQuery({
    queryKey: [
      "github-issue-detail",
      resource.owner,
      resource.repo,
      resource.number,
    ],
    queryFn: async () => {
      const result = await todoCommands.githubIssueDetail(
        resource.owner,
        resource.repo,
        resource.number,
      );
      if (result.status === "error") {
        throw new Error(result.error);
      }
      return result.data;
    },
    staleTime: 60_000,
  });

  const { data: comments } = useQuery({
    queryKey: [
      "github-issue-comments",
      resource.owner,
      resource.repo,
      resource.number,
    ],
    queryFn: async () => {
      const result = await todoCommands.githubIssueComments(
        resource.owner,
        resource.repo,
        resource.number,
      );
      if (result.status === "error") {
        throw new Error(result.error);
      }
      return result.data;
    },
    staleTime: 60_000,
    enabled: !!issue,
  });

  const isPR = resource.type === "github_pr" || issue?.pull_request != null;
  const urlPath = isPR ? "pull" : "issues";
  const url = `https://github.com/${resource.owner}/${resource.repo}/${urlPath}/${resource.number}`;
  const isMerged = issue?.pull_request?.merged_at != null;
  const isClosed = issue?.state === "closed";

  return (
    <div className="w-full max-w-3xl px-6 py-6">
      {isLoading ? (
        <div className="text-muted-foreground flex items-center justify-center py-12">
          Loading...
        </div>
      ) : null}
      {error ? (
        <div className="text-muted-foreground flex items-center justify-center py-12">
          Failed to load {isPR ? "pull request" : "issue"}
        </div>
      ) : null}
      {issue ? (
        <>
          <div className="mb-4">
            <div className="flex items-start justify-between gap-3">
              <h1 className="text-foreground text-xl leading-snug font-semibold">
                {issue.title}
                <span className="text-muted-foreground ml-2 font-normal">
                  #{issue.number}
                </span>
              </h1>
              <button
                type="button"
                className="text-muted-foreground hover:bg-accent hover:text-muted-foreground shrink-0 rounded-md p-1.5 transition-colors"
                onClick={() => openerCommands.openUrl(url, null)}
                title="Open on GitHub"
              >
                <ExternalLinkIcon className="size-4" />
              </button>
            </div>
            <div className="text-muted-foreground mt-2 flex items-center gap-2 text-sm">
              <StateBadge isPR={isPR} isMerged={isMerged} isClosed={isClosed} />
              <span>
                {issue.user?.login} opened on{" "}
                {new Date(issue.created_at).toLocaleDateString("en-US", {
                  month: "short",
                  day: "numeric",
                  year: "numeric",
                })}
              </span>
              {issue.comments != null && issue.comments > 0 ? (
                <span>
                  · {issue.comments}{" "}
                  {issue.comments === 1 ? "comment" : "comments"}
                </span>
              ) : null}
            </div>
          </div>

          {issue.labels && issue.labels.length > 0 ? (
            <div className="mb-4 flex flex-wrap gap-1.5">
              {issue.labels.map((label) => (
                <span
                  key={label.id}
                  className="inline-flex items-center rounded-full px-2.5 py-0.5 text-xs font-medium"
                  style={{
                    backgroundColor: label.color
                      ? `#${label.color}20`
                      : "#e5e5e5",
                    color: label.color ? `#${label.color}` : "#525252",
                    border: `1px solid ${
                      label.color ? `#${label.color}40` : "#d4d4d4"
                    }`,
                  }}
                >
                  {label.name}
                </span>
              ))}
            </div>
          ) : null}

          {issue.assignees && issue.assignees.length > 0 ? (
            <div className="text-muted-foreground mb-4 flex items-center gap-2 text-sm">
              <span className="text-muted-foreground font-medium">
                Assignees:
              </span>
              {issue.assignees.map((assignee) => (
                <span key={assignee.id} className="flex items-center gap-1">
                  {assignee.avatar_url ? (
                    <img
                      src={assignee.avatar_url}
                      alt={assignee.login}
                      className="size-5 rounded-full"
                    />
                  ) : null}
                  {assignee.login}
                </span>
              ))}
            </div>
          ) : null}

          {issue.body ? (
            <div className="border-border border-t pt-4">
              <Streamdown
                className="text-muted-foreground mt-1 text-sm"
                components={streamdownComponents}
                isAnimating={false}
                rehypePlugins={rehypePlugins}
              >
                {issue.body}
              </Streamdown>
            </div>
          ) : (
            <div className="border-border text-muted-foreground border-t pt-4 text-sm italic">
              No description provided.
            </div>
          )}

          {comments && comments.length > 0 ? (
            <div className="border-border mt-6 border-t pt-4">
              <div className="text-muted-foreground mb-4 flex items-center gap-2 text-sm font-medium">
                <MessageSquareIcon className="size-4" />
                <span>
                  {comments.length}{" "}
                  {comments.length === 1 ? "comment" : "comments"}
                </span>
              </div>
              <div className="space-y-4">
                {comments.map((comment) => (
                  <div
                    key={comment.id}
                    className="border-border bg-muted/50 rounded-lg border"
                  >
                    <div className="border-border flex items-center gap-2 border-b px-4 py-2.5 text-sm">
                      {comment.user?.avatar_url ? (
                        <img
                          src={comment.user.avatar_url}
                          alt={comment.user.login}
                          className="size-5 rounded-full"
                        />
                      ) : null}
                      <span className="text-muted-foreground font-medium">
                        {comment.user?.login}
                      </span>
                      <span className="text-muted-foreground">
                        commented on{" "}
                        {new Date(comment.created_at).toLocaleDateString(
                          "en-US",
                          {
                            month: "short",
                            day: "numeric",
                            year: "numeric",
                          },
                        )}
                      </span>
                    </div>
                    <div className="px-4 py-3">
                      {comment.body ? (
                        <Streamdown
                          className="text-muted-foreground mt-1 text-sm"
                          components={streamdownComponents}
                          isAnimating={false}
                          rehypePlugins={rehypePlugins}
                        >
                          {comment.body}
                        </Streamdown>
                      ) : (
                        <span className="text-muted-foreground text-sm italic">
                          No content.
                        </span>
                      )}
                    </div>
                  </div>
                ))}
              </div>
            </div>
          ) : null}
        </>
      ) : null}
    </div>
  );
}

function StateBadge({
  isPR,
  isMerged,
  isClosed,
}: {
  isPR: boolean;
  isMerged: boolean;
  isClosed: boolean;
}) {
  let label: string;
  let colorClass: string;
  let Icon: typeof CircleDotIcon;

  if (isPR && isMerged) {
    label = "Merged";
    colorClass = "bg-purple-100 text-purple-700";
    Icon = GitMergeIcon;
  } else if (isClosed) {
    label = "Closed";
    colorClass = isPR
      ? "bg-red-100 text-red-700"
      : "bg-purple-100 text-purple-700";
    Icon = XCircleIcon;
  } else {
    label = "Open";
    colorClass = "bg-green-100 text-green-700";
    Icon = isPR ? GitPullRequestIcon : CircleDotIcon;
  }

  return (
    <span
      className={cn([
        "inline-flex items-center gap-1 rounded-full px-2.5 py-0.5 text-xs font-medium",
        colorClass,
      ])}
    >
      <Icon className="size-3.5" />
      {label}
    </span>
  );
}
