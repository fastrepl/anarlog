import { cn } from "@hypr/utils";

import { ClassicMainSidebar } from "./shell-sidebar";
import { ClassicMainTabContent } from "./tab-content";
import { TopMeetingTimeline } from "./top-meeting-timeline";
import { useClassicMainTabsShortcuts } from "./useTabsShortcuts";

import { useShell } from "~/contexts/shell";
import { ToastArea } from "~/sidebar/toast";
import { hasCustomSidebarTab } from "~/sidebar/use-custom-sidebar";
import { type Tab, uniqueIdfromTab, useTabs } from "~/store/zustand/tabs";

export function ClassicMainBody() {
  const { leftsidebar } = useShell();
  const currentTab = useTabs((state) => state.currentTab);
  useClassicMainTabsShortcuts();

  const isOnboarding = currentTab?.type === "onboarding";
  const hasCustomSidebar = hasCustomSidebarTab(currentTab);
  const showTopTimeline =
    leftsidebar.expanded &&
    !leftsidebar.showDevtool &&
    !hasCustomSidebar &&
    !isOnboarding;
  const showFloatingToast =
    !leftsidebar.showDevtool && !hasCustomSidebar && !isOnboarding;

  return (
    <div className="relative flex h-full min-w-0 flex-1 flex-col">
      <div
        data-tauri-drag-region
        className={cn(["relative shrink-0", showTopTimeline ? "h-12" : "h-10"])}
      >
        <div
          data-tauri-drag-region
          className="flex h-full min-w-0 items-start pt-1 pl-[76px]"
        >
          {showTopTimeline ? (
            <div className="min-w-0 flex-1">
              <TopMeetingTimeline currentTab={currentTab} />
            </div>
          ) : null}
        </div>
      </div>
      <div className="flex min-h-0 min-w-0 flex-1 gap-1">
        <ClassicMainSidebar />
        <div className="min-h-0 min-w-0 flex-1 overflow-auto">
          {currentTab ? (
            <ClassicMainTabContent
              key={uniqueIdfromTab(currentTab)}
              tab={currentTab as Tab}
            />
          ) : null}
        </div>
      </div>
      {showFloatingToast ? (
        <div className="absolute bottom-1 left-1 z-30 w-[200px]">
          <ToastArea />
        </div>
      ) : null}
    </div>
  );
}
