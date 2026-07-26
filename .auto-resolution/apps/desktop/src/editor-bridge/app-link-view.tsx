import {
  type NodeViewComponentProps,
  useEditorEventCallback,
} from "@handlewithcare/react-prosemirror";
import { Figma, Github, Google, Notion } from "@lobehub/icons";
import { useQuery } from "@tanstack/react-query";
import { CheckIcon } from "lucide-react";
import { forwardRef } from "react";

import {
  getAppLinkDisplayParts,
  getAppLinkLabel,
  type GitHubAttrs,
  type AppLinkAttrs,
} from "@hypr/editor/app-link";
import { getSafeNodePos } from "@hypr/editor/node-views";
import { commands as openerCommands } from "@hypr/plugin-opener2";
import { commands as todoCommands } from "@hypr/plugin-todo";
import { cn } from "@hypr/utils";

import { collectSiblingResources, openTaskTab } from "~/task/open-task-tab";

function SlackIcon({ className }: { className?: string }) {
  return (
    <svg className={className} viewBox="0 0 127 127" fill="none">
      <path
        d="M27.2 80c0 7.3-5.9 13.2-13.2 13.2C6.7 93.2.8 87.3.8 80c0-7.3 5.9-13.2 13.2-13.2h13.2V80zm6.6 0c0-7.3 5.9-13.2 13.2-13.2 7.3 0 13.2 5.9 13.2 13.2v33c0 7.3-5.9 13.2-13.2 13.2-7.3 0-13.2-5.9-13.2-13.2V80z"
        fill="#E01E5A"
      />
      <path
        d="M47 27c-7.3 0-13.2-5.9-13.2-13.2C33.8 6.5 39.7.6 47 .6c7.3 0 13.2 5.9 13.2 13.2V27H47zm0 6.7c7.3 0 13.2 5.9 13.2 13.2 0 7.3-5.9 13.2-13.2 13.2H13.9C6.6 60.1.7 54.2.7 46.9c0-7.3 5.9-13.2 13.2-13.2H47z"
        fill="#36C5F0"
      />
      <path
        d="M99.9 46.9c0-7.3 5.9-13.2 13.2-13.2 7.3 0 13.2 5.9 13.2 13.2 0 7.3-5.9 13.2-13.2 13.2H99.9V46.9zm-6.6 0c0 7.3-5.9 13.2-13.2 13.2-7.3 0-13.2-5.9-13.2-13.2V13.8C66.9 6.5 72.8.6 80.1.6c7.3 0 13.2 5.9 13.2 13.2v33.1z"
        fill="#2EB67D"
      />
      <path
        d="M80.1 99.8c7.3 0 13.2 5.9 13.2 13.2 0 7.3-5.9 13.2-13.2 13.2-7.3 0-13.2-5.9-13.2-13.2V99.8h13.2zm0-6.6c-7.3 0-13.2-5.9-13.2-13.2 0-7.3 5.9-13.2 13.2-13.2h33.1c7.3 0 13.2 5.9 13.2 13.2 0 7.3-5.9 13.2-13.2 13.2H80.1z"
        fill="#ECB22E"
      />
    </svg>
  );
}

function LinearIcon({ className }: { className?: string }) {
  return (
    <svg className={className} viewBox="0 0 256 256" fill="none">
      <path
        fill="currentColor"
        d="m8.174 102.613l145.213 145.213c2.12 2.12 1.097 5.72-1.85 6.27a128 128 0 0 1-15.02 1.896a3.78 3.78 0 0 1-2.92-1.109L1.117 122.403a3.78 3.78 0 0 1-1.109-2.92c.34-5.095.978-10.107 1.896-15.02c.55-2.947 4.15-3.97 6.27-1.85m-4.092 58.796c-.97-3.614 3.3-5.894 5.946-3.248l87.81 87.811c2.647 2.646.367 6.915-3.247 5.946c-44.03-11.805-78.704-46.478-90.51-90.509m12.727-97.245c1.233-2.135 4.147-2.463 5.89-.719L192.556 233.3c1.744 1.744 1.417 4.658-.72 5.891a128 128 0 0 1-11.1 5.705c-1.43.65-3.11.322-4.22-.79L11.893 79.487c-1.111-1.112-1.439-2.79-.79-4.221a128 128 0 0 1 5.706-11.1M127.86 0C198.63 0 256 57.37 256 128.14c0 37.57-16.168 71.362-41.926 94.8c-1.487 1.354-3.768 1.264-5.19-.157L33.217 47.116c-1.421-1.422-1.51-3.703-.158-5.19C56.498 16.168 90.291 0 127.86 0"
      />
    </svg>
  );
}

