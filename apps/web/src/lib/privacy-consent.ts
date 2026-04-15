export type PrivacyConsentRegion = "california" | "europe" | "default";

const EUROPEAN_COUNTRY_CODES = new Set([
  "AT",
  "BE",
  "BG",
  "CH",
  "CY",
  "CZ",
  "DE",
  "DK",
  "EE",
  "ES",
  "FI",
  "FR",
  "GB",
  "GR",
  "HR",
  "HU",
  "IE",
  "IS",
  "IT",
  "LI",
  "LT",
  "LU",
  "LV",
  "MT",
  "NL",
  "NO",
  "PL",
  "PT",
  "RO",
  "SE",
  "SI",
  "SK",
  "UK",
]);

function normalizeCode(value?: string | null) {
  const normalized = value?.trim().toUpperCase();
  return normalized ? normalized : undefined;
}

function normalizeSubdivisionCode(value?: string | null) {
  const normalized = normalizeCode(value);
  if (!normalized) {
    return undefined;
  }

  const [, subdivisionCode = normalized] = normalized.split("-");
  return subdivisionCode;
}

export function resolvePrivacyConsentRegion({
  countryCode,
  subdivisionCode,
}: {
  countryCode?: string | null;
  subdivisionCode?: string | null;
}): PrivacyConsentRegion {
  const normalizedCountryCode = normalizeCode(countryCode);
  const normalizedSubdivisionCode = normalizeSubdivisionCode(subdivisionCode);

  if (normalizedCountryCode === "US" && normalizedSubdivisionCode === "CA") {
    return "california";
  }

  if (
    normalizedCountryCode &&
    EUROPEAN_COUNTRY_CODES.has(normalizedCountryCode)
  ) {
    return "europe";
  }

  return "default";
}
