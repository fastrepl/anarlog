const displayNames = new Intl.DisplayNames(["en"], { type: "language" });
const regionDisplayNames = new Intl.DisplayNames(["en"], { type: "region" });

export type LanguageOption = {
  value: string;
  label: string;
  searchTerms: string[];
};

function getLocaleDisplayName(code: string): string {
  return displayNames.of(code) ?? code;
}

function getRegionDisplayName(code: string): string | undefined {
  const { region } = parseLocale(code);
  return region ? regionDisplayNames.of(region) : undefined;
}

export function parseLocale(code: string): {
  language: string;
  region?: string;
} {
  try {
    const locale = new Intl.Locale(code);
    return {
      language: locale.language || code.split(/[-_]/)[0]?.toLowerCase() || code,
      region: locale.region,
    };
  } catch {
    const [language = code, ...rest] = code.split(/[-_]/);
    const region = rest.find(
      (part) => /^[a-z]{2}$/i.test(part) || /^\d{3}$/.test(part),
    );

    return {
      language: language.toLowerCase(),
      region: region?.toUpperCase(),
    };
  }
}

export function normalizeLanguageCode(code: string): string {
  return parseLocale(code).language;
}

export function normalizeLanguageCodes(codes: readonly string[]): string[] {
  const normalized: string[] = [];
  const seen = new Set<string>();

  for (const code of codes) {
    const value = normalizeLanguageCode(code);
    if (!value || seen.has(value)) {
      continue;
    }

    seen.add(value);
    normalized.push(value);
  }

  return normalized;
}

export function getBaseLanguageDisplayName(code: string): string {
  const { language } = parseLocale(code);
  return displayNames.of(language) ?? code;
}

export function getLanguageDisplayName(code: string): string {
  return getBaseLanguageDisplayName(code);
}

export function getLanguageOptions(
  supportedLanguages: readonly string[],
): LanguageOption[] {
  const grouped = new Map<string, string[]>();
  for (const code of supportedLanguages) {
    const value = normalizeLanguageCode(code);
    if (!value) {
      continue;
    }

    const group = grouped.get(value);
    if (group) {
      group.push(code);
    } else {
      grouped.set(value, [code]);
    }
  }

  return [...grouped.entries()].map(([value, codes]) => {
    const label = getBaseLanguageDisplayName(value);
    const searchTerms = [
      label,
      value,
      ...codes.flatMap((code) => [
        code,
        code.replace(/-/g, " "),
        getLocaleDisplayName(code),
        getRegionDisplayName(code),
      ]),
    ].filter((term): term is string => Boolean(term));

    return {
      value,
      label,
      searchTerms: [...new Set(searchTerms)],
    };
  });
}
