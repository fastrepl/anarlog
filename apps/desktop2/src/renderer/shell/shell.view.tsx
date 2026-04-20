import {
  ArrowLeftIcon,
  ArrowRightIcon,
  HouseIcon,
  SearchIcon,
} from "lucide-react";
import { Reorder } from "motion/react";

import { Button } from "@hypr/ui/components/ui/button";
import { cn } from "@hypr/utils";

import { TabItemView } from "~/tabs";
import type { Tab } from "~/tabs/tabs.types";

export function ShellView({
  tabItems,
  currentTab,
  canGoBack,
  canGoNext,
  isMac,
  isChatOpen,
  shortcutIndexes,
  openNoteDialog,
  profileMenu,
  updateBanner,
  body,
  onSelect,
  onClose,
  onReorder,
  onGoBack,
  onGoNext,
  onHome,
  onAdHoc,
  onOpenSearch,
  onToggleChat,
}: {
  tabItems: Array<{ tab: Tab; id: string; title: string }>;
  currentTab: Tab | null;
  canGoBack: boolean;
  canGoNext: boolean;
  isMac: boolean;
  isChatOpen: boolean;
  shortcutIndexes: Map<string, number | undefined>;
  openNoteDialog?: React.ReactNode;
  profileMenu?: React.ReactNode;
  updateBanner?: React.ReactNode;
  body: React.ReactNode;
  onSelect: (id: string) => void;
  onClose: (id: string) => void;
  onReorder: (tabs: Tab[]) => void;
  onGoBack: () => void;
  onGoNext: () => void;
  onHome: () => void;
  onAdHoc: () => void;
  onOpenSearch: () => void;
  onToggleChat: () => void;
}) {
  const isHomeActive = currentTab === null;

  return (
    <div
      className="flex h-full min-w-0 gap-1 overflow-hidden bg-stone-50 px-1 pb-1"
      data-testid="main-app-shell"
    >
      {openNoteDialog}
      <div className="flex h-full min-w-0 flex-1 flex-col">
        <div className="drag flex h-10 w-full min-w-0 shrink-0 items-center gap-1 pr-1 pl-3">
          <div
            className={cn([
              "no-drag flex shrink-0 items-center gap-1",
              isMac && "pl-16",
            ])}
          >
            <Button
              onClick={onHome}
              variant="ghost"
              size="icon"
              className={cn([
                "text-neutral-600",
                isHomeActive &&
                  "bg-neutral-200 text-neutral-900 hover:bg-neutral-200",
              ])}
              aria-pressed={isHomeActive}
              title="Home"
            >
              <HouseIcon size={16} />
            </Button>
            {!isHomeActive ? (
              <>
                <Button
                  onClick={onGoBack}
                  disabled={!canGoBack}
                  variant="ghost"
                  size="icon"
                  className="text-neutral-600"
                >
                  <ArrowLeftIcon size={16} />
                </Button>
                <Button
                  onClick={onGoNext}
                  disabled={!canGoNext}
                  variant="ghost"
                  size="icon"
                  className="text-neutral-600"
                >
                  <ArrowRightIcon size={16} />
                </Button>
              </>
            ) : null}
          </div>

          <div className="relative h-full min-w-0 flex-1">
            <div
              className={cn([
                "[-ms-overflow-style:none] [scrollbar-width:none] [&::-webkit-scrollbar]:hidden",
                "h-full w-full overflow-x-auto overflow-y-hidden",
              ])}
            >
              <Reorder.Group
                as="div"
                axis="x"
                values={tabItems.map((item) => item.tab)}
                onReorder={onReorder}
                className="flex h-full w-max items-center gap-1"
              >
                {tabItems.map(({ tab, id, title }) => {
                  return (
                    <Reorder.Item
                      key={id}
                      value={tab}
                      as="div"
                      style={{ position: "relative" }}
                      className="z-10 flex h-full items-center"
                      transition={{ layout: { duration: 0.15 } }}
                    >
                      <TabItemView
                        tab={tab}
                        selected={
                          currentTab ? id === getTabId(currentTab) : false
                        }
                        title={title}
                        shortcutIndex={shortcutIndexes.get(id)}
                        onSelect={() => onSelect(id)}
                        onClose={() => onClose(id)}
                      />
                    </Reorder.Item>
                  );
                })}
              </Reorder.Group>
            </div>
          </div>

          <div className="no-drag ml-auto flex shrink-0 items-center gap-2">
            <Button
              type="button"
              onClick={onAdHoc}
              title="New ad-hoc session"
              aria-label="New ad-hoc session"
              variant="ghost"
              size="icon"
              className="group shrink-0"
            >
              <span className="relative h-3.5 w-3.5 overflow-hidden rounded-full border border-red-500/60 bg-linear-to-b from-red-400 to-red-500 shadow-[inset_0_1px_0_rgba(255,255,255,0.22),0_1px_2px_rgba(127,29,29,0.14)] transition-[filter] group-hover:brightness-110">
                <span className="pointer-events-none absolute top-[1px] left-1/2 h-[22%] w-[68%] -translate-x-1/2 rounded-full bg-white/18" />
              </span>
            </Button>
            <Button
              onClick={onOpenSearch}
              variant="ghost"
              size="icon"
              className="text-neutral-600"
              title="Search (⌘K)"
            >
              <SearchIcon size={16} />
            </Button>
            <Button
              onClick={onToggleChat}
              variant="ghost"
              size="icon"
              className={cn([
                "text-neutral-600",
                isChatOpen &&
                  "bg-neutral-200 text-neutral-900 hover:bg-neutral-200",
              ])}
              aria-label={isChatOpen ? "Close chat" : "Chat with notes"}
              aria-pressed={isChatOpen}
              title={isChatOpen ? "Close chat" : "Chat with notes"}
            >
              <img
                src="/assets/char-chat-bubble.svg"
                alt="Char"
                className={cn([
                  "size-[16px] shrink-0 object-contain opacity-65",
                  isChatOpen && "opacity-100",
                ])}
              />
            </Button>
            {profileMenu}
          </div>
        </div>

        {updateBanner}
        <div className="flex h-full min-h-0 flex-1 overflow-hidden rounded-xl border border-neutral-200 bg-white shadow-xs">
          <div className="h-full min-h-0 w-full overflow-auto">{body}</div>
        </div>
      </div>
    </div>
  );
}

function getTabId(tab: Tab): string {
  return tab.type === "sessions" ? `sessions-${tab.id}` : tab.type;
}
