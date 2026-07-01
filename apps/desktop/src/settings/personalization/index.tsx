import { Trans, useLingui } from "@lingui/react/macro";
import { useForm } from "@tanstack/react-form";
import { PlusIcon, XIcon } from "lucide-react";

import { Button } from "@hypr/ui/components/ui/button";
import {
  InputGroup,
  InputGroupAddon,
  InputGroupButton,
  InputGroupInput,
} from "@hypr/ui/components/ui/input-group";

import { SettingsPageTitle } from "~/settings/page-title";
import { useConfigValue } from "~/shared/config";
import * as settings from "~/store/tinybase/store/settings";
import { normalizeKeywordList, parseDictionaryTermsText } from "~/stt/keywords";

const EXAMPLE_DICTIONARY_TERMS = ["Anarlog", "FastConformer", "Parakeet TDT"];

export function SettingsPersonalization() {
  const terms = useConfigValue("personalization_dictionary_terms");
  const setTerms = settings.UI.useSetValueCallback(
    "personalization_dictionary_terms",
    (value: string) => value,
    [],
    settings.STORE_ID,
  );

  return (
    <div className="flex flex-col gap-8">
      <SettingsPageTitle title={<Trans>Personalization</Trans>} />
      <DictionarySettings terms={terms} onSave={setTerms} />
    </div>
  );
}

export function DictionarySettings({
  terms,
  onSave,
}: {
  terms: string[];
  onSave: (value: string) => void;
}) {
  const { t } = useLingui();
  const normalizedTerms = normalizeKeywordList(terms);

  const form = useForm({
    defaultValues: {
      term: "",
    },
    onSubmit: ({ value }) => {
      const nextTerms = appendDictionaryTerms(normalizedTerms, value.term);
      if (nextTerms.length === normalizedTerms.length) {
        return;
      }

      onSave(JSON.stringify(nextTerms));
      form.setFieldValue("term", "");
    },
  });

  const removeTerm = (term: string) => {
    onSave(JSON.stringify(normalizedTerms.filter((value) => value !== term)));
  };

  return (
    <form
      className="flex flex-col gap-4"
      onSubmit={(event) => {
        event.preventDefault();
        event.stopPropagation();
        void form.handleSubmit();
      }}
    >
      <h2 className="font-sans text-lg font-semibold">
        <Trans>Dictionary</Trans>
      </h2>

      <InputGroup className="bg-card min-h-12 rounded-full shadow-none">
        <form.Field name="term">
          {(field) => (
            <InputGroupInput
              className="h-12 px-4 py-3"
              placeholder={t`Add names, jargon, or product terms to prefer`}
              value={field.state.value}
              onChange={(event) => field.handleChange(event.target.value)}
              onBlur={field.handleBlur}
            />
          )}
        </form.Field>
        <InputGroupAddon align="inline-end" className="pr-1.5">
          <form.Subscribe selector={(state) => state.values.term}>
            {(value) => (
              <InputGroupButton
                type="submit"
                size="sm"
                variant="secondary"
                className="h-10 rounded-full px-4 shadow-none"
                disabled={
                  appendDictionaryTerms(normalizedTerms, value).length ===
                  normalizedTerms.length
                }
              >
                <PlusIcon className="size-3.5" />
                <Trans>Add</Trans>
              </InputGroupButton>
            )}
          </form.Subscribe>
        </InputGroupAddon>
      </InputGroup>

      {normalizedTerms.length > 0 ? (
        <div className="border-border bg-card divide-border divide-y overflow-hidden rounded-2xl border">
          {normalizedTerms.map((term) => (
            <div
              key={term}
              className="flex min-h-12 items-center justify-between gap-3 px-4 py-3"
            >
              <span className="text-sm">{term}</span>
              <Button
                type="button"
                variant="ghost"
                size="icon"
                className="text-muted-foreground hover:text-foreground size-7 shrink-0"
                onClick={() => removeTerm(term)}
                aria-label={t`Remove ${term}`}
              >
                <XIcon className="size-3.5" />
              </Button>
            </div>
          ))}
        </div>
      ) : (
        <div className="border-border bg-card flex flex-col gap-3 rounded-2xl border px-4 py-4">
          <p className="text-muted-foreground text-sm">
            <Trans>Examples</Trans>
          </p>
          <div className="flex flex-wrap gap-2">
            {EXAMPLE_DICTIONARY_TERMS.map((term) => (
              <span
                key={term}
                className="border-border bg-muted text-muted-foreground rounded-full border px-3 py-1.5 text-sm"
              >
                {term}
              </span>
            ))}
          </div>
        </div>
      )}
    </form>
  );
}

function appendDictionaryTerms(terms: string[], value: string): string[] {
  return normalizeKeywordList([...terms, ...parseDictionaryTermsText(value)]);
}
