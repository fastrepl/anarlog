import { useShallow } from "zustand/shallow";

import { ClassicMainSidebar } from "./shell-sidebar";
import { ClassicMainTabChrome } from "./tab-chrome";
import { ClassicMainTabContent } from "./tab-content";
import { TopMeetingTimeline } from "./top-meeting-timeline";

import { useShell } from "~/contexts/shell";
import { ToastArea } from "~/sidebar/toast";
import { hasCustomSidebarTab } from "~/sidebar/use-custom-sidebar";
import { type Tab, uniqueIdfromTab, useTabs } from "~/store/zustand/tabs";

export function ClassicMainBody() {
  const { leftsidebar } = useShell();
  const { tabs, currentTab } = useTabs(
    useShallow((state) => ({
      tabs: state.tabs,
      currentTab: state.currentTab,
    })),
  );

  if (!currentTab) {
    return null;
  }

  const isOnboarding = currentTab.type === "onboarding";
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
      <ClassicMainTabChrome tabs={tabs} />
      {showTopTimeline ? <TopMeetingTimeline currentTab={currentTab} /> : null}
      <div className="flex min-h-0 min-w-0 flex-1 gap-1">
        <ClassicMainSidebar />
        <div className="min-h-0 min-w-0 flex-1 overflow-auto">
          <ClassicMainTabContent
            key={uniqueIdfromTab(currentTab)}
            tab={currentTab as Tab}
          />
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
