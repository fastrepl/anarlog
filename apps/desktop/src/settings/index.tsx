import {
  BellIcon,
  FlaskConical,
  MonitorIcon,
  SettingsIcon,
  SmartphoneIcon,
  UserIcon,
} from "lucide-react";
import { useCallback, useRef } from "react";

import { Button } from "@hypr/ui/components/ui/button";
import {
  ScrollFadeOverlay,
  useScrollFade,
} from "@hypr/ui/components/ui/scroll-fade";
import { cn } from "@hypr/utils";

import {
  SettingsAccount,
  SettingsApp,
  SettingsNotifications,
  SettingsSystem,
} from "./general";
import { SettingsLab } from "./lab";

import { StandardTabWrapper } from "~/shared/main";
import { type TabItem, TabItemBase } from "~/shared/tabs";
import { type SettingsTab, type Tab, useTabs } from "~/store/zustand/tabs";

export const TabItemSettings: TabItem<Extract<Tab, { type: "settings" }>> = ({
  tab,
  tabIndex,
  handleCloseThis,
  handleSelectThis,
  handleCloseOthers,
  handleCloseAll,
  handlePinThis,
  handleUnpinThis,
}) => {
  return (
    <TabItemBase
      icon={<SettingsIcon className="h-4 w-4" />}
      title={"Settings"}
      selected={tab.active}
      pinned={tab.pinned}
      tabIndex={tabIndex}
      handleCloseThis={() => handleCloseThis(tab)}
      handleSelectThis={() => handleSelectThis(tab)}
      handleCloseOthers={handleCloseOthers}
      handleCloseAll={handleCloseAll}
      handlePinThis={() => handlePinThis(tab)}
      handleUnpinThis={() => handleUnpinThis(tab)}
    />
  );
};

export function TabContentSettings({
  tab,
}: {
  tab: Extract<Tab, { type: "settings" }>;
}) {
  return (
    <StandardTabWrapper>
      <SettingsView tab={tab} />
    </StandardTabWrapper>
  );
}

const SECTIONS: {
  id: SettingsTab;
  label: string;
  icon: typeof UserIcon;
}[] = [
  { id: "account", label: "Account", icon: UserIcon },
  { id: "app", label: "App", icon: SmartphoneIcon },
  { id: "notifications", label: "Notifications", icon: BellIcon },
  { id: "system", label: "System", icon: MonitorIcon },
  { id: "lab", label: "Lab", icon: FlaskConical },
];

function SettingsView({ tab }: { tab: Extract<Tab, { type: "settings" }> }) {
  const updateSettingsTabState = useTabs(
    (state) => state.updateSettingsTabState,
  );
  const activeTab = tab.state.tab ?? "account";
  const ref = useRef<HTMLDivElement>(null);
  const { atStart, atEnd } = useScrollFade(ref, "vertical", [activeTab]);

  const setActiveTab = useCallback(
    (newTab: SettingsTab) => {
      updateSettingsTabState(tab, { tab: newTab });
    },
    [updateSettingsTabState, tab],
  );

  const renderContent = () => {
    switch (activeTab) {
      case "account":
        return <SettingsAccount />;
      case "app":
        return <SettingsApp />;
      case "notifications":
        return <SettingsNotifications />;
      case "system":
        return <SettingsSystem />;
      case "lab":
        return <SettingsLab />;
    }
  };

  return (
    <div className="flex w-full flex-1 flex-col overflow-hidden">
      <div className="flex flex-wrap gap-1 px-6 pt-6 pb-2">
        {SECTIONS.map(({ id, label, icon: Icon }) => (
          <Button
            key={id}
            variant="ghost"
            size="sm"
            onClick={() => setActiveTab(id)}
            className={cn([
              "h-7 shrink-0 gap-1.5 border border-transparent px-1",
              id === "lab" &&
                "ml-2 text-amber-600 hover:bg-amber-50 hover:text-amber-700",
              activeTab === id &&
                (id === "lab"
                  ? "border-amber-300 bg-amber-100 text-amber-800"
                  : "border-neutral-200 bg-neutral-100"),
            ])}
          >
            <Icon size={14} />
            <span className="text-xs">{label}</span>
          </Button>
        ))}
      </div>
      <div className="relative w-full flex-1 overflow-hidden">
        <div
          ref={ref}
          className="scrollbar-hide h-full w-full flex-1 overflow-y-auto px-6 pb-6"
        >
          {renderContent()}
        </div>
        {!atStart && <ScrollFadeOverlay position="top" />}
        {!atEnd && <ScrollFadeOverlay position="bottom" />}
      </div>
    </div>
  );
}
