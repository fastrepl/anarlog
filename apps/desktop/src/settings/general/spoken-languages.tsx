import { Trans, useLingui } from "@lingui/react/macro";
import { MagnifyingGlass, X } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { useMemo, useState } from "react";

import { colors } from "@anlg/design-system/tokens.stylex";
import { Badge } from "@anlg/ui/components/ui/badge";
import { Button } from "@anlg/ui/components/ui/button";

import {
  getAdditionalSpokenLanguages,
  getBaseLanguageCode,
  getBaseLanguageDisplayName,
} from "./language";

interface SpokenLanguagesViewProps {
  mainLanguage: string;
  value: string[];
  onChange: (value: string[]) => void;
  supportedLanguages: readonly string[];
}

export function SpokenLanguagesView({
  mainLanguage,
  value,
  onChange,
  supportedLanguages,
}: SpokenLanguagesViewProps) {
  const { i18n, t } = useLingui();
  const [languageSearchQuery, setLanguageSearchQuery] = useState("");
  const [languageInputFocused, setLanguageInputFocused] = useState(false);
  const [languageSelectedIndex, setLanguageSelectedIndex] = useState(-1);

  const supportedLanguageCodes = useMemo(() => {
    const seen = new Set<string>();
    const codes: string[] = [];

    for (const langCode of supportedLanguages) {
      const baseCode = getBaseLanguageCode(langCode);
      if (seen.has(baseCode)) continue;
      seen.add(baseCode);
      codes.push(baseCode);
    }

    return codes;
  }, [supportedLanguages]);

  const mainLanguageCode = getBaseLanguageCode(mainLanguage);
  const selectedLanguageCodes = useMemo(
    () => getAdditionalSpokenLanguages(mainLanguage, value),
    [mainLanguage, value],
  );

  const filteredLanguages = useMemo(() => {
    if (!languageSearchQuery.trim()) {
      return [];
    }
    const query = languageSearchQuery.toLowerCase();
    return supportedLanguageCodes.filter((langCode) => {
      if (langCode === mainLanguageCode) return false;
      if (selectedLanguageCodes.includes(langCode)) return false;
      const langName = getBaseLanguageDisplayName(langCode, i18n.locale);
      return langName.toLowerCase().includes(query);
    });
  }, [
    i18n.locale,
    languageSearchQuery,
    mainLanguageCode,
    selectedLanguageCodes,
    supportedLanguageCodes,
  ]);

  const handleLanguageKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (
      e.key === "Backspace" &&
      !languageSearchQuery &&
      selectedLanguageCodes.length > 0
    ) {
      e.preventDefault();
      onChange(selectedLanguageCodes.slice(0, -1));
      return;
    }

    if (!languageSearchQuery.trim() || filteredLanguages.length === 0) {
      return;
    }

    if (e.key === "ArrowDown") {
      e.preventDefault();
      setLanguageSelectedIndex((prev) =>
        prev < filteredLanguages.length - 1 ? prev + 1 : prev,
      );
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setLanguageSelectedIndex((prev) => (prev > 0 ? prev - 1 : 0));
    } else if (e.key === "Enter") {
      e.preventDefault();
      if (
        languageSelectedIndex >= 0 &&
        languageSelectedIndex < filteredLanguages.length
      ) {
        const selectedCode = filteredLanguages[languageSelectedIndex];
        onChange([...selectedLanguageCodes, selectedCode]);
        setLanguageSearchQuery("");
        setLanguageSelectedIndex(-1);
      }
    } else if (e.key === "Escape") {
      e.preventDefault();
      setLanguageInputFocused(false);
      setLanguageSearchQuery("");
    }
  };

  return (
    <div>
      <h3 {...stylex.props(styles.heading)}>
        <Trans>Additional spoken languages</Trans>
      </h3>
      <p {...stylex.props(styles.description)}>
        <Trans>Transcribe meetings that use more than one language.</Trans>
      </p>
      <div {...stylex.props(styles.controlWrapper)}>
        <div
          {...stylex.props(styles.control)}
          onClick={() =>
            document.getElementById("language-search-input")?.focus()
          }
        >
          {selectedLanguageCodes.map((code) => (
            <Badge key={code} variant="secondary" sx={styles.badge}>
              {getBaseLanguageDisplayName(code, i18n.locale)}
              <Button
                type="button"
                variant="ghost"
                size="sm"
                sx={styles.removeButton}
                onClick={(e) => {
                  e.stopPropagation();
                  onChange(selectedLanguageCodes.filter((c) => c !== code));
                }}
              >
                <X {...stylex.props(styles.removeIcon)} />
              </Button>
            </Badge>
          ))}
          {selectedLanguageCodes.length === 0 && (
            <MagnifyingGlass {...stylex.props(styles.searchIcon)} />
          )}
          <input
            id="language-search-input"
            type="text"
            value={languageSearchQuery}
            onChange={(e) => {
              setLanguageSearchQuery(e.target.value);
              setLanguageSelectedIndex(-1);
            }}
            onKeyDown={handleLanguageKeyDown}
            onFocus={() => setLanguageInputFocused(true)}
            onBlur={() => setLanguageInputFocused(false)}
            role="combobox"
            aria-haspopup="listbox"
            aria-expanded={languageInputFocused && !!languageSearchQuery.trim()}
            aria-controls="language-options"
            aria-activedescendant={
              languageSelectedIndex >= 0
                ? `language-option-${languageSelectedIndex}`
                : undefined
            }
            aria-label={t`Add spoken language`}
            placeholder={
              selectedLanguageCodes.length === 0 ? t`Add language` : ""
            }
            {...stylex.props(styles.input)}
          />
        </div>

        {languageInputFocused && languageSearchQuery.trim() && (
          <div
            id="language-options"
            role="listbox"
            {...stylex.props(styles.options)}
          >
            {filteredLanguages.length > 0 ? (
              filteredLanguages.map((langCode, index) => (
                <button
                  key={langCode}
                  id={`language-option-${index}`}
                  type="button"
                  role="option"
                  aria-selected={languageSelectedIndex === index}
                  onClick={() => {
                    onChange([...selectedLanguageCodes, langCode]);
                    setLanguageSearchQuery("");
                    setLanguageSelectedIndex(-1);
                  }}
                  onMouseDown={(e) => e.preventDefault()}
                  onMouseEnter={() => setLanguageSelectedIndex(index)}
                  {...stylex.props(
                    styles.option,
                    languageSelectedIndex === index && styles.selectedOption,
                  )}
                >
                  <span {...stylex.props(styles.optionLabel)}>
                    {getBaseLanguageDisplayName(langCode, i18n.locale)}
                  </span>
                </button>
              ))
            ) : (
              <div {...stylex.props(styles.empty)}>
                <Trans>No matching languages found</Trans>
              </div>
            )}
          </div>
        )}
      </div>
    </div>
  );
}

