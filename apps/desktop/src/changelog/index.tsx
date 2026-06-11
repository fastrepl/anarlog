import { XIcon } from "lucide-react";

import { ChangelogContent } from "@hypr/changelog";
import { commands as openerCommands } from "@hypr/plugin-opener2";
import { Button } from "@hypr/ui/components/ui/button";
import { cn } from "@hypr/utils";

import { useChangelogContent } from "./data";

import { useShell } from "~/contexts/shell";
import { useMountEffect } from "~/shared/hooks/useMountEffect";
import { StandardTabWrapper } from "~/shared/main";
import { type Tab, useTabs } from "~/store/zustand/tabs";

export { getLatestVersion } from "./data";

export function TabContentChangelog({
  tab,
}: {
  tab: Extract<Tab, { type: "changelog" }>;
}) {
  const { current } = tab.state;
  const { chat, leftsidebar } = useShell();
  const close = useTabs((state) => state.close);
  const showSidebarTimelineHeaderGutter = !leftsidebar.expanded;
  const showExpandedSidebarTimelineHeader = leftsidebar.expanded;

  useMountEffect(() => {
    if (chat.mode !== "FloatingClosed") {
      chat.sendEvent({ type: "CLOSE" });
    }
  });

  const { content, loading } = useChangelogContent(current);

  return (
    <StandardTabWrapper>
      <div className="flex h-full flex-col">
        <div data-tauri-drag-region className="shrink-0 pr-1 pl-3">
          <ChangelogHeader
            version={current}
            showSidebarTimelineHeaderGutter={showSidebarTimelineHeaderGutter}
            showExpandedSidebarTimelineHeader={
              showExpandedSidebarTimelineHeader
            }
            onClose={() => close(tab)}
          />
        </div>

        <div className="relative mt-2 min-h-0 flex-1 overflow-hidden">
          <div className="scroll-fade-y h-full overflow-y-auto px-3 pb-4">
            <ChangelogBody content={content} loading={loading} />
          </div>
        </div>
      </div>
    </StandardTabWrapper>
  );
}

function ExternalLink({
  href,
  children,
}: {
  href: string;
  children: React.ReactNode;
}) {
  return (
    <a
      className="text-blue-600 underline decoration-blue-400/40 underline-offset-2 hover:text-blue-700 dark:text-blue-400 dark:decoration-blue-500/50 dark:hover:text-blue-300"
      href={href}
      onClick={(e) => {
        e.preventDefault();
        void openerCommands.openUrl(href, null);
      }}
    >
      {children}
    </a>
  );
}

function ChangelogBody({
  content,
  loading,
}: {
  content: string | null;
  loading: boolean;
}) {
  if (loading) {
    return <p className="text-muted-foreground">Loading...</p>;
  }

  if (content) {
    return (
      <ChangelogContent
        content={content}
        components={{
          a: ({
            href,
            children,
          }: {
            href?: string;
            children?: React.ReactNode;
          }) =>
            href ? (
              <ExternalLink href={href}>{children}</ExternalLink>
            ) : (
              <>{children}</>
            ),
        }}
      />
    );
  }

  return (
    <p className="text-muted-foreground">
      No changelog available for this version.
    </p>
  );
}

function ChangelogHeader({
  showExpandedSidebarTimelineHeader,
  showSidebarTimelineHeaderGutter,
  version,
  onClose,
}: {
  showExpandedSidebarTimelineHeader: boolean;
  showSidebarTimelineHeaderGutter: boolean;
  version: string;
  onClose: () => void;
}) {
  return (
    <div
      data-tauri-drag-region
      className={cn([
        "relative flex h-12 w-full items-center",
        showSidebarTimelineHeaderGutter && "pl-[156px]",
      ])}
    >
      <div
        data-tauri-drag-region
        className={cn([
          "pointer-events-none absolute inset-y-0 flex items-center",
          showExpandedSidebarTimelineHeader
            ? "right-[70px] left-0 justify-start"
            : showSidebarTimelineHeaderGutter
              ? "right-[70px] left-[104px] justify-start"
              : "left-1/2 w-[min(640px,calc(100%_-_160px))] -translate-x-1/2 justify-center",
        ])}
      >
        <h1
          className={cn([
            "text-foreground truncate text-xl font-semibold",
            showExpandedSidebarTimelineHeader || showSidebarTimelineHeaderGutter
              ? "text-left"
              : "text-center",
          ])}
        >
          What's new in {version}?
        </h1>
      </div>

      <div className="relative z-10 ml-auto flex shrink-0 items-center gap-0 pr-1">
        <Button
          size="icon"
          variant="ghost"
          data-tauri-drag-region="false"
          className="text-muted-foreground hover:text-foreground"
          aria-label="Close changelog"
          title="Close"
          onClick={onClose}
        >
          <XIcon size={16} />
        </Button>
      </div>
    </div>
  );
}
