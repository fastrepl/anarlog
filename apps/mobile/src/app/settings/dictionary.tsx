import {
  Button,
  FieldGroup,
  Row,
  Spacer,
  Text,
  TextInput,
  useNativeState,
} from "@expo/ui";
import { useForm } from "@tanstack/react-form";

import { SettingsError, SettingsPage } from "@/settings/components";
import { usePreferenceMutation, usePreferences } from "@/settings/preferences";
import { normalizeDictionary } from "@/settings/preferences-model";

export default function DictionarySettings() {
  const preferences = usePreferences();
  const save = usePreferenceMutation("personalization_dictionary_terms");
  const input = useNativeState("");
  const form = useForm({
    defaultValues: { term: "" },
    onSubmit: async ({ value }) => {
      if (
        !value.term.trim() ||
        value.term.trim().length > 100 ||
        preferences.personalization_dictionary_terms.length >= 100
      )
        return;
      await save.mutateAsync(
        normalizeDictionary([
          ...preferences.personalization_dictionary_terms,
          value.term,
        ]),
      );
      input.value = "";
      form.reset();
    },
  });
  return (
    <SettingsPage title="Dictionary">
      <FieldGroup.Section>
        <TextInput
          value={input}
          placeholder="Add a name, acronym, or term"
          autoCorrect={false}
          onChangeText={(value) => {
            input.value = value;
            form.setFieldValue("term", value);
          }}
          onSubmitEditing={() => {
            void form.handleSubmit().catch(() => {});
          }}
        />
        <form.Subscribe selector={(state) => state.values.term}>
          {(term) => (
            <Button
              label="Add term"
              disabled={
                save.isPending ||
                !term.trim() ||
                term.trim().length > 100 ||
                preferences.personalization_dictionary_terms.length >= 100
              }
              onPress={() => {
                void form.handleSubmit().catch(() => {});
              }}
            />
          )}
        </form.Subscribe>
        <FieldGroup.SectionFooter>
          <Text>
            Help transcription recognize teammate names, company jargon, and
            product terms. Up to 100 terms, 100 characters each.
          </Text>
        </FieldGroup.SectionFooter>
        <SettingsError error={save.error} />
      </FieldGroup.Section>
      <FieldGroup.Section
        title={`${preferences.personalization_dictionary_terms.length} terms`}
      >
        {preferences.personalization_dictionary_terms.length === 0 ? (
          <Text>Your dictionary is empty.</Text>
        ) : (
          preferences.personalization_dictionary_terms.map((term) => (
            <Row key={term} alignment="center">
              <Text>{term}</Text>
              <Spacer flexible />
              <Button
                label="Remove"
                variant="text"
                disabled={save.isPending}
                onPress={() =>
                  save.mutate(
                    preferences.personalization_dictionary_terms.filter(
                      (value) => value !== term,
                    ),
                  )
                }
              />
            </Row>
          ))
        )}
      </FieldGroup.Section>
    </SettingsPage>
  );
}
