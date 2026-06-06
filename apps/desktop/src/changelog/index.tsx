import { XIcon } from "lucide-react";

import { ChangelogContent } from "@hypr/changelog";
import { commands as openerCommands } from "@hypr/plugin-opener2";
import { Button } from "@hypr/ui/components/ui/button";
import { cn } from "@hypr/utils";

import { useChangelogContent } from "./data";

import { useShell } from "~/contexts/shell";
import { useConfigValue } from "~/shared/config";
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
  const sidebarTimelineEnabled = useConfigValue("sidebar_timeline_enabled");
  const showSidebarTimelineHeaderGutter =
    sidebarTimelineEnabled && !leftsidebar.expanded;

  useMountEffect(() => {
    if (chat.mode !== "FloatingClosed") {
      chat.sendEvent({ type: "CLOSE" });
    }
  });

  const { content, loading } = useChangelogContent(current);

  return (
    <StandardTabWrapper>
      <div className="flex h-full flex-col">
        <div className="shrink-0 pr-1 pl-3">
          <ChangelogHeader
            version={current}
            showSidebarTimelineHeaderGutter={showSidebarTimelineHeaderGutter}
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
  showSidebarTimelineHeaderGutter,
  version,
  onClose,
}: {
  showSidebarTimelineHeaderGutter: boolean;
  version: string;
  onClose: () => void;
}) {
  return (
    <div
      className={cn([
        "flex h-12 w-full items-center",
        showSidebarTimelineHeaderGutter && "pl-[156px]",
      ])}
    >
      <div className="flex w-full min-w-0 items-center justify-between gap-0">
        <div className="min-w-0 flex-1">
          <h1 className="text-foreground truncate text-xl font-semibold">
            What's new in {version}?
          </h1>
        </div>

        <div className="flex shrink-0 items-center gap-0 pr-1">
          <Button
            size="icon"
            variant="ghost"
            className="text-muted-foreground hover:text-foreground"
            aria-label="Close changelog"
            title="Close"
            onClick={onClose}
          >
            <XIcon size={15} />
          </Button>
        </div>
      </div>
    </div>
  );
}
