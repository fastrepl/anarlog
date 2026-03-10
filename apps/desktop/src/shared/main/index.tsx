import { useQuery } from "@tanstack/react-query";
import { platform } from "@tauri-apps/plugin-os";
import {
  ArrowLeftIcon,
  ArrowRightIcon,
  PanelLeftOpenIcon,
  PlusIcon,
} from "lucide-react";
import { Reorder } from "motion/react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useHotkeys } from "react-hotkeys-hook";
import { useResizeObserver } from "usehooks-ts";
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

import { TabContentEmpty, TabItemEmpty } from "./empty";
import { HeaderListenButton } from "./header-listen-button";
import { useNewNote, useNewNoteAndListen } from "./useNewNote";

import { TabContentAI, TabItemAI } from "~/ai";
import { TabContentCalendar, TabItemCalendar } from "~/calendar";
import { ChatFloatingButton } from "~/chat/components/floating-button";
import { TabContentChat } from "~/chat/tab/tab-content";
import { TabItemChat } from "~/chat/tab/tab-item";
import { TabContentChatShortcut, TabItemChatShortcut } from "~/chat_shortcuts";
import { TabContentContact, TabItemContact } from "~/contacts";
import { TabContentHuman, TabItemHuman } from "~/contacts/humans";
import { useNotifications } from "~/contexts/notifications";
import { useShell } from "~/contexts/shell";
import { TabContentEdit, TabItemEdit } from "~/edit";
import { TabContentFolder, TabItemFolder } from "~/folders";
import { TabContentOnboarding, TabItemOnboarding } from "~/onboarding";
import { TabContentPlugin, TabItemPlugin } from "~/plugins";
import { loadPlugins } from "~/plugins/loader";
import { TabContentSearch, TabItemSearch } from "~/search/advanced";
import { TabContentNote, TabItemNote } from "~/session";
import { useCaretPosition } from "~/session/components/caret-position-context";
import { TabContentSettings, TabItemSettings } from "~/settings";
import { useNativeContextMenu } from "~/shared/hooks/useNativeContextMenu";
import { NotificationBadge } from "~/shared/ui/notification-badge";
import { TrafficLights } from "~/shared/ui/traffic-lights";
import { TabContentChangelog, TabItemChangelog } from "~/sidebar/changelog";
import { Update } from "~/sidebar/update";
import { type Tab, uniqueIdfromTab, useTabs } from "~/store/zustand/tabs";
import { useListener } from "~/stt/contexts";
import { TabContentTemplate, TabItemTemplate } from "~/templates";

export function Body() {
  const { tabs, currentTab } = useTabs(
    useShallow((state) => ({
      tabs: state.tabs,
      currentTab: state.currentTab,
    })),
  );

  useEffect(() => {
    void loadPlugins();
  }, []);

  if (!currentTab) {
    return null;
  }

  return (
    <div className="relative flex h-full flex-1 flex-col gap-1">
      <Header tabs={tabs} />
      <div className="flex-1 overflow-auto">
        <ContentWrapper key={uniqueIdfromTab(currentTab)} tab={currentTab} />
      </div>
    </div>
  );
}

