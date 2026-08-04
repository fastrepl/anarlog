import assert from "node:assert/strict";
import test from "node:test";

import { getCanonicalUrl, getSoftwareApplicationJsonLd } from "./seo.ts";

test("builds canonical urls with a trailing slash", () => {
  assert.equal(getCanonicalUrl(), "https://anarlog.so/");
  assert.equal(getCanonicalUrl("/blog"), "https://anarlog.so/blog/");
  assert.equal(
    getCanonicalUrl("/blog/granola-ai-alternatives"),
    "https://anarlog.so/blog/granola-ai-alternatives/",
  );
});

test("leaves an existing trailing slash untouched", () => {
  assert.equal(getCanonicalUrl("/download/"), "https://anarlog.so/download/");
});

test("tolerates paths without a leading slash", () => {
  assert.equal(getCanonicalUrl("download"), "https://anarlog.so/download/");
});

test("points downloadUrl at the canonical download page", () => {
  const jsonLd = getSoftwareApplicationJsonLd({ description: "Anarlog" });

  assert.equal(jsonLd.downloadUrl, "https://anarlog.so/download/");
});

test("emits offers when pricing is supplied", () => {
  const jsonLd = getSoftwareApplicationJsonLd({
    description: "Anarlog",
    aggregateOffer: { lowPrice: 0, highPrice: 15, offerCount: 2 },
  });

  assert.deepEqual(jsonLd.offers, {
    "@type": "AggregateOffer",
    url: "https://anarlog.so/",
    priceCurrency: "USD",
    lowPrice: 0,
    highPrice: 15,
    offerCount: 2,
  });
});
