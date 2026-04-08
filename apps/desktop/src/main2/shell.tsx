import { platform } from "@tauri-apps/plugin-os";
import {
  ArrowLeftIcon,
  ArrowRightIcon,
  HouseIcon,
  SearchIcon,
} from "lucide-react";
import { Reorder } from "motion/react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
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
import { useNewNoteAndListen } from "~/shared/useNewNote";
import { LeftSidebar } from "~/sidebar";
import { uniqueIdfromTab, useTabs } from "~/store/zustand/tabs";
import { useListener } from "~/stt/contexts";
import { commands } from "~/types/tauri.gen";

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

  const hasCustomSidebar =
    currentTab?.type === "calendar" ||
    currentTab?.type === "settings" ||
    currentTab?.type === "contacts" ||
    currentTab?.type === "templates";
  const showSidebar = hasCustomSidebar || leftsidebar.showDevtool;

  const wasSidebarVisibleRef = useRef(false);
  useEffect(() => {
    if (showSidebar && !wasSidebarVisibleRef.current) {
      leftsidebar.setExpanded(true);
      leftsidebar.setLocked(true);
      commands.resizeWindowForSidebar().catch(console.error);
    } else if (!showSidebar && wasSidebarVisibleRef.current) {
      leftsidebar.setLocked(false);
      leftsidebar.setExpanded(false);
    }
    wasSidebarVisibleRef.current = showSidebar;
  }, [showSidebar, leftsidebar]);

  const stop = useListener((state) => state.stop);
  const isRecording = useListener((state) => {
    return state.live.status === "active" || state.live.status === "finalizing";
  });
  const newNoteAndListen = useNewNoteAndListen();
  const isHomeActive = currentTab === null;
  const isChatOpen =
    chat.mode === "FloatingOpen" || chat.mode === "RightPanelOpen";

  useMain2TabsShortcuts();

  const [openNoteDialogOpen, setOpenNoteDialogOpen] = useState(false);
  useHotkeys(
    "mod+k",
    () => setOpenNoteDialogOpen(true),
    { preventDefault: true, enableOnFormTags: true },
    [setOpenNoteDialogOpen],
  );

  const handleHome = useCallback(() => {
    if (isHomeActive) {
      window.dispatchEvent(new CustomEvent("scroll-to-today"));
    } else {
      clearSelection();
    }
  }, [isHomeActive, clearSelection]);

  const handleRecord = useCallback(() => {
    if (isRecording) {
      stop();
      return;
    }

    newNoteAndListen();
  }, [isRecording, newNoteAndListen, stop]);

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
          className="flex h-9 w-full min-w-0 shrink-0 items-center gap-1 px-3"
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
                className="flex h-full w-max gap-1"
              >
                {tabs.map((tab) => (
                  <Reorder.Item
                    key={uniqueIdfromTab(tab)}
                    value={tab}
                    as="div"
                    ref={(el) => setTabRef(tab, el)}
                    style={{ position: "relative" }}
                    className="z-10 h-full"
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

          {isHomeActive && (
            <button
              type="button"
              onClick={handleRecord}
              className={cn([
                "ml-1 shrink-0",
                "group flex h-7 items-center gap-1.5 text-xs font-medium",
                isRecording
                  ? "rounded-md bg-red-50 px-2.5 text-red-700 hover:bg-red-100"
                  : "rounded-full bg-neutral-800 px-3 text-white hover:bg-neutral-700",
              ])}
              title={isRecording ? "Stop recording" : "Start recording"}
            >
              <span
                className={cn([
                  "relative h-2.5 w-2.5 overflow-hidden border transition-all",
                  isRecording
                    ? [
                        "rounded-[2px]",
                        "border-red-700/60 bg-linear-to-b from-red-500 to-red-600",
                        "shadow-[inset_0_1px_0_rgba(255,255,255,0.18)]",
                      ]
                    : [
                        "rounded-full",
                        "border-red-500/60 bg-linear-to-b from-red-400 to-red-500",
                        "shadow-[inset_0_1px_0_rgba(255,255,255,0.22),0_1px_2px_rgba(127,29,29,0.14)]",
                      ],
                ])}
              >
                <span className="pointer-events-none absolute top-[1px] left-1/2 h-[22%] w-[68%] -translate-x-1/2 rounded-full bg-white/18" />
              </span>
              <span>{isRecording ? "Stop recording" : "Start recording"}</span>
            </button>
          )}

          <div className="ml-auto flex shrink-0 items-center gap-1">
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
