import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, test } from "vitest";

import { createI18n } from "./catalogs";
import { resolveDisplayLocale, SUPPORTED_DISPLAY_LOCALES } from "./locales";

const localesDir = join(dirname(fileURLToPath(import.meta.url)), "locales");

function readPoField(entry: string, field: "msgid" | "msgstr") {
  const lines = entry.split("\n");
  const start = lines.findIndex((line) => line.startsWith(`${field} `));
  if (start < 0) {
    return null;
  }

  const parts: string[] = [];
  for (let i = start; i < lines.length; i++) {
    const line = lines[i];
    const match =
      i === start
        ? line.match(new RegExp(`^${field} "(.*)"$`))
        : line.match(/^"(.*)"$/);

    if (!match) {
      break;
    }

    parts.push(match[1]);
  }

  return parts.join("");
}

function untranslatedMessages(locale: string) {
  const po = readFileSync(join(localesDir, locale, "messages.po"), "utf8");

  return po
    .split(/\n\n+/)
    .map((entry) => ({
      msgid: readPoField(entry, "msgid"),
      msgstr: readPoField(entry, "msgstr"),
    }))
    .filter(({ msgid, msgstr }) => msgid && msgstr === "")
    .map(({ msgid }) => msgid);
}

describe("resolveDisplayLocale", () => {
  test("uses exact supported locales", () => {
    expect(resolveDisplayLocale("ko")).toBe("ko");
  });

  test("uses base language for regional variants", () => {
    expect(resolveDisplayLocale("ja-JP")).toBe("ja");
  });

  test("uses supported main languages for display", () => {
    expect(resolveDisplayLocale("pl")).toBe("pl");
  });

  test("falls back to English for invalid values", () => {
    expect(resolveDisplayLocale("not a locale")).toBe("en");
  });

  test("supports every shipped display locale", () => {
    for (const locale of SUPPORTED_DISPLAY_LOCALES) {
      expect(resolveDisplayLocale(locale)).toBe(locale);
    }
  });
});

describe("message catalogs", () => {
  test("loads every supported display locale catalog", () => {
    for (const locale of SUPPORTED_DISPLAY_LOCALES) {
      expect(() => createI18n(locale)).not.toThrow();
    }
  });

  test("does not leave manually translated locale messages untranslated", () => {
    expect(untranslatedMessages("ko")).toEqual([]);
    expect(untranslatedMessages("ja")).toEqual([]);
  });
});
