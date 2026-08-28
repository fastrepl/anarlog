import * as stylex from "@stylexjs/stylex";

import { colors } from "@anlg/design-system/tokens.stylex";
import { mergeStyleXProps } from "@anlg/ui/lib/stylex";

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
import { SettingsDevelopers } from "~/settings/developers";
import { SettingsDictionary } from "~/settings/dictionary";
import { SettingsHydrationBoundary } from "~/settings/hydration-boundary";
import { SettingsImports } from "~/settings/imports";
import { SettingsPrivacy } from "~/settings/privacy";
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
      case "permissions":
        return <SettingsPermissions />;
      case "privacy":
        return <SettingsPrivacy />;
      case "developers":
        return <SettingsDevelopers />;
      case "dictionary":
        return <SettingsDictionary />;
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
    <div data-settings-content {...stylex.props(styles.root)}>
      <div {...stylex.props(styles.viewport)}>
        <div {...mergeStyleXProps(styles.scroller, "scroll-fade-y")}>
          {renderContent()}
        </div>
      </div>
    </div>
  );
}

const styles = stylex.create({
  root: {
    backgroundColor: {
      default: colors.card,
      ":is(.dark *)": colors.accent,
    },
    display: "flex",
    flex: "1",
    flexDirection: "column",
    overflow: "hidden",
    width: "100%",
  },
  scroller: {
    display: {
      default: null,
      "::-webkit-scrollbar": "none",
    },
    flex: "1",
    height: "100%",
    overflowY: "auto",
    padding: "1.5rem",
    scrollbarWidth: "none",
    width: "100%",
  },
  viewport: {
    flex: "1",
    overflow: "hidden",
    position: "relative",
    width: "100%",
  },
});
