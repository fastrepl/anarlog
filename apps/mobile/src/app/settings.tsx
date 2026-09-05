import { FieldGroup, Icon } from "@expo/ui";
import { useRouter } from "expo-router";

import { SettingsPage, SettingsRow } from "@/settings/components";

export default function SettingsScreen() {
  const router = useRouter();
  return (
    <SettingsPage title="Settings">
      <FieldGroup.Section>
        <SettingsRow
          title="Account"
          icon={Icon.select({
            ios: "person.crop.circle",
            android: import("@expo/material-symbols/account_circle.xml"),
          })}
          onPress={() => router.push("/settings/account")}
        />
      </FieldGroup.Section>
      <FieldGroup.Section>
        <SettingsRow
          title="Sync & storage"
          icon={Icon.select({
            ios: "icloud",
            android: import("@expo/material-symbols/cloud.xml"),
          })}
          onPress={() => router.push("/settings/sync")}
        />
      </FieldGroup.Section>
      <FieldGroup.Section>
        <SettingsRow
          title="Recording"
          icon={Icon.select({
            ios: "mic",
            android: import("@expo/material-symbols/mic.xml"),
          })}
          onPress={() => router.push("/settings/recording")}
        />
        <SettingsRow
          title="Transcription & summaries"
          icon={Icon.select({
            ios: "text.bubble",
            android: import("@expo/material-symbols/subtitles.xml"),
          })}
          onPress={() => router.push("/settings/transcription")}
        />
        <SettingsRow
          title="Appearance"
          icon={Icon.select({
            ios: "sun.max",
            android: import("@expo/material-symbols/light_mode.xml"),
          })}
          onPress={() => router.push("/settings/appearance")}
        />
      </FieldGroup.Section>
      <FieldGroup.Section>
        <SettingsRow
          title="Privacy"
          icon={Icon.select({
            ios: "hand.raised",
            android: import("@expo/material-symbols/privacy_tip.xml"),
          })}
          onPress={() => router.push("/settings/privacy")}
        />
        <SettingsRow
          title="Help & about"
          icon={Icon.select({
            ios: "questionmark.circle",
            android: import("@expo/material-symbols/help.xml"),
          })}
          onPress={() => router.push("/settings/help")}
        />
      </FieldGroup.Section>
    </SettingsPage>
  );
}
