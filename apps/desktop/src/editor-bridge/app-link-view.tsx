import {
  type NodeViewComponentProps,
  useEditorEventCallback,
} from "@handlewithcare/react-prosemirror";
import { useQuery } from "@tanstack/react-query";
import { CheckIcon } from "lucide-react";
import { forwardRef } from "react";

import {
  getAppLinkDisplayParts,
  getAppLinkLabel,
  type GitHubAttrs,
  type AppLinkAttrs,
} from "@hypr/editor/app-link";
import { commands as openerCommands } from "@hypr/plugin-opener2";
import { commands as todoCommands } from "@hypr/plugin-todo";
import { cn } from "@hypr/utils";

import { collectSiblingResources, openTaskTab } from "~/task/open-task-tab";

function SlackIcon({ className }: { className?: string }) {
  return (
    <svg className={className} viewBox="0 0 24 24" fill="none">
      <path
        d="M6 15a2 2 0 0 1-2 2 2 2 0 0 1-2-2 2 2 0 0 1 2-2h2v2ZM7 15a2 2 0 0 1 2-2 2 2 0 0 1 2 2v5a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-5ZM9 6a2 2 0 0 1-2-2 2 2 0 0 1 2-2 2 2 0 0 1 2 2v2H9ZM9 7a2 2 0 0 1 2 2 2 2 0 0 1-2 2H4a2 2 0 0 1-2-2 2 2 0 0 1 2-2h5ZM18 9a2 2 0 0 1 2-2 2 2 0 0 1 2 2 2 2 0 0 1-2 2h-2V9ZM17 9a2 2 0 0 1-2 2 2 2 0 0 1-2-2V4a2 2 0 0 1 2-2 2 2 0 0 1 2 2v5ZM15 18a2 2 0 0 1 2 2 2 2 0 0 1-2 2 2 2 0 0 1-2-2v-2h2ZM15 17a2 2 0 0 1-2-2 2 2 0 0 1 2-2h5a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-5Z"
        stroke="currentColor"
        strokeWidth="2"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}

function DiscordIcon({ className }: { className?: string }) {
  return (
    <svg className={className} viewBox="0 0 24 24" fill="currentColor">
      <path d="M20.317 4.37a19.791 19.791 0 0 0-4.885-1.515.074.074 0 0 0-.079.037c-.21.375-.444.864-.608 1.25a18.27 18.27 0 0 0-5.487 0 12.64 12.64 0 0 0-.617-1.25.077.077 0 0 0-.079-.037A19.736 19.736 0 0 0 3.677 4.37a.07.07 0 0 0-.032.027C.533 9.046-.32 13.58.099 18.057a.082.082 0 0 0 .031.057 19.9 19.9 0 0 0 5.993 3.03.078.078 0 0 0 .084-.028c.462-.63.874-1.295 1.226-1.994a.076.076 0 0 0-.041-.106 13.107 13.107 0 0 1-1.872-.892.077.077 0 0 1-.008-.128 10.2 10.2 0 0 0 .372-.292.074.074 0 0 1 .077-.01c3.928 1.793 8.18 1.793 12.062 0a.074.074 0 0 1 .078.01c.12.098.246.198.373.292a.077.077 0 0 1-.006.127 12.299 12.299 0 0 1-1.873.892.077.077 0 0 0-.041.107c.36.698.772 1.362 1.225 1.993a.076.076 0 0 0 .084.028 19.839 19.839 0 0 0 6.002-3.03.077.077 0 0 0 .032-.054c.5-5.177-.838-9.674-3.549-13.66a.061.061 0 0 0-.031-.03ZM8.02 15.33c-1.183 0-2.157-1.085-2.157-2.419 0-1.333.956-2.419 2.157-2.419 1.21 0 2.176 1.095 2.157 2.42 0 1.333-.956 2.418-2.157 2.418Zm7.975 0c-1.183 0-2.157-1.085-2.157-2.419 0-1.333.955-2.419 2.157-2.419 1.21 0 2.176 1.095 2.157 2.42 0 1.333-.946 2.418-2.157 2.418Z" />
    </svg>
  );
}

function isCheckboxKind(kind: string | null | undefined): boolean {
  return kind === "issue" || kind === "pull_request";
}

function useGitHubIssueState(attrs: AppLinkAttrs) {
  const gh = attrs as GitHubAttrs;
  const enabled = isCheckboxKind(gh.kind) && !!gh.number;

  const { data } = useQuery({
    queryKey: ["github-issue-state", gh.owner, gh.repo, gh.number],
    queryFn: async () => {
      const result = await todoCommands.githubIssueState(
        gh.owner,
        gh.repo,
        gh.number!,
      );
      if (result.status === "error") {
        throw new Error(result.error);
      }
      return result.data;
    },
    enabled,
    staleTime: 60_000,
  });

  return data ?? null;
}

function Checkbox({ checked }: { checked: boolean }) {
  return (
    <span
      className={cn([
        "flex size-5 shrink-0 items-center justify-center rounded-md border-2",
        checked
          ? "border-neutral-400 bg-neutral-400"
          : "border-neutral-300 bg-white",
      ])}
    >
      {checked && <CheckIcon className="size-3.5 text-white" strokeWidth={3} />}
    </span>
  );
}

export const AppLinkView = forwardRef<HTMLSpanElement, NodeViewComponentProps>(
  function AppLinkView({ nodeProps, ...htmlAttrs }, ref) {
    const attrs = nodeProps.node.attrs as AppLinkAttrs;
    const { header, subline } = getAppLinkDisplayParts(attrs);
    const href = attrs.url;
    const ghState = useGitHubIssueState(attrs);
    const showCheckbox = isCheckboxKind(attrs.kind);
    const checked = ghState === "Closed" || ghState === "Merged";

    const handleClick = useEditorEventCallback(async (view) => {
      if (!href) {
        return;
      }

      if (
        attrs.provider === "github" &&
        (attrs.kind === "issue" || attrs.kind === "pull_request") &&
        attrs.owner &&
        attrs.repo &&
        attrs.number
      ) {
        const pos = nodeProps.getPos();
        const resources = collectSiblingResources(view.state.doc, pos);
        openTaskTab(resources);
        return;
      }

      await openerCommands.openUrl(href, null);
    });

    return (
      <span ref={ref} {...htmlAttrs}>
        <button
          type="button"
          contentEditable={false}
          data-app-link-chip
          data-kind={attrs.kind}
          title={getAppLinkLabel(attrs)}
          onMouseDown={(event) => {
            event.preventDefault();
            event.stopPropagation();
          }}
          onClick={(event) => {
            event.preventDefault();
            event.stopPropagation();
            void handleClick();
          }}
          className={cn([
            "inline-flex max-w-full items-center gap-2.5 rounded-lg px-2 py-1 text-left align-middle",
            "transition-colors hover:bg-neutral-100",
          ])}
        >
          {showCheckbox ? (
            <Checkbox checked={checked} />
          ) : attrs.provider === "slack" ? (
            <SlackIcon className="size-4 shrink-0 text-neutral-500" />
          ) : attrs.provider === "discord" ? (
            <DiscordIcon className="size-4 shrink-0 text-neutral-500" />
          ) : (
            <img
              src="/assets/github-icon.svg"
              alt="github"
              className="size-4 shrink-0 opacity-70"
            />
          )}
          <span className="min-w-0">
            <span className="block truncate text-sm font-medium text-neutral-800">
              {header}
            </span>
            <span className="block truncate text-xs text-neutral-500">
              {subline}
            </span>
          </span>
        </button>
      </span>
    );
  },
);
