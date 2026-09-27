import { cn } from "@anlg/utils";

import {
  SettingsAccount,
  SettingsApp,
  SettingsMeetings,
  SettingsNotifications,
  SettingsPermissions,
} from "./general";
import { SettingsTodo } from "./todo";

import { LLM } from "~/settings/ai/llm";
import { STT } from "~/settings/ai/stt";
import { SettingsAppearance } from "~/settings/appearance";
import { SettingsCrm } from "~/settings/crm";
import { SettingsDevelopers } from "~/settings/developers";
import { SettingsDictation } from "~/settings/dictation";
import { SettingsDictionary } from "~/settings/dictionary";
import { SettingsBilling } from "~/settings/general/billing";
import { SettingsHydrationBoundary } from "~/settings/hydration-boundary";
import { SettingsImports } from "~/settings/imports";
import { SettingsPrivacy } from "~/settings/privacy";
import { SettingsInsights } from "~/settings/stats";
import { SettingsSync } from "~/settings/sync";
import { SettingsTeam } from "~/settings/team";
import { StandardContentWrapper } from "~/shared/main";
import { type Tab } from "~/store/zustand/tabs";

export function TabContentSettings({
  tab,
}: {
  tab: Extract<Tab, { type: "settings" }>;
}) {
  return (
    <StandardContentWrapper>
      <SettingsHydrationBoundary>
        <SettingsView tab={tab} />
      </SettingsHydrationBoundary>
    </StandardContentWrapper>
  );
}

function SettingsView({ tab }: { tab: Extract<Tab, { type: "settings" }> }) {
  const requestedTab = tab.state.tab as string | undefined;
  const activeTab =
    requestedTab === "data"
      ? "imports"
      : requestedTab === "personalization"
        ? "dictionary"
        : requestedTab === "audio"
          ? "meetings"
          : (tab.state.tab ?? "app");

  const renderContent = () => {
    switch (activeTab) {
      case "account":
        return <SettingsAccount />;
      case "billing":
        return <SettingsBilling />;
      case "stats":
      case "insights":
        return <SettingsInsights />;
      case "app":
        return <SettingsApp />;
      case "meetings":
        return <SettingsMeetings />;
      case "appearance":
        return <SettingsAppearance />;
      case "notifications":
        return <SettingsNotifications />;
      case "sync":
        return <SettingsSync />;
      case "team":
        return <SettingsTeam />;
      case "imports":
        return <SettingsImports />;
      case "crm":
        return <SettingsCrm />;
      case "permissions":
        return <SettingsPermissions />;
      case "privacy":
        return <SettingsPrivacy />;
      case "developers":
        return <SettingsDevelopers />;
      case "dictionary":
        return <SettingsDictionary />;
      case "dictation":
        return <SettingsDictation />;
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
      className="bg-card dark:bg-accent flex h-full min-h-0 w-full min-w-0 flex-1 flex-col overflow-hidden"
    >
      <div className="relative min-h-0 w-full min-w-0 flex-1 overflow-hidden">
        <div
          className={cn([
            "scroll-fade-y scrollbar-hide h-full min-h-0 w-full min-w-0 overflow-x-hidden overflow-y-auto px-6 pt-6 pb-10",
          ])}
        >
          {renderContent()}
        </div>
      </div>
    </div>
  );
}