function DiscordIcon({ className }: { className?: string }) {
  return (
    <svg className={className} viewBox="0 0 24 24" fill="#5865F2">
      <path d="M20.317 4.37a19.791 19.791 0 0 0-4.885-1.515.074.074 0 0 0-.079.037c-.21.375-.444.864-.608 1.25a18.27 18.27 0 0 0-5.487 0 12.64 12.64 0 0 0-.617-1.25.077.077 0 0 0-.079-.037A19.736 19.736 0 0 0 3.677 4.37a.07.07 0 0 0-.032.027C.533 9.046-.32 13.58.099 18.057a.082.082 0 0 0 .031.057 19.9 19.9 0 0 0 5.993 3.03.078.078 0 0 0 .084-.028c.462-.63.874-1.295 1.226-1.994a.076.076 0 0 0-.041-.106 13.107 13.107 0 0 1-1.872-.892.077.077 0 0 1-.008-.128 10.2 10.2 0 0 0 .372-.292.074.074 0 0 1 .077-.01c3.928 1.793 8.18 1.793 12.062 0a.074.074 0 0 1 .078.01c.12.098.246.198.373.292a.077.077 0 0 1-.006.127 12.299 12.299 0 0 1-1.873.892.077.077 0 0 0-.041.107c.36.698.772 1.362 1.225 1.993a.076.076 0 0 0 .084.028 19.839 19.839 0 0 0 6.002-3.03.077.077 0 0 0 .032-.054c.5-5.177-.838-9.674-3.549-13.66a.061.061 0 0 0-.031-.03ZM8.02 15.33c-1.183 0-2.157-1.085-2.157-2.419 0-1.333.956-2.419 2.157-2.419 1.21 0 2.176 1.095 2.157 2.42 0 1.333-.956 2.418-2.157 2.418Zm7.975 0c-1.183 0-2.157-1.085-2.157-2.419 0-1.333.955-2.419 2.157-2.419 1.21 0 2.176 1.095 2.157 2.42 0 1.333-.946 2.418-2.157 2.418Z" />
    </svg>
  );
}

function AtlassianIcon({ className }: { className?: string }) {
  return (
    <svg className={className} viewBox="0 0 24 24" fill="none">
      <path
        d="M7.6 3.4c.4-.7 1.4-.7 1.8 0l6.7 11.6c.4.7-.1 1.6-.9 1.6h-3.4c-.4 0-.7-.2-.9-.5L5.9 7.5c-.2-.3-.2-.7 0-1l1.7-3.1Z"
        fill="#2684FF"
      />
      <path
        d="M16.4 3.4c-.4-.7-1.4-.7-1.8 0L7.9 15c-.4.7.1 1.6.9 1.6h3.4c.4 0 .7-.2.9-.5l5-8.6c.2-.3.2-.7 0-1l-1.7-3.1Z"
        fill="#0052CC"
      />
      <path
        d="M12 12.9 8.7 18.6c-.4.7.1 1.6.9 1.6h4.8c.8 0 1.3-.9.9-1.6L12 12.9Z"
        fill="#172B4D"
      />
    </svg>
  );
}

function AsanaIcon({ className }: { className?: string }) {
  return (
    <svg className={className} viewBox="0 0 24 24" fill="none">
      <circle cx="12" cy="7" r="3.2" fill="#FF7361" />
      <circle cx="7.5" cy="15.5" r="3.2" fill="#F06A6A" />
      <circle cx="16.5" cy="15.5" r="3.2" fill="#E65AA0" />
    </svg>
  );
}

