import { Icon } from "@expo/ui";
import { useRouter } from "expo-router";

import { SettingsPage } from "@/settings/components";
import { SettingsMenuGroup, SettingsMenuLink } from "@/settings/menu";

export default function SettingsScreen() {
  const router = useRouter();
  return (
    <SettingsPage title="Settings" layout="menu">
      <SettingsMenuGroup>
        <SettingsMenuLink
          title="Account"
          icon={Icon.select({
            ios: "person.crop.circle",
            android: import("@expo/material-symbols/account_circle.xml"),
          })}
          onPress={() => router.push("/settings/account")}
        />
      </SettingsMenuGroup>
      <SettingsMenuGroup>
        <SettingsMenuLink
          title="Sync & storage"
          icon={Icon.select({
            ios: "icloud",
            android: import("@expo/material-symbols/cloud.xml"),
          })}
          onPress={() => router.push("/settings/sync")}
        />
      </SettingsMenuGroup>
      <SettingsMenuGroup>
        <SettingsMenuLink
          title="Recording"
          icon={Icon.select({
            ios: "mic",
            android: import("@expo/material-symbols/mic.xml"),
          })}
          onPress={() => router.push("/settings/recording")}
        />
        <SettingsMenuLink
          title="Transcription & summaries"
          icon={Icon.select({
            ios: "text.bubble",
            android: import("@expo/material-symbols/subtitles.xml"),
          })}
          onPress={() => router.push("/settings/transcription")}
        />
        <SettingsMenuLink
          title="Appearance"
          icon={Icon.select({
            ios: "sun.max",
            android: import("@expo/material-symbols/light_mode.xml"),
          })}
          onPress={() => router.push("/settings/appearance")}
        />
      </SettingsMenuGroup>
      <SettingsMenuGroup>
        <SettingsMenuLink
          title="Privacy"
          icon={Icon.select({
            ios: "hand.raised",
            android: import("@expo/material-symbols/privacy_tip.xml"),
          })}
          onPress={() => router.push("/settings/privacy")}
        />
        <SettingsMenuLink
          title="Help & about"
          icon={Icon.select({
            ios: "questionmark.circle",
            android: import("@expo/material-symbols/help.xml"),
          })}
          onPress={() => router.push("/settings/help")}
        />
      </SettingsMenuGroup>
    </SettingsPage>
  );
}
