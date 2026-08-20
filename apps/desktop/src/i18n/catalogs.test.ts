import { i18n } from "@lingui/core";
import { describe, expect, it } from "vitest";

import { createI18n, getCatalogLocalesForDisplayLocale } from "./catalogs";

describe("i18n catalogs", () => {
  it("loads only English when it is the active locale", () => {
    expect(getCatalogLocalesForDisplayLocale("en")).toEqual(["en"]);
  });

  it("loads the active locale with English as its fallback", () => {
    expect(getCatalogLocalesForDisplayLocale("ko")).toEqual(["en", "ko"]);
  });

  it("caches and activates dynamically imported catalogs", async () => {
    const first = await createI18n("ko");
    const second = await createI18n("ko");

    expect(first).toBe(i18n);
    expect(first.locale).toBe("ko");
    expect(second.locale).toBe("ko");
    expect(first._("dEgA5A")).not.toBe("dEgA5A");
  });

  it("does not let a slower catalog overwrite a newer locale", async () => {
    const stale = createI18n("ko");
    const latest = createI18n("ja");

    await latest;
    expect(i18n.locale).toBe("ja");

    await stale;
    expect(i18n.locale).toBe("ja");
  });
});
