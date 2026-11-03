import { Row, Spacer } from "@expo/ui";

import { translate, useLanguage } from "@/i18n";
import { SettingsError, SettingsPage } from "@/settings/components";
import { FieldGroup } from "@/settings/field-group";
import { Picker, Switch, Text } from "@/settings/fields";
import { usePreferenceMutation, usePreferences } from "@/settings/preferences";

export default function AppearanceSettings() {
  const { language } = useLanguage();
  const preferences = usePreferences();
  const folder = usePreferenceMutation("sidebar_show_folder");
  const tags = usePreferenceMutation("sidebar_show_tags");
  const theme = usePreferenceMutation("theme");
  return (
    <SettingsPage title="Appearance">
      <FieldGroup.Section>
        <Row alignment="center">
          <Text>Theme</Text>
          <Spacer flexible />
          <Picker
            selectedValue={preferences.theme}
            onValueChange={(value) => theme.mutate(value)}
            enabled={!theme.isPending}
          >
            <Picker.Item value="system" label={translate("System", language)} />
            <Picker.Item value="light" label={translate("Light", language)} />
            <Picker.Item value="dark" label={translate("Dark", language)} />
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
