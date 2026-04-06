import { useQuery } from "@tanstack/react-query";
import { platform } from "@tauri-apps/plugin-os";
import {
  ArrowLeftIcon,
  ArrowRightIcon,
  PanelLeftOpenIcon,
  PlusIcon,
} from "lucide-react";
import { Reorder } from "motion/react";
import { useCallback, useMemo, useRef } from "react";
import { useShallow } from "zustand/shallow";

import { commands as flagCommands } from "@hypr/plugin-flag";
import { Button } from "@hypr/ui/components/ui/button";
import { Kbd } from "@hypr/ui/components/ui/kbd";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@hypr/ui/components/ui/tooltip";
import { cn } from "@hypr/utils";

import { useNotifications } from "~/contexts/notifications";
import { useShell } from "~/contexts/shell";
import { ClassicMainTabItem } from "~/main/tab-item";
import { useClassicMainTabsShortcuts } from "~/main/useTabsShortcuts";
import { useNativeContextMenu } from "~/shared/hooks/useNativeContextMenu";
import { useScrollActiveTabIntoView } from "~/shared/main";
import { NotificationBadge } from "~/shared/ui/notification-badge";
import { TrafficLights } from "~/shared/ui/traffic-lights";
import { useNewNote, useNewNoteAndListen } from "~/shared/useNewNote";
import { Update } from "~/sidebar/update";
import { type Tab, uniqueIdfromTab, useTabs } from "~/store/zustand/tabs";
import { useListener } from "~/stt/contexts";

export function ClassicMainTabChrome({ tabs }: { tabs: Tab[] }) {
  const { leftsidebar } = useShell();
  const currentPlatform = platform();
  const isLinux = currentPlatform === "linux";
  const chatShortcutLabel = currentPlatform === "macos" ? "⌘ J" : "Ctrl J";
  const notifications = useNotifications();
  const currentTab = useTabs((state) => state.currentTab);
  const isOnboarding = currentTab?.type === "onboarding";
  const isSidebarHidden = isOnboarding || !leftsidebar.expanded;
  const {
    select,
    close,
    reorder,
    goBack,
    goNext,
    canGoBack,
    canGoNext,
    closeOthers,
    closeAll,
    pin,
    unpin,
    pendingCloseConfirmationTab,
    setPendingCloseConfirmationTab,
  } = useTabs(
    useShallow((state) => ({
      select: state.select,
      close: state.close,
      reorder: state.reorder,
      goBack: state.goBack,
      goNext: state.goNext,
      canGoBack: state.canGoBack,
      canGoNext: state.canGoNext,
      closeOthers: state.closeOthers,
      closeAll: state.closeAll,
      pin: state.pin,
      unpin: state.unpin,
      pendingCloseConfirmationTab: state.pendingCloseConfirmationTab,
      setPendingCloseConfirmationTab: state.setPendingCloseConfirmationTab,
    })),
  );

  const liveSessionId = useListener((state) => state.live.sessionId);
  const liveStatus = useListener((state) => state.live.status);
  const isListening = liveStatus === "active" || liveStatus === "finalizing";

  const listeningTab = useMemo(
    () =>
      isListening && liveSessionId
        ? tabs.find(
            (tab) => tab.type === "sessions" && tab.id === liveSessionId,
          )
        : null,
    [isListening, liveSessionId, tabs],
  );
  const regularTabs = useMemo(
    () =>
      listeningTab
        ? tabs.filter(
            (tab) => !(tab.type === "sessions" && tab.id === liveSessionId),
          )
        : tabs,
    [listeningTab, tabs, liveSessionId],
  );

  const handleNewEmptyTab = useNewEmptyTab();
  const handleNewNote = useNewNote();
  const handleNewNoteAndListen = useNewNoteAndListen();
  const newNoteAccelerator = currentPlatform === "macos" ? "Cmd+N" : "Ctrl+N";
  const showNewTabMenu = useNativeContextMenu([
    {
      id: "new-note",
      text: "Create Empty Note",
      accelerator: newNoteAccelerator,
      action: handleNewNote,
    },
    {
      id: "new-meeting",
      text: "Start New Meeting",
      action: handleNewNoteAndListen,
    },
  ]);

  const tabsScrollContainerRef = useRef<HTMLDivElement>(null);
  const setTabRef = useScrollActiveTabIntoView(regularTabs);
  useClassicMainTabsShortcuts();

  return (
    <div
      data-tauri-drag-region
      className={cn([
        "flex h-9 w-full items-center",
        isSidebarHidden && (isLinux ? "pl-3" : "pl-20"),
      ])}
      data-testid="main-tab-chrome"
    >
      {isSidebarHidden && isLinux && <TrafficLights className="mr-2" />}
      {!leftsidebar.expanded && !isOnboarding && (
        <div className="relative">
          <Tooltip>
            <TooltipTrigger asChild>
              <Button
                size="icon"
                variant="ghost"
                className="shrink-0"
                onClick={() => leftsidebar.setExpanded(true)}
              >
                <PanelLeftOpenIcon size={16} className="text-neutral-600" />
              </Button>
            </TooltipTrigger>
            <TooltipContent side="bottom" className="flex items-center gap-2">
              <span>Toggle sidebar</span>
              <Kbd className="animate-kbd-press">⌘ \</Kbd>
            </TooltipContent>
          </Tooltip>
          <NotificationBadge show={notifications.shouldShowBadge} />
        </div>
      )}

      {!isOnboarding && (
        <div className="flex h-full shrink-0 items-center">
          <Button
            onClick={goBack}
            disabled={!canGoBack}
            variant="ghost"
            size="icon"
          >
            <ArrowLeftIcon size={16} />
          </Button>
          <Button
            onClick={goNext}
            disabled={!canGoNext}
            variant="ghost"
            size="icon"
          >
            <ArrowRightIcon size={16} />
          </Button>
        </div>
      )}

      {listeningTab && (
        <div className="mr-1 flex h-full shrink-0 items-center">
          <ClassicMainTabItem
            tab={listeningTab}
            handleClose={close}
            handleSelect={select}
            handleCloseOthersCallback={closeOthers}
            handleCloseAll={closeAll}
            handlePin={pin}
            handleUnpin={unpin}
            tabIndex={1}
            pendingCloseConfirmationTab={pendingCloseConfirmationTab}
            setPendingCloseConfirmationTab={setPendingCloseConfirmationTab}
          />
        </div>
      )}

      <div className="relative h-full min-w-0">
        <div
          ref={tabsScrollContainerRef}
          data-tauri-drag-region
          className={cn([
            "scroll-fade-x",
            "[-ms-overflow-style:none] [scrollbar-width:none] [&::-webkit-scrollbar]:hidden",
            "h-full w-full overflow-x-auto overflow-y-hidden",
          ])}
        >
          <Reorder.Group
            key={leftsidebar.expanded ? "expanded" : "collapsed"}
            as="div"
            axis="x"
            values={regularTabs}
            onReorder={reorder}
            className="flex h-full w-max gap-1"
          >
            {regularTabs.map((tab, index) => {
              const isLastTab = index === regularTabs.length - 1;
              const shortcutIndex = listeningTab
                ? index < 7
                  ? index + 2
                  : isLastTab
                    ? 9
                    : undefined
                : index < 8
                  ? index + 1
                  : isLastTab
                    ? 9
                    : undefined;

              return (
                <Reorder.Item
                  key={uniqueIdfromTab(tab)}
                  value={tab}
                  as="div"
                  ref={(el) => setTabRef(tab, el)}
                  style={{ position: "relative" }}
                  className="z-10 h-full"
                  transition={{ layout: { duration: 0.15 } }}
                >
                  <ClassicMainTabItem
                    tab={tab}
                    handleClose={close}
                    handleSelect={select}
                    handleCloseOthersCallback={closeOthers}
                    handleCloseAll={closeAll}
                    handlePin={pin}
                    handleUnpin={unpin}
                    tabIndex={shortcutIndex}
                    pendingCloseConfirmationTab={pendingCloseConfirmationTab}
                    setPendingCloseConfirmationTab={
                      setPendingCloseConfirmationTab
                    }
                  />
                </Reorder.Item>
              );
            })}
          </Reorder.Group>
        </div>
      </div>

      <div
        data-tauri-drag-region
        className="flex h-full flex-1 items-center justify-between"
      >
        <Button
          onClick={isOnboarding ? undefined : handleNewEmptyTab}
          onContextMenu={isOnboarding ? undefined : showNewTabMenu}
          disabled={isOnboarding}
          variant="ghost"
          size="icon"
          className={cn([
            "text-neutral-600",
            isOnboarding && "cursor-not-allowed opacity-40",
          ])}
        >
          <PlusIcon size={16} />
        </Button>

        <div className="ml-auto flex h-full items-center gap-1">
          <Update />
          <TabChatButton shortcutLabel={chatShortcutLabel} />
        </div>
      </div>
    </div>
  );
}