const styles = stylex.create({
  badge: {
    alignItems: "center",
    backgroundColor: colors.muted,
    display: "flex",
    fontSize: "0.75rem",
    gap: "0.25rem",
    lineHeight: "1rem",
    paddingBlock: "0.125rem",
    paddingInline: "0.5rem",
  },
  control: {
    alignItems: "center",
    backgroundColor: colors.card,
    borderColor: colors.border,
    borderRadius: "1rem",
    borderStyle: "solid",
    borderWidth: "1px",
    display: "flex",
    flexWrap: "wrap",
    gap: "0.375rem",
    minHeight: "38px",
    paddingBlock: "0.375rem",
    paddingInline: "0.5rem",
    width: "100%",
  },
  controlWrapper: {
    position: "relative",
  },
  description: {
    color: colors.mutedForeground,
    fontSize: "0.75rem",
    lineHeight: "1rem",
    marginBottom: "0.75rem",
  },
  empty: {
    color: colors.mutedForeground,
    fontSize: "0.875rem",
    lineHeight: "1.25rem",
    paddingBlock: "0.5rem",
    paddingInline: "0.75rem",
    textAlign: "center",
  },
  heading: {
    fontSize: "0.875rem",
    fontWeight: 500,
    lineHeight: "1.25rem",
    marginBottom: "0.25rem",
  },
  input: {
    "::placeholder": {
      color: colors.mutedForeground,
    },
    backgroundColor: "transparent",
    flex: "1",
    fontSize: "0.875rem",
    lineHeight: "1.25rem",
    minWidth: "120px",
    outline: {
      default: null,
      ":focus": "2px solid transparent",
    },
    outlineOffset: {
      default: null,
      ":focus": "2px",
    },
  },
  option: {
    alignItems: "center",
    backgroundColor: {
      default: "transparent",
      ":hover": colors.accent,
    },
    display: "flex",
    fontSize: "0.875rem",
    justifyContent: "space-between",
    lineHeight: "1.25rem",
    paddingBlock: "0.5rem",
    paddingInline: "0.75rem",
    textAlign: "left",
    transitionDuration: "150ms",
    transitionProperty:
      "color, background-color, border-color, text-decoration-color, fill, stroke",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
    width: "100%",
  },
  optionLabel: {
    fontWeight: 500,
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
  },
  options: {
    backgroundColor: colors.card,
    borderColor: colors.border,
    borderRadius: "1rem",
    borderStyle: "solid",
    borderWidth: "1px",
    boxShadow:
      "0 4px 6px -1px rgb(0 0 0 / 0.1), 0 2px 4px -2px rgb(0 0 0 / 0.1)",
    display: "flex",
    flexDirection: "column",
    left: 0,
    marginTop: "0.25rem",
    maxHeight: "15rem",
    overflowX: "hidden",
    overflowY: "auto",
    position: "absolute",
    right: 0,
    top: "100%",
    width: "100%",
  },
  removeButton: {
    backgroundColor: {
      default: null,
      ":hover": "transparent",
    },
    height: "0.75rem",
    marginLeft: "0.125rem",
    padding: 0,
    width: "0.75rem",
  },
  removeIcon: {
    height: "0.625rem",
    width: "0.625rem",
  },
  searchIcon: {
    color: colors.mutedForeground,
    flexShrink: 0,
    height: "1rem",
    width: "1rem",
  },
  selectedOption: {
    backgroundColor: colors.accent,
  },
});
