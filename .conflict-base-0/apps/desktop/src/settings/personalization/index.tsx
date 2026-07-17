import { Trans, useLingui } from "@lingui/react/macro";
import { useForm } from "@tanstack/react-form";
import { BracesIcon, CircleMinusIcon, PlusIcon } from "lucide-react";
import { useRef } from "react";

import { PromptEditor, type PromptEditorHandle } from "@hypr/editor/prompt";
import { Button } from "@hypr/ui/components/ui/button";
import {
  InputGroup,
  InputGroupAddon,
  InputGroupButton,
  InputGroupInput,
} from "@hypr/ui/components/ui/input-group";
import { cn } from "@hypr/utils";

import { SettingsPageTitle } from "~/settings/page-title";
import { useSetSettingValue, useSetSettingValues } from "~/settings/queries";
import { useConfigValue } from "~/shared/config";
import {
  DEFAULT_SUMMARY_PROMPT,
  getTokenAwareSummaryPrompt,
  hasSummaryTemplateToken,
  isDefaultSummaryPrompt,
} from "~/shared/summary-prompt";
import { normalizeKeywordList, parseDictionaryTermsText } from "~/stt/keywords";

export function SettingsPersonalization() {
  const terms = useConfigValue("personalization_dictionary_terms");
  const setTerms = useSetSettingValue("personalization_dictionary_terms");
  const summaryInstructions = useConfigValue("custom_summary_instructions");
  const summaryInstructionsTokenAware = useConfigValue(
    "custom_summary_instructions_token_aware",
  );
  const setSummarySettings = useSetSettingValues();
  const editableSummaryInstructions = getTokenAwareSummaryPrompt(
    summaryInstructions,
    summaryInstructionsTokenAware,
  );

  return (
    <div className="flex flex-col gap-8">
      <SettingsPageTitle title={<Trans>Personalization</Trans>} />
      <DictionarySettings terms={terms} onSave={setTerms} />
      <SummaryInstructionsSettings
        key={editableSummaryInstructions}
        instructions={editableSummaryInstructions}
        onSave={(value) =>
          setSummarySettings({
            custom_summary_instructions: value,
            custom_summary_instructions_token_aware: true,
          })
        }
      />
    </div>
  );
}

export function SummaryInstructionsSettings({
  instructions,
  onSave,
}: {
  instructions: string;
  onSave: (value: string) => void;
}) {
  const { t } = useLingui();
  const editorRef = useRef<PromptEditorHandle>(null);
  const savedInstructions = instructions.trim() || DEFAULT_SUMMARY_PROMPT;
  const form = useForm({
    defaultValues: { instructions: savedInstructions },
    onSubmit: ({ value }) => {
      const nextInstructions =
        value.instructions.trim() || DEFAULT_SUMMARY_PROMPT;
      onSave(isDefaultSummaryPrompt(nextInstructions) ? "" : nextInstructions);
      form.reset({ instructions: nextInstructions });
      editorRef.current?.setValue(nextInstructions);
    },
  });

  const resetToDefault = () => {
    onSave("");
    form.reset({ instructions: DEFAULT_SUMMARY_PROMPT });
    editorRef.current?.setValue(DEFAULT_SUMMARY_PROMPT);
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
      <div>
        <h2 className="font-sans text-lg font-semibold">
          <Trans>Summary instructions</Trans>
        </h2>
        <p className="text-muted-foreground mt-1 text-sm">
          <Trans>
            Applied to every generated or regenerated meeting summary.
          </Trans>
        </p>
      </div>

      <form.Field name="instructions">
        {(field) => {
          const includesTemplate = hasSummaryTemplateToken(field.state.value);

          return (
            <div className="flex flex-col gap-3">
              <div className="border-border bg-card overflow-hidden rounded-2xl border">
                <PromptEditor
                  ref={editorRef}
                  ariaLabel={t`Summary instructions`}
                  className="min-h-40 px-4 py-3 text-sm leading-5"
                  initialValue={field.state.value}
                  maxLength={4000}
                  placeholder={t`Example: Start with a two-sentence overview, then list decisions and action items with owners. Do not use headings.`}
                  onChange={field.handleChange}
                  onBlur={field.handleBlur}
                />
                <div className="border-border bg-muted/40 flex items-center justify-between gap-3 border-t px-3 py-2">
                  <span className="text-muted-foreground text-xs font-medium">
                    Variables
                  </span>
                  <Button
                    type="button"
                    variant="outline"
                    size="sm"
                    className="h-7 rounded-full px-2.5 text-xs"
                    disabled={includesTemplate}
                    onClick={() => editorRef.current?.insertToken("template")}
                  >
                    <BracesIcon className="size-3.5" />
                    Template
                  </Button>
                </div>
              </div>

              <p className="text-muted-foreground text-sm">
                The Template chip inserts the selected template. Remove it to
                ignore templates and use only these instructions.
              </p>

              {includesTemplate ? (
                <p className="text-muted-foreground text-sm">
                  <Trans>
                    These instructions take priority over the selected template
                    when they conflict. Clear them to use templates as written.
                  </Trans>
                </p>
              ) : null}

              {!includesTemplate && field.state.value.trim() ? (
                <p
                  role="status"
                  className="rounded-xl border border-amber-200 bg-amber-50 px-3 py-2 text-sm text-amber-900 dark:border-amber-900/60 dark:bg-amber-950/40 dark:text-amber-200"
                >
                  Template is not included. Selected templates will be ignored
                  when summaries are generated.
                </p>
              ) : null}
            </div>
          );
        }}
      </form.Field>

      <div className="flex items-center justify-end gap-2">
        <form.Subscribe selector={(state) => state.values.instructions}>
          {(value) => (
            <Button
              type="button"
              variant="ghost"
              disabled={
                isDefaultSummaryPrompt(value) &&
                isDefaultSummaryPrompt(instructions)
              }
              onClick={resetToDefault}
            >
              <Trans>Reset to default</Trans>
            </Button>
          )}
        </form.Subscribe>
        <form.Subscribe
          selector={(state) => [state.canSubmit, state.isDirty] as const}
        >
          {([canSubmit, isDirty]) => (
            <Button type="submit" disabled={!canSubmit || !isDirty}>
              <Trans>Save</Trans>
            </Button>
          )}
        </form.Subscribe>
      </div>
    </form>
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
        <InputGroupAddon align="inline-end" className="pr-1.5">
          <form.Subscribe selector={(state) => state.values.term}>
            {(value) => {
              const hasInput = parseDictionaryTermsText(value).length > 0;
              const canAdd =
                appendDictionaryTerms(normalizedTerms, value).length !==
                normalizedTerms.length;

              return (
                <InputGroupButton
                  type="submit"
                  variant="ghost"
                  size="sm"
                  className={cn([
                    "rounded-full px-3",
                    hasInput
                      ? "bg-black text-white hover:bg-black/90 hover:text-white dark:bg-white dark:text-black dark:hover:bg-white/90 dark:hover:text-black"
                      : null,
                  ])}
                  disabled={!canAdd}
                >
                  <PlusIcon className="size-3.5" />
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
                    <CircleMinusIcon className="size-4" />
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
