import { SettingsIcon } from "lucide-react";

import { cn } from "@hypr/utils";

import {
  SettingsAccount,
  SettingsApp,
  SettingsNotifications,
  SettingsPermissions,
} from "./general";

import { LLM } from "~/settings/ai/llm";
import { STT } from "~/settings/ai/stt";
import { StandardTabWrapper } from "~/shared/main";
import { type TabItem, TabItemBase } from "~/shared/tabs";
import { type Tab } from "~/store/zustand/tabs";

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

function SettingsView({ tab }: { tab: Extract<Tab, { type: "settings" }> }) {
  const activeTab = tab.state.tab ?? "app";

  const renderContent = () => {
    switch (activeTab) {
      case "account":
        return <SettingsAccount />;
      case "app":
        return <SettingsApp />;
      case "notifications":
        return <SettingsNotifications />;
      case "permissions":
        return <SettingsPermissions />;
      case "transcription":
        return <STT />;
      case "intelligence":
        return <LLM />;
    }
  };

  return (
    <div className="flex w-full flex-1 flex-col overflow-hidden">
      <div className="relative w-full flex-1 overflow-hidden">
        <div
          className={cn([
            "scroll-fade-y scrollbar-hide h-full w-full flex-1 overflow-y-auto px-6 pt-3 pb-6",
          ])}
        >
          {renderContent()}
        </div>
      </div>
    </div>
  );
}