function Header({ tabs }: { tabs: Tab[] }) {
  const { leftsidebar } = useShell();
  const isLinux = platform() === "linux";
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
        ? tabs.find((t) => t.type === "sessions" && t.id === liveSessionId)
        : null,
    [isListening, liveSessionId, tabs],
  );
  const regularTabs = useMemo(
    () =>
      listeningTab
        ? tabs.filter((t) => !(t.type === "sessions" && t.id === liveSessionId))
        : tabs,
    [listeningTab, tabs, liveSessionId],
  );

  const tabsScrollContainerRef = useRef<HTMLDivElement>(null);
  const handleNewEmptyTab = useNewEmptyTab();
  const handleNewNote = useNewNote({ behavior: "new" });
  const handleNewNoteAndListen = useNewNoteAndListen();
  const showNewTabMenu = useNativeContextMenu([
    { id: "empty-tab", text: "Open Empty Tab", action: handleNewEmptyTab },
    { id: "new-note", text: "Create New Note", action: handleNewNote },
    {
      id: "new-note-listen",
      text: "Create and Start Listening",
      action: handleNewNoteAndListen,
    },
  ]);

  const scrollState = useScrollState(
    tabsScrollContainerRef,
    regularTabs.length,
  );

  const setTabRef = useScrollActiveTabIntoView(regularTabs);
  useTabsShortcuts();

  return (
    <div
      data-tauri-drag-region
      className={cn([
        "flex h-9 w-full items-center",
        isSidebarHidden && (isLinux ? "pl-3" : "pl-20"),
      ])}
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
          <TabItem
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
                  <TabItem
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
        {!scrollState.atStart && (
          <div className="pointer-events-none absolute top-0 left-0 z-20 h-full w-8 bg-linear-to-r from-stone-50 to-transparent" />
        )}
        {!scrollState.atEnd && (
          <div className="pointer-events-none absolute top-0 right-0 z-20 h-full w-8 bg-linear-to-l from-stone-50 to-transparent" />
        )}
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
          <HeaderListenButton />
          <Update />
        </div>
      </div>
    </div>
  );
}

