import { SettingsIcon } from "lucide-react";
import { useRef } from "react";

import {
  ScrollFadeOverlay,
  useScrollFade,
} from "@hypr/ui/components/ui/scroll-fade";

import { SettingsCalendar } from "./calendar";
import { SettingsApp, SettingsNotifications, SettingsSystem } from "./general";
import { SettingsLab } from "./lab";
import { TemplatesContent } from "./templates-content";

import { LLM } from "~/settings/ai/llm";
import { STT } from "~/settings/ai/stt";
import { SettingsMemory } from "~/settings/memory";
import { StandardTabWrapper } from "~/shared/main";
import { type TabItem, TabItemBase } from "~/shared/tabs";
import type { Tab } from "~/store/zustand/tabs";

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
  const activeTab =
    tab.state.tab === "account" ? "app" : (tab.state.tab ?? "app");
  const ref = useRef<HTMLDivElement>(null);
  const { atStart, atEnd } = useScrollFade(ref, "vertical", [activeTab]);

  const renderContent = () => {
    switch (activeTab) {
      case "app":
        return <SettingsApp />;
      case "notifications":
        return <SettingsNotifications />;
      case "calendar":
        return <SettingsCalendar />;
      case "system":
        return <SettingsSystem />;
      case "lab":
        return <SettingsLab />;
      case "transcription":
        return <STT />;
      case "intelligence":
        return <LLM />;
      case "templates":
        return <TemplatesContent />;
      case "memory":
        return <SettingsMemory />;
    }
  };

  return (
    <div className="flex w-full flex-1 flex-col overflow-hidden">
      <div className="relative w-full flex-1 overflow-hidden">
        <div
          ref={ref}
          className="scrollbar-hide h-full w-full flex-1 overflow-y-auto px-6 py-6"
        >
          {renderContent()}
        </div>
        {!atStart && <ScrollFadeOverlay position="top" />}
        {!atEnd && <ScrollFadeOverlay position="bottom" />}
      </div>
    </div>
  );
}
