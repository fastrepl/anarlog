import { Trans, useLingui } from "@lingui/react/macro";
import { BookOpen, MinusCircle, Plus } from "@phosphor-icons/react";
import { useForm } from "@tanstack/react-form";

import { Button } from "@anlg/ui/components/ui/button";
import {
  InputGroup,
  InputGroupAddon,
  InputGroupButton,
  InputGroupInput,
} from "@anlg/ui/components/ui/input-group";

import { trackAnalyticsEvent } from "~/analytics";
import { SettingsPageTitle } from "~/settings/page-title";
import { useSetSettingValue } from "~/settings/queries";
import { useConfigValue } from "~/shared/config";
import { normalizeKeywordList, parseDictionaryTermsText } from "~/stt/keywords";

export function SettingsDictionary() {
  const terms = useConfigValue("personalization_dictionary_terms");
  const setTerms = useSetSettingValue("personalization_dictionary_terms");

  return (
    <div className="flex flex-col gap-8">
      <SettingsPageTitle title={<Trans>Dictionary</Trans>} />
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
      trackAnalyticsEvent("dictionary_updated", {
        operation: "added",
        term_count: nextTerms.length,
        added_count: nextTerms.length - normalizedTerms.length,
      });
      form.setFieldValue("term", "");
    },
  });

  const removeTerm = (term: string) => {
    const nextTerms = normalizedTerms.filter((value) => value !== term);
    onSave(JSON.stringify(nextTerms));
    trackAnalyticsEvent("dictionary_updated", {
      operation: "removed",
      term_count: nextTerms.length,
      removed_count: normalizedTerms.length - nextTerms.length,
    });
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
      <InputGroup className="border-border bg-card has-[[data-slot=input-group-control]:focus-visible]:border-border rounded-full shadow-none has-[[data-slot=input-group-control]:focus-visible]:ring-0">
        <form.Field name="term">
          {(field) => (
            <InputGroupInput
              className="pr-4 pl-4"
              placeholder={t`Add names, jargon, or product terms to prefer`}
              value={field.state.value}
              onChange={(event) => field.handleChange(event.target.value)}
              onBlur={field.handleBlur}
            />
          )}
        </form.Field>
        <InputGroupAddon align="inline-end">
          <form.Subscribe selector={(state) => state.values.term}>
            {(value) => {
              const canAdd =
                appendDictionaryTerms(normalizedTerms, value).length !==
                normalizedTerms.length;

              return (
                <InputGroupButton
                  type="submit"
                  variant="ghost"
                  size="xs"
                  className="rounded-full bg-black text-white hover:bg-black/90 hover:text-white dark:bg-white dark:text-black dark:hover:bg-white/90 dark:hover:text-black"
                  disabled={!canAdd}
                  aria-label={t`Add`}
                >
                  <Plus className="size-3.5" />
                  <Trans>Add</Trans>
                </InputGroupButton>
              );
            }}
          </form.Subscribe>
        </InputGroupAddon>
      </InputGroup>

      <form.Subscribe selector={(state) => state.values.term}>
        {(value) => {
          const visibleTerms = getVisibleDictionaryTerms(
            normalizedTerms,
            value,
          );
          const hasSearch = parseDictionaryTermsText(value).length > 0;

          if (normalizedTerms.length === 0) {
            return (
              <div className="border-border bg-card flex min-h-40 flex-col items-center justify-center rounded-2xl border px-6 text-center">
                <BookOpen className="text-muted-foreground mb-3 size-5" />
                <p className="text-sm font-medium">
                  <Trans>Your dictionary is empty</Trans>
                </p>
                <p className="text-muted-foreground mt-1 max-w-sm text-xs">
                  <Trans>
                    Tip: Add teammate names, acronyms, company jargon, and
                    product terms.
                  </Trans>
                </p>
              </div>
            );
          }

          if (visibleTerms.length === 0) {
            return hasSearch ? (
              <p className="text-muted-foreground px-4 text-sm">
                <Trans>No match</Trans>
              </p>
            ) : null;
          }

          return (
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
                    <MinusCircle className="size-4" />
                  </Button>
                </div>
              ))}
            </div>
          );
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