function TrelloIcon({ className }: { className?: string }) {
  return (
    <svg className={className} viewBox="0 0 24 24" fill="none">
      <rect width="20" height="20" x="2" y="2" rx="4" fill="#0079BF" />
      <rect width="5" height="12" x="6" y="6" rx="1.5" fill="white" />
      <rect width="5" height="8" x="13" y="6" rx="1.5" fill="white" />
    </svg>
  );
}

function AirtableIcon({ className }: { className?: string }) {
  return (
    <svg className={className} viewBox="0 0 24 24" fill="none">
      <path d="M12 3 3.5 6.8 12 10.6l8.5-3.8L12 3Z" fill="#FCB400" />
      <path d="M4 9.2 10.8 12v7L4 15.9V9.2Z" fill="#18BFFF" />
      <path d="M20 9.2 13.2 12v7l6.8-3.1V9.2Z" fill="#F82B60" />
    </svg>
  );
}

function MiroIcon({ className }: { className?: string }) {
  return (
    <svg className={className} viewBox="0 0 24 24" fill="none">
      <rect width="20" height="20" x="2" y="2" rx="4" fill="#FFD02F" />
      <path
        d="M7 17V7h2.2l1.6 4.1L12.8 7H15v10h-2.1v-5.4l-1.4 3.2h-1.4l-1.1-3.1V17H7Z"
        fill="#050038"
      />
    </svg>
  );
}

function LoomIcon({ className }: { className?: string }) {
  return (
    <svg className={className} viewBox="0 0 24 24" fill="none">
      <circle cx="12" cy="12" r="3.2" fill="#625DF5" />
      <circle cx="12" cy="5" r="2.2" fill="#625DF5" />
      <circle cx="12" cy="19" r="2.2" fill="#625DF5" />
      <circle cx="5" cy="12" r="2.2" fill="#625DF5" />
      <circle cx="19" cy="12" r="2.2" fill="#625DF5" />
      <circle cx="7.1" cy="7.1" r="2" fill="#625DF5" />
      <circle cx="16.9" cy="16.9" r="2" fill="#625DF5" />
      <circle cx="16.9" cy="7.1" r="2" fill="#625DF5" />
      <circle cx="7.1" cy="16.9" r="2" fill="#625DF5" />
    </svg>
  );
}

function DropboxIcon({ className }: { className?: string }) {
  return (
    <svg className={className} viewBox="0 0 24 24" fill="none">
      <path d="m7 3 5 3.2L7 9.4 2 6.2 7 3Z" fill="#0061FF" />
      <path d="m17 3 5 3.2-5 3.2-5-3.2L17 3Z" fill="#0061FF" />
      <path d="m7 10.6 5 3.2L7 17l-5-3.2 5-3.2Z" fill="#0061FF" />
      <path d="m17 10.6 5 3.2-5 3.2-5-3.2 5-3.2Z" fill="#0061FF" />
      <path d="m12 15 5 3.2-5 3.2-5-3.2 5-3.2Z" fill="#0061FF" />
    </svg>
  );
}

function ZoomIcon({ className }: { className?: string }) {
  return (
    <svg className={className} viewBox="0 0 24 24" fill="none">
      <rect width="20" height="20" x="2" y="2" rx="5" fill="#0B5CFF" />
      <path d="M6.5 8.5h7a2 2 0 0 1 2 2v3a2 2 0 0 1-2 2h-7v-7Z" fill="white" />
      <path d="m15.5 11 3-2v6l-3-2v-2Z" fill="white" />
    </svg>
  );
}

function CalendlyIcon({ className }: { className?: string }) {
  return (
    <svg className={className} viewBox="0 0 24 24" fill="none">
      <rect width="18" height="18" x="3" y="3" rx="5" fill="#006BFF" />
      <path
        d="M15.8 9.1a4.8 4.8 0 1 0 .1 5.7"
        stroke="white"
        strokeLinecap="round"
        strokeWidth="2.2"
      />
    </svg>
  );
}

