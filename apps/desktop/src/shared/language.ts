const displayNames = new Intl.DisplayNames(["en"], { type: "language" });
const regionDisplayNames = new Intl.DisplayNames(["en"], { type: "region" });

export type LanguageOption = {
  value: string;
  label: string;
  searchTerms: string[];
};

export type SpokenLanguageOption = LanguageOption & {
  selectionKey: string;
};

type SpokenLanguageDescriptor = {
  optionKey: string;
  selectionKey: string;
  label: string;
  canonicalValue: string;
};

const EXACT_VARIANT_DESCRIPTORS: Record<string, SpokenLanguageDescriptor> = {
  "en-US": {
    optionKey: "en-US",
    selectionKey: "en",
    label: "English (US)",
    canonicalValue: "en-US",
  },
  "en-GB": {
    optionKey: "en-GB",
    selectionKey: "en",
    label: "English (UK)",
    canonicalValue: "en-GB",
  },
  "es-419": {
    optionKey: "es-419",
    selectionKey: "es",
    label: "Spanish (Latin America)",
    canonicalValue: "es-419",
  },
  "pt-BR": {
    optionKey: "pt-BR",
    selectionKey: "pt",
    label: "Portuguese (Brazil)",
    canonicalValue: "pt-BR",
  },
  "pt-PT": {
    optionKey: "pt-PT",
    selectionKey: "pt",
    label: "Portuguese (Portugal)",
    canonicalValue: "pt-PT",
  },
  "fr-CA": {
    optionKey: "fr-CA",
    selectionKey: "fr",
    label: "French (Canada)",
    canonicalValue: "fr-CA",
  },
  "zh-HK": {
    optionKey: "zh-HK",
    selectionKey: "zh-cantonese",
    label: "Cantonese",
    canonicalValue: "zh-HK",
  },
};

const MANDARIN_CODES = new Set(["zh", "zh-CN", "zh-Hans", "zh-TW", "zh-Hant"]);
const MANDARIN_PREFERRED_VALUES = [
  "zh",
  "zh-CN",
  "zh-Hans",
  "zh-TW",
  "zh-Hant",
];
const RUNTIME_PRESERVED_CODES = new Set([
  ...Object.keys(EXACT_VARIANT_DESCRIPTORS),
  ...MANDARIN_CODES,
  "da-DK",
  "de-CH",
  "en-AU",
  "en-CA",
  "en-IE",
  "en-IN",
  "en-NZ",
  "ko-KR",
  "nl-BE",
  "sv-SE",
  "th-TH",
]);

function getLocaleDisplayName(code: string): string {
  return displayNames.of(code) ?? code;
}

function getRegionDisplayName(code: string): string | undefined {
  const { region } = parseLocale(code);
  return region ? regionDisplayNames.of(region) : undefined;
}

function choosePreferredValue(
  aliases: readonly string[],
  preferredValues: readonly string[],
  fallback: string,
): string {
  for (const preferred of preferredValues) {
    if (aliases.includes(preferred)) {
      return preferred;
    }
  }

  return fallback;
}