function TabItem({
  tab,
  handleClose,
  handleSelect,
  handleCloseOthersCallback,
  handleCloseAll,
  handlePin,
  handleUnpin,
  tabIndex,
  pendingCloseConfirmationTab,
  setPendingCloseConfirmationTab,
}: {
  tab: Tab;
  handleClose: (tab: Tab) => void;
  handleSelect: (tab: Tab) => void;
  handleCloseOthersCallback: (tab: Tab) => void;
  handleCloseAll: () => void;
  handlePin: (tab: Tab) => void;
  handleUnpin: (tab: Tab) => void;
  tabIndex?: number;
  pendingCloseConfirmationTab?: Tab | null;
  setPendingCloseConfirmationTab?: (tab: Tab | null) => void;
}) {
  const handleCloseOthers = () => handleCloseOthersCallback(tab);
  const handlePinThis = () => handlePin(tab);
  const handleUnpinThis = () => handleUnpin(tab);

  if (tab.type === "sessions") {
    return (
      <TabItemNote
        tab={tab}
        tabIndex={tabIndex}
        handleCloseThis={handleClose}
        handleSelectThis={handleSelect}
        handleCloseOthers={handleCloseOthers}
        handleCloseAll={handleCloseAll}
        handlePinThis={handlePinThis}
        handleUnpinThis={handleUnpinThis}
        pendingCloseConfirmationTab={pendingCloseConfirmationTab}
        setPendingCloseConfirmationTab={setPendingCloseConfirmationTab}
      />
    );
  }
  if (tab.type === "folders") {
    return (
      <TabItemFolder
        tab={tab}
        tabIndex={tabIndex}
        handleCloseThis={handleClose}
        handleSelectThis={handleSelect}
        handleCloseOthers={handleCloseOthers}
        handleCloseAll={handleCloseAll}
        handlePinThis={handlePinThis}
        handleUnpinThis={handleUnpinThis}
      />
    );
  }
  if (tab.type === "humans") {
    return (
      <TabItemHuman
        tab={tab}
        tabIndex={tabIndex}
        handleCloseThis={handleClose}
        handleSelectThis={handleSelect}
        handleCloseOthers={handleCloseOthers}
        handleCloseAll={handleCloseAll}
        handlePinThis={handlePinThis}
        handleUnpinThis={handleUnpinThis}
      />
    );
  }
  if (tab.type === "contacts") {
    return (
      <TabItemContact
        tab={tab}
        tabIndex={tabIndex}
        handleCloseThis={handleClose}
        handleSelectThis={handleSelect}
        handleCloseOthers={handleCloseOthers}
        handleCloseAll={handleCloseAll}
        handlePinThis={handlePinThis}
        handleUnpinThis={handleUnpinThis}
      />
    );
  }

  if (tab.type === "empty") {
    return (
      <TabItemEmpty
        tab={tab}
        tabIndex={tabIndex}
        handleCloseThis={handleClose}
        handleSelectThis={handleSelect}
        handleCloseOthers={handleCloseOthers}
        handleCloseAll={handleCloseAll}
        handlePinThis={handlePinThis}
        handleUnpinThis={handleUnpinThis}
      />
    );
  }
  if (tab.type === "calendar") {
    return (
      <TabItemCalendar
        tab={tab}
        tabIndex={tabIndex}
        handleCloseThis={handleClose}
        handleSelectThis={handleSelect}
        handleCloseOthers={handleCloseOthers}
        handleCloseAll={handleCloseAll}
        handlePinThis={handlePinThis}
        handleUnpinThis={handleUnpinThis}
      />
    );
  }
  if (tab.type === "extension") {
    return (
      <TabItemPlugin
        tab={tab}
        tabIndex={tabIndex}
        handleCloseThis={handleClose}
        handleSelectThis={handleSelect}
        handleCloseOthers={handleCloseOthers}
        handleCloseAll={handleCloseAll}
        handlePinThis={handlePinThis}
        handleUnpinThis={handleUnpinThis}
      />
    );
  }
  if (tab.type === "changelog") {
    return (
      <TabItemChangelog
        tab={tab}
        tabIndex={tabIndex}
        handleCloseThis={handleClose}
        handleSelectThis={handleSelect}
        handleCloseOthers={handleCloseOthers}
        handleCloseAll={handleCloseAll}
        handlePinThis={handlePinThis}
        handleUnpinThis={handleUnpinThis}
      />
    );
  }
  if (tab.type === "settings") {
    return (
      <TabItemSettings
        tab={tab}
        tabIndex={tabIndex}
        handleCloseThis={handleClose}
        handleSelectThis={handleSelect}
        handleCloseOthers={handleCloseOthers}
        handleCloseAll={handleCloseAll}
        handlePinThis={handlePinThis}
        handleUnpinThis={handleUnpinThis}
      />
    );
  }
  if (tab.type === "ai") {
    return (
      <TabItemAI
        tab={tab}
        tabIndex={tabIndex}
        handleCloseThis={handleClose}
        handleSelectThis={handleSelect}
        handleCloseOthers={handleCloseOthers}
        handleCloseAll={handleCloseAll}
        handlePinThis={handlePinThis}
        handleUnpinThis={handleUnpinThis}
      />
    );
  }
  if (tab.type === "templates") {
    return (
      <TabItemTemplate
        tab={tab}
        tabIndex={tabIndex}
        handleCloseThis={handleClose}
        handleSelectThis={handleSelect}
        handleCloseOthers={handleCloseOthers}
        handleCloseAll={handleCloseAll}
        handlePinThis={handlePinThis}
        handleUnpinThis={handleUnpinThis}
      />
    );
  }
  if (tab.type === "chat_shortcuts") {
    return (
      <TabItemChatShortcut
        tab={tab}
        tabIndex={tabIndex}
        handleCloseThis={handleClose}
        handleSelectThis={handleSelect}
        handleCloseOthers={handleCloseOthers}
        handleCloseAll={handleCloseAll}
        handlePinThis={handlePinThis}
        handleUnpinThis={handleUnpinThis}
      />
    );
  }
  if (tab.type === "search") {
    return (
      <TabItemSearch
        tab={tab}
        tabIndex={tabIndex}
        handleCloseThis={handleClose}
        handleSelectThis={handleSelect}
        handleCloseOthers={handleCloseOthers}
        handleCloseAll={handleCloseAll}
        handlePinThis={handlePinThis}
        handleUnpinThis={handleUnpinThis}
      />
    );
  }
  if (tab.type === "chat_support") {
    return (
      <TabItemChat
        tab={tab}
        tabIndex={tabIndex}
        handleCloseThis={handleClose}
        handleSelectThis={handleSelect}
        handleCloseOthers={handleCloseOthers}
        handleCloseAll={handleCloseAll}
        handlePinThis={handlePinThis}
        handleUnpinThis={handleUnpinThis}
      />
    );
  }
  if (tab.type === "onboarding") {
    return (
      <TabItemOnboarding
        tab={tab}
        tabIndex={tabIndex}
        handleCloseThis={handleClose}
        handleSelectThis={handleSelect}
        handleCloseOthers={handleCloseOthers}
        handleCloseAll={handleCloseAll}
        handlePinThis={handlePinThis}
        handleUnpinThis={handleUnpinThis}
      />
    );
  }
  if (tab.type === "edit") {
    return (
      <TabItemEdit
        tab={tab}
        tabIndex={tabIndex}
        handleCloseThis={handleClose}
        handleSelectThis={handleSelect}
        handleCloseOthers={handleCloseOthers}
        handleCloseAll={handleCloseAll}
        handlePinThis={handlePinThis}
        handleUnpinThis={handleUnpinThis}
      />
    );
  }
  return null;
}

