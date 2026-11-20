import { Trans, useLingui } from "@lingui/react/macro";
import { useForm } from "@tanstack/react-form";

import { ArrowRight, MinusCircle, Plus } from "@anlg/ui/components/icons";
import { Button } from "@anlg/ui/components/ui/button";
import { Input } from "@anlg/ui/components/ui/input";

import { useAuth } from "~/auth";
import {
  parseReplacements,
  parseSuggestions,
  settleSuggestion,
  type Replacement,
} from "~/dictation/learning";
import { useSetSettingValue } from "~/settings/queries";
import { useConfigValue } from "~/shared/config";
import { normalizeKeywordList } from "~/stt/keywords";

const MAX_REPLACEMENTS = 100;

export function DictionaryReplacements() {
  const { t } = useLingui();
  const stored = useConfigValue("personalization_dictionary_replacements");
  const setStored = useSetSettingValue(
    "personalization_dictionary_replacements",
  );
  const replacements = parseReplacements(stored);
  const save = (next: Replacement[]) => setStored(JSON.stringify(next));

  const form = useForm({
    defaultValues: { from: "", to: "" },
    onSubmit: ({ value }) => {
      const from = value.from.trim();
      const to = value.to.trim();
      if (!canAdd(replacements, from, to)) return;
      save([...replacements, { from, to }]);
      form.reset();
    },
  });

  return (
    <section className="flex flex-col gap-3">
      <div>
        <h3 className="text-sm font-medium">
          <Trans>Replacements</Trans>
        </h3>
        <p className="text-muted-foreground text-xs">
          <Trans>
            Swap a spoken phrase for the text you want, like "my email" for your
            address. Not applied when editing a selection.
          </Trans>
        </p>
      </div>
      <form
        className="flex items-center gap-2"
        onSubmit={(event) => {
          event.preventDefault();
          event.stopPropagation();
          void form.handleSubmit();
        }}
      >
        <form.Field name="from">
          {(field) => (
            <Input
              className="h-8 min-w-0 flex-1"
              placeholder={t`When I say`}
              aria-label={t`When I say`}
              maxLength={100}
              value={field.state.value}
              onChange={(event) => field.handleChange(event.target.value)}
            />
          )}
        </form.Field>
        <ArrowRight className="text-muted-foreground size-4 shrink-0" />
        <form.Field name="to">
          {(field) => (
            <Input
              className="h-8 min-w-0 flex-1"
              placeholder={t`Type`}
              aria-label={t`Type`}
              maxLength={2000}
              value={field.state.value}
              onChange={(event) => field.handleChange(event.target.value)}
            />
          )}
        </form.Field>
        <form.Subscribe selector={(state) => state.values}>
          {(values) => (
            <Button
              type="submit"
              variant="outline"
              size="sm"
              disabled={
                !canAdd(replacements, values.from.trim(), values.to.trim())
              }
            >
              <Plus className="size-3.5" />
              <Trans>Add</Trans>
            </Button>
          )}
        </form.Subscribe>
      </form>
      {replacements.length > 0 && (
        <div className="border-border bg-card divide-border divide-y overflow-hidden rounded-2xl border">
          {replacements.map(({ from, to }) => (
            <div
              key={from}
              className="flex min-h-12 items-center justify-between gap-3 py-3 pr-3 pl-4"
            >
              <span className="flex min-w-0 items-center gap-2 text-sm">
                <span className="truncate">{from}</span>
                <ArrowRight className="text-muted-foreground size-3.5 shrink-0" />
                <span className="truncate">{to}</span>
              </span>
              <Button
                type="button"
                variant="ghost"
                size="icon"
                className="text-muted-foreground hover:text-foreground size-7 shrink-0"
                onClick={() =>
                  save(replacements.filter((entry) => entry.from !== from))
                }
                aria-label={t`Remove ${from}`}
              >
                <MinusCircle className="size-4" />
              </Button>
            </div>
          ))}
        </div>
      )}
    </section>
  );
}

/** Terms suggested from the person's own corrections, kept until they add or dismiss them. */
export function DictionarySuggestions({
  terms,
  onSave,
}: {
  terms: string[];
  onSave: (value: string) => void;
}) {
  const { t } = useLingui();
  const { session } = useAuth();
  const stored = useConfigValue("dictation_dictionary_suggestions");
  const setStored = useSetSettingValue("dictation_dictionary_suggestions");
  const suggestions = parseSuggestions(stored, session?.user.id ?? null);
  if (!suggestions?.terms.length) return null;

  const settle = (term: string) =>
    setStored(JSON.stringify(settleSuggestion(suggestions, term)));

  return (
    <section className="flex flex-col gap-3">
      <div>
        <h3 className="text-sm font-medium">
          <Trans>Suggested from your corrections</Trans>
        </h3>
        <p className="text-muted-foreground text-xs">
          <Trans>
            Words you fixed after dictating. Add them so dictation spells them
            right next time.
          </Trans>
        </p>
      </div>
      <div className="border-border bg-card divide-border divide-y overflow-hidden rounded-2xl border">
        {suggestions.terms.map((term) => (
          <div
            key={term}
            className="flex min-h-12 items-center justify-between gap-3 py-3 pr-3 pl-4"
          >
            <span className="text-sm">{term}</span>
            <div className="flex items-center gap-1">
              <Button
                type="button"
                variant="outline"
                size="sm"
                onClick={() => {
                  onSave(
                    JSON.stringify(normalizeKeywordList([...terms, term])),
                  );
                  settle(term);
                }}
                aria-label={t`Add ${term}`}
              >
                <Trans>Add</Trans>
              </Button>
              <Button
                type="button"
                variant="ghost"
                size="sm"
                onClick={() => settle(term)}
                aria-label={t`Dismiss ${term}`}
              >
                <Trans>Dismiss</Trans>
              </Button>
            </div>
          </div>
        ))}
      </div>
    </section>
  );
}

function canAdd(replacements: Replacement[], from: string, to: string) {
  if (!from || !to || replacements.length >= MAX_REPLACEMENTS) return false;
  const key = from.toLocaleLowerCase();
  return !replacements.some((entry) => entry.from.toLocaleLowerCase() === key);
}
