import { Trans } from "@lingui/react/macro";
import { useForm } from "@tanstack/react-form";

import { Badge } from "@hypr/ui/components/ui/badge";
import { Button } from "@hypr/ui/components/ui/button";
import { Textarea } from "@hypr/ui/components/ui/textarea";

import { SettingsPageTitle } from "~/settings/page-title";
import { useConfigValue } from "~/shared/config";
import * as settings from "~/store/tinybase/store/settings";
import {
  formatDictionaryTerms,
  parseDictionaryTermsText,
} from "~/stt/keywords";

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

function DictionarySettings({
  terms,
  onSave,
}: {
  terms: string[];
  onSave: (value: string) => void;
}) {
  const form = useForm({
    defaultValues: {
      terms: formatDictionaryTerms(terms),
    },
    onSubmit: ({ value }) => {
      onSave(JSON.stringify(parseDictionaryTermsText(value.terms)));
    },
  });

  return (
    <form
      className="flex flex-col gap-4"
      onSubmit={(event) => {
        event.preventDefault();
        event.stopPropagation();
        void form.handleSubmit();
      }}
    >
      <div className="flex items-start justify-between gap-4">
        <div>
          <h3 className="mb-1 text-sm font-medium">
            <Trans>Dictionary</Trans>
          </h3>
          <p className="text-muted-foreground text-xs">
            <Trans>Names, jargon, and product terms to prefer.</Trans>
          </p>
        </div>
        <form.Subscribe selector={(state) => state.values.terms}>
          {(value) => {
            const count = parseDictionaryTermsText(value).length;
            return (
              <Badge variant="secondary" className="shrink-0">
                {count}
              </Badge>
            );
          }}
        </form.Subscribe>
      </div>

      <form.Field name="terms">
        {(field) => (
          <Textarea
            className="min-h-44 resize-y"
            placeholder={"Anarlog\nFastConformer\nParakeet TDT"}
            value={field.state.value}
            onChange={(event) => field.handleChange(event.target.value)}
            onBlur={field.handleBlur}
          />
        )}
      </form.Field>

      <div className="flex justify-end">
        <Button type="submit" size="sm">
          <Trans>Save</Trans>
        </Button>
      </div>
    </form>
  );
}
