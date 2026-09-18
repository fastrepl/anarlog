import { expect, it } from "vitest";

import { formatProfilePhone } from "./phone";

it.each([
  ["+16693299320", "ko-KR", "+1 669 329 9320"],
  ["+821012345678", "en-US", "+82 10 1234 5678"],
  ["+442079460018", "en-US", "+44 20 7946 0018"],
  ["6693299320", "en-US", "(669) 329-9320"],
  ["01012345678", "ko-KR", "010-1234-5678"],
  ["", "en-US", ""],
  ["+123", "en-US", "+123"],
  ["call +16693299320", "en-US", "call +16693299320"],
  ["6693299320", "en", "6693299320"],
  ["6693299320", "invalid_locale", "6693299320"],
  ["+16693299320 ext. 42", "en-US", "+1 669 329 9320 ext. 42"],
])("formats %s with locale %s", (value, locale, expected) => {
  expect(formatProfilePhone(value, locale)).toBe(expected);
  expect(formatProfilePhone(expected, locale)).toBe(expected);
});