function TabChatButton({ shortcutLabel }: { shortcutLabel?: string }) {
  const { chat } = useShell();
  const isChatOpen =
    chat.mode === "FloatingOpen" || chat.mode === "RightPanelOpen";
  const isTabbarSelected = isChatOpen || chat.mode === "FullTab";

  const { data: isChatEnabled } = useQuery({
    refetchInterval: 10_000,
    queryKey: ["flag", "chat"],
    queryFn: async () => {
      const result = await flagCommands.isEnabled("chat");
      if (result.status === "error") {
        throw new Error(result.error);
      }
      return result.data;
    },
  });

  if (!isChatEnabled) {
    return null;
  }

  const buttonTitle = isTabbarSelected ? "Close chat" : "Chat with notes";

  const handleClick = () =>
    chat.sendEvent(isTabbarSelected ? { type: "TOGGLE" } : { type: "OPEN" });

  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <Button
          onClick={handleClick}
          variant="ghost"
          size="icon"
          className={cn([
            "text-neutral-600",
            isTabbarSelected &&
              "bg-neutral-200 text-neutral-900 hover:bg-neutral-200",
          ])}
          aria-label={buttonTitle}
          aria-pressed={isTabbarSelected}
          title={buttonTitle}
        >
          <img
            src="/assets/char-chat-bubble.svg"
            alt="Char"
            className={cn([
              "size-[16px] shrink-0 object-contain opacity-65",
              isTabbarSelected && "opacity-100",
            ])}
          />
        </Button>
      </TooltipTrigger>
      <TooltipContent side="bottom" className="flex items-center gap-2">
        <span>{buttonTitle}</span>
        {shortcutLabel && (
          <Kbd className="animate-kbd-press">{shortcutLabel}</Kbd>
        )}
      </TooltipContent>
    </Tooltip>
  );
}

function useNewEmptyTab() {
  const openNew = useTabs((state) => state.openNew);

  const handler = useCallback(() => {
    openNew({ type: "empty" });
  }, [openNew]);

  return handler;
}
