import { Trans, useLingui } from "@lingui/react/macro";
import {
  ArrowSquareOut,
  Chat,
  GitMerge,
  GitPullRequest,
  RadioButton,
  XCircle,
} from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { useQuery } from "@tanstack/react-query";
import { defaultRehypePlugins, Streamdown } from "streamdown";

import { colors, radii } from "@anlg/design-system/tokens.stylex";
import { commands as openerCommands } from "@anlg/plugin-opener2";
import { commands as todoCommands } from "@anlg/plugin-todo";
import { mergeStyleXProps } from "@anlg/ui/lib/stylex";

import { streamdownComponents } from "~/session/components/streamdown";
import { type TaskResource } from "~/store/zustand/tabs";

const rehypePlugins = [defaultRehypePlugins.raw, defaultRehypePlugins.sanitize];

export function ResourceView({ resource }: { resource: TaskResource }) {
  const { t } = useLingui();
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
    <div {...stylex.props(styles.root)}>
      {isLoading ? (
        <div {...stylex.props(styles.centerMessage)}>
          <Trans>Loading...</Trans>
        </div>
      ) : null}
      {error ? (
        <div {...stylex.props(styles.centerMessage)}>
          {isPR ? (
            <Trans>Failed to load pull request</Trans>
          ) : (
            <Trans>Failed to load issue</Trans>
          )}
        </div>
      ) : null}
      {issue ? (
        <>
          <div {...stylex.props(styles.header)}>
            <div {...stylex.props(styles.headerRow)}>
              <h1 {...stylex.props(styles.title)}>
                {issue.title}
                <span {...stylex.props(styles.issueNumber)}>
                  #{issue.number}
                </span>
              </h1>
              <button
                type="button"
                {...stylex.props(styles.openButton)}
                onClick={() => openerCommands.openUrl(url, null)}
                title={t`Open on GitHub`}
              >
                <ArrowSquareOut {...stylex.props(styles.icon)} />
              </button>
            </div>
            <div {...stylex.props(styles.metadata)}>
              <StateBadge isPR={isPR} isMerged={isMerged} isClosed={isClosed} />
              <span>
                <Trans>{issue.user?.login} opened on</Trans>{" "}
                {new Date(issue.created_at).toLocaleDateString(undefined, {
                  month: "short",
                  day: "numeric",
                  year: "numeric",
                })}
              </span>
              {issue.comments != null && issue.comments > 0 ? (
                <span>
                  · {issue.comments}{" "}
                  {issue.comments === 1 ? t`comment` : t`comments`}
                </span>
              ) : null}
            </div>
          </div>

          {issue.labels && issue.labels.length > 0 ? (
            <div {...stylex.props(styles.labels)}>
              {issue.labels.map((label) => (
                <span
                  key={label.id}
                  {...mergeStyleXProps(styles.label, undefined, {
                    backgroundColor: label.color
                      ? `#${label.color}20`
                      : "#e5e5e5",
                    color: label.color ? `#${label.color}` : "#525252",
                    border: `1px solid ${
                      label.color ? `#${label.color}40` : "#d4d4d4"
                    }`,
                  })}
                >
                  {label.name}
                </span>
              ))}
            </div>
          ) : null}

          {issue.assignees && issue.assignees.length > 0 ? (
            <div {...stylex.props(styles.assignees)}>
              <span {...stylex.props(styles.mediumText)}>
                <Trans>Assignees:</Trans>
              </span>
              {issue.assignees.map((assignee) => (
                <span key={assignee.id} {...stylex.props(styles.assignee)}>
                  {assignee.avatar_url ? (
                    <img
                      src={assignee.avatar_url}
                      alt={assignee.login}
                      {...stylex.props(styles.avatar)}
                    />
                  ) : null}
                  {assignee.login}
                </span>
              ))}
            </div>
          ) : null}

          {issue.body ? (
            <div {...stylex.props(styles.borderedSection)}>
              <Streamdown
                {...mergeStyleXProps(styles.markdown, "note-typography")}
                components={streamdownComponents}
                controls={false}
                isAnimating={false}
                rehypePlugins={rehypePlugins}
              >
                {issue.body}
              </Streamdown>
            </div>
          ) : (
            <div
              {...stylex.props(styles.borderedSection, styles.emptyDescription)}
            >
              <Trans>No description provided.</Trans>
            </div>
          )}

          {comments && comments.length > 0 ? (
            <div
              {...stylex.props(styles.borderedSection, styles.commentsSection)}
            >
              <div {...stylex.props(styles.commentsHeading)}>
                <Chat {...stylex.props(styles.icon)} />
                <span>
                  {comments.length}{" "}
                  {comments.length === 1 ? t`comment` : t`comments`}
                </span>
              </div>
              <div {...stylex.props(styles.commentList)}>
                {comments.map((comment) => (
                  <div key={comment.id} {...stylex.props(styles.comment)}>
                    <div {...stylex.props(styles.commentHeader)}>
                      {comment.user?.avatar_url ? (
                        <img
                          src={comment.user.avatar_url}
                          alt={comment.user.login}
                          {...stylex.props(styles.avatar)}
                        />
                      ) : null}
                      <span {...stylex.props(styles.mediumText)}>
                        {comment.user?.login}
                      </span>
                      <span {...stylex.props(styles.mutedText)}>
                        <Trans>commented on</Trans>{" "}
                        {new Date(comment.created_at).toLocaleDateString(
                          undefined,
                          {
                            month: "short",
                            day: "numeric",
                            year: "numeric",
                          },
                        )}
                      </span>
                    </div>
                    <div {...stylex.props(styles.commentBody)}>
                      {comment.body ? (
                        <Streamdown
                          {...mergeStyleXProps(
                            styles.markdown,
                            "note-typography",
                          )}
                          components={streamdownComponents}
                          controls={false}
                          isAnimating={false}
                          rehypePlugins={rehypePlugins}
                        >
                          {comment.body}
                        </Streamdown>
                      ) : (
                        <span {...stylex.props(styles.emptyDescription)}>
                          <Trans>No content.</Trans>
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
  const { t } = useLingui();
  let label: string;
  let colorStyle: stylex.StyleXStyles;
  let Icon: typeof RadioButton;

  if (isPR && isMerged) {
    label = t`Merged`;
    colorStyle = styles.statePurple;
    Icon = GitMerge;
  } else if (isClosed) {
    label = t`Closed`;
    colorStyle = isPR ? styles.stateRed : styles.statePurple;
    Icon = XCircle;
  } else {
    label = t`Open`;
    colorStyle = styles.stateGreen;
    Icon = isPR ? GitPullRequest : RadioButton;
  }

  return (
    <span {...stylex.props(styles.stateBadge, colorStyle)}>
      <Icon {...stylex.props(styles.stateIcon)} />
      {label}
    </span>
  );
}

const styles = stylex.create({
  assignee: {
    alignItems: "center",
    display: "flex",
    gap: "0.25rem",
  },
  assignees: {
    alignItems: "center",
    color: colors.mutedForeground,
    display: "flex",
    fontSize: "0.875rem",
    gap: "0.5rem",
    marginBottom: "1rem",
  },
  avatar: {
    borderRadius: radii.full,
    height: "1.25rem",
    width: "1.25rem",
  },
  borderedSection: {
    borderTopColor: colors.border,
    borderTopStyle: "solid",
    borderTopWidth: "1px",
    paddingTop: "1rem",
  },
  centerMessage: {
    alignItems: "center",
    color: colors.mutedForeground,
    display: "flex",
    justifyContent: "center",
    paddingBlock: "3rem",
  },
  comment: {
    backgroundColor: `color-mix(in srgb, ${colors.muted} 50%, transparent)`,
    borderColor: colors.border,
    borderRadius: radii.lg,
    borderStyle: "solid",
    borderWidth: "1px",
  },
  commentBody: {
    paddingBlock: "0.75rem",
    paddingInline: "1rem",
  },
  commentHeader: {
    alignItems: "center",
    borderBottomColor: colors.border,
    borderBottomStyle: "solid",
    borderBottomWidth: "1px",
    display: "flex",
    fontSize: "0.875rem",
    gap: "0.5rem",
    paddingBlock: "0.625rem",
    paddingInline: "1rem",
  },
  commentList: {
    display: "flex",
    flexDirection: "column",
    gap: "1rem",
  },
  commentsHeading: {
    alignItems: "center",
    color: colors.mutedForeground,
    display: "flex",
    fontSize: "0.875rem",
    fontWeight: 500,
    gap: "0.5rem",
    marginBottom: "1rem",
  },
  commentsSection: {
    marginTop: "1.5rem",
  },
  emptyDescription: {
    color: colors.mutedForeground,
    fontSize: "0.875rem",
    fontStyle: "italic",
  },
  header: {
    marginBottom: "1rem",
  },
  headerRow: {
    alignItems: "flex-start",
    display: "flex",
    gap: "0.75rem",
    justifyContent: "space-between",
  },
  icon: {
    height: "1rem",
    width: "1rem",
  },
  issueNumber: {
    color: colors.mutedForeground,
    fontWeight: 400,
    marginLeft: "0.5rem",
  },
  label: {
    alignItems: "center",
    borderRadius: radii.full,
    display: "inline-flex",
    fontSize: "0.75rem",
    fontWeight: 500,
    paddingBlock: "0.125rem",
    paddingInline: "0.625rem",
  },
  labels: {
    display: "flex",
    flexWrap: "wrap",
    gap: "0.375rem",
    marginBottom: "1rem",
  },
  markdown: {
    "--note-editor-font-size": "0.875rem",
    color: colors.mutedForeground,
    marginTop: "0.25rem",
  },
  mediumText: {
    color: colors.mutedForeground,
    fontWeight: 500,
  },
  metadata: {
    alignItems: "center",
    color: colors.mutedForeground,
    display: "flex",
    fontSize: "0.875rem",
    gap: "0.5rem",
    marginTop: "0.5rem",
  },
  mutedText: {
    color: colors.mutedForeground,
  },
  openButton: {
    borderRadius: radii.md,
    color: {
      default: colors.mutedForeground,
      ":hover": colors.mutedForeground,
    },
    flexShrink: 0,
    padding: "0.375rem",
    transitionDuration: "150ms",
    transitionProperty: "color, background-color",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
    backgroundColor: {
      default: "transparent",
      ":hover": colors.accent,
    },
  },
  root: {
    maxWidth: "48rem",
    padding: "1.5rem",
    width: "100%",
  },
  stateBadge: {
    alignItems: "center",
    borderRadius: radii.full,
    display: "inline-flex",
    fontSize: "0.75rem",
    fontWeight: 500,
    gap: "0.25rem",
    paddingBlock: "0.125rem",
    paddingInline: "0.625rem",
  },
  stateGreen: {
    backgroundColor: "#dcfce7",
    color: "#15803d",
  },
  stateIcon: {
    height: "0.875rem",
    width: "0.875rem",
  },
  statePurple: {
    backgroundColor: "#f3e8ff",
    color: "#7e22ce",
  },
  stateRed: {
    backgroundColor: "#fee2e2",
    color: "#b91c1c",
  },
  title: {
    color: colors.foreground,
    fontSize: "1.25rem",
    fontWeight: 600,
    lineHeight: 1.375,
  },
});