function ContentWrapper({ tab }: { tab: Tab }) {
  if (tab.type === "sessions") {
    return <TabContentNote tab={tab} />;
  }
  if (tab.type === "folders") {
    return <TabContentFolder tab={tab} />;
  }
  if (tab.type === "humans") {
    return <TabContentHuman tab={tab} />;
  }
  if (tab.type === "contacts") {
    return <TabContentContact tab={tab} />;
  }

  if (tab.type === "empty") {
    return <TabContentEmpty tab={tab} />;
  }
  if (tab.type === "calendar") {
    return <TabContentCalendar />;
  }
  if (tab.type === "extension") {
    return <TabContentPlugin tab={tab} />;
  }
  if (tab.type === "changelog") {
    return <TabContentChangelog tab={tab} />;
  }
  if (tab.type === "settings") {
    return <TabContentSettings tab={tab} />;
  }
  if (tab.type === "ai") {
    return <TabContentAI tab={tab} />;
  }
  if (tab.type === "templates") {
    return <TabContentTemplate tab={tab} />;
  }
  if (tab.type === "chat_shortcuts") {
    return <TabContentChatShortcut tab={tab} />;
  }
  if (tab.type === "search") {
    return <TabContentSearch tab={tab} />;
  }
  if (tab.type === "chat_support") {
    return <TabContentChat tab={tab} />;
  }
  if (tab.type === "onboarding") {
    return <TabContentOnboarding tab={tab} />;
  }
  if (tab.type === "edit") {
    return <TabContentEdit tab={tab} />;
  }
  return null;
}

function TabChatButton({
  isCaretNearBottom = false,
  showTimeline = false,
}: {
  isCaretNearBottom?: boolean;
  showTimeline?: boolean;
}) {
  const { chat } = useShell();
  const currentTab = useTabs((state) => state.currentTab);

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

  if (chat.mode === "RightPanelOpen" || chat.mode === "FullTab") {
    return null;
  }

  if (
    currentTab?.type === "ai" ||
    currentTab?.type === "settings" ||
    currentTab?.type === "chat_support" ||
    currentTab?.type === "onboarding"
  ) {
    return null;
  }

  return (
    <ChatFloatingButton
      isCaretNearBottom={isCaretNearBottom}
      showTimeline={showTimeline}
    />
  );
}

export function StandardTabWrapper({
  children,
  afterBorder,
  floatingButton,
  showTimeline = false,
}: {
  children: React.ReactNode;
  afterBorder?: React.ReactNode;
  floatingButton?: React.ReactNode;
  showTimeline?: boolean;
}) {
  return (
    <div className="flex h-full flex-col">
      <div className="relative flex flex-1 flex-col overflow-hidden rounded-xl border border-neutral-200 bg-white">
        {children}
        {floatingButton}
        <StandardTabChatButton showTimeline={showTimeline} />
      </div>
      {afterBorder && <div className="mt-1">{afterBorder}</div>}
    </div>
  );
}

