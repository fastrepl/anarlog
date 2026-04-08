import { useShallow } from "zustand/shallow";

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
    <div className="relative flex h-full flex-1 flex-col gap-1">
      <ClassicMainTabChrome tabs={tabs} />
      <div className="flex-1 overflow-auto">
        <ClassicMainTabContent
          key={uniqueIdfromTab(currentTab)}
          tab={currentTab as Tab}
        />
      </div>
    </div>
  );
}
