import {
  isSupportedCountry,
  parsePhoneNumberFromString,
} from "libphonenumber-js/min";

export function formatProfilePhone(value: string, locale: string) {
  let region: string | undefined;
  try {
    region = new Intl.Locale(locale).region;
  } catch {
    // An unavailable region must not imply a country for a local number.
  }
  const phone = parsePhoneNumberFromString(value, {
    defaultCountry: region && isSupportedCountry(region) ? region : undefined,
    extract: false,
  });
  if (!phone?.isPossible()) return value;
  return value.trimStart().startsWith("+") || phone.country !== region
    ? phone.formatInternational()
    : phone.formatNational();
}
