import { FieldGroup, Picker, Row, Spacer, Text } from "@expo/ui";
import { useRouter } from "expo-router";

import {
  SettingsError,
  SettingsPage,
  SettingsRow,
} from "@/settings/components";
import { LANGUAGE_CODES, languageName } from "@/settings/languages";
import { usePreferenceMutation, usePreferences } from "@/settings/preferences";

export default function TranscriptionSettings() {
  const router = useRouter();
  const preferences = usePreferences();
  const language = usePreferenceMutation("ai_language");
  const length = usePreferenceMutation("summary_length");
  const languages = [
    ...new Set([preferences.ai_language, ...LANGUAGE_CODES]),
  ].sort((a, b) => languageName(a).localeCompare(languageName(b)));
  return (
    <SettingsPage title="Transcription & summaries">
      <FieldGroup.Section>
        <SettingsRow
          title="Transcription provider"
          onPress={() => router.push("/settings/transcription-provider")}
        />
        <SettingsRow
          title="Summary provider"
          onPress={() => router.push("/settings/summary-provider")}
        />
      </FieldGroup.Section>
      <FieldGroup.Section title="Language">
        <Row alignment="center">
          <Text>Main language</Text>
          <Spacer />
          <Picker
            selectedValue={preferences.ai_language}
            onValueChange={(value) => language.mutate(value)}
            enabled={!language.isPending}
          >
            {languages.map((code) => (
              <Picker.Item key={code} value={code} label={languageName(code)} />
            ))}
          </Picker>
        </Row>
        <SettingsRow
          title="Additional spoken languages"
          value={
            preferences.spoken_languages
              .filter((code) => code !== preferences.ai_language)
              .map(languageName)
              .join(", ") || "None"
          }
          onPress={() => router.push("/settings/languages")}
        />
        <SettingsRow
          title="Dictionary"
          value={`${preferences.personalization_dictionary_terms.length} terms`}
          onPress={() => router.push("/settings/dictionary")}
        />
        <FieldGroup.SectionFooter>
          <Text>
            The main language guides transcription and is used for summaries.
          </Text>
        </FieldGroup.SectionFooter>
        <SettingsError error={language.error} />
      </FieldGroup.Section>
      <FieldGroup.Section>
        <Row alignment="center">
          <Text>Summary length</Text>
          <Spacer />
          <Picker
            selectedValue={preferences.summary_length}
            onValueChange={(value) => length.mutate(value)}
            enabled={!length.isPending}
          >
            <Picker.Item value="crisp" label="Crisp" />
            <Picker.Item value="balanced" label="Balanced" />
            <Picker.Item value="detailed" label="Detailed" />
          </Picker>
        </Row>
        <SettingsError error={length.error} />
      </FieldGroup.Section>
    </SettingsPage>
  );
}
