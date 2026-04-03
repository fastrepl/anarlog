import { useShallow } from "zustand/shallow";

import { MainTabChrome } from "./tab-chrome";
import { MainTabContent } from "./tab-content";

import { type Tab, uniqueIdfromTab, useTabs } from "~/store/zustand/tabs";

export function Body() {
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
    <div className="relative flex h-full flex-1 flex-col gap-1">
      <MainTabChrome tabs={tabs} />
      <div className="flex-1 overflow-auto">
        <MainTabContent
          key={uniqueIdfromTab(currentTab)}
          tab={currentTab as Tab}
        />
      </div>
    </div>
  );
}
