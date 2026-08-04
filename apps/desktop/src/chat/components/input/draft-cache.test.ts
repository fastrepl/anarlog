import { describe, expect, it } from "vitest";

import { DraftCache, measureDraftBytes } from "./draft-cache";

function draft(text: string) {
  return {
    type: "doc",
    content: [{ type: "paragraph", content: [{ type: "text", text }] }],
  };
}

function store(
  cache: DraftCache,
  key: string,
  content: ReturnType<typeof draft>,
) {
  cache.acquire(key, undefined);
  cache.update(key, content);
  return cache.release(key);
}

describe("DraftCache", () => {
  it("preserves the current draft and reports an older draft removal", () => {
    const cache = new DraftCache({
      maxCount: 2,
      maxDraftBytes: 10_000,
      maxRetainedBytes: 10_000,
    });

    store(cache, "first", draft("one"));
    store(cache, "second", draft("two"));
    expect(store(cache, "third", draft("three"))).toEqual({
      draftKey: "third",
      reason: "older-draft-removed",
      removedDraftCount: 1,
    });

    expect(cache.peek("first")).toBeUndefined();
    expect(cache.peek("second")).toEqual(draft("two"));
    expect(cache.peek("third")).toEqual(draft("three"));
  });

  it("reports per-draft and aggregate byte-limit refusals", () => {
    const first = draft("a".repeat(200));
    const second = draft("b".repeat(200));
    const draftBytes = measureDraftBytes(first);
    const cache = new DraftCache({
      maxCount: 10,
      maxDraftBytes: draftBytes,
      maxRetainedBytes: draftBytes * 2 - 1,
    });

    store(cache, "first", first);
    expect(store(cache, "second", second)).toEqual({
      draftKey: "second",
      reason: "older-draft-removed",
      removedDraftCount: 1,
    });
    expect(cache.peek("first")).toBeUndefined();
    expect(cache.peek("second")).toEqual(second);

    expect(store(cache, "oversized", draft("x".repeat(201)))).toEqual({
      draftKey: "oversized",
      reason: "draft-too-large",
    });
    expect(cache.peek("oversized")).toBeUndefined();
  });

  it("keeps an active draft until its final lease and then preserves it", () => {
    const activeDraft = draft("currently editing");
    const entryBytes = measureDraftBytes(activeDraft);
    const cache = new DraftCache({
      maxCount: 1,
      maxDraftBytes: 10_000,
      maxRetainedBytes: entryBytes,
    });

    cache.acquire("active", undefined);
    cache.update("active", activeDraft);
    store(cache, "other", draft("inactive"));

    expect(cache.peek("active")).toEqual(activeDraft);
    expect(cache.release("active")).toEqual({
      draftKey: "active",
      reason: "older-draft-removed",
      removedDraftCount: 1,
    });
    expect(cache.peek("active")).toEqual(activeDraft);
    expect(cache.peek("other")).toBeUndefined();
  });

  it("reports cache refusal when the configured bounds cannot retain a draft", () => {
    const cache = new DraftCache({
      maxCount: 0,
      maxDraftBytes: 10_000,
      maxRetainedBytes: 10_000,
    });

    expect(store(cache, "current", draft("current"))).toEqual({
      draftKey: "current",
      reason: "cache-full",
    });
    expect(cache.peek("current")).toBeUndefined();
  });

  it("defers sizing until the final lease releases", () => {
    let contentReads = 0;
    const content = {
      type: "doc",
      get content() {
        contentReads += 1;
        return [{ type: "paragraph" }];
      },
    };
    const cache = new DraftCache({
      maxCount: 2,
      maxDraftBytes: 10_000,
      maxRetainedBytes: 10_000,
    });

    cache.acquire("active", undefined);
    cache.update("active", content);
    expect(contentReads).toBe(0);

    cache.acquire("active", undefined);
    cache.release("active");
    expect(contentReads).toBe(0);

    cache.release("active");
    expect(contentReads).toBe(1);
  });

  it("measures JSON bytes without materializing the serialized draft", () => {
    const content = draft('quoted " text \\ with unicode é 😀 and\nline');
    const expected = new TextEncoder().encode(
      JSON.stringify(content),
    ).byteLength;

    expect(measureDraftBytes(content)).toBe(expected);
    expect(measureDraftBytes(content, expected - 1)).toBe(expected);
  });
});
