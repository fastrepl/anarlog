import { CalendarIcon, ExternalLinkIcon, SparklesIcon } from "lucide-react";
import { useEffect, useRef } from "react";

import { ChangelogContent } from "@hypr/changelog";
import { commands as openerCommands } from "@hypr/plugin-opener2";
import {
  Breadcrumb,
  BreadcrumbItem,
  BreadcrumbList,
  BreadcrumbPage,
  BreadcrumbSeparator,
} from "@hypr/ui/components/ui/breadcrumb";
import { Button } from "@hypr/ui/components/ui/button";
import {
  ScrollFadeOverlay,
  useScrollFade,
} from "@hypr/ui/components/ui/scroll-fade";
import { safeFormat } from "@hypr/utils";

import { useChangelogContent } from "./data";

import { useShell } from "~/contexts/shell";
import { StandardTabWrapper } from "~/shared/main";
import { type TabItem, TabItemBase } from "~/shared/tabs";
import { type Tab } from "~/store/zustand/tabs";

export { getLatestVersion } from "./data";

export const TabItemChangelog: TabItem<Extract<Tab, { type: "changelog" }>> = ({
  tab,
  tabIndex,
  handleCloseThis,
  handleSelectThis,
  handleCloseOthers,
  handleCloseAll,
  handlePinThis,
  handleUnpinThis,
}) => (
  <TabItemBase
    icon={<SparklesIcon className="h-4 w-4" />}
    title="What's New"
    selected={tab.active}
    pinned={tab.pinned}
    tabIndex={tabIndex}
    handleCloseThis={() => handleCloseThis(tab)}
    handleSelectThis={() => handleSelectThis(tab)}
    handleCloseOthers={handleCloseOthers}
    handleCloseAll={handleCloseAll}
    handlePinThis={() => handlePinThis(tab)}
    handleUnpinThis={() => handleUnpinThis(tab)}
  />
);

export function TabContentChangelog({
  tab,
}: {
  tab: Extract<Tab, { type: "changelog" }>;
}) {
  const { current } = tab.state;
  const { leftsidebar, chat } = useShell();

  useEffect(() => {
    leftsidebar.setExpanded(false);
    if (chat.mode === "RightPanelOpen" || chat.mode === "FloatingOpen") {
      chat.sendEvent({ type: "CLOSE" });
    }
  }, []);

  const { content, date, loading } = useChangelogContent(current);
  const scrollRef = useRef<HTMLDivElement>(null);
  const { atStart, atEnd } = useScrollFade(scrollRef);

  return (
    <StandardTabWrapper>
      <div className="flex h-full flex-col">
        <div className="shrink-0 pr-1 pl-2">
          <ChangelogHeader version={current} date={date} />
        </div>

        <div className="mt-2 shrink-0 px-3">
          <h1 className="text-xl font-semibold text-neutral-900">
            What's new in {current}?
          </h1>
        </div>

        <div className="relative mt-4 min-h-0 flex-1 overflow-hidden">
          {!atStart && <ScrollFadeOverlay position="top" />}
          {!atEnd && <ScrollFadeOverlay position="bottom" />}
          <div ref={scrollRef} className="h-full overflow-y-auto px-3 pb-4">
            {loading ? (
              <p className="text-neutral-500">Loading...</p>
            ) : content ? (
              <ChangelogContent
                content={content}
                components={{
                  a: ({
                    href,
                    children,
                  }: {
                    href?: string;
                    children?: React.ReactNode;
                  }) => (
                    <a
                      className="text-blue-600 underline hover:text-blue-800"
                      href={href}
                      onClick={(e) => {
                        e.preventDefault();
                        if (href) {
                          void openerCommands.openUrl(href, null);
                        }
                      }}
                    >
                      {children}
                    </a>
                  ),
                }}
              />
            ) : (
              <p className="text-neutral-500">
                No changelog available for this version.
              </p>
            )}
          </div>
        </div>
      </div>
    </StandardTabWrapper>
  );
}

function ChangelogHeader({
  version,
  date,
}: {
  version: string;
  date: string | null;
}) {
  const formattedDate = date ? safeFormat(date, "MMM d, yyyy") : null;

  return (
    <div className="w-full pt-1">
      <div className="flex items-center gap-2">
        <div className="min-w-0 flex-1">
          <Breadcrumb className="ml-1.5 min-w-0">
            <BreadcrumbList className="flex-nowrap gap-0.5 overflow-hidden text-xs text-neutral-700">
              <BreadcrumbItem className="shrink-0">
                <span className="text-neutral-500">Changelog</span>
              </BreadcrumbItem>
              <BreadcrumbSeparator className="shrink-0" />
              <BreadcrumbItem className="overflow-hidden">
                <BreadcrumbPage className="truncate">{version}</BreadcrumbPage>
              </BreadcrumbItem>
            </BreadcrumbList>
          </Breadcrumb>
        </div>

        <div className="flex shrink-0 items-center">
          {formattedDate && (
            <Button
              size="sm"
              variant="ghost"
              className="pointer-events-none text-neutral-600"
            >
              <CalendarIcon size={14} className="shrink-0" />
              <span>{formattedDate}</span>
            </Button>
          )}
          <Button
            size="sm"
            variant="ghost"
            className="gap-1.5 text-neutral-600 hover:text-black"
            onClick={() =>
              openerCommands.openUrl(
                `https://char.com/changelog/${version}`,
                null,
              )
            }
          >
            <ExternalLinkIcon size={14} />
            <span>Open in web</span>
          </Button>
        </div>
      </div>
    </div>
  );
}
