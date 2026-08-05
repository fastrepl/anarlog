import assert from "node:assert/strict";
import test from "node:test";

import {
  getBlogPostingJsonLd,
  getCanonicalUrl,
  getSoftwareApplicationJsonLd,
} from "./seo.ts";

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

test("describes downloads for every supported desktop platform", () => {
  const jsonLd = getSoftwareApplicationJsonLd({ description: "Anarlog" });

  assert.equal(jsonLd.downloadUrl, "https://anarlog.so/download/");
  assert.deepEqual(jsonLd.operatingSystem, ["macOS", "Windows", "Linux"]);
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

test("builds blog posting metadata with typed authors", () => {
  const url = getCanonicalUrl("/blog/local-ai-meeting-notes");
  const jsonLd = getBlogPostingJsonLd({
    url,
    headline: "Local AI meeting notes",
    description: "A practical guide.",
    image: "https://anarlog.so/api/og/blog/local-ai-meeting-notes",
    datePublished: "2026-01-01",
    authors: ["Jeehoon Ong", "Anarlog Team"],
  });

  assert.deepEqual(jsonLd.author, [
    { "@type": "Person", name: "Jeehoon Ong" },
    { "@type": "Organization", name: "Anarlog Team" },
  ]);
  assert.deepEqual(jsonLd.mainEntityOfPage, {
    "@type": "WebPage",
    "@id": url,
  });
  assert.equal(jsonLd.publisher.name, "Anarlog");
});