function StandardTabChatButton({
  showTimeline = false,
}: {
  showTimeline?: boolean;
}) {
  const caretPosition = useCaretPosition();
  const isCaretNearBottom = caretPosition?.isCaretNearBottom ?? false;

  return (
    <TabChatButton
      isCaretNearBottom={isCaretNearBottom}
      showTimeline={showTimeline}
    />
  );
}

function useScrollState(
  ref: React.RefObject<HTMLDivElement | null>,
  tabCount: number,
) {
  const [scrollState, setScrollState] = useState({
    atStart: true,
    atEnd: true,
  });

  const updateScrollState = useCallback(() => {
    const container = ref.current;
    if (!container) return;

    const { scrollLeft, scrollWidth, clientWidth } = container;
    const hasOverflow = scrollWidth > clientWidth + 1;
    const newState = {
      atStart: !hasOverflow || scrollLeft <= 1,
      atEnd: !hasOverflow || scrollLeft + clientWidth >= scrollWidth - 1,
    };
    setScrollState((prev) => {
      if (prev.atStart === newState.atStart && prev.atEnd === newState.atEnd) {
        return prev;
      }
      return newState;
    });
  }, [ref]);

  useResizeObserver({
    ref: ref as React.RefObject<HTMLDivElement>,
    onResize: updateScrollState,
  });

  useEffect(() => {
    const container = ref.current;
    if (!container) return;

    updateScrollState();
    requestAnimationFrame(updateScrollState);
    const timerId = setTimeout(updateScrollState, 200);
    container.addEventListener("scroll", updateScrollState);

    return () => {
      container.removeEventListener("scroll", updateScrollState);
      clearTimeout(timerId);
    };
  }, [updateScrollState, tabCount]);

  return scrollState;
}

function useScrollActiveTabIntoView(tabs: Tab[]) {
  const tabRefsMap = useRef<Map<string, HTMLDivElement>>(new Map());
  const activeTab = tabs.find((tab) => tab.active);
  const activeTabKey = activeTab ? uniqueIdfromTab(activeTab) : null;

  useEffect(() => {
    if (activeTabKey) {
      const tabElement = tabRefsMap.current.get(activeTabKey);
      if (tabElement) {
        tabElement.scrollIntoView({
          behavior: "smooth",
          inline: "nearest",
          block: "nearest",
        });
      }
    }
  }, [activeTabKey]);

  const setTabRef = useCallback((tab: Tab, el: HTMLDivElement | null) => {
    if (el) {
      tabRefsMap.current.set(uniqueIdfromTab(tab), el);
    } else {
      tabRefsMap.current.delete(uniqueIdfromTab(tab));
    }
  }, []);

  return setTabRef;
}

