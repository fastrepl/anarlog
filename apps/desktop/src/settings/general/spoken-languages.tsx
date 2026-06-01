import { Trans, useLingui } from "@lingui/react/macro";
import { Search, X } from "lucide-react";
import { useMemo, useState } from "react";

import { Badge } from "@hypr/ui/components/ui/badge";
import { Button } from "@hypr/ui/components/ui/button";
import { cn } from "@hypr/utils";

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
      <h3 className="mb-1 text-sm font-medium">
        <Trans>Additional spoken languages</Trans>
      </h3>
      <p className="mb-3 text-xs text-neutral-600">
        <Trans>The main language is always included for transcription</Trans>
      </p>
      <div className="relative">
        <div
          className={cn([
            "flex min-h-[38px] w-full flex-wrap items-center gap-1.5 rounded-2xl border border-neutral-200 bg-white px-2 py-1.5 focus-within:border-neutral-300",
            languageInputFocused && "border-neutral-300",
          ])}
          onClick={() =>
            document.getElementById("language-search-input")?.focus()
          }
        >
          {selectedLanguageCodes.map((code) => (
            <Badge
              key={code}
              variant="secondary"
              className="bg-muted flex items-center gap-1 px-2 py-0.5 text-xs"
            >
              {getBaseLanguageDisplayName(code, i18n.locale)}
              <Button
                type="button"
                variant="ghost"
                size="sm"
                className="ml-0.5 h-3 w-3 p-0 hover:bg-transparent"
                onClick={(e) => {
                  e.stopPropagation();
                  onChange(selectedLanguageCodes.filter((c) => c !== code));
                }}
              >
                <X className="h-2.5 w-2.5" />
              </Button>
            </Badge>
          ))}
          {selectedLanguageCodes.length === 0 && (
            <Search className="size-4 shrink-0 text-neutral-700" />
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
            className="min-w-[120px] flex-1 bg-transparent text-sm placeholder:text-neutral-500 focus:outline-hidden"
          />
        </div>

        {languageInputFocused && languageSearchQuery.trim() && (
          <div
            id="language-options"
            role="listbox"
            className="absolute top-full right-0 left-0 z-10 mt-1 flex max-h-60 w-full flex-col overflow-hidden overflow-y-auto rounded-2xl border border-neutral-200 bg-white shadow-md"
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
                  className={cn([
                    "flex w-full items-center justify-between px-3 py-2 text-left text-sm transition-colors",
                    languageSelectedIndex === index
                      ? "bg-neutral-200"
                      : "hover:bg-neutral-100",
                  ])}
                >
                  <span className="truncate font-medium">
                    {getBaseLanguageDisplayName(langCode, i18n.locale)}
                  </span>
                </button>
              ))
            ) : (
              <div className="px-3 py-2 text-center text-sm text-neutral-500">
                <Trans>No matching languages found</Trans>
              </div>
            )}
          </div>
        )}
      </div>
    </div>
  );
}