function BrandIcon({ attrs }: { attrs: AppLinkAttrs }) {
  switch (attrs.provider) {
    case "slack":
      return <SlackIcon className="size-3.5 shrink-0" />;
    case "discord":
      return <DiscordIcon className="size-3.5 shrink-0" />;
    case "linear":
      return <LinearIcon className="text-muted-foreground size-3.5 shrink-0" />;
    case "notion":
      return (
        <Notion className="text-muted-foreground size-3.5 shrink-0" size={14} />
      );
    case "google":
      return <Google className="size-3.5 shrink-0" size={14} />;
    case "figma":
      return <Figma className="size-3.5 shrink-0" size={14} />;
    case "atlassian":
      return <AtlassianIcon className="size-3.5 shrink-0" />;
    case "asana":
      return <AsanaIcon className="size-3.5 shrink-0" />;
    case "trello":
      return <TrelloIcon className="size-3.5 shrink-0" />;
    case "airtable":
      return <AirtableIcon className="size-3.5 shrink-0" />;
    case "miro":
      return <MiroIcon className="size-3.5 shrink-0" />;
    case "loom":
      return <LoomIcon className="size-3.5 shrink-0" />;
    case "dropbox":
      return <DropboxIcon className="size-3.5 shrink-0" />;
    case "zoom":
      return <ZoomIcon className="size-3.5 shrink-0" />;
    case "calendly":
      return <CalendlyIcon className="size-3.5 shrink-0" />;
    case "github":
      return (
        <Github className="text-muted-foreground size-3.5 shrink-0" size={14} />
      );
  }
}

function isGitHubCheckboxKind(attrs: AppLinkAttrs): attrs is GitHubAttrs {
  return (
    attrs.provider === "github" &&
    (attrs.kind === "issue" || attrs.kind === "pull_request")
  );
}

function useGitHubIssueState(attrs: AppLinkAttrs) {
  const gh = isGitHubCheckboxKind(attrs) ? attrs : null;
  const enabled = !!gh?.number;

  const { data } = useQuery({
    queryKey: ["github-issue-state", gh?.owner, gh?.repo, gh?.number],
    queryFn: async () => {
      const result = await todoCommands.githubIssueState(
        gh!.owner,
        gh!.repo,
        gh!.number!,
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
          ? "border-muted-foreground bg-muted-foreground"
          : "border-border bg-card",
      ])}
    >
      {checked && (
        <CheckIcon
          className="text-primary-foreground size-3.5"
          strokeWidth={3}
        />
      )}
    </span>
  );
}

export const AppLinkView = forwardRef<HTMLSpanElement, NodeViewComponentProps>(
  function AppLinkView({ nodeProps, ...htmlAttrs }, ref) {
    const attrs = nodeProps.node.attrs as AppLinkAttrs;
    const { header, subline } = getAppLinkDisplayParts(attrs);
    const href = attrs.url;
    const ghState = useGitHubIssueState(attrs);
    const showCheckbox = isGitHubCheckboxKind(attrs);
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
        const pos = getSafeNodePos(nodeProps.getPos);
        if (pos !== null) {
          const resources = collectSiblingResources(view.state.doc, pos);
          openTaskTab(resources);
          return;
        }
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
            "border-border/60 bg-muted/35 inline-flex h-6 max-w-full items-center gap-1.5 rounded-md border px-1.5 text-left align-baseline",
            "hover:bg-accent transition-colors",
          ])}
        >
          {showCheckbox ? (
            <Checkbox checked={checked} />
          ) : (
            <BrandIcon attrs={attrs} />
          )}
          <span className="flex min-w-0 items-baseline gap-1.5">
            <span className="text-foreground truncate text-sm leading-none font-medium">
              {header}
            </span>
            <span className="text-muted-foreground truncate text-xs leading-none">
              {subline}
            </span>
          </span>
        </button>
      </span>
    );
  },
);