function getSpokenLanguageDescriptor(code: string): SpokenLanguageDescriptor {
  const exactMatch = EXACT_VARIANT_DESCRIPTORS[code];
  if (exactMatch) {
    return exactMatch;
  }

  if (MANDARIN_CODES.has(code)) {
    return {
      optionKey: "zh-mandarin",
      selectionKey: "zh-mandarin",
      label: "Mandarin",
      canonicalValue: "zh",
    };
  }

  const { language } = parseLocale(code);
  const label = displayNames.of(language) ?? code;

  return {
    optionKey: language,
    selectionKey: language,
    label,
    canonicalValue: language,
  };
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

export function normalizeBaseLanguageCode(code: string): string {
  return parseLocale(code).language;
}

export function normalizeBaseLanguageCodes(codes: readonly string[]): string[] {
  const normalized: string[] = [];
  const seen = new Set<string>();

  for (const code of codes) {
    const value = normalizeBaseLanguageCode(code);
    if (!value || seen.has(value)) {
      continue;
    }

    seen.add(value);
    normalized.push(value);
  }

  return normalized;
}

export function normalizeSpokenLanguageCode(code: string): string {
  if (RUNTIME_PRESERVED_CODES.has(code)) {
    return code;
  }

  return getSpokenLanguageDescriptor(code).canonicalValue;
}

export function normalizeSpokenLanguageCodes(
  codes: readonly string[],
): string[] {
  const normalized: string[] = [];
  const seen = new Set<string>();

  for (const code of codes) {
    const descriptor = getSpokenLanguageDescriptor(code);
    if (seen.has(descriptor.selectionKey)) {
      continue;
    }

    seen.add(descriptor.selectionKey);
    normalized.push(normalizeSpokenLanguageCode(code));
  }

  return normalized;
}

export function getBaseLanguageDisplayName(code: string): string {
  const { language } = parseLocale(code);
  return displayNames.of(language) ?? code;
}

export function getSpokenLanguageDisplayName(code: string): string {
  return getSpokenLanguageDescriptor(code).label;
}

export function getLanguageDisplayName(code: string): string {
  return getSpokenLanguageDisplayName(code);
}

export function getBaseLanguageOptions(
  supportedLanguages: readonly string[],
): LanguageOption[] {
  return normalizeBaseLanguageCodes(supportedLanguages).map((value) => ({
    value,
    label: getBaseLanguageDisplayName(value),
    searchTerms: [value, getBaseLanguageDisplayName(value)],
  }));
}

export function getSpokenLanguageOptions(
  supportedLanguages: readonly string[],
): SpokenLanguageOption[] {
  const grouped = new Map<
    string,
    {
      descriptor: SpokenLanguageDescriptor;
      aliases: string[];
    }
  >();

  for (const code of supportedLanguages) {
    const descriptor = getSpokenLanguageDescriptor(code);
    const group = grouped.get(descriptor.optionKey);

    if (group) {
      group.aliases.push(code);
      continue;
    }

    grouped.set(descriptor.optionKey, {
      descriptor,
      aliases: [code],
    });
  }

  return [...grouped.values()].map(({ descriptor, aliases }) => {
    const value =
      descriptor.optionKey === "zh-mandarin"
        ? choosePreferredValue(
            aliases,
            MANDARIN_PREFERRED_VALUES,
            aliases[0] ?? descriptor.canonicalValue,
          )
        : choosePreferredValue(
            aliases,
            [descriptor.canonicalValue],
            aliases[0] ?? descriptor.canonicalValue,
          );

    const searchTerms = [
      descriptor.label,
      value,
      ...aliases.flatMap((alias) => [
        alias,
        alias.replace(/-/g, " "),
        getLocaleDisplayName(alias),
        getRegionDisplayName(alias),
      ]),
    ].filter((term): term is string => Boolean(term));

    return {
      value,
      label: descriptor.label,
      selectionKey: descriptor.selectionKey,
      searchTerms: [...new Set(searchTerms)],
    };
  });
}

export function normalizeSelectedSpokenLanguages(
  codes: readonly string[],
  supportedLanguages: readonly string[],
): string[] {
  const options = getSpokenLanguageOptions(supportedLanguages);
  const optionsBySelectionKey = new Map(
    options.map((option) => [option.selectionKey, option.value]),
  );
  const normalized: string[] = [];
  const seen = new Set<string>();

  for (const code of codes) {
    const descriptor = getSpokenLanguageDescriptor(code);
    if (seen.has(descriptor.selectionKey)) {
      continue;
    }

    seen.add(descriptor.selectionKey);
    normalized.push(
      optionsBySelectionKey.get(descriptor.selectionKey) ??
        descriptor.canonicalValue,
    );
  }

  return normalized;
}
