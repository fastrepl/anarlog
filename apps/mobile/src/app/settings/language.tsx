import { Row, Spacer } from "@expo/ui";

import { useLanguage } from "@/i18n";
import { SettingsPage } from "@/settings/components";
import { FieldGroup } from "@/settings/field-group";
import { Picker, Text } from "@/settings/fields";

export default function LanguageSettings() {
  const { language, setLanguage } = useLanguage();
  return (
    <SettingsPage title="Language">
      <FieldGroup.Section>
        <Row alignment="center">
          <Text>Language</Text>
          <Spacer flexible />
          <Picker
            selectedValue={language}
            onValueChange={(value) => {
              if (value === "vi" || value === "en") setLanguage(value);
            }}
          >
            <Picker.Item value="vi" label="Tiếng Việt" />
            <Picker.Item value="en" label="English" />
          </Picker>
        </Row>
      </FieldGroup.Section>
    </SettingsPage>
  );
}
