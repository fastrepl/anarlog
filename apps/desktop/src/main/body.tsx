import { useShallow } from "zustand/shallow";

import { ClassicMainSidebar } from "./shell-sidebar";
import { ClassicMainTabChrome } from "./tab-chrome";
import { ClassicMainTabContent } from "./tab-content";

import { type Tab, uniqueIdfromTab, useTabs } from "~/store/zustand/tabs";

export function ClassicMainBody() {
  const { tabs, currentTab } = useTabs(
    useShallow((state) => ({
      tabs: state.tabs,
      currentTab: state.currentTab,
    })),
  );

  if (!currentTab) {
    return null;
  }

  return (
    <div className="relative flex h-full min-w-0 flex-1 flex-col">
      <ClassicMainTabChrome tabs={tabs} />
      <div className="flex min-h-0 min-w-0 flex-1 gap-1">
        <ClassicMainSidebar />
        <div className="min-h-0 min-w-0 flex-1 overflow-auto">
          <ClassicMainTabContent
            key={uniqueIdfromTab(currentTab)}
            tab={currentTab as Tab}
          />
        </div>
      </div>
    </div>
  );
}
