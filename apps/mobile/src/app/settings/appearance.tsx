import { FieldGroup, Picker, Row, Spacer, Switch, Text } from "@expo/ui";

import { SettingsError, SettingsPage } from "@/settings/components";
import { usePreferenceMutation, usePreferences } from "@/settings/preferences";

export default function AppearanceSettings() {
  const preferences = usePreferences();
  const folder = usePreferenceMutation("sidebar_show_folder");
  const tags = usePreferenceMutation("sidebar_show_tags");
  const theme = usePreferenceMutation("theme");
  return (
    <SettingsPage title="Appearance">
      <FieldGroup.Section>
        <Row alignment="center">
          <Text>Theme</Text>
          <Spacer />
          <Picker
            selectedValue={preferences.theme}
            onValueChange={(value) => theme.mutate(value)}
            enabled={!theme.isPending}
          >
            <Picker.Item value="system" label="System" />
            <Picker.Item value="light" label="Light" />
            <Picker.Item value="dark" label="Dark" />
          </Picker>
        </Row>
        <SettingsError error={theme.error} />
      </FieldGroup.Section>
      <FieldGroup.Section title="Notes list">
        <Switch
          label="Show folder"
          value={preferences.sidebar_show_folder}
          onValueChange={(value) => folder.mutate(value)}
        />
        <Switch
          label="Show tags"
          value={preferences.sidebar_show_tags}
          onValueChange={(value) => tags.mutate(value)}
        />
        <SettingsError error={folder.error || tags.error} />
      </FieldGroup.Section>
    </SettingsPage>
  );
}
