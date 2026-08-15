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

    expect(first.locale).toBe("ko");
    expect(second.locale).toBe("ko");
    expect(first._("dEgA5A")).not.toBe("dEgA5A");
  });
});
