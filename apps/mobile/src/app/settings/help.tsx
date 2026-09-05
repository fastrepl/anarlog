import { FieldGroup } from "@expo/ui";
import { useMutation } from "@tanstack/react-query";
import Constants from "expo-constants";
import * as WebBrowser from "expo-web-browser";
import { Linking } from "react-native";

import {
  SettingsError,
  SettingsPage,
  SettingsRow,
} from "@/settings/components";

export default function HelpSettings() {
  const open = useMutation({
    mutationFn: async (url: string) => {
      if (url.startsWith("mailto:")) await Linking.openURL(url);
      else await WebBrowser.openBrowserAsync(url);
    },
  });
  return (
    <SettingsPage title="Help & about">
      <FieldGroup.Section>
        <SettingsRow
          title="Help center"
          onPress={() => open.mutate("https://docs.anarlog.so")}
        />
        <SettingsRow
          title="Contact support"
          onPress={() =>
            open.mutate(
              "mailto:founders@anarlog.so?subject=Anarlog%20mobile%20support",
            )
          }
        />
        <SettingsError error={open.error} />
      </FieldGroup.Section>
      <FieldGroup.Section>
        <SettingsRow
          title="Privacy policy"
          onPress={() => open.mutate("https://anarlog.so/privacy")}
        />
        <SettingsRow
          title="Terms of service"
          onPress={() => open.mutate("https://anarlog.so/terms")}
        />
        <SettingsRow
          title="Version"
          value={Constants.expoConfig?.version ?? "Unknown"}
        />
      </FieldGroup.Section>
    </SettingsPage>
  );
}
