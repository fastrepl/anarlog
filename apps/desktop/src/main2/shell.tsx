import { useRouteContext } from "@tanstack/react-router";
import { platform } from "@tauri-apps/plugin-os";
import {
  ArrowLeftIcon,
  ArrowRightIcon,
  HouseIcon,
  SearchIcon,
} from "lucide-react";
import { Reorder } from "motion/react";
import { useCallback, useMemo, useState } from "react";
import { useHotkeys } from "react-hotkeys-hook";
import { useShallow } from "zustand/shallow";

import { Button } from "@hypr/ui/components/ui/button";
import { cn } from "@hypr/utils";

import { useShell } from "~/contexts/shell";
import { Main2Home } from "~/main2/home";
import { ProfileMenu } from "~/main2/profile-menu";
import { UpdateBanner } from "~/main2/update";
import { useMain2TabsShortcuts } from "~/main2/useTabsShortcuts";
import {
  MainShellBodyFrame,
  MainShellScaffold,
  MainTabContent,
  MainTabItem,
  useScrollActiveTabIntoView,
} from "~/shared/main";
import { OpenNoteDialog } from "~/shared/open-note-dialog";
import { TrafficLights } from "~/shared/ui/traffic-lights";
import { id } from "~/shared/utils";
import { LeftSidebar } from "~/sidebar";
import {
  hasCustomSidebarTab,
  useCustomSidebarEffect,
} from "~/sidebar/use-custom-sidebar";
import { uniqueIdfromTab, useTabs } from "~/store/zustand/tabs";
import { useListener } from "~/stt/contexts";