function useTabsShortcuts() {
  const {
    tabs,
    currentTab,
    close,
    select,
    selectNext,
    selectPrev,
    restoreLastClosedTab,
    openNew,
    unpin,
    setPendingCloseConfirmationTab,
  } = useTabs(
    useShallow((state) => ({
      tabs: state.tabs,
      currentTab: state.currentTab,
      close: state.close,
      select: state.select,
      selectNext: state.selectNext,
      selectPrev: state.selectPrev,
      restoreLastClosedTab: state.restoreLastClosedTab,
      openNew: state.openNew,
      unpin: state.unpin,
      setPendingCloseConfirmationTab: state.setPendingCloseConfirmationTab,
    })),
  );
  const liveSessionId = useListener((state) => state.live.sessionId);
  const liveStatus = useListener((state) => state.live.status);
  const isListening = liveStatus === "active" || liveStatus === "finalizing";
  const { chat } = useShell();

  const newNote = useNewNote({ behavior: "new" });
  const newNoteCurrent = useNewNote({ behavior: "current" });
  const newNoteAndListen = useNewNoteAndListen();
  const newEmptyTab = useNewEmptyTab();

  useHotkeys(
    "mod+n",
    () => {
      if (currentTab?.type === "empty") {
        newNoteCurrent();
      } else {
        newNote();
      }
    },
    {
      preventDefault: true,
      enableOnFormTags: true,
      enableOnContentEditable: true,
    },
    [currentTab, newNote, newNoteCurrent],
  );

  useHotkeys(
    "mod+t",
    () => newEmptyTab(),
    {
      preventDefault: true,
      enableOnFormTags: true,
      enableOnContentEditable: true,
    },
    [newEmptyTab],
  );

  useHotkeys(
    "mod+w",
    async () => {
      if (currentTab) {
        const isCurrentTabListening =
          isListening &&
          currentTab.type === "sessions" &&
          currentTab.id === liveSessionId;
        if (isCurrentTabListening) {
          setPendingCloseConfirmationTab(currentTab);
        } else if (currentTab.pinned) {
          unpin(currentTab);
        } else {
          if (currentTab.type === "chat_support") {
            chat.sendEvent({ type: "CLOSE" });
          }
          close(currentTab);
        }
      }
    },
    {
      preventDefault: true,
      enableOnFormTags: true,
      enableOnContentEditable: true,
    },
    [
      currentTab,
      close,
      unpin,
      isListening,
      liveSessionId,
      setPendingCloseConfirmationTab,
      chat,
    ],
  );

  useHotkeys(
    "mod+1, mod+2, mod+3, mod+4, mod+5, mod+6, mod+7, mod+8, mod+9",
    (event) => {
      const key = event.key;
      const targetIndex =
        key === "9" ? tabs.length - 1 : Number.parseInt(key, 10) - 1;
      const target = tabs[targetIndex];
      if (target) {
        select(target);
      }
    },
    {
      preventDefault: true,
      enableOnFormTags: true,
      enableOnContentEditable: true,
    },
    [tabs, select],
  );

  useHotkeys(
    "mod+alt+left",
    () => selectPrev(),
    {
      preventDefault: true,
      enableOnFormTags: true,
      enableOnContentEditable: true,
    },
    [selectPrev],
  );

  useHotkeys(
    "mod+alt+right",
    () => selectNext(),
    {
      preventDefault: true,
      enableOnFormTags: true,
      enableOnContentEditable: true,
    },
    [selectNext],
  );

  useHotkeys(
    "mod+shift+t",
    () => restoreLastClosedTab(),
    {
      preventDefault: true,
      enableOnFormTags: true,
      enableOnContentEditable: true,
    },
    [restoreLastClosedTab],
  );

  useHotkeys(
    "mod+shift+c",
    () => openNew({ type: "calendar" }),
    {
      preventDefault: true,
      enableOnFormTags: true,
      enableOnContentEditable: true,
    },
    [openNew],
  );

  useHotkeys(
    "mod+shift+o",
    () =>
      openNew({
        type: "contacts",
        state: { selected: null },
      }),
    {
      preventDefault: true,
      enableOnFormTags: true,
      enableOnContentEditable: true,
    },
    [openNew],
  );

  useHotkeys(
    "mod+shift+comma",
    () => openNew({ type: "ai" }),
    {
      preventDefault: true,
      enableOnFormTags: true,
      enableOnContentEditable: true,
    },
    [openNew],
  );

  useHotkeys(
    "mod+shift+l",
    () => openNew({ type: "folders", id: null }),
    {
      preventDefault: true,
      enableOnFormTags: true,
      enableOnContentEditable: true,
    },
    [openNew],
  );

  useHotkeys(
    "mod+shift+f",
    () => openNew({ type: "search" }),
    {
      preventDefault: true,
      enableOnFormTags: true,
      enableOnContentEditable: true,
    },
    [openNew],
  );

  useHotkeys(
    "mod+shift+n",
    () => newNoteAndListen(),
    {
      preventDefault: true,
      enableOnFormTags: true,
      enableOnContentEditable: true,
    },
    [newNoteAndListen],
  );

  return {};
}

function useNewEmptyTab() {
  const openNew = useTabs((state) => state.openNew);

  const handler = useCallback(() => {
    openNew({ type: "empty" });
  }, [openNew]);

  return handler;
}
