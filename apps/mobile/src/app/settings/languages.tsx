import { useLanguage } from "@/i18n";
import { SettingsError, SettingsPage } from "@/settings/components";
import { FieldGroup } from "@/settings/field-group";
import { Switch, Text } from "@/settings/fields";
import { LANGUAGE_CODES, languageName } from "@/settings/languages";
import { usePreferenceMutation, usePreferences } from "@/settings/preferences";

export default function LanguageSettings() {
  const { language } = useLanguage();
  const preferences = usePreferences();
  const save = usePreferenceMutation("spoken_languages");
  const selected = preferences.spoken_languages.filter(
    (code) => code !== preferences.ai_language,
  );
  const codes = [...new Set([...selected, ...LANGUAGE_CODES])]
    .filter((code) => code !== preferences.ai_language)
    .sort((a, b) =>
      languageName(a, language).localeCompare(languageName(b, language)),
    );
  return (
    <SettingsPage title="Spoken languages">
      <FieldGroup.Section>
        <FieldGroup.SectionFooter>
          <Text>
            {language === "vi"
              ? `Ngôn ngữ chính: ${languageName(preferences.ai_language, language)}. Chọn tối đa tám ngôn ngữ bổ sung.`
              : `Main language: ${languageName(preferences.ai_language, language)}. Choose up to eight additional languages.`}
          </Text>
        </FieldGroup.SectionFooter>
        {codes.map((code) => (
          <Switch
            key={code}
            label={languageName(code, language)}
            value={selected.includes(code)}
            disabled={
              save.isPending ||
              (selected.length >= 8 && !selected.includes(code))
            }
            onValueChange={(enabled) =>
              save.mutate(
                enabled
                  ? [...selected, code]
                  : selected.filter((value) => value !== code),
              )
            }
          />
        ))}
        <SettingsError error={save.error} />
      </FieldGroup.Section>
    </SettingsPage>
  );
}
