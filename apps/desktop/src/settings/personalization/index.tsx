import { Trans, useLingui } from "@lingui/react/macro";
import { useForm } from "@tanstack/react-form";
import { CircleMinusIcon, PlusIcon } from "lucide-react";

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

      <InputGroup className="border-border/60 bg-card has-[[data-slot=input-group-control]:focus-visible]:border-border/70 min-h-12 rounded-full shadow-none has-[[data-slot=input-group-control]:focus-visible]:ring-0">
        <form.Field name="term">
          {(field) => (
            <InputGroupInput
              className="h-12 py-3 pr-6 pl-4"
              placeholder={t`Add names, jargon, or product terms to prefer`}
              value={field.state.value}
              onChange={(event) => field.handleChange(event.target.value)}
              onBlur={field.handleBlur}
            />
          )}
        </form.Field>
        <InputGroupAddon align="inline-end" className="pr-2.5">
          <form.Subscribe selector={(state) => state.values.term}>
            {(value) => (
              <InputGroupButton
                type="submit"
                size="sm"
                className="h-10 rounded-full px-5 shadow-xs"
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

      <form.Subscribe selector={(state) => state.values.term}>
        {(value) => {
          const visibleTerms = getVisibleDictionaryTerms(
            normalizedTerms,
            value,
          );

          return visibleTerms.length > 0 ? (
            <div className="border-border bg-card divide-border divide-y overflow-hidden rounded-2xl border">
              {visibleTerms.map((term) => (
                <div
                  key={term}
                  className="group flex min-h-12 items-center justify-between gap-3 py-3 pr-3 pl-4"
                >
                  <span className="text-sm">{term}</span>
                  <Button
                    type="button"
                    variant="ghost"
                    size="icon"
                    className="text-muted-foreground hover:text-foreground size-7 shrink-0 opacity-0 transition-opacity group-hover:opacity-100 focus-visible:opacity-100"
                    onClick={() => removeTerm(term)}
                    aria-label={t`Remove ${term}`}
                  >
                    <CircleMinusIcon className="size-4" />
                  </Button>
                </div>
              ))}
            </div>
          ) : null;
        }}
      </form.Subscribe>
    </form>
  );
}

function appendDictionaryTerms(terms: string[], value: string): string[] {
  return normalizeKeywordList([...terms, ...parseDictionaryTermsText(value)]);
}

function getVisibleDictionaryTerms(terms: string[], value: string): string[] {
  const queries = parseDictionaryTermsText(value).map((term) =>
    term.toLocaleLowerCase(),
  );
  if (queries.length === 0) {
    return terms;
  }

  return terms.filter((term) => {
    const key = term.toLocaleLowerCase();
    return queries.some((query) => key.includes(query) || query.includes(key));
  });
}
