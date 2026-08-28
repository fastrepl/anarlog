import { Trans, useLingui } from "@lingui/react/macro";
import { BookOpen, LockSimple, MinusCircle, Plus } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { useForm } from "@tanstack/react-form";

import { colors, radii } from "@anlg/design-system/tokens.stylex";
import { Button } from "@anlg/ui/components/ui/button";
import {
  InputGroup,
  InputGroupAddon,
  InputGroupButton,
  InputGroupInput,
} from "@anlg/ui/components/ui/input-group";

import { trackAnalyticsEvent } from "~/analytics";
import { useBillingAccess } from "~/auth/billing-context";
import { SettingsPageTitle } from "~/settings/page-title";
import { useSetSettingValue } from "~/settings/queries";
import { useConfigValue } from "~/shared/config";
import { normalizeKeywordList, parseDictionaryTermsText } from "~/stt/keywords";

export function SettingsDictionary() {
  const terms = useConfigValue("personalization_dictionary_terms");
  const setTerms = useSetSettingValue("personalization_dictionary_terms");
  const { isPro, upgradeToPro, isUpgradingToPro } = useBillingAccess();

  return (
    <div {...stylex.props(styles.page)}>
      <SettingsPageTitle title={<Trans>Dictionary</Trans>} />
      {isPro ? (
        <DictionarySettings terms={terms} onSave={setTerms} />
      ) : (
        <div {...stylex.props(styles.upsell)}>
          <div {...stylex.props(styles.upsellCopy)}>
            <div {...stylex.props(styles.lockBadge)}>
              <LockSimple {...stylex.props(styles.lockIcon)} />
            </div>
            <div>
              <h3 {...stylex.props(styles.heading)}>
                <Trans>Build a custom dictionary with Anarlog Pro</Trans>
              </h3>
              <p {...stylex.props(styles.description)}>
                <Trans>
                  Help transcription recognize names, jargon, and product terms.
                </Trans>
              </p>
            </div>
          </div>
          <Button
            type="button"
            size="sm"
            onClick={upgradeToPro}
            disabled={isUpgradingToPro}
          >
            <Trans>Upgrade to Pro</Trans>
          </Button>
        </div>
      )}
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
      {...stylex.props(styles.form)}
      onSubmit={(event) => {
        event.preventDefault();
        event.stopPropagation();
        void form.handleSubmit();
      }}
    >
      <InputGroup sx={styles.inputGroup}>
        <form.Field name="term">
          {(field) => (
            <InputGroupInput
              sx={styles.input}
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
                  sx={styles.addButton}
                  disabled={!canAdd}
                  aria-label={t`Add`}
                >
                  <Plus {...stylex.props(styles.smallIcon)} />
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
              <div {...stylex.props(styles.empty)}>
                <BookOpen {...stylex.props(styles.emptyIcon)} />
                <p {...stylex.props(styles.heading)}>
                  <Trans>Your dictionary is empty</Trans>
                </p>
                <p {...stylex.props(styles.emptyDescription)}>
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
              <p {...stylex.props(styles.noMatch)}>
                <Trans>No match</Trans>
              </p>
            ) : null;
          }

          return (
            <div {...stylex.props(styles.terms)}>
              {visibleTerms.map((term) => (
                <div
                  key={term}
                  data-dictionary-term
                  {...stylex.props(styles.term)}
                >
                  <span {...stylex.props(styles.termLabel)}>{term}</span>
                  <Button
                    type="button"
                    variant="ghost"
                    size="icon"
                    sx={styles.removeButton}
                    onClick={() => removeTerm(term)}
                    aria-label={t`Remove ${term}`}
                  >
                    <MinusCircle {...stylex.props(styles.icon)} />
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

const styles = stylex.create({
  addButton: {
    backgroundColor: {
      default: "black",
      ":hover": "rgb(0 0 0 / 0.9)",
      ":is(.dark *)": "white",
      ":is(.dark *):hover": "rgb(255 255 255 / 0.9)",
    },
    borderRadius: radii.full,
    color: {
      default: "white",
      ":hover": "white",
      ":is(.dark *)": "black",
      ":is(.dark *):hover": "black",
    },
  },
  description: {
    color: colors.mutedForeground,
    fontSize: "0.75rem",
    lineHeight: "1.25rem",
    marginTop: "0.25rem",
  },
  empty: {
    alignItems: "center",
    backgroundColor: colors.card,
    borderColor: colors.border,
    borderRadius: "1rem",
    borderStyle: "solid",
    borderWidth: "1px",
    display: "flex",
    flexDirection: "column",
    justifyContent: "center",
    minHeight: "10rem",
    paddingInline: "1.5rem",
    textAlign: "center",
  },
  emptyDescription: {
    color: colors.mutedForeground,
    fontSize: "0.75rem",
    marginTop: "0.25rem",
    maxWidth: "24rem",
  },
  emptyIcon: {
    color: colors.mutedForeground,
    height: "1.25rem",
    marginBottom: "0.75rem",
    width: "1.25rem",
  },
  form: {
    display: "flex",
    flexDirection: "column",
    gap: "1rem",
  },
  heading: {
    fontSize: "0.875rem",
    fontWeight: 500,
  },
  icon: {
    height: "1rem",
    width: "1rem",
  },
  input: {
    paddingInline: "1rem",
  },
  inputGroup: {
    backgroundColor: colors.card,
    borderColor: colors.border,
    borderRadius: radii.full,
    boxShadow: "none",
  },
  lockBadge: {
    alignItems: "center",
    backgroundColor: colors.muted,
    borderRadius: radii.full,
    display: "flex",
    flexShrink: 0,
    height: "2.25rem",
    justifyContent: "center",
    width: "2.25rem",
  },
  lockIcon: {
    color: colors.mutedForeground,
    height: "1rem",
    width: "1rem",
  },
  noMatch: {
    color: colors.mutedForeground,
    fontSize: "0.875rem",
    paddingInline: "1rem",
  },
  page: {
    display: "flex",
    flexDirection: "column",
    gap: "2rem",
  },
  removeButton: {
    color: {
      default: colors.mutedForeground,
      ":hover": colors.foreground,
    },
    flexShrink: 0,
    height: "1.75rem",
    opacity: {
      default: 0,
      ":is([data-dictionary-term]:hover *)": 1,
      ":focus-visible": 1,
    },
    transitionDuration: "150ms",
    transitionProperty: "opacity",
    width: "1.75rem",
  },
  smallIcon: {
    height: "0.875rem",
    width: "0.875rem",
  },
  term: {
    alignItems: "center",
    borderTopColor: colors.border,
    borderTopStyle: {
      default: "solid",
      ":first-child": "none",
    },
    borderTopWidth: {
      default: "1px",
      ":first-child": 0,
    },
    display: "flex",
    gap: "0.75rem",
    justifyContent: "space-between",
    minHeight: "3rem",
    paddingBlock: "0.75rem",
    paddingLeft: "1rem",
    paddingRight: "0.75rem",
  },
  termLabel: {
    fontSize: "0.875rem",
  },
  terms: {
    backgroundColor: colors.card,
    borderColor: colors.border,
    borderRadius: "1rem",
    borderStyle: "solid",
    borderWidth: "1px",
    overflow: "hidden",
  },
  upsell: {
    alignItems: "flex-start",
    backgroundColor: colors.card,
    borderColor: colors.border,
    borderRadius: "1rem",
    borderStyle: "solid",
    borderWidth: "1px",
    display: "flex",
    gap: "1rem",
    justifyContent: "space-between",
    padding: "1.25rem",
  },
  upsellCopy: {
    display: "flex",
    gap: "0.75rem",
  },
});

export { styles as dictionarySettingsStyles };