export function Main2Shell() {
  const currentPlatform = platform();
  const isLinux = currentPlatform === "linux";
  const {
    tabs,
    currentTab,
    select,
    close,
    reorder,
    goBack,
    goNext,
    canGoBack,
    canGoNext,
    closeOthers,
    closeAll,
    clearSelection,
    pin,
    unpin,
    pendingCloseConfirmationTab,
    setPendingCloseConfirmationTab,
  } = useTabs(
    useShallow((state) => ({
      tabs: state.tabs,
      currentTab: state.currentTab,
      select: state.select,
      close: state.close,
      reorder: state.reorder,
      goBack: state.goBack,
      goNext: state.goNext,
      canGoBack: state.canGoBack,
      canGoNext: state.canGoNext,
      closeOthers: state.closeOthers,
      closeAll: state.closeAll,
      clearSelection: state.clearSelection,
      pin: state.pin,
      unpin: state.unpin,
      pendingCloseConfirmationTab: state.pendingCloseConfirmationTab,
      setPendingCloseConfirmationTab: state.setPendingCloseConfirmationTab,
    })),
  );
  const setTabRef = useScrollActiveTabIntoView(tabs);
  const { chat, leftsidebar } = useShell();
  const { persistedStore, internalStore } = useRouteContext({
    from: "__root__",
  });
  const openNew = useTabs((state) => state.openNew);

  const hasCustomSidebar = hasCustomSidebarTab(currentTab);
  const showSidebar = hasCustomSidebar || leftsidebar.showDevtool;

  useCustomSidebarEffect(showSidebar, leftsidebar, {
    restoreExpandedOnExit: false,
  });

  const isHomeActive = currentTab === null;
  const isChatOpen =
    chat.mode === "FloatingOpen" || chat.mode === "RightPanelOpen";
  const liveSessionId = useListener((state) => state.live.sessionId);
  const liveStatus = useListener((state) => state.live.status);
  const showAdHocButton = !(
    currentTab?.type === "sessions" &&
    currentTab.id === liveSessionId &&
    liveStatus === "active"
  );

  useMain2TabsShortcuts();

  const [openNoteDialogOpen, setOpenNoteDialogOpen] = useState(false);
  useHotkeys(
    "mod+k",
    () => setOpenNoteDialogOpen(true),
    { preventDefault: true, enableOnFormTags: true },
    [setOpenNoteDialogOpen],
  );

  const handleAdHoc = useCallback(() => {
    const now = new Date();
    const h = now.getHours();
    const m = now.getMinutes();
    const period = h < 12 ? "am" : "pm";
    const hour = h % 12 || 12;
    const time =
      m === 0
        ? `${hour}${period}`
        : `${hour}:${String(m).padStart(2, "0")}${period}`;
    const title = `Ad-hoc conversation at ${time}`;

    const userId = internalStore?.getValue("user_id");
    const sessionId = id();

    persistedStore?.setRow("sessions", sessionId, {
      user_id: userId,
      created_at: now.toISOString(),
      title,
    });

    if (typeof userId === "string") {
      persistedStore?.setRow("mapping_session_participant", id(), {
        user_id: userId,
        session_id: sessionId,
        human_id: userId,
        source: "manual",
      });
    }

    openNew({
      type: "sessions",
      id: sessionId,
      state: { view: null, autoStart: true },
    });
  }, [persistedStore, internalStore, openNew]);

  const handleHome = useCallback(() => {
    if (isHomeActive) {
      window.dispatchEvent(new CustomEvent("scroll-to-today"));
    } else {
      clearSelection();
    }
  }, [isHomeActive, clearSelection]);

  const handleChat = useCallback(() => {
    chat.sendEvent(isChatOpen ? { type: "TOGGLE" } : { type: "OPEN" });
  }, [chat, isChatOpen]);

  const shortcutIndexes = useMemo(() => {
    return new Map(
      tabs.map((tab, index) => [
        uniqueIdfromTab(tab),
        index < 8 ? index + 1 : index === tabs.length - 1 ? 9 : undefined,
      ]),
    );
  }, [tabs]);

  return (
    <MainShellScaffold>
      <OpenNoteDialog
        open={openNoteDialogOpen}
        onOpenChange={setOpenNoteDialogOpen}
      />
      {showSidebar && <LeftSidebar />}
      <div className="flex h-full min-w-0 flex-1 flex-col">
        <div
          data-tauri-drag-region
          className="flex h-10 w-full min-w-0 shrink-0 items-center gap-1 pr-1 pl-3"
        >
          <div
            className={cn([
              "flex shrink-0 items-center gap-1",
              isLinux ? "mr-1" : !showSidebar && "pl-16",
            ])}
          >
            {isLinux && <TrafficLights className="mr-1" />}
            <Button
              onClick={handleHome}
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
            {!isHomeActive && (
              <>
                <Button
                  onClick={goBack}
                  disabled={!canGoBack}
                  variant="ghost"
                  size="icon"
                  className="text-neutral-600"
                >
                  <ArrowLeftIcon size={16} />
                </Button>
                <Button
                  onClick={goNext}
                  disabled={!canGoNext}
                  variant="ghost"
                  size="icon"
                  className="text-neutral-600"
                >
                  <ArrowRightIcon size={16} />
                </Button>
              </>
            )}
          </div>

          <div className="relative h-full min-w-0 flex-shrink">
            <div
              data-tauri-drag-region
              className={cn([
                "[-ms-overflow-style:none] [scrollbar-width:none] [&::-webkit-scrollbar]:hidden",
                "h-full w-full overflow-x-auto overflow-y-hidden",
              ])}
            >
              <Reorder.Group
                as="div"
                axis="x"
                values={tabs}
                onReorder={reorder}
                className="flex h-full w-max items-center gap-1"
              >
                {tabs.map((tab) => (
                  <Reorder.Item
                    key={uniqueIdfromTab(tab)}
                    value={tab}
                    as="div"
                    ref={(el) => setTabRef(tab, el)}
                    style={{ position: "relative" }}
                    className="z-10 flex h-full items-center"
                    transition={{ layout: { duration: 0.15 } }}
                  >
                    <MainTabItem
                      tab={tab}
                      handleClose={close}
                      handleSelect={select}
                      handleCloseOthersCallback={closeOthers}
                      handleCloseAll={closeAll}
                      handlePin={pin}
                      handleUnpin={unpin}
                      tabIndex={shortcutIndexes.get(uniqueIdfromTab(tab))}
                      pendingCloseConfirmationTab={pendingCloseConfirmationTab}
                      setPendingCloseConfirmationTab={
                        setPendingCloseConfirmationTab
                      }
                    />
                  </Reorder.Item>
                ))}
              </Reorder.Group>
            </div>
          </div>

          <div className="ml-auto flex shrink-0 items-center gap-2">
            {showAdHocButton && (
              <Button
                type="button"
                onClick={handleAdHoc}
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
            )}
            <Button
              onClick={() => setOpenNoteDialogOpen(true)}
              variant="ghost"
              size="icon"
              className="text-neutral-600"
              title="Search (⌘K)"
            >
              <SearchIcon size={16} />
            </Button>
            <Button
              onClick={handleChat}
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
            <ProfileMenu />
          </div>
        </div>

        <UpdateBanner />
        <MainShellBodyFrame autoSaveId="main2-chat">
          <div className="h-full min-h-0 overflow-auto">
            {currentTab ? (
              <MainTabContent
                key={uniqueIdfromTab(currentTab)}
                tab={currentTab}
              />
            ) : (
              <Main2Home />
            )}
          </div>
        </MainShellBodyFrame>
      </div>
    </MainShellScaffold>
  );
}
