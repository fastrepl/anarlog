import { cn } from "@hypr/utils";

import {
  SettingsAccount,
  SettingsApp,
  SettingsNotifications,
  SettingsPermissions,
} from "./general";
import { SettingsTodo } from "./todo";

import { LLM } from "~/settings/ai/llm";
import { STT } from "~/settings/ai/stt";
import { SettingsDevelopers } from "~/settings/developers";
import { SettingsPersonalization } from "~/settings/personalization";
import { StandardContentWrapper } from "~/shared/main";
import { type Tab } from "~/store/zustand/tabs";

export function TabContentSettings({
  tab,
}: {
  tab: Extract<Tab, { type: "settings" }>;
}) {
  return (
    <StandardContentWrapper>
      <SettingsView tab={tab} />
    </StandardContentWrapper>
  );
}

function SettingsView({ tab }: { tab: Extract<Tab, { type: "settings" }> }) {
  const requestedTab = tab.state.tab as string | undefined;
  const activeTab = requestedTab === "data" ? "app" : (tab.state.tab ?? "app");

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
      case "developers":
        return <SettingsDevelopers />;
      case "personalization":
        return <SettingsPersonalization />;
      case "transcription":
        return <STT />;
      case "intelligence":
        return <LLM />;
      case "todo":
        return <SettingsTodo />;
      default:
        return <SettingsApp />;
    }
  };

  return (
    <div
      data-settings-content
      className="bg-card dark:bg-accent flex w-full flex-1 flex-col overflow-hidden"
    >
      <div className="relative w-full flex-1 overflow-hidden">
        <div
          className={cn([
            "scroll-fade-y scrollbar-hide h-full w-full flex-1 overflow-y-auto p-6",
          ])}
        >
          {renderContent()}
        </div>
      </div>
    </div>
  );
}
