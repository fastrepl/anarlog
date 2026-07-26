import { describe, expect, it } from "vitest";

import { getTranscriptMatches, type SearchOptions } from "./matching";

const defaultOptions: SearchOptions = {
  caseSensitive: false,
  wholeWord: false,
};

describe("getTranscriptMatches", () => {
  it("returns transcript word spans in match order", () => {
    const spans = [
      createSpan("word-1", "Hello"),
      createSpan("word-2", "world"),
      createSpan("word-3", "hello"),
    ];

    expect(
      getTranscriptMatches(spans, "hello", defaultOptions).map(
        (match) => match.id,
      ),
    ).toEqual(["word-1", "word-3"]);
  });

  it("maps phrase matches to the first matching word", () => {
    const spans = [
      createSpan("word-1", "plan"),
      createSpan("word-2", "the"),
      createSpan("word-3", "launch"),
    ];

    expect(
      getTranscriptMatches(spans, "the launch", defaultOptions).map(
        (match) => match.id,
      ),
    ).toEqual(["word-2"]);
  });

  it("keeps whole-word matching behavior", () => {
    const spans = [
      createSpan("word-1", "sync"),
      createSpan("word-2", "async"),
      createSpan("word-3", "syncing"),
    ];

    expect(
      getTranscriptMatches(spans, "sync", {
        ...defaultOptions,
        wholeWord: true,
      }).map((match) => match.id),
    ).toEqual(["word-1"]);
  });
});

function createSpan(id: string, text: string) {
  const span = document.createElement("span");
  span.dataset.wordId = id;
  span.textContent = text;
  return span;
}
